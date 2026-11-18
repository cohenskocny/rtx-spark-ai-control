//! vLLM lifecycle: start, stop, restart, adopt, health and log streaming.
//!
//! Ownership rule, inherited from V1 and enforced everywhere: this application
//! only ever stops a process whose identity (pid plus creation time) it recorded
//! when it launched or explicitly adopted it. A server started outside V2 is
//! detected, displayed, and left alone. Nothing here enumerates processes by
//! name and kills them, because `python.exe` on this machine also means vLLM
//! workers, the blackout guard and whatever else the operator is running.
//!
//! The launch command is V1's, verbatim: the same `vllm.exe serve` with the same
//! optimized YAML and chat template. The checkpoint and configuration are read,
//! never rewritten.

use std::collections::VecDeque;
use std::io::{BufReader, Read};
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::core::ActionResult;
use crate::settings::{self, OwnedServer, RuntimeState};
use crate::telemetry::V1_CONFIG;
use crate::winproc;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
const LOG_TAIL: usize = 140;

const READY_HINTS: [&str; 2] = ["application startup complete", "uvicorn running on"];
const ERROR_HINTS: [&str; 5] = [
    "traceback (most recent call last)",
    "cuda out of memory",
    "assertionerror",
    "failed to load",
    "error:",
];

#[derive(Serialize, Clone, Debug, Default)]
pub struct Health {
    pub ok: bool,
    pub latency_ms: Option<f64>,
    pub models: Vec<String>,
    pub error: Option<String>,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct ServerView {
    pub state: String,
    pub detail: String,
    pub stage: String,
    pub progress_pct: Option<f64>,
    pub owned: bool,
    pub adopted: bool,
    pub pid: Option<u32>,
    pub listening: bool,
    pub process_name: Option<String>,
    pub uptime_s: Option<f64>,
    pub health_ok: bool,
    pub latency_ms: Option<f64>,
    pub health_error: Option<String>,
    pub health_failures: u32,
    pub models: Vec<String>,
    pub endpoint: String,
    pub port: u16,
    pub started_ms: Option<u64>,
    pub log_tail: Vec<String>,
    pub last_error: String,
    pub paths_ok: bool,
    pub can_start: bool,
    pub can_stop: bool,
    pub can_restart: bool,
    pub can_adopt: bool,
}

pub struct LaunchSpec {
    pub exe: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub port: u16,
}

pub struct Discovery {
    pub listener_pid: Option<u32>,
    pub owned: bool,
}

pub struct VllmManager {
    child: Option<Child>,
    record: Option<OwnedServer>,
    pub state: String,
    pub detail: String,
    pub stage: String,
    pub progress_pct: Option<f64>,
    pub health: Health,
    pub health_failures: u32,
    pub log_tail: VecDeque<String>,
    pub last_error: String,
    pub started_ms: Option<u64>,
    pub stop_timeout_s: u64,
    /// Set while a stop is in flight so the exit is not reported as a crash.
    pub stopping: bool,
}

impl VllmManager {
    pub fn load(stop_timeout_s: u64) -> Self {
        let runtime = settings::load_runtime();
        let record = runtime.server.clone();
        let mut manager = Self {
            child: None,
            record: None,
            state: "stopped".to_string(),
            detail: "not running".to_string(),
            stage: "idle".to_string(),
            progress_pct: None,
            health: Health::default(),
            health_failures: 0,
            log_tail: VecDeque::new(),
            last_error: String::new(),
            started_ms: None,
            stop_timeout_s,
            stopping: false,
        };
        if let Some(saved) = record {
            // A record from a previous run is only trusted while the process it
            // names is still the one running: a recycled pid must not inherit
            // the right to be stopped.
            if winproc::same_process(saved.pid, saved.created_ticks) {
                manager.started_ms = Some(saved.started_ms);
                manager.detail = if saved.adopted {
                    "adopted from a previous session".to_string()
                } else {
                    "started by this application".to_string()
                };
                manager.record = Some(saved);
            } else {
                let _ = settings::save_runtime(&RuntimeState {
                    schema: 1,
                    server: None,
                });
            }
        }
        manager
    }

