//! V2's own persisted state: operator settings and the process-ownership record.
//!
//! Two files, both under `%APPDATA%\com.rtxspark.aicontrol.v2`, both written
//! atomically (temp file + rename) so a crash mid-write cannot leave a half
//! document that the next launch has to guess about.
//!
//! V1's `config.json` is never written: it supplies paths and defaults, and V1
//! stays exactly as it was so it remains a usable fallback.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const APP_DIR: &str = "com.rtxspark.aicontrol.v2";

fn data_dir() -> PathBuf {
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(base).join(APP_DIR)
}

fn settings_path() -> PathBuf {
    data_dir().join("settings.json")
}

fn runtime_path() -> PathBuf {
    data_dir().join("runtime-state.json")
}

/// Operator settings that outlive the window.
///
/// `default` at the container level so a settings file written by an older build
/// keeps its known keys instead of being discarded for a missing one.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    pub schema: u32,
    /// Automatic blackout on/off.
    pub blackout_enabled: bool,
    /// Idle timeout in seconds. The validated default is 180.
    pub idle_seconds: u64,
    /// Only arm blackout while the inference server is healthy.
    pub follow_server: bool,
    /// `internal` covers the OLED panel only.
    pub cover: String,
    /// How long a failing health check is tolerated before blackout is released.
    pub grace_s: u64,
    /// How long a new health state must hold before it is believed.
    pub hysteresis_s: u64,
    /// Supervisor health-check interval.
    pub poll_s: u64,
    /// How long a graceful stop may take before the tree is terminated.
    pub stop_timeout_s: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema: 1,
            blackout_enabled: false,
            idle_seconds: 180,
            follow_server: false,
            cover: "internal".to_string(),
            grace_s: 300,
            hysteresis_s: 60,
            poll_s: 15,
            stop_timeout_s: 45,
        }
    }
}

impl Settings {
    /// Clamp anything a person or an old file could plausibly get wrong.
    pub fn clamped(mut self) -> Self {
        self.idle_seconds = self.idle_seconds.clamp(5, 7200);
        self.grace_s = self.grace_s.clamp(5, 3600);
        self.hysteresis_s = self.hysteresis_s.clamp(5, 600);
        self.poll_s = self.poll_s.clamp(5, 120);
        self.stop_timeout_s = self.stop_timeout_s.clamp(5, 300);
        if !matches!(self.cover.as_str(), "internal" | "all" | "primary") {
            self.cover = "internal".to_string();
        }
        self.schema = 1;
        self
    }
}

pub fn load_settings() -> Settings {
    match std::fs::read_to_string(settings_path()) {
        Ok(text) => serde_json::from_str::<Settings>(&text)
            .unwrap_or_default()
            .clamped(),
        Err(_) => Settings::default().clamped(),
    }
}

pub fn save_settings(settings: &Settings) -> std::io::Result<()> {
    write_json(&settings_path(), &serde_json::to_value(settings.clone().clamped())?)
}

/// The process this application started or adopted, persisted so ownership
/// survives a restart of the GUI.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct OwnedServer {
    pub pid: u32,
    pub created_ticks: u64,
    pub adopted: bool,
    pub port: u16,
    pub started_ms: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct RuntimeState {
    pub schema: u32,
    #[serde(default)]
    pub server: Option<OwnedServer>,
}

pub fn load_runtime() -> RuntimeState {
    match std::fs::read_to_string(runtime_path()) {
        Ok(text) => serde_json::from_str::<RuntimeState>(&text).unwrap_or_default(),
        Err(_) => RuntimeState::default(),
    }
}

pub fn save_runtime(state: &RuntimeState) -> std::io::Result<()> {
    write_json(&runtime_path(), &serde_json::to_value(state)?)
}

fn write_json(path: &PathBuf, value: &serde_json::Value) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, serde_json::to_vec_pretty(value)?)?;
    std::fs::rename(&temp, path)?;
    Ok(())
}
