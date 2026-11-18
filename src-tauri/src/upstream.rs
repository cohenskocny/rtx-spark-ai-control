//! Upstream GitHub awareness for `devjeonghwan/vllm-for-rtx-spark`.
//!
//! This is an **information service**, not an updater. It reads the public
//! release list and a few recent commits and reports what it found. It never
//! downloads, installs, upgrades, replaces or configures vLLM — the inference
//! installation stays under the operator's control, and this application has no
//! business rewriting it.
//!
//! Behaviour that matters when the network is not what you hoped:
//!
//!   * **Daily by default.** One automatic check per 24 h; the manual button
//!     forces one, with a 30 s floor so a held-down button cannot burn the quota.
//!   * **Cached.** Every successful response is written to disk with its
//!     timestamp, so an offline launch still shows the last thing that was true.
//!   * **ETag conditional.** A `304 Not Modified` refreshes the timestamp without
//!     re-downloading or re-counting against the quota.
//!   * **Timeouts.** Connect and read are bounded; a stalled socket is reported as
//!     offline rather than hanging the page.
//!   * **Rate limits are a state, not an error.** GitHub's unauthenticated limit
//!     is 60 requests an hour: `403`/`429` with a reset time is shown as such, and
//!     the cached data stays visible.

use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// The upstream project this application treats as an external dependency.
pub const REPO: &str = "devjeonghwan/vllm-for-rtx-spark";
pub const REPO_URL: &str = "https://github.com/devjeonghwan/vllm-for-rtx-spark";
pub const RELEASES_URL: &str = "https://github.com/devjeonghwan/vllm-for-rtx-spark/releases";

pub const DAY_MS: u128 = 24 * 60 * 60 * 1000;
const MANUAL_FLOOR_MS: u128 = 30 * 1000;
const CACHE_VERSION: u32 = 1;

fn now_ms() -> u128 {
    crate::events::now_ms() as u128
}

/// Allow a test or a proxy to point the client somewhere else. Defaults to the
/// public API; nothing else in the module knows about GitHub's address.
fn api_base() -> String {
    std::env::var("V2_TEST_GITHUB_API")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .unwrap_or_else(|| "https://api.github.com".to_string())
}

fn cache_path() -> PathBuf {
    if let Ok(path) = std::env::var("V2_TEST_CACHE_PATH") {
        if !path.trim().is_empty() {
            return PathBuf::from(path);
        }
    }
    crate::settings::data_dir().join("upstream-cache.json")
}

fn timeouts() -> (Duration, Duration) {
    if let Some(ms) = std::env::var("V2_TEST_HTTP_TIMEOUT_MS")
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
    {
        return (Duration::from_millis(ms), Duration::from_millis(ms));
    }
    (Duration::from_secs(5), Duration::from_secs(10))
}

