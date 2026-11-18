//! RTX Spark AI Control V2 — local inference control center.
//!
//! Three responsibilities, kept apart on purpose:
//!   * `telemetry.rs` reads what is in the machine (GPU, model configuration);
//!   * `vllm.rs` owns the inference server's lifecycle, and only touches
//!     processes whose identity it recorded;
//!   * `blackout.rs` arms and disarms the validated blackout engine.
//!
//! `supervisor.rs` decides when the blackout guard should exist, and
//! `core.rs` holds the shared state every command and the interface read.
//!
//! V1 (`C:\Users\cohen\AI\RTX-Spark-AI-Control`) keeps its `config.json`
//! untouched: it is read for paths and defaults, never written.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod blackout;
mod core;
mod events;
mod settings;
mod supervisor;
mod telemetry;
mod vllm;
mod winproc;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use tauri::{Emitter, Manager};

use crate::core::{ActionResult, Core, Shared, Snapshot};
use crate::vllm::StopPlan;

static SHUTDOWN: AtomicBool = AtomicBool::new(false);
static STARTED: OnceLock<Instant> = OnceLock::new();

fn shared() -> Result<Shared, String> {
    core::try_shared().ok_or_else(|| "backend is still starting".to_string())
}

fn lock(shared: &Shared) -> std::sync::MutexGuard<'_, Core> {
    // A poisoned lock means a previous command panicked while holding it. The
    // state is still structurally valid, and refusing to serve the interface
    // afterwards would be worse than continuing.
    shared.lock().unwrap_or_else(|error| error.into_inner())
}

// ------------------------------------------------------------------- snapshots

#[tauri::command]
async fn snapshot(last_event_id: Option<u64>) -> Result<Snapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let shared = shared()?;
        let mut core = lock(&shared);
        Ok::<Snapshot, String>(core.snapshot(last_event_id.unwrap_or(0)))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[derive(serde::Serialize)]
struct ReadyReport {
    startup_ms: u128,
    arch: &'static str,
    version: &'static str,
}

/// Called once by the frontend after the first paint. Writes a small report next
/// to the process temp directory so startup time can be verified without a
/// console, and returns it for display in the interface.
#[tauri::command]
fn frontend_ready() -> ReadyReport {
    let ms = STARTED
        .get()
        .map(|started| started.elapsed().as_millis())
        .unwrap_or(0);
    let report = ReadyReport {
        startup_ms: ms,
        arch: std::env::consts::ARCH,
        version: env!("CARGO_PKG_VERSION"),
    };
    if let Ok(text) = serde_json::to_string_pretty(&report) {
        let path = std::env::temp_dir().join("rtx-v2-startup.json");
        let _ = std::fs::write(path, text);
    }
    report
}

// --------------------------------------------------------------- server control

#[tauri::command]
async fn server_start() -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let shared = shared()?;
        let mut core = lock(&shared);
        let result = core.with_vllm(|vllm, events| vllm.start(events));
        core.record_action(&result);
        drop(core);
        supervisor::request_probe();
        Ok::<ActionResult, String>(result)
    })
    .await
    .map_err(|error| error.to_string())?
}

/// Blocking stop, shared by Stop and Restart. The terminating runs with no lock
/// held; only the bookkeeping around it takes the state lock.
fn do_stop(shared: &Shared) -> ActionResult {
    let plan = {
        let mut core = lock(shared);
        core.with_vllm(|vllm, events| vllm.begin_stop(events))
    };
    match plan {
        StopPlan::Nothing => {
            let mut core = lock(shared);
            let result = ActionResult::ok("Server is not running");
            core.record_action(&result);
            result
        }
        StopPlan::NotOwned(pid) => {
            let mut core = lock(shared);
            let result = ActionResult::fail(format!(
                "Not stopping pid {pid}: it was not started by this application"
            ));
            core.record_action(&result);
            result
        }
        StopPlan::Terminate { root, timeout_ms } => {
            let stopped = winproc::terminate_tree(root, timeout_ms);
            let mut core = lock(shared);
            let result = core.with_vllm(|vllm, events| vllm.finish_stop(stopped, events));
            core.record_action(&result);
            drop(core);
            supervisor::request_probe();
            result
        }
    }
}

