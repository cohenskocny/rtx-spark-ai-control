//! Controller for the validated blackout engine.
//!
//! V2 never paints an overlay, creates a display power request or duplicates the
//! engine's logic. It launches `blackout.py` — the engine that was already
//! validated — and decides whether that engine should exist. Painting, cursor
//! hiding, immediate input recovery, per-monitor DPI and the lock-screen drop
//! all stay where they were proven.
//!
//! Control surfaces are the engine's own, exactly as V1 uses them:
//!   * `--idle-minutes`, `--cover`, `--now` on the command line,
//!   * a `STOP` file beside the script to ask a running guard to exit,
//!   * `state.json`, which the engine rewrites every tick.

use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::events::EventRing;
use crate::settings::Settings;
use crate::winproc;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

/// The engine rewrites `state.json` on every tick (about four times a second),
/// so a fresh file is as reliable as asking it directly and costs no IPC.
const STATE_FRESH_S: u64 = 4;

#[derive(Serialize, Clone, Debug, Default)]
pub struct EngineState {
    pub active: bool,
    pub locked: bool,
    pub stopped: bool,
    pub display_held: bool,
    pub display_request: bool,
    pub idle_s: Option<f64>,
    pub idle_remaining_s: Option<f64>,
    pub utc: String,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct BlackoutView {
    pub running: bool,
    pub owned: bool,
    pub mode: String,
    pub active: bool,
    pub phase: String,
    pub display_held: bool,
    pub display_request: bool,
    pub locked: bool,
    pub idle_s: Option<f64>,
    pub idle_remaining_s: Option<f64>,
    pub updated_utc: String,
    pub cover: String,
    pub pid: Option<u32>,
    pub state_path: String,
    pub stop_file_present: bool,
    pub error: String,
    pub since_s: Option<f64>,
    pub idle_threshold_s: u64,
}

pub struct BlackoutManager {
    child: Option<Child>,
    pid: Option<u32>,
    pub mode: String,
    pub started_ms: Option<u64>,
    pub last_error: String,
}

impl Default for BlackoutManager {
    fn default() -> Self {
        Self::new()
    }
}

impl BlackoutManager {
    pub fn new() -> Self {
        Self {
            child: None,
            pid: None,
            mode: "manual".to_string(),
            started_ms: None,
            last_error: String::new(),
        }
    }

    // --------------------------------------------------------------- locations

    /// The interpreter that runs the engine: the configured blackout interpreter,
    /// or the inference environment's own Python when only that is set.
    fn python_path() -> String {
        let config = crate::paths::current();
        let python = config.blackout_python.trim();
        if !python.is_empty() {
            return python.to_string();
        }
        config.python_exe.trim().to_string()
    }

    fn script_path() -> PathBuf {
        PathBuf::from(crate::paths::current().blackout_engine.trim())
    }

    fn state_path() -> PathBuf {
        Self::script_path()
            .parent()
            .map(|dir| dir.join("state.json"))
            .unwrap_or_else(|| PathBuf::from("state.json"))
    }

    fn stop_path() -> PathBuf {
        Self::script_path()
            .parent()
            .map(|dir| dir.join("STOP"))
            .unwrap_or_else(|| PathBuf::from("STOP"))
    }

    // ------------------------------------------------------------------ status

    pub fn engine_state() -> EngineState {
        let text = match std::fs::read_to_string(Self::state_path()) {
            Ok(text) => text,
            Err(_) => return EngineState::default(),
        };
        let value: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
        EngineState {
            active: value.get("active").and_then(|v| v.as_bool()).unwrap_or(false),
            locked: value.get("locked").and_then(|v| v.as_bool()).unwrap_or(false),
            stopped: value.get("stopped").and_then(|v| v.as_bool()).unwrap_or(true),
            display_held: value
                .get("display_held")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            display_request: value
                .get("display_request")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            idle_s: value.get("idle_s").and_then(|v| v.as_f64()),
            idle_remaining_s: value.get("idle_remaining_s").and_then(|v| v.as_f64()),
            utc: value
                .get("utc")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
        }
    }

    fn state_age_s() -> Option<f64> {
        let meta = std::fs::metadata(Self::state_path()).ok()?;
        let modified = meta.modified().ok()?;
        modified.elapsed().ok().map(|d| d.as_secs_f64())
    }

