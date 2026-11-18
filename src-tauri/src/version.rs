//! What inference engine is actually installed here, and how it compares to the
//! upstream project.
//!
//! The installed version is read from disk, never by importing the package: an
//! `import vllm` pulls in torch and initialises CUDA, which is exactly the cost
//! this application exists to avoid paying by accident. Two files are enough:
//!
//!   * `<root>/build-info.json`, written by the native build, which records the
//!     project version, the exact upstream commit and the wheel tag;
//!   * `<env>/Lib/site-packages/vllm-<version>.dist-info/METADATA`, the pip
//!     record, which is authoritative about what is installed.
//!
//! `build-info.json` also supplies the source revision, so "installed revision"
//! is a fact rather than an inference. Nothing here runs git.

use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::Serialize;

use crate::paths::Paths;

/// The combination this release of the application has actually been run against.
/// Anything newer is reported as available but explicitly *not* verified.
pub const TESTED_PROJECT_VERSION: &str = "0.2.0";
pub const TESTED_UPSTREAM_VLLM: &str = "0.32.0";
pub const TESTED_PLATFORM: &str = "windows-arm64";

#[derive(Serialize, Clone, Debug, Default)]
pub struct Installed {
    /// The full installed version, such as `0.32.0+rtx_spark.0.2.0`.
    pub version: String,
    /// The fork's own version, such as `0.2.0`.
    pub project_version: String,
    /// The upstream vLLM version this build is based on.
    pub upstream_vllm: String,
    pub upstream_commit: String,
    /// Commit of the fork this build came from, when the install records it.
    pub revision: String,
    pub revision_short: String,
    /// `build-info.json` · `dist-info` · `unavailable`
    pub version_source: String,
    pub wheel_tag: String,
    pub python: String,
    pub arch: String,
    pub read_ms: u128,
    pub notes: Vec<String>,
}

/// Read the installed version and revision. Read-only, bounded, no processes.
pub fn read(paths: &Paths) -> Installed {
    let began = Instant::now();
    let mut installed = Installed {
        python: paths.python_exe.clone(),
        arch: std::env::consts::ARCH.to_string(),
        version_source: "unavailable".to_string(),
        ..Default::default()
    };
    let mut notes = Vec::new();

    let root = resolve_root(paths);

    // 1. The native build record: the only place the fork's commit is written down.
    if let Some(root) = &root {
        let build_info = root.join("build-info.json");
        match std::fs::read_to_string(&build_info) {
            Ok(text) => match serde_json::from_str::<serde_json::Value>(&text) {
                Ok(value) => {
                    let field = |key: &str| -> String {
                        value
                            .get(key)
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .trim()
                            .to_string()
                    };
                    installed.project_version = field("project_version");
                    installed.upstream_vllm = field("upstream_version");
                    installed.upstream_commit = field("upstream_commit");
                    installed.wheel_tag = field("wheel_tag");
                    let revision = field("project_commit");
                    installed.revision_short = revision.chars().take(10).collect();
                    installed.revision = revision;
                    let wheel_version = field("wheel_version");
                    if !wheel_version.is_empty() {
                        installed.version = wheel_version;
                        installed.version_source = "build-info.json".to_string();
                    }
                }
                Err(error) => notes.push(format!("build-info.json is not valid JSON: {error}")),
            },
            Err(_) => {}
        }
    } else {
        notes.push("no installation root configured, so build-info.json was not read".to_string());
    }

    // 2. pip's own record wins on "what is installed" when it is present.
    for site_packages in site_package_dirs(paths, root.as_deref()) {
        let Some(found) = find_dist_info(&site_packages) else {
            continue;
        };
        match read_metadata_version(&found) {
            Some(version) => {
                let project = project_from_local(&version);
                installed.version = version;
                installed.version_source = "dist-info".to_string();
                if installed.project_version.is_empty() {
                    installed.project_version = project.unwrap_or_default();
                }
                break;
            }
            None => notes.push(format!(
                "{} has no readable Version field",
                found.display()
            )),
        }
    }

    if installed.version.is_empty() {
        notes.push(
            "no installed vLLM was found at the configured paths; the version is unknown".to_string(),
        );
    }
    installed.read_ms = began.elapsed().as_millis();
    installed.notes = notes;
    installed
}

