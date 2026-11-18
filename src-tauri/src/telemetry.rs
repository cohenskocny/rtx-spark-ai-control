//! Telemetry collection: GPU via nvidia-smi, vLLM over loopback HTTP, and the
//! read-only V1 configuration/blackout state on disk.
//!
//! Nothing here mutates the environment: no spawn of vLLM, no display power
//! requests, no writes to V1's directory.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Where V1 keeps its config. Read-only.
pub const V1_ROOT: &str = r"C:\Users\cohen\AI\RTX-Spark-AI-Control";
pub const V1_CONFIG: &str = r"C:\Users\cohen\AI\RTX-Spark-AI-Control\config.json";
pub const DEFAULT_BLACKOUT_STATE: &str =
    r"C:\Users\cohen\AI\RTX-Spark-AI-Blackout\state.json";

#[derive(Serialize, Clone, Debug, Default)]
pub struct GpuInfo {
    pub available: bool,
    pub name: String,
    pub util_pct: Option<f64>,
    pub mem_used_mib: Option<f64>,
    pub mem_total_mib: Option<f64>,
    pub temp_c: Option<f64>,
    pub power_w: Option<f64>,
    pub clock_mhz: Option<f64>,
    pub throttle: String,
    pub error: Option<String>,
    pub collected_ms: u128,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct ModelInfo {
    pub served_name: String,
    pub checkpoint: String,
    pub checkpoint_exists: bool,
    pub endpoint: String,
    pub config_path: String,
    pub chat_template: String,
    pub dtype: String,
    pub kv_cache_dtype: String,
    pub max_model_len: Option<u64>,
    pub gpu_memory_utilization: Option<f64>,
    pub mtp_depth: Option<u64>,
    pub max_num_seqs: Option<u64>,
    pub prefix_caching: bool,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct ServerInfo {
    pub state: String,
    pub listening: bool,
    pub pid: Option<u32>,
    pub process_name: Option<String>,
    pub uptime_s: Option<f64>,
    pub health_ok: bool,
    pub latency_ms: Option<f64>,
    pub models: Vec<String>,
    pub error: Option<String>,
    pub endpoint: String,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct BlackoutInfo {
    pub engine_active: bool,
    pub display_held: bool,
    pub stopped: bool,
    pub reason: String,
    pub updated_utc: String,
    pub idle_seconds: u64,
    pub follow_server: bool,
    pub cover: String,
    pub enabled: bool,
    pub stop_file_present: bool,
    pub state_path: String,
    pub phase: String,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct Snapshot {
    pub seq: u64,
    pub collected_ms: u128,
    pub collect_duration_ms: u128,
    pub host: String,
    pub model: ModelInfo,
    pub server: ServerInfo,
    pub gpu: GpuInfo,
    pub blackout: BlackoutInfo,
    pub notes: Vec<String>,
}

fn hidden(command: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        command.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(windows)]
    {
        command.stdin(Stdio::null());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::null());
    }
    command
}

fn num(text: &str) -> Option<f64> {
    let t = text.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("n/a") || t.contains("Not Supported") || t.contains("[N/A]") {
        return None;
    }
    t.parse::<f64>().ok()
}

// ----------------------------------------------------------------------------- GPU

pub fn sample_gpu() -> GpuInfo {
    let began = Instant::now();
    let mut info = GpuInfo {
        error: None,
        ..Default::default()
    };
    let mut cmd = Command::new("nvidia-smi");
    cmd.args([
        "--query-gpu=name,utilization.gpu,memory.used,memory.total,temperature.gpu,power.draw,clocks.sm,clocks_throttle_reasons.active",
        "--format=csv,noheader,nounits",
    ]);
    let out = hidden(&mut cmd).output();

    let text = match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        Ok(o) => {
            info.error = Some(format!("nvidia-smi exited {}", o.status));
            info.collected_ms = began.elapsed().as_millis();
            return info;
        }
        Err(e) => {
            info.error = Some(format!("nvidia-smi unavailable: {e}"));
            info.collected_ms = began.elapsed().as_millis();
            return info;
        }
    };

    // Device 1 of this part reports N/A for utilisation, so take the first row
    // that yields a number, exactly as V1 does.
    for line in text.lines() {
        let parts: Vec<&str> = line.split(',').map(|p| p.trim()).collect();
        if parts.len() < 8 {
            continue;
        }
        let Some(util) = num(parts[1]) else { continue };
        info.available = true;
        info.name = parts[0].to_string();
        info.util_pct = Some(util);
        info.mem_used_mib = num(parts[2]);
        info.mem_total_mib = num(parts[3]);
        info.temp_c = num(parts[4]);
        info.power_w = num(parts[5]);
        info.clock_mhz = num(parts[6]);
        info.throttle = parts[7].to_string();
        break;
    }
    if !info.available && info.error.is_none() {
        info.error = Some("no device reported utilisation".into());
    }
    info.collected_ms = began.elapsed().as_millis();
    info
}

// --------------------------------------------------------------------------- HTTP

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_millis(1200))
        .timeout(Duration::from_millis(2500))
        .build()
}