    fn our_pid_alive(&mut self) -> bool {
        if let Some(child) = self.child.as_mut() {
            match child.try_wait() {
                Ok(None) => return true,
                Ok(Some(_status)) => {
                    self.child = None;
                }
                Err(_) => {
                    self.child = None;
                }
            }
        }
        if let Some(pid) = self.pid {
            if winproc::process_facts(pid).is_some() {
                return true;
            }
            self.pid = None;
        }
        false
    }

    /// Is any guard running, whoever started it?
    ///
    /// A guard we launched is tracked by handle. A guard launched elsewhere is
    /// inferred from `state.json`: the engine rewrites it on every change and at
    /// least once a second, and marks `stopped` in its exit path, so the
    /// inference is fresh in both directions.
    pub fn engine_present(&mut self) -> bool {
        if self.our_pid_alive() {
            return true;
        }
        let fresh = matches!(Self::state_age_s(), Some(age) if age <= STATE_FRESH_S as f64);
        fresh && !Self::engine_state().stopped
    }

    pub fn owns(&mut self) -> bool {
        self.our_pid_alive()
    }

    // ----------------------------------------------------------------- control

    /// Start the engine. `now` paints immediately instead of waiting for idle.
    pub fn start(
        &mut self,
        now: bool,
        mode: &str,
        settings: &Settings,
        events: &mut EventRing,
    ) -> crate::core::ActionResult {
        let state = Self::engine_state();
        if self.engine_present() {
            if now && !state.active {
                // The engine has no IPC for "paint now", so the honest path is a
                // restart with --now. Stopping first keeps it a single instance.
                events.push("info", "Activating the running blackout engine now.");
                let _ = self.stop(events);
                std::thread::sleep(Duration::from_millis(400));
                // Whoever owned the decision keeps it: a forced paint must not
                // take a guard out of Follow Server's control.
                let owner = self.mode.clone();
                return self.start(true, &owner, settings, events);
            }
            self.mode = mode.to_string();
            return crate::core::ActionResult::ok("Blackout engine already running");
        }

        let script = Self::script_path();
        let python = Self::python_path();
        if !script.exists() {
            self.last_error = format!("missing {}", script.display());
            events.push("error", format!("Blackout engine not found: {}", script.display()));
            return crate::core::ActionResult::fail(format!(
                "Blackout engine not found: {}",
                script.display()
            ));
        }
        if !PathBuf::from(&python).exists() {
            self.last_error = format!("missing {python}");
            events.push("error", format!("Blackout interpreter not found: {python}"));
            return crate::core::ActionResult::fail(format!("Interpreter not found: {python}"));
        }

        let idle_minutes = settings.idle_seconds as f64 / 60.0;
        let mut args = vec![
            script.to_string_lossy().to_string(),
            "--idle-minutes".to_string(),
            format!("{idle_minutes:.4}"),
            "--cover".to_string(),
            settings.cover.clone(),
        ];
        if now {
            args.push("--now".to_string());
        }
        let _ = std::fs::remove_file(Self::stop_path());

        let mut command = Command::new(&python);
        command
            .args(&args)
            .current_dir(script.parent().map(|p| p.to_path_buf()).unwrap_or_default())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);

        match command.spawn() {
            Ok(child) => {
                let pid = child.id();
                self.pid = Some(pid);
                self.child = Some(child);
                self.mode = mode.to_string();
                self.started_ms = Some(crate::events::now_ms());
                self.last_error.clear();
                events.push(
                    "blackout",
                    if now {
                        format!("Blackout forced on (cover={}, pid {pid}).", settings.cover)
                    } else {
                        format!(
                            "Blackout armed after {}s idle (cover={}, pid {pid}).",
                            settings.idle_seconds, settings.cover
                        )
                    },
                );
                crate::core::ActionResult::ok(if now {
                    "Blackout activated".to_string()
                } else {
                    "Blackout armed".to_string()
                })
            }
            Err(error) => {
                self.last_error = error.to_string();
                events.push("error", format!("Could not start blackout: {error}"));
                crate::core::ActionResult::fail(format!("Could not start blackout: {error}"))
            }
        }
    }