/// The installation root: configured, or inferred from the launcher/interpreter
/// layout (`<root>\.venv\Scripts\python.exe`).
pub fn resolve_root(paths: &Paths) -> Option<PathBuf> {
    if !paths.vllm_root.trim().is_empty() {
        let root = PathBuf::from(paths.vllm_root.trim());
        if root.is_dir() {
            return Some(root);
        }
    }
    for exe in [&paths.python_exe, &paths.vllm_exe] {
        if exe.trim().is_empty() {
            continue;
        }
        let path = Path::new(exe.trim());
        let Some(scripts) = path.parent() else {
            continue;
        };
        // `<root>\.venv\Scripts` → `<root>`; `<root>\Scripts` → `<root>`.
        let Some(env) = scripts.parent() else {
            continue;
        };
        let candidate = if env
            .file_name()
            .map(|name| name.to_string_lossy().starts_with('.'))
            .unwrap_or(false)
        {
            env.parent().unwrap_or(env).to_path_buf()
        } else {
            env.to_path_buf()
        };
        if candidate.is_dir() {
            return Some(candidate);
        }
    }
    None
}

fn site_package_dirs(paths: &Paths, root: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut push = |dir: PathBuf| {
        if dir.is_dir() && !dirs.contains(&dir) {
            dirs.push(dir);
        }
    };
    for exe in [&paths.python_exe, &paths.vllm_exe] {
        if exe.trim().is_empty() {
            continue;
        }
        if let Some(env) = Path::new(exe.trim()).parent().and_then(|p| p.parent()) {
            push(env.join("Lib").join("site-packages"));
        }
    }
    if let Some(root) = root {
        push(root.join("Lib").join("site-packages"));
        if let Ok(entries) = std::fs::read_dir(root) {
            for entry in entries.flatten().take(20) {
                let dir = entry.path();
                if dir.is_dir() {
                    push(dir.join("Lib").join("site-packages"));
                }
            }
        }
    }
    dirs
}

fn find_dist_info(site_packages: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(site_packages).ok()?;
    let mut best: Option<PathBuf> = None;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("vllm-") && name.ends_with(".dist-info") && entry.path().is_dir() {
            // Prefer the alphabetically last match: a stale parallel install must
            // not shadow the newer one.
            best = match best {
                Some(current) if current.file_name().unwrap_or_default() >= entry.file_name() => {
                    Some(current)
                }
                _ => Some(entry.path()),
            };
        }
    }
    best
}

fn read_metadata_version(dist_info: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dist_info.join("METADATA")).ok()?;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("Version:") {
            let value = rest.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
        if line.trim().is_empty() {
            break; // headers end at the first blank line
        }
    }
    None
}

/// The fork's own version out of a PEP 440 local version.
///
/// `0.32.0+rtx_spark.0.2.0` → `0.2.0`. The scheme is
/// `<upstream>+<project>_<project version>`, so the tail after the last
/// underscore is the project version; a local part without one falls back to its
/// first numeric run.
fn project_from_local(version: &str) -> Option<String> {
    let local = version.split('+').nth(1)?.trim();
    if let Some(index) = local.rfind('_') {
        let tail = &local[index + 1..];
        if parse_version(tail).is_some() {
            return Some(tail.to_string());
        }
    }
    let start = local.find(|c: char| c.is_ascii_digit())?;
    Some(local[start..].to_string())
}

/// Leading numeric segments of a version, so `v0.2.0`, `0.2.0` and
/// `0.32.0+rtx_spark.0.2.0` all compare sensibly.
pub fn parse_version(text: &str) -> Option<Vec<u64>> {
    let trimmed = text.trim().trim_start_matches(['v', 'V']);
    let head: String = trimmed
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    if head.is_empty() {
        return None;
    }
    let parts: Vec<u64> = head
        .split('.')
        .filter(|part| !part.is_empty())
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts)
    }
}

pub fn cmp_versions(a: &str, b: &str) -> Option<Ordering> {
    let mut left = parse_version(a)?;
    let mut right = parse_version(b)?;
    let width = left.len().max(right.len());
    left.resize(width, 0);
    right.resize(width, 0);
    Some(left.cmp(&right))
}

