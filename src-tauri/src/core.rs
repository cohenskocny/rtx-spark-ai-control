//! Shared runtime state and the snapshot the interface reads.
//!
//! One `Mutex<Core>` holds everything mutable: settings, the event ring, the
//! vLLM manager, the blackout controller and the automation state. A single lock
//! with short critical sections is easier to reason about than five locks with
//! an ordering nobody remembers, and the contention here is trivial — one
//! supervisor tick per second and one UI poll every two.
//!
//! Anything that blocks for a noticeable time (an HTTP probe, a process-tree
//! termination) is deliberately run *outside* the lock; see `supervisor.rs` and
//! the command handlers in `main.rs`.

use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use serde::Serialize;

use crate::blackout::BlackoutManager;
use crate::events::{Event, EventRing};
use crate::settings::{self, Settings};
use crate::telemetry::{self, GpuInfo, ModelInfo};
use crate::vllm::VllmManager;

pub type Shared = Arc<Mutex<Core>>;

static SHARED: OnceLock<Shared> = OnceLock::new();

pub fn set_shared(shared: Shared) {
    let _ = SHARED.set(shared);
}

pub fn try_shared() -> Option<Shared> {
    SHARED.get().cloned()
}

/// Result of a user action, echoed into the interface so a click always has a
/// visible consequence even when the backend does something asynchronous.
#[derive(Serialize, Clone, Debug)]
pub struct ActionResult {
    pub ok: bool,
    pub message: String,
}

impl ActionResult {
    pub fn ok(message: impl Into<String>) -> Self {
        Self {
            ok: true,
            message: message.into(),
        }
    }

    pub fn fail(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            message: message.into(),
        }
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct ActionEcho {
    pub ok: bool,
    pub message: String,
    pub at: u64,
}

/// Idle/follow-server bookkeeping. The tick itself lives in `supervisor.rs`.
pub struct Automation {
    /// Idle is counted from arming, never from stale system idle: launching the
    /// guard on a machine that has been idle for ten minutes must not paint it
    /// immediately.
    pub anchor: Instant,
    pub last_tick: u32,
    pub idle_s: f64,
    pub armed: bool,
    pub server_ok: bool,
    pub ok_since: Option<Instant>,
    pub fail_since: Option<Instant>,
    pub suspended: bool,
    pub phase: String,
    pub reason: String,
}

impl Default for Automation {
    fn default() -> Self {
        Self {
            anchor: Instant::now(),
            last_tick: crate::winproc::input_tick(),
            idle_s: 0.0,
            armed: false,
            server_ok: false,
            ok_since: None,
            fail_since: None,
            suspended: false,
            phase: "disabled".to_string(),
            reason: "Automatic blackout is off".to_string(),
        }
    }
}

pub struct Core {
    pub settings: Settings,
    pub events: EventRing,
    pub vllm: VllmManager,
    pub blackout: BlackoutManager,
    pub auto: Automation,
    pub last_action: Option<ActionEcho>,
    pub started: Instant,
}

impl Core {
    pub fn new() -> Self {
        let settings = settings::load_settings();
        let mut events = EventRing::new();
        events.push("info", "Control center started; telemetry and supervisors online.");
        Self {
            vllm: VllmManager::load(settings.stop_timeout_s),
            blackout: BlackoutManager::new(),
            auto: Automation::default(),
            settings,
            events,
            last_action: None,
            started: Instant::now(),
        }
    }

    /// Run `f` with both the vLLM manager and the event ring.
    pub fn with_vllm<T>(&mut self, f: impl FnOnce(&mut VllmManager, &mut EventRing) -> T) -> T {
        let mut events = std::mem::take(&mut self.events);
        let result = f(&mut self.vllm, &mut events);
        self.events = events;
        result
    }

    /// Run `f` with both the blackout controller and the event ring.
    pub fn with_blackout<T>(
        &mut self,
        f: impl FnOnce(&mut BlackoutManager, &mut EventRing) -> T,
    ) -> T {
        let mut events = std::mem::take(&mut self.events);
        let result = f(&mut self.blackout, &mut events);
        self.events = events;
        result
    }