    /// Ask the engine to exit via its STOP sentinel, then escalate if needed.
    pub fn stop(&mut self, events: &mut EventRing) -> crate::core::ActionResult {
        if !self.engine_present() {
            let _ = std::fs::remove_file(Self::stop_path());
            self.child = None;
            self.pid = None;
            return crate::core::ActionResult::ok("Blackout already stopped");
        }
        let owned = self.owns();
        let tracked = self.pid;
        if let Err(error) = std::fs::write(Self::stop_path(), b"v2 stop") {
            self.last_error = error.to_string();
            events.push("error", format!("Could not write the STOP sentinel: {error}"));
            return crate::core::ActionResult::fail(format!("Could not stop blackout: {error}"));
        }

        let deadline = Instant::now() + Duration::from_secs(8);
        while Instant::now() < deadline {
            if !self.engine_present() {
                let _ = std::fs::remove_file(Self::stop_path());
                self.child = None;
                self.pid = None;
                events.push("blackout", "Blackout stopped; display request released.");
                return crate::core::ActionResult::ok("Blackout stopped");
            }
            std::thread::sleep(Duration::from_millis(150));
        }

        // Only ever a process this application spawned: an engine started from a
        // terminal is left to the operator.
        let mut terminated = false;
        if owned {
            if let Some(pid) = tracked.or(self.pid) {
                terminated = winproc::terminate_tree(pid, 4000);
            }
        }
        self.child = None;
        self.pid = None;
        let _ = std::fs::remove_file(Self::stop_path());
        if terminated {
            events.push(
                "warn",
                "Blackout did not exit on STOP; terminated. Windows drops the display \
                 request when the process handle closes.",
            );
            crate::core::ActionResult::ok("Blackout terminated")
        } else {
            events.push(
                "warn",
                "Blackout did not exit on STOP and was not started here, so it was left \
                 running.",
            );
            crate::core::ActionResult::fail("Blackout did not exit")
        }
    }

    /// Restart a running engine so a new timeout takes effect.
    pub fn apply_timeout(
        &mut self,
        settings: &Settings,
        events: &mut EventRing,
    ) -> crate::core::ActionResult {
        if !self.engine_present() {
            return crate::core::ActionResult::ok(format!(
                "Inactivity timeout set to {}s",
                settings.idle_seconds
            ));
        }
        let was_active = Self::engine_state().active;
        let mode = self.mode.clone();
        let _ = self.stop(events);
        std::thread::sleep(Duration::from_millis(300));
        let result = self.start(was_active, &mode, settings, events);
        events.push(
            "blackout",
            format!(
                "Idle timeout set to {}s (engine restarted to apply it).",
                settings.idle_seconds
            ),
        );
        result
    }

    /// Notice a guard that exited on its own (including a refused duplicate,
    /// which the engine reports as exit code 3).
    pub fn refresh(&mut self, events: &mut EventRing) {
        if let Some(child) = self.child.as_mut() {
            if let Ok(Some(status)) = child.try_wait() {
                let code = status.code().unwrap_or(-1);
                self.child = None;
                self.pid = None;
                if code == 3 {
                    self.last_error = "another blackout instance is already running".to_string();
                    events.push(
                        "warn",
                        "Blackout engine refused to start: another instance already holds the \
                         single-instance mutex.",
                    );
                } else if code != 0 {
                    self.last_error = format!("engine exited with code {code}");
                    events.push(
                        "error",
                        format!("Blackout engine exited with code {code}."),
                    );
                } else {
                    events.push("blackout", "Blackout engine exited.");
                }
            }
        }
    }

    pub fn view(&mut self, settings: &Settings) -> BlackoutView {
        let state = Self::engine_state();
        let running = self.engine_present();
        let owned = running && self.owns();
        let phase = if !running {
            "off"
        } else if state.active {
            "active"
        } else {
            "armed"
        };
        BlackoutView {
            running,
            owned,
            mode: if self.pid.is_some() || running {
                self.mode.clone()
            } else {
                "idle".to_string()
            },
            active: state.active,
            phase: phase.to_string(),
            display_held: state.display_held,
            display_request: state.display_request,
            locked: state.locked,
            idle_s: state.idle_s,
            idle_remaining_s: state.idle_remaining_s,
            updated_utc: state.utc.clone(),
            cover: settings.cover.clone(),
            pid: self.pid,
            state_path: Self::state_path().to_string_lossy().to_string(),
            stop_file_present: Self::stop_path().exists(),
            error: self.last_error.clone(),
            since_s: self
                .started_ms
                .filter(|_| running)
                .map(|started| (crate::events::now_ms().saturating_sub(started)) as f64 / 1000.0),
            idle_threshold_s: settings.idle_seconds,
        }
    }
}
