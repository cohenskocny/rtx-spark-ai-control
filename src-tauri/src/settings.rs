//! V2's own persisted state: operator settings and the process-ownership record.
//!
//! Two files, both under `%APPDATA%\com.rtxspark.aicontrol.v2`, both written
//! atomically (temp file + rename) so a crash mid-write cannot leave a half
//! document that the next launch has to guess about.
//!
//! Nothing outside that directory is ever written. The inference installation,
//! its configuration and any legacy controller configuration are read only, so
//! V1 stays exactly as it was and remains a usable fallback.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::paths::Paths;

pub const APP_DIR: &str = "com.rtxspark.aicontrol.v2";

pub fn data_dir() -> PathBuf {
    if let Ok(path) = std::env::var("V2_TEST_DATA_DIR") {
        if !path.trim().is_empty() {
            return PathBuf::from(path);
        }
    }
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
    /// Start inference when the application launches. Off by default, and never
    /// honoured on battery (see `power.rs`).
    pub autostart_inference: bool,
    /// Where the inference installation and its files live on this machine.
    pub paths: Paths,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema: 2,
            blackout_enabled: false,
            idle_seconds: 180,
            follow_server: false,
            cover: "internal".to_string(),
            grace_s: 300,
            hysteresis_s: 60,
            poll_s: 15,
            stop_timeout_s: 45,
            autostart_inference: false,
            paths: Paths::default(),
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
        // A port outside the range a listener can bind is a typo, not a choice.
        if self.paths.api_port == 0 {
            self.paths.api_port = 8000;
        }
        if self.paths.api_host.trim().is_empty() {
            self.paths.api_host = "127.0.0.1".to_string();
        }
        // Hosts are compared, never executed: keep them to hostname characters so
        // a stray scheme or path cannot end up in an endpoint string.
        let host = self.paths.api_host.trim().to_string();
        let host = host.split("://").last().unwrap_or(&host).to_string();
        let clean: String = host
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '[' | ']'))
            .collect();
        self.paths.api_host = if clean.is_empty() {
            "127.0.0.1".to_string()
        } else {
            clean
        };
        self.schema = 2;
        self
    }

    /// True when nothing has been configured yet, which is what first-run setup keys on.
    pub fn needs_setup(&self) -> bool {
        self.paths.launch_missing().len() == 4
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

// ------------------------------------------------------------------- unit tests

#[cfg(test)]
mod tests {
    use super::*;

    /// Serialise the tests that redirect the data directory.
    fn with_data_dir<T>(name: &str, body: impl FnOnce(&std::path::Path) -> T) -> T {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let dir = std::env::temp_dir().join(format!("v2-settings-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("V2_TEST_DATA_DIR", &dir);
        let result = body(&dir);
        std::env::remove_var("V2_TEST_DATA_DIR");
        let _ = std::fs::remove_dir_all(&dir);
        result
    }

    #[test]
    fn a_fresh_install_needs_setup_and_holds_no_paths() {
        let settings = Settings::default();
        assert!(settings.needs_setup());
        assert!(settings.paths.vllm_exe.is_empty());
        assert_eq!(settings.paths.endpoint(), "http://127.0.0.1:8000/v1");
    }

    #[test]
    fn settings_and_paths_survive_a_save_and_load() {
        with_data_dir("roundtrip", |_dir| {
            let mut settings = Settings::default();
            settings.idle_seconds = 240;
            settings.autostart_inference = true;
            settings.paths.vllm_root = r"C:\somewhere\native-vLLM".to_string();
            settings.paths.model_checkpoint = r"D:\models\demo".to_string();
            settings.paths.api_port = 8011;
            save_settings(&settings).expect("save");

            let loaded = load_settings();
            assert_eq!(loaded.idle_seconds, 240);
            assert!(loaded.autostart_inference);
            assert_eq!(loaded.paths.vllm_root, r"C:\somewhere\native-vLLM");
            assert_eq!(loaded.paths.model_checkpoint, r"D:\models\demo");
            assert_eq!(loaded.paths.api_port, 8011);
            assert!(!loaded.needs_setup());
        });
    }

    #[test]
    fn a_settings_file_from_before_paths_existed_still_loads() {
        with_data_dir("legacy", |dir| {
            std::fs::write(
                dir.join("settings.json"),
                r#"{"schema":1,"blackout_enabled":true,"idle_seconds":180,"follow_server":true,
                    "cover":"internal","grace_s":300,"hysteresis_s":60,"poll_s":15,
                    "stop_timeout_s":45}"#,
            )
            .unwrap();
            let loaded = load_settings();
            // Known keys are kept, and the new block is simply empty.
            assert!(loaded.blackout_enabled);
            assert!(loaded.follow_server);
            assert!(!loaded.autostart_inference);
            assert!(loaded.paths.vllm_exe.is_empty());
            assert!(loaded.needs_setup());
        });
    }

    #[test]
    fn invalid_ports_and_hosts_are_clamped_not_trusted() {
        let mut settings = Settings::default();
        settings.paths.api_port = 0;
        settings.paths.api_host = "  ".to_string();
        let clamped = settings.clamped();
        assert_eq!(clamped.paths.api_port, 8000);
        assert_eq!(clamped.paths.api_host, "127.0.0.1");

        let mut settings = Settings::default();
        settings.paths.api_host = "http://evil.example/path?x=1".to_string();
        let clamped = settings.clamped();
        assert_eq!(clamped.paths.api_host, "evil.examplepathx1");
        assert!(!clamped.paths.api_host.contains('/'));
        assert!(!clamped.paths.api_host.contains('?'));
        assert!(!clamped.paths.api_host.contains(':') || clamped.paths.api_host.len() < 30);
    }

    #[test]
    fn out_of_range_intervals_are_clamped() {
        let clamped = Settings {
            idle_seconds: 1,
            grace_s: 99_999,
            hysteresis_s: 0,
            poll_s: 5000,
            stop_timeout_s: 0,
            ..Settings::default()
        }
        .clamped();
        assert_eq!(clamped.idle_seconds, 5);
        assert_eq!(clamped.grace_s, 3600);
        assert_eq!(clamped.hysteresis_s, 5);
        assert_eq!(clamped.poll_s, 120);
        assert_eq!(clamped.stop_timeout_s, 5);
    }
}