    // ------------------------------------------------------------------- config

    fn v1_config() -> serde_json::Value {
        std::fs::read_to_string(V1_CONFIG)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or(serde_json::Value::Null)
    }

    pub fn port() -> u16 {
        if let Ok(raw) = std::env::var("V2_TEST_PORT") {
            if let Ok(port) = raw.parse::<u16>() {
                return port;
            }
        }
        Self::v1_config()
            .get("server")
            .and_then(|s| s.get("port"))
            .and_then(|p| p.as_u64())
            .unwrap_or(8000) as u16
    }

    fn cfg_str(value: &serde_json::Value, key: &str) -> String {
        value
            .get("server")
            .and_then(|s| s.get(key))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    }

    /// The command V1 launches, so both controllers start the same server.
    pub fn launch_spec() -> Result<LaunchSpec, String> {
        let port = Self::port();
        if let Ok(exe) = std::env::var("V2_TEST_VLLM_EXE") {
            // QA hook: lets the lifecycle be exercised end to end against a mock
            // without touching the production server or its configuration.
            let args = std::env::var("V2_TEST_VLLM_ARGS")
                .unwrap_or_default()
                .split(' ')
                .filter(|part| !part.is_empty())
                .map(|part| part.replace("{port}", &port.to_string()))
                .collect::<Vec<_>>();
            let cwd = Path::new(&exe)
                .parent()
                .map(|p| p.to_string_lossy().to_string());
            if !Path::new(&exe).exists() {
                return Err(format!("mock executable not found: {exe}"));
            }
            return Ok(LaunchSpec {
                exe,
                args,
                cwd,
                port,
            });
        }

        let cfg = Self::v1_config();
        let exe = Self::cfg_str(&cfg, "vllm_exe");
        let model = Self::cfg_str(&cfg, "model_path");
        let config_path = Self::cfg_str(&cfg, "config_path");
        let template = Self::cfg_str(&cfg, "chat_template");
        for (label, path) in [
            ("vllm.exe", &exe),
            ("model checkpoint", &model),
            ("vLLM config", &config_path),
            ("chat template", &template),
        ] {
            if path.is_empty() || !Path::new(path).exists() {
                return Err(format!("{label} missing: {path}"));
            }
        }
        let args = vec![
            "serve".to_string(),
            model,
            "--config".to_string(),
            config_path,
            "--chat-template".to_string(),
            template,
            "--port".to_string(),
            port.to_string(),
        ];
        let cwd = Path::new(&exe)
            .parent()
            .map(|p| p.to_string_lossy().to_string());
        Ok(LaunchSpec {
            exe,
            args,
            cwd,
            port,
        })
    }

    // ---------------------------------------------------------------- discovery

    pub fn discover(&self) -> Discovery {
        let port = Self::port();
        let listener_pid = winproc::listeners_on(port).first().map(|l| l.pid);
        match &self.record {
            Some(record) if winproc::same_process(record.pid, record.created_ticks) => {
                let belongs = match listener_pid {
                    None => true,
                    Some(pid) => {
                        pid == record.pid || winproc::tree_pids(record.pid).contains(&pid)
                    }
                };
                Discovery {
                    listener_pid,
                    owned: belongs,
                }
            }
            _ => Discovery {
                listener_pid,
                owned: false,
            },
        }
    }

    // ------------------------------------------------------------------- spawn

    pub fn start(&mut self, events: &mut crate::events::EventRing) -> ActionResult {
        let discovery = self.discover();
        if discovery.listener_pid.is_some() {
            events.push(
                "warn",
                format!(
                    "A server is already listening on port {}; not starting a second one.",
                    Self::port()
                ),
            );
            return ActionResult::fail("A server is already running on this port");
        }
        if discovery.owned {
            return ActionResult::fail("This application's server is still starting");
        }
        let spec = match Self::launch_spec() {
            Ok(spec) => spec,
            Err(message) => {
                self.state = "error".to_string();
                self.last_error = message.clone();
                events.push("error", format!("Cannot launch vLLM: {message}"));
                return ActionResult::fail(message);
            }
        };

        let mut command = Command::new(&spec.exe);
        command.args(&spec.args);
        if let Some(cwd) = &spec.cwd {
            command.current_dir(cwd);
        }
        command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                let message = format!("spawn failed: {error}");
                self.state = "error".to_string();
                self.last_error = message.clone();
                events.push("error", format!("Could not launch vLLM: {error}"));
                return ActionResult::fail(message);
            }
        };