/// How the installed engine relates to the newest upstream release.
#[derive(Serialize, Clone, Debug, Default)]
pub struct Compat {
    /// `current` · `update-available` · `ahead` · `unknown` · `unsupported-platform`
    pub status: String,
    /// True only when the newest upstream release is the exact version this
    /// application has been tested against, on the tested platform. It answers
    /// "is this release known good here?", never "is this the newest thing?".
    pub verified: bool,
    pub tested_project: String,
    pub tested_upstream: String,
    pub tested_platform: String,
    pub message: String,
}

/// Compare an installed build with the newest upstream release tag.
///
/// Deliberately conservative: a newer release is news, not permission. `verified`
/// stays false unless the newest release is the tested baseline, so "there is new
/// upstream activity" and "this is verified compatible" can never be confused.
pub fn assess(installed: &Installed, latest_tag: Option<&str>) -> Compat {
    let mut compat = Compat {
        tested_project: TESTED_PROJECT_VERSION.to_string(),
        tested_upstream: TESTED_UPSTREAM_VLLM.to_string(),
        tested_platform: TESTED_PLATFORM.to_string(),
        ..Default::default()
    };

    if std::env::consts::ARCH != "aarch64" {
        compat.status = "unsupported-platform".to_string();
        compat.message = format!(
            "This application is built for Windows on ARM64; this machine reports {}. Controls that launch inference are untested here.",
            std::env::consts::ARCH
        );
        return compat;
    }

    let Some(latest) = latest_tag.filter(|tag| !tag.trim().is_empty()) else {
        compat.status = "unknown".to_string();
        compat.message =
            "No upstream release is known yet; check for updates to compare.".to_string();
        return compat;
    };

    // The release itself is only "verified" if it is the version tested here.
    // Set only once we know what is installed: an unreadable install must not
    // inherit a claim made about the release.
    compat.verified = false;

    let project = if installed.project_version.trim().is_empty() {
        installed.version.clone()
    } else {
        installed.project_version.clone()
    };
    if project.trim().is_empty() {
        compat.status = "unknown".to_string();
        compat.message =
            "The installed vLLM version could not be read, so compatibility cannot be assessed."
                .to_string();
        return compat;
    }

    compat.verified = cmp_versions(latest, TESTED_PROJECT_VERSION) == Some(Ordering::Equal);

    match cmp_versions(&project, latest) {
        Some(Ordering::Equal) => {
            compat.status = "current".to_string();
            compat.message = if compat.verified {
                format!(
                    "Installed {project} matches the newest upstream release and the tested baseline."
                )
            } else {
                format!(
                    "Installed {project} matches the newest upstream release; that release has not been verified on Windows ARM64 here."
                )
            };
        }
        Some(Ordering::Less) => {
            compat.status = "update-available".to_string();
            compat.message = format!(
                "Upstream released {latest}; this machine has {project}. A newer release is unverified until it has been built and run here."
            );
        }
        Some(Ordering::Greater) => {
            compat.status = "ahead".to_string();
            compat.message = format!(
                "Installed {project} is newer than the latest upstream release ({latest})."
            );
        }
        None => {
            compat.status = "unknown".to_string();
            compat.message = format!("Could not compare {project} with {latest}.");
        }
    }
    compat
}

#[cfg(test)]
mod tests {
    use super::*;