// ------------------------------------------------------------------- data types

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Release {
    pub tag: String,
    pub name: String,
    pub published_at: String,
    pub html_url: String,
    pub notes: String,
    pub prerelease: bool,
    pub draft: bool,
    pub assets: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Commit {
    pub sha: String,
    pub short: String,
    pub title: String,
    pub date: String,
    pub url: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Rate {
    pub limit: Option<u64>,
    pub remaining: Option<u64>,
    /// Unix milliseconds when the window resets.
    pub reset_ms: Option<u128>,
}

/// What is persisted between runs.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Cache {
    pub schema: u32,
    pub checked_ms: u128,
    pub last_success_ms: u128,
    pub etag: String,
    pub rate: Rate,
    pub releases: Vec<Release>,
    pub commits: Vec<Commit>,
}

/// What the Updates page draws.
#[derive(Serialize, Clone, Debug, Default)]
pub struct UpstreamView {
    /// `never` · `ok` · `offline` · `rate-limited` · `error`
    pub status: String,
    pub message: String,
    pub checked_ms: u128,
    pub last_success_ms: u128,
    pub from_cache: bool,
    pub repo_url: String,
    pub releases_url: String,
    pub latest: Option<Release>,
    pub releases: Vec<Release>,
    pub commits: Vec<Commit>,
    pub rate: Rate,
    /// True when upstream has commits newer than the installed source revision.
    pub commits_ahead: bool,
    pub installed_revision: String,
    /// When the automatic check is next due.
    pub next_auto_ms: u128,
}

impl UpstreamView {
    fn from_cache(cache: &Cache) -> Self {
        Self {
            status: if cache.last_success_ms == 0 {
                "never".to_string()
            } else {
                "ok".to_string()
            },
            message: if cache.last_success_ms == 0 {
                "No update check has completed yet.".to_string()
            } else {
                String::new()
            },
            checked_ms: cache.checked_ms,
            last_success_ms: cache.last_success_ms,
            from_cache: true,
            repo_url: REPO_URL.to_string(),
            releases_url: RELEASES_URL.to_string(),
            latest: latest_release(&cache.releases),
            releases: cache.releases.clone(),
            commits: cache.commits.clone(),
            rate: cache.rate.clone(),
            commits_ahead: false,
            installed_revision: String::new(),
            next_auto_ms: cache.last_success_ms.saturating_add(DAY_MS),
        }
    }

    pub fn due_for_auto(&self) -> bool {
        self.last_success_ms == 0 || now_ms().saturating_sub(self.last_success_ms) >= DAY_MS
    }
}

// ------------------------------------------------------------------- HTTP layer

enum Outcome {
    Ok {
        body: String,
        etag: Option<String>,
        rate: Rate,
    },
    NotModified {
        rate: Rate,
    },
    RateLimited {
        rate: Rate,
        message: String,
    },
    Failed {
        status: String,
        message: String,
    },
}

/// The HTTP agent every request goes through.
///
/// `ureq` only installs a TLS engine automatically for the rustls feature, and
/// that feature builds `ring`, which has no Windows ARM64 support without clang.
/// With the `native-tls` feature the connector has to be supplied explicitly —
/// without this the first https request fails at runtime with "no TLS backend is
/// configured", which is exactly what happened the first time this page ran.
fn agent() -> ureq::Agent {
    let (connect, read) = timeouts();
    let mut builder = ureq::AgentBuilder::new()
        .timeout_connect(connect)
        .timeout_read(read)
        .user_agent("RTX-Spark-AI-Control/0.1.0 (+https://github.com/devjeonghwan/vllm-for-rtx-spark)");
    if let Ok(native) = ureq::native_tls::TlsConnector::new() {
        builder = builder.tls_connector(std::sync::Arc::new(native));
    }
    builder.build()
}

fn rate_from_headers(headers: &[(String, String)]) -> Rate {
    let get = |name: &str| -> Option<String> {
        headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.clone())
    };
    let number = |name: &str| get(name).and_then(|v| v.trim().parse::<u64>().ok());
    Rate {
        limit: number("x-ratelimit-limit"),
        remaining: number("x-ratelimit-remaining"),
        reset_ms: number("x-ratelimit-reset").map(|seconds| (seconds as u128) * 1000),
    }
}