#[tauri::command]
async fn server_stop() -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let shared = shared()?;
        Ok::<ActionResult, String>(do_stop(&shared))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn server_restart() -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let shared = shared()?;
        let stopped = do_stop(&shared);
        if !stopped.ok {
            return Ok::<ActionResult, String>(stopped);
        }
        // A listener that has just closed still holds the port for a moment.
        std::thread::sleep(Duration::from_millis(600));
        let mut core = lock(&shared);
        let result = core.with_vllm(|vllm, events| vllm.start(events));
        core.record_action(&result);
        drop(core);
        supervisor::request_probe();
        Ok::<ActionResult, String>(result)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn server_adopt() -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let shared = shared()?;
        let mut core = lock(&shared);
        let result = core.with_vllm(|vllm, events| vllm.adopt(events));
        if result.ok {
            core.note(
                "warn",
                "Adopted server: Stop and Restart now apply to the process this application did not launch.",
            );
        }
        core.record_action(&result);
        Ok::<ActionResult, String>(result)
    })
    .await
    .map_err(|error| error.to_string())?
}

// -------------------------------------------------------------- blackout control

#[tauri::command]
async fn blackout_now() -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let shared = shared()?;
        let mut core = lock(&shared);
        let settings = core.settings.clone();
        // While automation is enabled the guard stays in automation's hands, so
        // Follow Server can still stand it down. Otherwise this is a manual guard.
        let mode = if settings.blackout_enabled && core.blackout.mode == "auto" {
            "auto"
        } else {
            "manual"
        };
        let result = core
            .with_blackout(|blackout, events| blackout.start(true, mode, &settings, events));
        core.record_action(&result);
        Ok::<ActionResult, String>(result)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn blackout_stop() -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let shared = shared()?;
        let mut core = lock(&shared);
        let result = core.with_blackout(|blackout, events| blackout.stop(events));
        if result.ok && core.settings.blackout_enabled {
            // Without this the supervisor would re-arm within a second.
            core.auto.suspended = true;
            core.note(
                "info",
                "Automatic blackout paused by hand; the next input restarts the idle interval.",
            );
        }
        core.record_action(&result);
        Ok::<ActionResult, String>(result)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn blackout_auto(enabled: bool) -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let shared = shared()?;
        let mut core = lock(&shared);
        core.settings.blackout_enabled = enabled;
        core.auto.suspended = false;
        core.auto.anchor = Instant::now();
        core.persist_settings();
        core.note(
            "info",
            if enabled {
                "Automatic blackout enabled."
            } else {
                "Automatic blackout disabled."
            },
        );
        // Take effect now instead of on the next tick.
        if !enabled {
            let owner = core.blackout.mode.clone();
            if owner == "auto" {
                core.with_blackout(|blackout, events| blackout.stop(events));
            }
        }
        let result = ActionResult::ok(if enabled {
            "Automatic blackout on"
        } else {
            "Automatic blackout off"
        });
        core.record_action(&result);
        Ok::<ActionResult, String>(result)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn follow_server(enabled: bool) -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let shared = shared()?;
        let mut core = lock(&shared);
        core.settings.follow_server = enabled;
        // A fresh health window, so enabling this cannot arm immediately on the
        // strength of one lucky probe.
        core.auto.ok_since = None;
        core.auto.fail_since = None;
        core.persist_settings();
        core.note(
            "info",
            if enabled {
                "Follow Server enabled: the guard tracks the inference server's health."
            } else {
                "Follow Server disabled: the guard no longer depends on the server."
            },
        );
        let result = ActionResult::ok(if enabled {
            "Following the server"
        } else {
            "Not following the server"
        });
        core.record_action(&result);
        Ok::<ActionResult, String>(result)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn blackout_timeout(seconds: u64) -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let shared = shared()?;
        let mut core = lock(&shared);
        core.settings.idle_seconds = seconds;
        let clamped = core.settings.clone().clamped();
        core.settings = clamped;
        let settings = core.settings.clone();
        core.persist_settings();
        let running = core.with_blackout(|blackout, _events| blackout.engine_present());
        let result = if running {
            core.with_blackout(|blackout, events| blackout.apply_timeout(&settings, events))
        } else {
            ActionResult::ok(format!(
                "Inactivity timeout set to {}s",
                settings.idle_seconds
            ))
        };
        core.record_action(&result);
        Ok::<ActionResult, String>(result)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn blackout_cover(cover: String) -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let shared = shared()?;
        let mut core = lock(&shared);
        core.settings.cover = cover;
        let clamped = core.settings.clone().clamped();
        core.settings = clamped;
        let settings = core.settings.clone();
        core.persist_settings();
        core.note(
            "info",
            format!("Blackout cover set to {}.", settings.cover),
        );
        let running = core.with_blackout(|blackout, _events| blackout.engine_present());
        let result = if running {
            core.with_blackout(|blackout, events| blackout.apply_timeout(&settings, events))
        } else {
            ActionResult::ok(format!("Cover set to {}", settings.cover))
        };
        core.record_action(&result);
        Ok::<ActionResult, String>(result)
    })
    .await
    .map_err(|error| error.to_string())?
}

// ------------------------------------------------------------- interface prefs