    pub fn note(&mut self, kind: &str, text: impl AsRef<str>) {
        self.events.push(kind, text);
    }

    pub fn absorb_log_line(&mut self, line: &str) {
        self.with_vllm(|vllm, events| vllm.absorb_line(line, events));
    }

    pub fn record_action(&mut self, result: &ActionResult) {
        self.last_action = Some(ActionEcho {
            ok: result.ok,
            message: result.message.clone(),
            at: crate::events::now_ms(),
        });
    }

    pub fn persist_settings(&mut self) {
        if let Err(error) = settings::save_settings(&self.settings) {
            self.note("error", format!("Could not save settings: {error}"));
        }
    }

    pub fn snapshot(&mut self, last_event_id: u64) -> Snapshot {
        let began = Instant::now();
        let system = telemetry::collect_system();
        let server = self.vllm.view();
        let blackout = self.blackout.view(&self.settings);
        let automation = self.automation_view();
        let events = self.events.since(last_event_id);
        let mut snapshot = Snapshot {
            seq: system.seq,
            collected_ms: system.collected_ms,
            collect_duration_ms: 0,
            server_uptime_s: self.started.elapsed().as_millis(),
            host: system.host,
            model: system.model,
            gpu: system.gpu,
            server,
            blackout,
            automation,
            settings: SettingsView::from(&self.settings),
            events,
            notes: system.notes,
            last_action: self.last_action.clone(),
        };
        // The whole snapshot, not just the system half: this is the number the
        // Collector row shows.
        snapshot.collect_duration_ms = began.elapsed().as_millis();
        snapshot
    }

    fn automation_view(&self) -> AutomationView {
        let threshold = self.settings.idle_seconds as f64;
        AutomationView {
            phase: self.auto.phase.clone(),
            reason: self.auto.reason.clone(),
            idle_s: round1(self.auto.idle_s),
            idle_remaining_s: round1((threshold - self.auto.idle_s).max(0.0)),
            armed: self.auto.armed,
            server_ok: self.auto.server_ok,
            suspended: self.auto.suspended,
            follow_server: self.settings.follow_server,
            enabled: self.settings.blackout_enabled,
            grace_s: self.settings.grace_s,
            hysteresis_s: self.settings.hysteresis_s,
            poll_s: self.settings.poll_s,
        }
    }
}

/// One snapshot of everything the interface paints.
#[derive(Serialize, Clone, Debug)]
pub struct Snapshot {
    pub seq: u64,
    pub collected_ms: u128,
    pub collect_duration_ms: u128,
    pub host: String,
    pub model: ModelInfo,
    pub gpu: GpuInfo,
    pub server: crate::vllm::ServerView,
    pub blackout: crate::blackout::BlackoutView,
    pub automation: AutomationView,
    pub settings: SettingsView,
    pub events: Vec<Event>,
    pub notes: Vec<String>,
    pub last_action: Option<ActionEcho>,
    pub server_uptime_s: u128,
}

#[derive(Serialize, Clone, Debug)]
pub struct AutomationView {
    pub phase: String,
    pub reason: String,
    pub idle_s: f64,
    pub idle_remaining_s: f64,
    pub armed: bool,
    pub server_ok: bool,
    pub suspended: bool,
    pub follow_server: bool,
    pub enabled: bool,
    pub grace_s: u64,
    pub hysteresis_s: u64,
    pub poll_s: u64,
}

#[derive(Serialize, Clone, Debug)]
pub struct SettingsView {
    pub blackout_enabled: bool,
    pub idle_seconds: u64,
    pub follow_server: bool,
    pub cover: String,
    pub grace_s: u64,
    pub hysteresis_s: u64,
    pub poll_s: u64,
    pub stop_timeout_s: u64,
}

impl From<&Settings> for SettingsView {
    fn from(settings: &Settings) -> Self {
        Self {
            blackout_enabled: settings.blackout_enabled,
            idle_seconds: settings.idle_seconds,
            follow_server: settings.follow_server,
            cover: settings.cover.clone(),
            grace_s: settings.grace_s,
            hysteresis_s: settings.hysteresis_s,
            poll_s: settings.poll_s,
            stop_timeout_s: settings.stop_timeout_s,
        }
    }
}

fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}