    fn installed(version: &str, project: &str) -> Installed {
        Installed {
            version: version.to_string(),
            project_version: project.to_string(),
            upstream_vllm: TESTED_UPSTREAM_VLLM.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn parses_leading_numeric_segments() {
        assert_eq!(parse_version("v0.2.0"), Some(vec![0, 2, 0]));
        assert_eq!(parse_version("0.32.0+rtx_spark.0.2.0"), Some(vec![0, 32, 0]));
        assert_eq!(parse_version("1"), Some(vec![1]));
        assert_eq!(parse_version(""), None);
        assert_eq!(parse_version("main"), None);
    }

    #[test]
    fn compares_versions_of_different_length() {
        assert_eq!(cmp_versions("0.2.0", "v0.2.0"), Some(Ordering::Equal));
        assert_eq!(cmp_versions("0.2", "0.2.0"), Some(Ordering::Equal));
        assert_eq!(cmp_versions("0.1.3", "0.2.0"), Some(Ordering::Less));
        assert_eq!(cmp_versions("0.10.0", "0.9.9"), Some(Ordering::Greater));
    }

    #[test]
    fn matching_release_and_baseline_is_verified() {
        let compat = assess(&installed("0.32.0+rtx_spark.0.2.0", "0.2.0"), Some("v0.2.0"));
        assert_eq!(compat.status, "current");
        assert!(compat.verified, "{}", compat.message);
    }

    #[test]
    fn a_newer_release_is_never_verified() {
        let compat = assess(&installed("0.32.0+rtx_spark.0.2.0", "0.2.0"), Some("v0.3.0"));
        assert_eq!(compat.status, "update-available");
        assert!(!compat.verified);
        assert!(compat.message.contains("unverified"), "{}", compat.message);
    }

    #[test]
    fn an_older_release_reports_the_install_as_ahead() {
        let compat = assess(&installed("0.32.0+rtx_spark.0.4.0", "0.4.0"), Some("v0.2.0"));
        assert_eq!(compat.status, "ahead");
    }

    #[test]
    fn version_unknown_is_reported_not_guessed() {
        let compat = assess(&installed("", ""), Some("v0.2.0"));
        assert_eq!(compat.status, "unknown");
        assert!(!compat.verified);
    }

    #[test]
    fn no_upstream_release_is_unknown() {
        let compat = assess(&installed("0.32.0+rtx_spark.0.2.0", "0.2.0"), None);
        assert_eq!(compat.status, "unknown");
    }

    #[test]
    fn reads_a_real_install_shape_from_a_temp_folder() {
        let root = std::env::temp_dir().join("v2-version-read-test");
        let env = root.join(".venv");
        let dist_info = env
            .join("Lib")
            .join("site-packages")
            .join("vllm-0.32.0+rtx_spark.0.2.0.dist-info");
        std::fs::create_dir_all(&dist_info).unwrap();
        std::fs::write(
            dist_info.join("METADATA"),
            "Metadata-Version: 2.4\nName: vllm\nVersion: 0.32.0+rtx_spark.0.2.0\n\nBody\n",
        )
        .unwrap();
        std::fs::write(
            root.join("build-info.json"),
            r#"{
              "project_version": "0.2.0",
              "project_commit": "e9412963ff4619da5f97f8b5e3c2dd274967df87",
              "upstream_version": "0.32.0",
              "upstream_commit": "5df63a7203cdb9328d824a27b682e77c7b9928be",
              "wheel_version": "0.32.0+rtx_spark.0.2.0",
              "wheel_tag": "cp313-cp313-win_arm64"
            }"#,
        )
        .unwrap();

        let paths = Paths {
            vllm_root: root.to_string_lossy().to_string(),
            python_exe: env
                .join("Scripts")
                .join("python.exe")
                .to_string_lossy()
                .to_string(),
            ..Default::default()
        };
        let read_back = read(&paths);
        assert_eq!(read_back.version, "0.32.0+rtx_spark.0.2.0");
        assert_eq!(read_back.project_version, "0.2.0");
        assert_eq!(read_back.upstream_vllm, "0.32.0");
        assert_eq!(
            read_back.revision,
            "e9412963ff4619da5f97f8b5e3c2dd274967df87"
        );
        assert_eq!(read_back.revision_short, "e9412963ff");
        assert_eq!(read_back.version_source, "dist-info");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_stale_wheel_file_does_not_win_over_the_installed_record() {
        // The wheel in the install folder can be older than what pip installed.
        let root = std::env::temp_dir().join("v2-version-stale-wheel");
        let env = root.join(".venv");
        let dist_info = env
            .join("Lib")
            .join("site-packages")
            .join("vllm-0.32.0+rtx_spark.0.2.0.dist-info");
        std::fs::create_dir_all(&dist_info).unwrap();
        std::fs::write(
            dist_info.join("METADATA"),
            "Name: vllm\nVersion: 0.32.0+rtx_spark.0.2.0\n",
        )
        .unwrap();
        std::fs::write(
            root.join("build-info.json"),
            r#"{"project_version":"0.1.2","wheel_version":"0.32.0+rtx_spark.0.1.2"}"#,
        )
        .unwrap();

        let paths = Paths {
            vllm_root: root.to_string_lossy().to_string(),
            ..Default::default()
        };
        let read_back = read(&paths);
        assert_eq!(read_back.version, "0.32.0+rtx_spark.0.2.0");
        let _ = std::fs::remove_dir_all(&root);
    }
}