fn get_json(url: &str, etag: Option<&str>) -> Outcome {
    let mut request = agent()
        .get(url)
        .set("Accept", "application/vnd.github+json")
        .set("X-GitHub-Api-Version", "2022-11-28");
    if let Some(etag) = etag.filter(|value| !value.is_empty()) {
        request = request.set("If-None-Match", etag);
    }

    match request.call() {
        Ok(response) => {
            let status = response.status();
            let headers: Vec<(String, String)> = response
                .headers_names()
                .into_iter()
                .filter_map(|name| response.header(&name).map(|value| (name.clone(), value.to_string())))
                .collect();
            let rate = rate_from_headers(&headers);
            if status == 304 {
                return Outcome::NotModified { rate };
            }
            let etag = response.header("etag").map(|value| value.to_string());
            let mut body = String::new();
            if let Err(error) = response.into_reader().read_to_string(&mut body) {
                return Outcome::Failed {
                    status: "error".to_string(),
                    message: format!("could not read the response body: {error}"),
                };
            }
            Outcome::Ok { body, etag, rate }
        }
        Err(ureq::Error::Status(code, response)) => {
            let headers: Vec<(String, String)> = response
                .headers_names()
                .into_iter()
                .filter_map(|name| response.header(&name).map(|value| (name.clone(), value.to_string())))
                .collect();
            let rate = rate_from_headers(&headers);
            let retry_after = response
                .header("retry-after")
                .and_then(|value| value.trim().parse::<u64>().ok());
            let limited = code == 403 || code == 429;
            if limited {
                let message = match (retry_after, rate.reset_ms) {
                    (Some(seconds), _) => format!("GitHub's rate limit was reached; it resets in {seconds}s."),
                    (None, Some(reset)) => {
                        let in_seconds = reset.saturating_sub(now_ms()).div_ceil(1000);
                        format!("GitHub's rate limit was reached; it resets in about {in_seconds}s.")
                    }
                    _ => "GitHub's rate limit was reached.".to_string(),
                };
                Outcome::RateLimited { rate, message }
            } else if code == 404 {
                Outcome::Failed {
                    status: "error".to_string(),
                    message: "The upstream repository or endpoint was not found (404).".to_string(),
                }
            } else {
                Outcome::Failed {
                    status: "error".to_string(),
                    message: format!("GitHub answered {code}."),
                }
            }
        }
        Err(ureq::Error::Transport(transport)) => Outcome::Failed {
            status: "offline".to_string(),
            message: format!("No connection to GitHub: {transport}"),
        },
    }
}

// ------------------------------------------------------------------- parsing

pub fn parse_releases(body: &str) -> Result<Vec<Release>, String> {
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|error| format!("unexpected release payload: {error}"))?;
    let array = value
        .as_array()
        .ok_or_else(|| "release payload was not a list".to_string())?;
    let mut releases: Vec<Release> = array
        .iter()
        .map(|item| {
            let text = |key: &str| item.get(key).and_then(|v| v.as_str()).unwrap_or_default();
            Release {
                tag: text("tag_name").to_string(),
                name: text("name").to_string(),
                published_at: text("published_at").to_string(),
                html_url: text("html_url").to_string(),
                // Bodies can be long; the page shows an excerpt, and an unbounded
                // string would bloat both the cache and every snapshot.
                notes: truncate(text("body"), 1200),
                prerelease: item
                    .get("prerelease")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
                draft: item.get("draft").and_then(|v| v.as_bool()).unwrap_or(false),
                assets: item
                    .get("assets")
                    .and_then(|v| v.as_array())
                    .map(|list| list.len() as u32)
                    .unwrap_or(0),
            }
        })
        .filter(|release| !release.tag.is_empty())
        .collect();
    releases.sort_by(|a, b| b.published_at.cmp(&a.published_at));
    releases.truncate(10);
    Ok(releases)
}

pub fn parse_commits(body: &str) -> Result<Vec<Commit>, String> {
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|error| format!("unexpected commit payload: {error}"))?;
    let array = value
        .as_array()
        .ok_or_else(|| "commit payload was not a list".to_string())?;
    let mut commits: Vec<Commit> = array
        .iter()
        .map(|item| {
            let sha = item.get("sha").and_then(|v| v.as_str()).unwrap_or_default();
            let commit = item.get("commit");
            let message = commit
                .and_then(|c| c.get("message"))
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            // `commit.author` is an object: name, email, date.
            let date = commit
                .and_then(|c| c.get("author"))
                .and_then(|a| a.get("date"))
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            Commit {
                short: sha.chars().take(10).collect(),
                sha: sha.to_string(),
                title: truncate(message.lines().next().unwrap_or_default().trim(), 180),
                date: date.to_string(),
                url: item
                    .get("html_url")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
            }
        })
        .filter(|commit| !commit.sha.is_empty())
        .collect();
    commits.truncate(12);
    Ok(commits)
}

fn truncate(text: &str, max: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max {
        return trimmed.to_string();
    }
    let cut: String = trimmed.chars().take(max).collect();
    format!("{cut}…")
}