#[tauri::command]
async fn telemetry_interval(seconds: u64) -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let shared = shared()?;
        let mut core = lock(&shared);
        core.settings.poll_s = seconds;
        let clamped = core.settings.clone().clamped();
        core.settings = clamped;
        core.persist_settings();
        let poll_s = core.settings.poll_s;
        core.note(
            "info",
            format!("Health telemetry interval set to {poll_s}s."),
        );
        let result = ActionResult::ok(format!("Telemetry interval set to {poll_s}s"));
        core.record_action(&result);
        Ok::<ActionResult, String>(result)
    })
    .await
    .map_err(|error| error.to_string())?
}

// --------------------------------------------------------------- window plumbing

fn emit_visibility(handle: &tauri::AppHandle, minimized: bool, visible: bool, focused: bool) {
    let _ = handle.emit(
        "v2://visibility",
        serde_json::json!({
            "minimized": minimized,
            "visible": visible,
            "focused": focused,
        }),
    );
}

/// One second of polling to notice minimise/restore. The check itself is three
/// window-state calls and emits only on change, so it costs nothing while the
/// window sits there.
fn watch_visibility(handle: tauri::AppHandle) {
    std::thread::spawn(move || {
        let mut last = (false, true, true);
        while !SHUTDOWN.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(1000));
            let Some(window) = handle.get_webview_window("main") else {
                continue;
            };
            let now = (
                window.is_minimized().unwrap_or(false),
                window.is_visible().unwrap_or(true),
                window.is_focused().unwrap_or(false),
            );
            if now != last {
                last = now;
                emit_visibility(&handle, now.0, now.1, now.2);
            }
        }
    });
}

#[cfg(windows)]
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS};
#[cfg(windows)]
use windows_sys::Win32::System::Threading::CreateMutexW;
#[cfg(windows)]
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowW, SetForegroundWindow, ShowWindow, SW_RESTORE,
};

/// A second instance cannot share the WebView2 browser profile: it aborts inside
/// the loader (`0xC0000409`) with no window and no message. Claim a per-session
/// named mutex first, the way V1 does, so a second launch is a no-op that raises
/// the window already on screen. The handle is intentionally never closed —
/// Windows releases it when the owning process exits.
#[cfg(windows)]
fn claim_single_instance() -> bool {
    let name: Vec<u16> = "Local\\RTXSparkAIControlV2\0".encode_utf16().collect();
    unsafe {
        let handle = CreateMutexW(std::ptr::null(), 1, name.as_ptr());
        if handle.is_null() {
            // Mutex unavailable: starting is better than refusing on principle.
            return true;
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            CloseHandle(handle);
            return false;
        }
    }
    true
}

/// Bring the running instance forward, so launching again behaves like clicking
/// its taskbar button rather than doing nothing at all.
#[cfg(windows)]
fn focus_existing() {
    let title: Vec<u16> = "RTX Spark AI Control\0".encode_utf16().collect();
    unsafe {
        let window = FindWindowW(std::ptr::null(), title.as_ptr());
        if !window.is_null() {
            ShowWindow(window, SW_RESTORE);
            SetForegroundWindow(window);
        }
    }
}

fn main() {
    #[cfg(windows)]
    if !claim_single_instance() {
        focus_existing();
        return;
    }

    let _ = STARTED.set(Instant::now());
    let shared: Shared = Arc::new(Mutex::new(Core::new()));
    core::set_shared(shared.clone());
    let supervisor_shared = shared.clone();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            snapshot,
            frontend_ready,
            server_start,
            server_stop,
            server_restart,
            server_adopt,
            blackout_now,
            blackout_stop,
            blackout_auto,
            blackout_timeout,
            blackout_cover,
            follow_server,
            telemetry_interval,
        ])
        .setup(move |app| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_theme(Some(tauri::Theme::Dark));
            }
            watch_visibility(app.handle().clone());
            supervisor::spawn(supervisor_shared.clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Focused(focused) = event {
                let minimized = window.is_minimized().unwrap_or(false);
                let visible = window.is_visible().unwrap_or(true);
                emit_visibility(window.app_handle(), minimized, visible, *focused);
            }
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                SHUTDOWN.store(true, Ordering::Relaxed);
            }
        })
        .build(tauri::generate_context!())
        .expect("failed to build RTX Spark AI Control V2")
        .run(move |_app, event| match event {
            tauri::RunEvent::ExitRequested { .. } => {
                SHUTDOWN.store(true, Ordering::Relaxed);
            }
            tauri::RunEvent::Exit => {
                SHUTDOWN.store(true, Ordering::Relaxed);
                // No orphaned guard: a blackout engine this application armed is
                // stopped on the way out, which also drops its display request.
                // A vLLM server it started is deliberately left running.
                supervisor::shutdown(&shared);
            }
            _ => {}
        });
}