pub fn probe_models(endpoint: &str) -> (bool, Option<f64>, Vec<String>, Option<String>) {
    let url = format!("{endpoint}/models");
    let began = Instant::now();
    match agent().get(&url).call() {
        Ok(resp) => {
            let latency = began.elapsed().as_secs_f64() * 1000.0;
            if resp.status() != 200 {
                return (false, Some(latency), Vec::new(), Some(format!("HTTP {}", resp.status())));
            }
            let body = resp.into_string().unwrap_or_default();
            let models = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| {
                    v.get("data").and_then(|d| d.as_array()).map(|arr| {
                        arr.iter()
                            .filter_map(|m| m.get("id").and_then(|i| i.as_str()).map(String::from))
                            .collect::<Vec<_>>()
                    })
                })
                .unwrap_or_default();
            (true, Some(latency), models, None)
        }
        Err(e) => (
            false,
            None,
            Vec::new(),
            Some(match e {
                ureq::Error::Status(code, _) => format!("HTTP {code}"),
                ureq::Error::Transport(t) => t.to_string(),
            }),
        ),
    }
}

// ------------------------------------------------------------------------ config

fn read_json(path: &str) -> Option<serde_json::Value> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn yaml_scalar(text: &str, key: &str) -> Option<String> {
    for line in text.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix(&format!("{key}:")) {
            let value = rest.trim();
            if !value.is_empty() {
                return Some(value.trim_matches('"').to_string());
            }
        }
    }
    None
}

fn yaml_all(text: &str, key: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            line.trim_start()
                .strip_prefix(&format!("{key}:"))
                .map(|rest| rest.trim().trim_matches('"').to_string())
                .filter(|v| !v.is_empty())
        })
        .collect()
}

pub fn read_model(cfg: &serde_json::Value) -> ModelInfo {
    let get = |path: &[&str]| -> String {
        let mut cur = cfg;
        for key in path {
            match cur.get(*key) {
                Some(v) => cur = v,
                None => return String::new(),
            }
        }
        cur.as_str().unwrap_or_default().to_string()
    };

    let checkpoint = get(&["server", "model_path"]);
    let config_path = get(&["server", "config_path"]);
    let host = get(&["server", "host"]);
    let port = cfg
        .get("server")
        .and_then(|s| s.get("port"))
        .and_then(|p| p.as_u64())
        .unwrap_or(8000);

    let yaml = std::fs::read_to_string(&config_path).unwrap_or_default();
    let num_of = |key: &str| -> Option<f64> { yaml_scalar(&yaml, key)?.parse::<f64>().ok() };

    ModelInfo {
        served_name: get(&["server", "served_model"]),
        checkpoint_exists: Path::new(&checkpoint).exists(),
        checkpoint,
        endpoint: format!("http://{host}:{port}/v1"),
        config_path,
        chat_template: get(&["server", "chat_template"]),
        dtype: yaml_scalar(&yaml, "dtype").unwrap_or_else(|| "unknown".into()),
        kv_cache_dtype: yaml_scalar(&yaml, "kv-cache-dtype").unwrap_or_else(|| "auto".into()),
        max_model_len: num_of("max-model-len").map(|v| v as u64),
        gpu_memory_utilization: num_of("gpu-memory-utilization"),
        mtp_depth: yaml_all(&yaml, "num_speculative_tokens")
            .first()
            .and_then(|v| v.parse().ok()),
        max_num_seqs: num_of("max-num-seqs").map(|v| v as u64),
        prefix_caching: yaml_scalar(&yaml, "enable-prefix-caching")
            .map(|v| v.eq_ignore_ascii_case("true"))
            .unwrap_or(false),
    }
}