// ------------------------------------------------------------------- the check

fn load_cache() -> Cache {
    let Ok(text) = std::fs::read_to_string(cache_path()) else {
        return Cache::default();
    };
    let mut cache: Cache = serde_json::from_str(&text).unwrap_or_default();
    if cache.schema != CACHE_VERSION {
        return Cache::default();
    }
    cache.schema = CACHE_VERSION;
    cache
}

fn save_cache(cache: &Cache) {
    let path = cache_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut stored = cache.clone();
    stored.schema = CACHE_VERSION;
    if let Ok(text) = serde_json::to_vec_pretty(&stored) {
        let temp = path.with_extension("json.tmp");
        if std::fs::write(&temp, text).is_ok() {
            let _ = std::fs::rename(&temp, &path);
        }
    }
}

/// The cached view, with no network access at all. Used at startup and by
/// snapshots, so the interface always has something to draw.
pub fn view_from_cache(installed_revision: &str) -> UpstreamView {
    let cache = load_cache();
    let mut view = UpstreamView::from_cache(&cache);
    view.commits_ahead = commits_ahead(&view.commits, installed_revision);
    view.installed_revision = installed_revision.to_string();
    view
}

/// True when upstream's newest commit is not the revision installed here, which
/// means there is activity newer than what this machine runs. An unknown installed
/// revision cannot be compared, so it reports no activity rather than guessing.
fn commits_ahead(commits: &[Commit], installed_revision: &str) -> bool {
    let installed = installed_revision.trim().to_lowercase();
    if installed.is_empty() {
        return false;
    }
    let Some(head) = commits.first() else {
        return false;
    };
    let head = head.sha.to_lowercase();
    !(head == installed || head.starts_with(&installed))
}

/// Run a check. `force` is the manual button; without it the 24 h window applies.
pub fn check(force: bool, installed_revision: &str) -> UpstreamView {
    let cache = load_cache();
    let mut view = UpstreamView::from_cache(&cache);
    view.commits_ahead = commits_ahead(&cache.commits, installed_revision);
    view.installed_revision = installed_revision.to_string();

    let now = now_ms();

    if !force && !view.due_for_auto() {
        view.message = "Checked within the last 24 hours; using the stored result.".to_string();
        view.from_cache = true;
        return view;
    }
    if force && cache.checked_ms > 0 && now.saturating_sub(cache.checked_ms) < MANUAL_FLOOR_MS {
        view.status = if cache.last_success_ms == 0 {
            "never".to_string()
        } else {
            "ok".to_string()
        };
        view.message =
            "Checked moments ago; wait a few seconds before asking GitHub again.".to_string();
        view.from_cache = true;
        return view;
    }

    let mut next = cache.clone();
    next.schema = CACHE_VERSION;
    next.checked_ms = now;

    // 1. Releases, conditionally on the cached ETag.
    let releases_url = format!("{}/repos/{REPO}/releases?per_page=10", api_base());
    match get_json(&releases_url, Some(&cache.etag)) {
        Outcome::Ok { body, etag, rate } => match parse_releases(&body) {
            Ok(releases) => {
                next.releases = releases;
                next.etag = etag.unwrap_or_default();
                next.rate = rate.clone();
                view.rate = rate;
                next.last_success_ms = now;
            }
            Err(message) => {
                view.status = "error".to_string();
                view.message = message;
                save_cache(&next);
                return view;
            }
        },
        Outcome::NotModified { rate } => {
            next.rate = rate.clone();
            view.rate = rate;
            next.last_success_ms = now;
        }
        Outcome::RateLimited { rate, message } => {
            view.status = "rate-limited".to_string();
            view.message = format!(
                "{message} The stored result from the last successful check is shown below."
            );
            view.rate = rate.clone();
            next.rate = rate;
            view.checked_ms = now;
            save_cache(&next);
            return view;
        }
        Outcome::Failed { status, message } => {
            view.status = status;
            view.message = message;
            view.checked_ms = now;
            save_cache(&next);
            return view;
        }
    }

    // 2. Recent commits, for "new activity" that is not yet a release. The commit
    //    list is a bonus: a release list without it is still a useful page, so a
    //    failure here is recorded as a note rather than replacing the status.
    let mut note: Option<String> = None;
    let commits_url = format!("{}/repos/{REPO}/commits?per_page=12", api_base());
    match get_json(&commits_url, None) {
        Outcome::Ok { body, rate, .. } => {
            if let Ok(commits) = parse_commits(&body) {
                next.commits = commits;
            }
            if rate.limit.is_some() {
                next.rate = rate.clone();
                view.rate = rate;
            }
        }
        Outcome::RateLimited { rate, message } => {
            next.rate = rate.clone();
            view.rate = rate;
            note = Some(message);
        }
        Outcome::NotModified { rate } => {
            next.rate = rate;
        }
        Outcome::Failed { message, .. } => {
            note = Some(message);
        }
    }

    view.status = "ok".to_string();
    view.message = match (cache.last_success_ms == 0, note) {
        (true, None) => "Checked upstream for the first time.".to_string(),
        (false, None) => "Upstream release list refreshed.".to_string(),
        (true, Some(note)) => format!("Checked upstream for the first time. {note}"),
        (false, Some(note)) => format!("Upstream release list refreshed. {note}"),
    };
    view.from_cache = false;
    view.checked_ms = now;
    view.last_success_ms = next.last_success_ms;
    view.next_auto_ms = next.last_success_ms.saturating_add(DAY_MS);
    view.latest = latest_release(&next.releases);
    view.releases = next.releases.clone();
    view.commits = next.commits.clone();
    view.commits_ahead = commits_ahead(&view.commits, installed_revision);
    view.installed_revision = installed_revision.to_string();
    save_cache(&next);
    view
}