        let pid = child.id();
        let created_ticks = winproc::process_facts(pid)
            .map(|facts| facts.created_ticks)
            .unwrap_or(0);
        let record = OwnedServer {
            pid,
            created_ticks,
            adopted: false,
            port: spec.port,
            started_ms: crate::events::now_ms(),
        };
        let _ = settings::save_runtime(&RuntimeState {
            schema: 1,
            server: Some(record.clone()),
        });

        self.log_tail.clear();
        self.progress_pct = None;
        self.stage = "launching".to_string();
        self.state = "starting".to_string();
        self.detail = format!("process {pid} launched");
        self.started_ms = Some(record.started_ms);
        self.last_error.clear();
        self.stopping = false;

        if let Some(stream) = child.stdout.take() {
            spawn_log_reader(stream, "stdout");
        }
        if let Some(stream) = child.stderr.take() {
            spawn_log_reader(stream, "stderr");
        }

        self.child = Some(child);
        self.record = Some(record);
        events.push(
            "ok",
            format!(
                "Launching vLLM on port {} with the optimized configuration (pid {pid}).",
                spec.port
            ),
        );
        ActionResult::ok(format!("Starting vLLM (pid {pid})"))
    }

    /// First half of a stop: decide whether we may, and take the record out of
    /// play. The terminating itself happens without the state lock held.
    pub fn begin_stop(&mut self, events: &mut crate::events::EventRing) -> StopPlan {
        let discovery = self.discover();
        if discovery.listener_pid.is_none() && !discovery.owned {
            self.reset_after_stop();
            return StopPlan::Nothing;
        }
        let Some(record) = self.record.clone() else {
            return StopPlan::NotOwned(discovery.listener_pid.unwrap_or(0));
        };
        if !discovery.owned {
            events.push(
                "warn",
                "Refusing to stop a server this application did not start. \
                 Use Adopt first if it should be managed from here.",
            );
            return StopPlan::NotOwned(discovery.listener_pid.unwrap_or(0));
        }
        let timeout = self.stop_timeout_s;
        self.state = "stopping".to_string();
        self.detail = format!("terminating pid {}", record.pid);
        self.stopping = true;
        events.push(
            "info",
            format!("Stopping the vLLM process tree rooted at pid {}.", record.pid),
        );
        StopPlan::Terminate {
            root: record.pid,
            timeout_ms: timeout * 1000,
        }
    }

    pub fn finish_stop(&mut self, ok: bool, events: &mut crate::events::EventRing) -> ActionResult {
        let root = self.record.as_ref().map(|r| r.pid).unwrap_or(0);
        self.reset_after_stop();
        if ok {
            events.push("ok", "Server stopped.");
            ActionResult::ok("Server stopped")
        } else {
            self.state = "error".to_string();
            self.last_error = format!("pid {root} survived termination");
            events.push(
                "error",
                format!("Server pid {root} did not exit after being terminated."),
            );
            ActionResult::fail(format!("pid {root} did not exit"))
        }
    }

    fn reset_after_stop(&mut self) {
        self.child = None;
        self.record = None;
        self.health = Health::default();
        self.health_failures = 0;
        self.state = "stopped".to_string();
        self.detail = "not running".to_string();
        self.stage = "idle".to_string();
        self.progress_pct = None;
        self.started_ms = None;
        self.stopping = false;
        let _ = settings::save_runtime(&RuntimeState {
            schema: 1,
            server: None,
        });
    }

    /// Take ownership of a server that was started outside this application.
    pub fn adopt(&mut self, events: &mut crate::events::EventRing) -> ActionResult {
        let port = Self::port();
        let Some(listener) = winproc::listeners_on(port).first().map(|l| l.pid) else {
            return ActionResult::fail("Nothing is listening on the inference port");
        };
        // The launcher shim, its python parent and the worker that finally binds
        // the port are three processes for one logical server. Own the outermost
        // one so a stop takes all of it — but only climb while the parent is
        // demonstrably running vLLM. A bare "python" ancestor is not enough:
        // interpreters that merely live in a folder named like the project would
        // otherwise be adopted along with the server.
        let launches_vllm = |pid: u32| {
            winproc::process_cmdline(pid)
                .map(|line| {
                    let line = line.to_ascii_lowercase();
                    line.contains("vllm.exe") || line.contains("vllm.entrypoints")
                })
                .unwrap_or(false)
        };
        let mut root = listener;
        if launches_vllm(listener) {
            for ancestor in winproc::ancestors(listener) {
                if launches_vllm(ancestor.pid) {
                    root = ancestor.pid;
                } else {
                    break;
                }
            }
        }
        let Some(facts) = winproc::process_facts(root) else {
            return ActionResult::fail("Could not read the server process");
        };
        let record = OwnedServer {
            pid: root,
            created_ticks: facts.created_ticks,
            adopted: true,
            port,
            started_ms: crate::events::now_ms(),
        };
        let _ = settings::save_runtime(&RuntimeState {
            schema: 1,
            server: Some(record.clone()),
        });
        self.started_ms = Some(record.started_ms);
        self.record = Some(record);
        self.state = if self.health.ok { "ready" } else { "loading" }.to_string();
        self.detail = "adopted; started outside this application".to_string();
        events.push(
            "warn",
            format!(
                "Adopted the externally started server (root pid {root}). Stop and \
                 Restart now apply to it."
            ),
        );
        ActionResult::ok(format!("Adopted pid {root}"))
    }

    // ------------------------------------------------------------------- health

    /// Blocking HTTP probe. Callers run this without the state lock held.
    pub fn probe_health(port: u16) -> Health {
        let url = format!("http://127.0.0.1:{port}/v1/models");
        let began = Instant::now();
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_millis(1200))
            .timeout(Duration::from_millis(2500))
            .build();
        match agent.get(&url).call() {
            Ok(response) => {
                let latency = began.elapsed().as_secs_f64() * 1000.0;
                if response.status() != 200 {
                    return Health {
                        ok: false,
                        latency_ms: Some(latency),
                        models: Vec::new(),
                        error: Some(format!("HTTP {}", response.status())),
                    };
                }
                let body = response.into_string().unwrap_or_default();
                let models = serde_json::from_str::<serde_json::Value>(&body)
                    .ok()
                    .and_then(|value| {
                        value.get("data").and_then(|d| d.as_array()).map(|arr| {
                            arr.iter()
                                .filter_map(|m| {
                                    m.get("id").and_then(|i| i.as_str()).map(String::from)
                                })
                                .collect::<Vec<_>>()
                        })
                    })
                    .unwrap_or_default();
                Health {
                    ok: true,
                    latency_ms: Some(latency),
                    models,
                    error: None,
                }
            }
            Err(error) => Health {
                ok: false,
                latency_ms: None,
                models: Vec::new(),
                error: Some(match error {
                    ureq::Error::Status(code, _) => format!("HTTP {code}"),
                    ureq::Error::Transport(transport) => transport.to_string(),
                }),
            },
        }
    }

    pub fn apply_health(&mut self, health: Health) {
        self.health_failures = if health.ok {
            0
        } else {
            self.health_failures.saturating_add(1)
        };
        self.health = health;
        let discovery = self.discover();
        if self.health.ok {
            if self.state != "stopping" {
                self.state = "ready".to_string();
                self.stage = "serving".to_string();
                // Saturate an observed weight-loading bar, but never invent one:
                // a server we adopted was already loaded before we looked.
                self.progress_pct = self.progress_pct.map(|_| 100.0);
                self.detail = "healthy".to_string();
            }
        } else if self.state == "ready" {
            self.state = "loading".to_string();
            self.detail = "health check failing".to_string();
        } else if self.state == "stopped" && discovery.listener_pid.is_some() {
            self.state = if discovery.owned { "loading" } else { "external" }.to_string();
            self.detail = if discovery.owned {
                "starting".to_string()
            } else {
                "started outside this application".to_string()
            };
        }
    }

    // -------------------------------------------------------------- log stream

    pub fn absorb_line(&mut self, line: &str, events: &mut crate::events::EventRing) {
        let text = line.trim();
        if text.is_empty() {
            return;
        }
        self.log_tail.push_back(text.chars().take(400).collect());
        while self.log_tail.len() > LOG_TAIL {
            self.log_tail.pop_front();
        }

        let lower = text.to_ascii_lowercase();
        if READY_HINTS.iter().any(|hint| lower.contains(hint)) {
            if self.stage != "serving" {
                self.stage = "serving".to_string();
                events.push("ok", "vLLM reports the API is serving.");
            }
            if self.state == "starting" || self.state == "loading" {
                self.detail = "API is serving".to_string();
            }
        }
        if let Some(percent) = scan_percent(text) {
            // vLLM's weight-loading bars are the one place a real percentage is
            // printed; anything else that says "50%" is not progress.
            let weight_loading = lower.contains("shard")
                || lower.contains("loading")
                || lower.contains("safetensors")
                || lower.contains("it/s")
                || lower.contains("downloading");
            if weight_loading {
                self.progress_pct = Some(percent.clamp(0.0, 99.0));
                self.stage = "weights".to_string();
                if self.state == "starting" {
                    self.state = "loading".to_string();
                }
            }
        }
        if ERROR_HINTS.iter().any(|hint| lower.contains(hint)) {
            self.last_error = text.chars().take(200).collect();
            events.push("error", format!("vLLM: {text}"));
            return;
        }
        // Ordinary output stays in the tail the Server page renders. Mirroring
        // every line into the event ring would evict the lifecycle events that
        // the Logs page exists to show.
    }

    /// Detect the child exiting on its own, and report it as an event.
    pub fn refresh(&mut self, events: &mut crate::events::EventRing) {
        if let Some(child) = self.child.as_mut() {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let code = status.code().unwrap_or(-1);
                    let intentional = self.stopping;
                    self.child = None;
                    self.record = None;
                    let _ = settings::save_runtime(&RuntimeState {
                        schema: 1,
                        server: None,
                    });
                    // A launcher shim (a venv redirector, a batch wrapper) can
                    // exit while the process it started keeps the port. Claiming
                    // a clean stop there would be a lie and would leave the
                    // server unmanaged, so check the port before deciding.
                    let still_listening = winproc::listeners_on(Self::port()).first().is_some();
                    if intentional {
                        self.state = "stopped".to_string();
                        self.detail = "stopped".to_string();
                    } else if code == 0 && still_listening {
                        self.state = "external".to_string();
                        self.detail = "launcher exited but the port is still in use".to_string();
                        events.push(
                            "warn",
                            "The launcher exited but something is still listening on the inference port; adopt it to keep control.",
                        );
                    } else if code == 0 {
                        self.state = "stopped".to_string();
                        self.detail = "exited cleanly".to_string();
                        events.push("warn", "vLLM exited on its own.");
                    } else {
                        self.state = "error".to_string();
                        self.detail = format!("exited with code {code}");
                        self.last_error = format!("exit code {code}");
                        events.push(
                            "error",
                            format!("vLLM exited unexpectedly with code {code}."),
                        );
                    }
                    self.stage = "idle".to_string();
                    self.progress_pct = None;
                    self.started_ms = None;
                }
                Ok(None) => {}
                Err(error) => {
                    self.detail = format!("could not query the child process: {error}");
                }
            }
        }

        // An adopted process is not our child: it disappears from the record
        // instead of exiting through `try_wait`.
        if self.child.is_none() {
            if let Some(record) = self.record.clone() {
                if !winproc::same_process(record.pid, record.created_ticks) {
                    self.record = None;
                    let _ = settings::save_runtime(&RuntimeState {
                        schema: 1,
                        server: None,
                    });
                    if !self.stopping {
                        self.state = "stopped".to_string();
                        self.stage = "idle".to_string();
                        self.detail = "process is gone".to_string();
                        events.push("warn", format!("Server pid {} exited.", record.pid));
                    }
                }
            }
        }
    }

    // ---------------------------------------------------------------- snapshot

    pub fn view(&self) -> ServerView {
        let port = Self::port();
        let discovery = self.discover();
        let listener = discovery.listener_pid;
        let process_name = listener
            .and_then(winproc::process_facts)
            .map(|facts| facts.name);
        let uptime = listener
            .and_then(winproc::process_facts)
            .and_then(|facts| winproc::process_uptime_s(facts.created_ticks));

        let mut state = self.state.clone();
        let mut detail = self.detail.clone();
        if listener.is_some() && state == "stopped" {
            state = if discovery.owned { "loading" } else { "external" }.to_string();
            detail = if discovery.owned {
                "starting".to_string()
            } else {
                "started outside this application".to_string()
            };
        }
        // A passing probe is only meaningful while the port still belongs to us:
        // the result outlives the process by up to one poll interval.
        if self.health.ok && (listener.is_some() || discovery.owned) {
            state = "ready".to_string();
        }

        let can_stop = discovery.owned;
        let can_start = listener.is_none() && !discovery.owned;
        let paths_ok = Self::launch_spec().is_ok();

        ServerView {
            state,
            detail,
            stage: self.stage.clone(),
            progress_pct: self.progress_pct,
            owned: discovery.owned,
            adopted: self.record.as_ref().map(|r| r.adopted).unwrap_or(false),
            pid: listener.or(self.record.as_ref().map(|r| r.pid)),
            listening: listener.is_some(),
            process_name,
            uptime_s: uptime,
            health_ok: self.health.ok,
            latency_ms: self.health.latency_ms,
            health_error: self.health.error.clone(),
            health_failures: self.health_failures,
            models: self.health.models.clone(),
            endpoint: format!("http://127.0.0.1:{port}/v1"),
            port,
            started_ms: self.started_ms,
            // Bounded twice on purpose: the tail is for reading, and a snapshot
            // is fetched every couple of seconds.
            log_tail: self
                .log_tail
                .iter()
                .skip(self.log_tail.len().saturating_sub(60))
                .cloned()
                .collect(),
            last_error: self.last_error.clone(),
            paths_ok,
            can_start,
            can_stop,
            can_restart: can_stop,
            can_adopt: listener.is_some() && !discovery.owned,
        }
    }
}

