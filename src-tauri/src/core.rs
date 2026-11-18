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
use crate::paths::{self, Paths};
use crate::power::{self, PowerView};
use crate::settings::{self, Settings};
use crate::telemetry::{self, GpuInfo, ModelInfo};
use crate::upstream::{self, UpstreamView};
use crate::version::{self, Compat, Installed};
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
    /// What is installed here, read from disk rather than by importing it.
    pub installed: Installed,
    /// How that compares with the newest upstream release.
    pub compat: Compat,
    /// The last known upstream state; refreshed daily by its own thread.
    pub upstream: UpstreamView,
    /// Battery/AC state, sampled with the snapshot.
    pub power: PowerView,
}

impl Core {
    pub fn new() -> Self {
        let settings = settings::load_settings();
        paths::install(&settings.paths);
        let mut events = EventRing::new();
        events.push("info", "Control center started; telemetry and supervisors online.");
        let power = power::sample();
        events.push(
            if power.on_battery() { "warn" } else { "info" },
            match power.source.as_str() {
                "battery" => "On battery power: automatic inference start is disabled.".to_string(),
                "ac" => "On AC power: configured automation may start inference.".to_string(),
                "no-battery" => "Mains power detected; no battery reported.".to_string(),
                _ => "Power source could not be read; automatic inference start stays disabled."
                    .to_string(),
            },
        );
        let installed = version::read(&settings.paths);
        let upstream = upstream::view_from_cache(&installed.revision);
        let compat = version::assess(
            &installed,
            upstream.latest.as_ref().map(|release| release.tag.as_str()),
        );
        if settings.needs_setup() {
            events.push(
                "warn",
                "No inference installation is configured yet; open Setup to choose one.",
            );
        }
        Self {
            vllm: VllmManager::load(settings.stop_timeout_s),
            blackout: BlackoutManager::new(),
            auto: Automation::default(),
            settings,
            events,
            last_action: None,
            started: Instant::now(),
            installed,
            compat,
            upstream,
            power,
        }
    }

    /// Re-read what is installed after the paths change, and re-angle the update
    /// view at the new revision.
    pub fn refresh_installed(&mut self) {
        self.installed = version::read(&self.settings.paths);
        self.upstream.installed_revision = self.installed.revision.clone();
        self.compat = version::assess(
            &self.installed,
            self.upstream.latest.as_ref().map(|release| release.tag.as_str()),
        );
    }

    /// Store the configured paths and make them the ones the backend reads.
    pub fn apply_paths(&mut self, next: Paths) {
        self.settings.paths = next;
        self.settings = self.settings.clone().clamped();
        paths::install(&self.settings.paths);
        self.persist_settings();
        self.refresh_installed();
    }

    /// Adopt the result of an update check.
    pub fn apply_upstream(&mut self, view: UpstreamView) {
        let latest = view.latest.as_ref().map(|release| release.tag.clone());
        let previous = self.upstream.latest.as_ref().map(|release| release.tag.clone());
        let was_ahead = self.upstream.commits_ahead;
        self.compat = version::assess(&self.installed, latest.as_deref());
        self.upstream = view;

        if latest != previous {
            match &self.upstream.latest {
                Some(release) => self.note(
                    "info",
                    format!(
                        "Upstream {} is the newest release; installed {} — {}",
                        release.tag, self.installed.version, self.compat.message
                    ),
                ),
                None => self.note("warn", "No upstream release could be read."),
            }
        }
        if self.upstream.commits_ahead && !was_ahead {
            self.note(
                "info",
                "Upstream has commits newer than the installed source revision (no new release).",
            );
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
        // One synchronous call into the kernel: cheap enough for every sample, and
        // it is what decides whether the interface offers Start at all.
        self.power = power::sample();
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
            config: ConfigView::from(&self.settings.paths),
            power: self.power.clone(),
            installed: self.installed.clone(),
            compat: self.compat.clone(),
            upstream: self.upstream.clone(),
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
    pub config: ConfigView,
    pub power: PowerView,
    pub installed: Installed,
    pub compat: Compat,
    pub upstream: UpstreamView,
    pub events: Vec<Event>,
    pub notes: Vec<String>,
    pub last_action: Option<ActionEcho>,
    pub server_uptime_s: u128,
}

/// One configured path, with the verdict the Setup page shows beside it.
#[derive(Serialize, Clone, Debug)]
pub struct FieldView {
    pub field: String,
    pub kind: String,
    pub value: String,
    pub ok: bool,
    pub message: String,
}

/// The portable configuration, pre-validated so the interface never has to guess.
#[derive(Serialize, Clone, Debug)]
pub struct ConfigView {
    pub paths: Paths,
    pub fields: Vec<FieldView>,
    pub api_host: String,
    pub api_port: u16,
    pub endpoint: String,
    pub launch_ready: bool,
    pub blackout_ready: bool,
    pub needs_setup: bool,
    pub missing_launch: Vec<String>,
    pub missing_blackout: Vec<String>,
}

impl From<&Paths> for ConfigView {
    fn from(paths: &Paths) -> Self {
        let fields = paths::FIELDS
            .iter()
            .map(|(field, kind)| {
                let value = paths.field(field).to_string();
                let verdict = paths::check(&value, *kind);
                let (ok, message) = match (&value, verdict) {
                    (_, Ok(_)) => (true, String::new()),
                    (text, Err(_message)) if text.trim().is_empty() && kind.optional() => {
                        (true, "not set (optional)".to_string())
                    }
                    (_, Err(message)) => (false, message),
                };
                FieldView {
                    field: field.to_string(),
                    kind: kind.label().to_string(),
                    value,
                    ok,
                    message,
                }
            })
            .collect();
        Self {
            paths: paths.clone(),
            fields,
            api_host: paths.host(),
            api_port: paths.port(),
            endpoint: paths.endpoint(),
            launch_ready: paths.launch_ready(),
            blackout_ready: paths.blackout_ready(),
            needs_setup: paths.launch_missing().len() == 4,
            missing_launch: paths.launch_missing().iter().map(|s| s.to_string()).collect(),
            missing_blackout: paths
                .blackout_missing()
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }
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
    pub autostart_inference: bool,
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
            autostart_inference: settings.autostart_inference,
        }
    }
}

fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}