pub fn latest_release(releases: &[Release]) -> Option<Release> {
    releases
        .iter()
        .find(|release| !release.draft && !release.prerelease)
        .or_else(|| releases.first())
        .cloned()
}

/// The daily checker. One thread, ten-minute granularity, never blocks the
/// interface and never touches the network twice in the same window.
pub fn spawn(shared: crate::core::Shared) {
    let _ = std::thread::Builder::new()
        .name("v2-upstream".to_string())
        .spawn(move || loop {
            let (due, revision) = {
                let Ok(core) = shared.lock() else {
                    return;
                };
                (core.upstream.due_for_auto(), core.installed.revision.clone())
            };
            if due {
                let mut view = check(false, &revision);
                view.commits_ahead = commits_ahead(&view.commits, &revision);
                if let Ok(mut core) = shared.lock() {
                    core.apply_upstream(view);
                }
            }
            std::thread::sleep(Duration::from_secs(600));
        });
}

/// Milliseconds since the Unix epoch, so the test clock and the rate-limit reset
/// share one source.
#[cfg(test)]
pub fn unix_ms() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

// ------------------------------------------------------------------- test server

/// A tiny HTTP/1.0 server used by the offline, rate-limit and timeout tests. It is
/// compiled into the test binary only.
#[cfg(test)]
pub mod mock {
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    /// Bound on how much a mock server will read from the client, so a test cannot
    /// hang on a request it does not understand.
    pub const MAX_REQUEST_BYTES: usize = 8192;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Mode {
        Ok,
        RateLimited,
        ServerError,
        Stall,
    }

    pub struct Mock {
        pub base: String,
        pub hits: Arc<AtomicU64>,
        stop: Arc<std::sync::atomic::AtomicBool>,
        mode: Arc<std::sync::Mutex<Mode>>,
    }

    impl Mock {
        pub fn start(mode: Mode) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock github");
            let port = listener.local_addr().unwrap().port();
            listener
                .set_nonblocking(true)
                .expect("non-blocking listener");
            let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let hits = Arc::new(AtomicU64::new(0));
            let mode = Arc::new(std::sync::Mutex::new(mode));