pub fn read_blackout(cfg: &serde_json::Value) -> BlackoutInfo {
    let idle_seconds = cfg
        .get("blackout")
        .and_then(|b| b.get("idle_seconds"))
        .and_then(|v| v.as_u64())
        .unwrap_or(180);
    let follow_server = cfg
        .get("automation")
        .and_then(|a| a.get("follow_server"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let enabled = cfg
        .get("blackout")
        .and_then(|b| b.get("enabled"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let cover = cfg
        .get("blackout")
        .and_then(|b| b.get("cover"))
        .and_then(|v| v.as_str())
        .unwrap_or("internal")
        .to_string();

    let script = cfg
        .get("blackout")
        .and_then(|b| b.get("script"))
        .and_then(|v| v.as_str())
        .map(PathBuf::from);
    let state_path = script
        .as_ref()
        .and_then(|p| p.parent().map(|d| d.join("state.json")))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_BLACKOUT_STATE));
    let stop_present = state_path
        .parent()
        .map(|d| d.join("STOP").exists())
        .unwrap_or(false);

    let state = read_json(&state_path.to_string_lossy()).unwrap_or(serde_json::Value::Null);
    let active = state.get("active").and_then(|v| v.as_bool()).unwrap_or(false);
    let display_held = state
        .get("display_held")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let stopped = state.get("stopped").and_then(|v| v.as_bool()).unwrap_or(true);
    let reason = state
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let updated = state
        .get("utc")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let phase = if !enabled && !active {
        "disabled"
    } else if active {
        "active"
    } else if enabled {
        "armed"
    } else {
        "idle"
    };

    BlackoutInfo {
        engine_active: active,
        display_held,
        stopped,
        reason,
        updated_utc: updated,
        idle_seconds,
        follow_server,
        cover,
        enabled,
        stop_file_present: stop_present,
        state_path: state_path.to_string_lossy().to_string(),
        phase: phase.to_string(),
    }
}

// ---------------------------------------------------------------------- snapshot

pub fn collect(seq: u64) -> Snapshot {
    let began = Instant::now();
    let cfg = read_json(V1_CONFIG).unwrap_or(serde_json::Value::Null);
    let model = read_model(&cfg);
    let port = cfg
        .get("server")
        .and_then(|s| s.get("port"))
        .and_then(|p| p.as_u64())
        .unwrap_or(8000) as u16;

    let listeners = crate::winproc::listeners_on(port);
    let listener = listeners.first();
    let facts = listener.and_then(|l| crate::winproc::process_facts(l.pid));
    let uptime = facts
        .as_ref()
        .and_then(|f| crate::winproc::process_uptime_s(f.created_ticks));

    let (health_ok, latency, models, http_error) = probe_models(&model.endpoint);
    let listening = listener.is_some();

    let state = if listening && health_ok && !models.is_empty() {
        "ready"
    } else if listening && health_ok {
        "loading"
    } else if listening {
        // Socket bound, API not answering yet: a model is still loading unless it
        // has been sitting there longer than any cold start takes.
        match uptime {
            Some(s) if s > 900.0 => "error",
            _ => "loading",
        }
    } else if health_ok {
        "ready"
    } else {
        "stopped"
    };

    let mut notes = Vec::new();
    if !model.checkpoint_exists {
        notes.push(format!("checkpoint not found at {}", model.checkpoint));
    }
    if cfg.is_null() {
        notes.push(format!("V1 config unreadable at {V1_CONFIG}"));
    }

    let server = ServerInfo {
        state: state.to_string(),
        listening,
        pid: listener.map(|l| l.pid),
        process_name: facts.as_ref().map(|f| f.name.clone()),
        uptime_s: uptime,
        health_ok,
        latency_ms: latency,
        models,
        error: http_error,
        endpoint: model.endpoint.clone(),
    };

    Snapshot {
        seq,
        collected_ms: began.elapsed().as_millis(),
        collect_duration_ms: 0,
        host: hostname(),
        model,
        server,
        gpu: sample_gpu(),
        blackout: read_blackout(&cfg),
        notes,
    }
}

fn hostname() -> String {
    std::env::var("COMPUTERNAME").unwrap_or_else(|_| "windows-arm64".into())
}
