//! System telemetry: GPU via nvidia-smi, plus the read-only model configuration.
//!
//! Server liveness lives in `vllm.rs` and the blackout guard in `blackout.rs`;
//! this module only answers "what is in the machine". Nothing here mutates the
//! environment: no spawn of vLLM, no display power requests, no writes to any
//! installation.

use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use serde::Serialize;

use crate::paths::Paths;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

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

/// The machine half of a snapshot.
#[derive(Serialize, Clone, Debug, Default)]
pub struct SystemView {
    pub seq: u64,
    pub collected_ms: u128,
    pub host: String,
    pub model: ModelInfo,
    pub gpu: GpuInfo,
    pub notes: Vec<String>,
}

fn hidden(command: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        command.creation_flags(CREATE_NO_WINDOW);
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

// ------------------------------------------------------------------------ config

/// Build the model view from configured paths alone.
///
/// The YAML serve configuration is read for the numbers it declares (context
/// length, quantisation, speculative depth); nothing here writes to it, and a
/// missing file degrades to "unknown" rather than an error the interface cannot
/// draw.
pub fn read_model(config: &Paths) -> ModelInfo {
    let checkpoint = config.model_checkpoint.trim().to_string();
    let config_path = config.launch_config.trim().to_string();
    let yaml = std::fs::read_to_string(&config_path).unwrap_or_default();
    let num_of = |key: &str| -> Option<f64> { yaml_scalar(&yaml, key)?.parse::<f64>().ok() };

    ModelInfo {
        served_name: config.served_name(),
        checkpoint_exists: !checkpoint.is_empty() && Path::new(&checkpoint).exists(),
        checkpoint,
        endpoint: config.endpoint(),
        config_path,
        chat_template: config.chat_template.trim().to_string(),
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

// ---------------------------------------------------------------------- sampling

static SEQ: AtomicU64 = AtomicU64::new(0);

pub fn collect_system() -> SystemView {
    let began = Instant::now();
    let config = crate::paths::current();
    let model = read_model(&config);

    let mut notes = Vec::new();
    if config.launch_missing().len() == 4 {
        notes.push("Installation paths are not configured yet; open Setup".to_string());
    } else if !model.checkpoint_exists {
        notes.push(format!("checkpoint not found at {}", model.checkpoint));
    }
    if !model.config_path.is_empty() && !Path::new(&model.config_path).exists() {
        notes.push(format!("serve configuration not found at {}", model.config_path));
    }

    SystemView {
        seq: SEQ.fetch_add(1, Ordering::Relaxed) + 1,
        collected_ms: began.elapsed().as_millis(),
        host: hostname(),
        model,
        gpu: sample_gpu(),
        notes,
    }
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

fn hostname() -> String {
    std::env::var("COMPUTERNAME").unwrap_or_else(|_| "windows-arm64".into())
}