            let stop_thread = stop.clone();
            let hits_thread = hits.clone();
            let mode_thread = mode.clone();
            std::thread::spawn(move || {
                while !stop_thread.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
                            let mut buffer = [0u8; MAX_REQUEST_BYTES];
                            let _ = stream.read(&mut buffer);
                            hits_thread.fetch_add(1, Ordering::Relaxed);
                            let current = *mode_thread.lock().unwrap();
                            if current == Mode::Stall {
                                // Accept, say nothing: this is the timeout case.
                                std::thread::sleep(Duration::from_secs(5));
                                continue;
                            }
                            let request = String::from_utf8_lossy(&buffer).to_string();
                            let wants_commits = request.contains("/commits");
                            let (status, body) = match current {
                                Mode::RateLimited => (
                                    "403 Forbidden",
                                    r#"{"message":"API rate limit exceeded"}"#.to_string(),
                                ),
                                Mode::ServerError => ("500 Internal Server Error", "{}".to_string()),
                                Mode::Ok => {
                                    if wants_commits {
                                        ("200 OK", FIXTURE_COMMITS.to_string())
                                    } else {
                                        ("200 OK", FIXTURE_RELEASES.to_string())
                                    }
                                }
                                Mode::Stall => unreachable!(),
                            };
                            let rate = if current == Mode::RateLimited {
                                "x-ratelimit-limit: 60\r\nx-ratelimit-remaining: 0\r\nx-ratelimit-reset: 4102444800\r\n"
                            } else {
                                "x-ratelimit-limit: 60\r\nx-ratelimit-remaining: 55\r\nx-ratelimit-reset: 4102444800\r\n"
                            };
                            let response = format!(
                                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\netag: W/\"fixture\"\r\n{rate}content-length: {}\r\nconnection: close\r\n\r\n{body}",
                                body.len()
                            );
                            let _ = stream.write_all(response.as_bytes());
                            let _ = stream.flush();
                        }
                        Err(_) => std::thread::sleep(Duration::from_millis(20)),
                    }
                }
            });

            Mock {
                base: format!("http://127.0.0.1:{port}"),
                hits,
                stop,
                mode,
            }
        }

        pub fn set_mode(&self, mode: Mode) {
            *self.mode.lock().unwrap() = mode;
        }
    }

    impl Drop for Mock {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
        }
    }

    pub const FIXTURE_RELEASES: &str = r#"[
      {
        "tag_name": "v0.2.0",
        "name": "v0.2.0",
        "published_at": "2026-11-07T20:10:43Z",
        "html_url": "https://github.com/devjeonghwan/vllm-for-rtx-spark/releases/tag/v0.2.0",
        "body": "Monitor and tray options.\n\n- unify monitor and tray\n- show config paths",
        "prerelease": false,
        "draft": false,
        "assets": [{"name": "a.whl"}, {"name": "b.json"}]
      },
      {
        "tag_name": "v0.1.3",
        "name": "v0.1.3",
        "published_at": "2026-11-07T04:36:32Z",
        "html_url": "https://github.com/devjeonghwan/vllm-for-rtx-spark/releases/tag/v0.1.3",
        "body": "Fixes",
        "prerelease": false,
        "draft": false,
        "assets": [{"name": "a.whl"}]
      }
    ]"#;

    pub const FIXTURE_COMMITS: &str = r#"[
      {
        "sha": "8b8a427a40aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "html_url": "https://github.com/devjeonghwan/vllm-for-rtx-spark/commit/8b8a427a40",
        "commit": {
          "message": "Bump project version to 0.2.1\n\nSecond line ignored",
          "author": {"date": "2026-11-07T20:11:30Z"}
        }
      },
      {
        "sha": "e9412963ffbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "html_url": "https://github.com/devjeonghwan/vllm-for-rtx-spark/commit/e9412963ff",
        "commit": {
          "message": "Unify monitor and tray options and show config paths",
          "author": {"date": "2026-11-07T20:01:08Z"}
        }
      }
    ]"#;
}

// ------------------------------------------------------------------- unit tests