pub enum StopPlan {
    Nothing,
    NotOwned(u32),
    Terminate { root: u32, timeout_ms: u64 },
}

/// Read a child's output and feed it to the shared state a line at a time.
///
/// vLLM's progress bars are written with carriage returns, so a "line" is
/// anything up to either separator and only the last fragment matters.
fn spawn_log_reader<R: Read + Send + 'static>(stream: R, _name: &'static str) {
    std::thread::spawn(move || {
        let Some(shared) = crate::core::try_shared() else {
            return;
        };
        let mut reader = BufReader::new(stream);
        let mut buffer = [0u8; 4096];
        let mut pending: Vec<u8> = Vec::new();
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => {
                    for byte in &buffer[..count] {
                        if *byte == b'\n' || *byte == b'\r' {
                            if !pending.is_empty() {
                                let line = String::from_utf8_lossy(&pending).to_string();
                                shared.lock().unwrap().absorb_log_line(&line);
                            }
                            pending.clear();
                        } else {
                            pending.push(*byte);
                            if pending.len() > 8192 {
                                pending.clear();
                            }
                        }
                    }
                }
                Err(_) => break,
            }
        }
        if !pending.is_empty() {
            let line = String::from_utf8_lossy(&pending).to_string();
            shared.lock().unwrap().absorb_log_line(&line);
        }
    });
}

/// Last `NN%` in a line, if any.
fn scan_percent(text: &str) -> Option<f64> {
    let bytes = text.as_bytes();
    let mut best = None;
    for (index, byte) in bytes.iter().enumerate() {
        if *byte != b'%' {
            continue;
        }
        let mut start = index;
        while start > 0 && (bytes[start - 1] == b'.' || bytes[start - 1].is_ascii_digit()) {
            start -= 1;
        }
        if start < index {
            if let Ok(value) = text[start..index].parse::<f64>() {
                best = Some(value);
            }
        }
    }
    best
}