#[cfg(test)]
mod tests {
    use super::mock::{Mock, Mode};
    use super::*;

    /// Every test that redirects the API base or the cache shares one lock: they
    /// mutate process-wide environment variables, so they must not overlap.
    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        LOCK.lock().unwrap_or_else(|error| error.into_inner())
    }

    fn with_mock<T>(mode: Mode, body: impl FnOnce(&Mock) -> T) -> T {
        let _guard = env_lock();
        let mock = Mock::start(mode);
        let cache = std::env::temp_dir().join(format!(
            "v2-upstream-cache-{}.json",
            std::process::id() as u64 * 1000 + unix_ms() as u64 % 1000
        ));
        let _ = std::fs::remove_file(&cache);
        std::env::set_var("V2_TEST_GITHUB_API", &mock.base);
        std::env::set_var("V2_TEST_CACHE_PATH", &cache);
        let result = body(&mock);
        std::env::remove_var("V2_TEST_GITHUB_API");
        std::env::remove_var("V2_TEST_CACHE_PATH");
        let _ = std::fs::remove_file(&cache);
        result
    }

    #[test]
    fn a_successful_check_parses_releases_and_commits() {
        with_mock(Mode::Ok, |_mock| {
            let view = check(true, "e9412963ff");
            assert_eq!(view.status, "ok", "{}", view.message);
            assert_eq!(view.releases.len(), 2);
            assert_eq!(view.releases[0].tag, "v0.2.0");
            assert_eq!(view.releases[0].assets, 2);
            assert_eq!(view.latest.as_ref().unwrap().tag, "v0.2.0");
            assert!(view.releases[0].notes.contains("Monitor and tray"));
            assert_eq!(view.commits.len(), 2);
            assert_eq!(view.commits[0].short, "8b8a427a40");
            assert_eq!(
                view.commits[0].title, "Bump project version to 0.2.1",
                "only the first line of a commit message is shown"
            );
            assert!(view.last_success_ms > 0);
            // The newest commit is not the installed revision, so this is new activity.
            assert!(view.commits_ahead);
        });
    }

    #[test]
    fn no_new_commits_is_not_reported_as_activity() {
        with_mock(Mode::Ok, |_mock| {
            let view = check(true, "8b8a427a40aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
            assert_eq!(view.status, "ok");
            assert!(!view.commits_ahead, "HEAD matches the installed revision");
        });
    }

    #[test]
    fn a_second_automatic_check_is_served_from_cache() {
        with_mock(Mode::Ok, |mock| {
            let first = check(true, "");
            assert_eq!(first.status, "ok");
            let hits_after_first = mock.hits.load(std::sync::atomic::Ordering::Relaxed);
            // Not forced: inside the 24 h window, so no request is made at all.
            let second = check(false, "");
            assert!(second.from_cache);
            assert_eq!(second.status, "ok");
            assert!(second.message.contains("24 hours"), "{}", second.message);
            assert_eq!(
                mock.hits.load(std::sync::atomic::Ordering::Relaxed),
                hits_after_first,
                "a cached check must not touch the network"
            );
            assert_eq!(second.latest.as_ref().unwrap().tag, "v0.2.0");
        });
    }

    #[test]
    fn rate_limiting_is_reported_with_the_cached_result_intact() {
        with_mock(Mode::Ok, |mock| {
            let ok = check(true, "");
            assert_eq!(ok.status, "ok");
            mock.set_mode(Mode::RateLimited);
            // Clear the manual floor so the request is actually attempted.
            let cache_file = std::env::var("V2_TEST_CACHE_PATH").unwrap();
            let mut stored: Cache =
                serde_json::from_str(&std::fs::read_to_string(&cache_file).unwrap()).unwrap();
            stored.checked_ms = 0;
            std::fs::write(&cache_file, serde_json::to_vec(&stored).unwrap()).unwrap();

            let limited = check(true, "");
            assert_eq!(limited.status, "rate-limited", "{}", limited.message);
            assert!(limited.message.contains("rate limit"), "{}", limited.message);
            assert_eq!(limited.rate.remaining, Some(0));
            assert!(limited.rate.reset_ms.is_some());
            // The previous releases are still there to read.
            assert_eq!(limited.releases.len(), 2);
        });
    }

    #[test]
    fn a_server_error_keeps_the_cache_and_says_so() {
        with_mock(Mode::ServerError, |_mock| {
            let view = check(true, "");
            assert_eq!(view.status, "error", "{}", view.message);
            assert!(view.message.contains("500"), "{}", view.message);
            assert_eq!(view.releases.len(), 0);
        });
    }

    #[test]
    fn an_unreachable_host_is_offline_not_an_error() {
        let _guard = env_lock();
        // Port 9 on loopback: nothing listens, so the connect is refused at once.
        let cache = std::env::temp_dir().join("v2-upstream-offline-cache.json");
        let _ = std::fs::remove_file(&cache);
        std::env::set_var("V2_TEST_GITHUB_API", "http://127.0.0.1:9");
        std::env::set_var("V2_TEST_CACHE_PATH", &cache);
        let view = check(true, "");
        std::env::remove_var("V2_TEST_GITHUB_API");
        std::env::remove_var("V2_TEST_CACHE_PATH");
        let _ = std::fs::remove_file(&cache);
        assert_eq!(view.status, "offline", "{}", view.message);
        assert!(view.message.to_lowercase().contains("github"), "{}", view.message);
    }

    #[test]
    fn a_stalled_socket_hits_the_configured_timeout() {
        let _guard = env_lock();
        let mock = Mock::start(Mode::Stall);
        let cache = std::env::temp_dir().join("v2-upstream-timeout-cache.json");
        let _ = std::fs::remove_file(&cache);
        std::env::set_var("V2_TEST_GITHUB_API", &mock.base);
        std::env::set_var("V2_TEST_CACHE_PATH", &cache);
        // 300 ms is enough to prove the timeout is applied and honoured.
        std::env::set_var("V2_TEST_HTTP_TIMEOUT_MS", "300");
        let began = std::time::Instant::now();
        let view = check(true, "");
        let elapsed = began.elapsed();
        std::env::remove_var("V2_TEST_GITHUB_API");
        std::env::remove_var("V2_TEST_CACHE_PATH");
        std::env::remove_var("V2_TEST_HTTP_TIMEOUT_MS");
        let _ = std::fs::remove_file(&cache);
        assert_eq!(view.status, "offline", "{}", view.message);
        assert!(
            elapsed < Duration::from_secs(3),
            "check returned after {elapsed:?}, so the timeout was not applied"
        );
    }

    #[test]
    fn cached_views_survive_a_restart_with_no_network() {
        with_mock(Mode::Ok, |_mock| {
            let first = check(true, "");
            assert_eq!(first.status, "ok");
            // Simulate the next launch: read the cache with no server reachable.
            std::env::set_var("V2_TEST_GITHUB_API", "http://127.0.0.1:9");
            let restored = view_from_cache("");
            std::env::remove_var("V2_TEST_GITHUB_API");
            assert_eq!(restored.status, "ok");
            assert!(restored.from_cache);
            assert_eq!(restored.releases.len(), 2);
            assert_eq!(restored.latest.as_ref().unwrap().tag, "v0.2.0");
            assert!(restored.last_success_ms > 0);
        });
    }

    #[test]
    fn release_and_commit_parsers_reject_junk() {
        assert!(parse_releases("not json").is_err());
        assert!(parse_releases(r#"{"message":"nope"}"#).is_err());
        assert!(parse_commits("[").is_err());
        assert_eq!(parse_releases("[]").unwrap().len(), 0);
    }

    #[test]
    fn notes_are_truncated_for_display_and_storage() {
        let long = "x".repeat(5000);
        let body = format!(r#"[{{"tag_name":"v1.0.0","body":"{long}"}}]"#);
        let releases = parse_releases(&body).unwrap();
        assert!(releases[0].notes.chars().count() <= 1201);
    }
}
