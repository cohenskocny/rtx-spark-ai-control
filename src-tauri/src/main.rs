//! RTX Spark AI Control V2 — local inference control center.
//!
//! Four responsibilities, kept apart on purpose:
//!   * `telemetry.rs` reads what is in the machine (GPU, model configuration);
//!   * `vllm.rs` owns the inference server's lifecycle, and only touches
//!     processes whose identity it recorded;
//!   * `blackout.rs` arms and disarms the validated blackout engine;
//!   * `upstream.rs` reads the upstream project's releases — information only,
//!     it never downloads or installs anything.
//!
//! `paths.rs` holds the installation locations the operator configured,
//! `power.rs` decides whether inference may start at all, `version.rs` reads what
//! is installed, `supervisor.rs` decides when the blackout guard should exist, and
//! `core.rs` holds the shared state every command and the interface read.
//!
//! Nothing here requires a particular directory layout, and nothing outside this
//! application's own data directory is ever written.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod blackout;
mod core;
mod events;
mod paths;
mod power;
mod settings;
mod supervisor;
mod telemetry;
mod upstream;
mod version;
mod vllm;
mod winproc;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use tauri::{Emitter, Manager};

use crate::core::{ActionResult, Core, Shared, Snapshot};
use crate::paths::Paths;
use crate::power::LaunchIntent;
use crate::upstream::UpstreamView;
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
async fn server_start(confirm_battery: Option<bool>) -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let intent = if confirm_battery.unwrap_or(false) {
            LaunchIntent::ManualConfirmed
        } else {
            LaunchIntent::Manual
        };
        let shared = shared()?;
        let mut core = lock(&shared);
        let result = core.with_vllm(|vllm, events| vllm.start(intent, events));
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
async fn server_restart(confirm_battery: Option<bool>) -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let intent = if confirm_battery.unwrap_or(false) {
            LaunchIntent::ManualConfirmed
        } else {
            LaunchIntent::Manual
        };
        let shared = shared()?;
        let stopped = do_stop(&shared);
        if !stopped.ok {
            return Ok::<ActionResult, String>(stopped);
        }
        // A listener that has just closed still holds the port for a moment.
        std::thread::sleep(Duration::from_millis(600));
        let mut core = lock(&shared);
        let result = core.with_vllm(|vllm, events| vllm.start(intent, events));
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

// ------------------------------------------------------- portable configuration

#[derive(serde::Serialize)]
struct Validation {
    ok: bool,
    message: String,
}

/// Look for existing installations. Read-only, bounded, and it never applies
/// anything: the operator gets a list to choose from.
#[tauri::command]
async fn config_detect() -> Result<Vec<paths::Candidate>, String> {
    tauri::async_runtime::spawn_blocking(paths::discover)
        .await
        .map_err(|error| error.to_string())
}

/// Validate one field as it is typed, so the Setup page can answer immediately.
#[tauri::command]
fn config_validate(field: String, value: String) -> Validation {
    match Paths::kind_of(&field) {
        None => Validation {
            ok: false,
            message: "unknown field".to_string(),
        },
        Some(kind) => match paths::check(&value, kind) {
            Ok(_) => Validation {
                ok: true,
                message: String::new(),
            },
            Err(message) if message.is_empty() => Validation {
                ok: true,
                message: "not set (optional)".to_string(),
            },
            Err(message) => Validation {
                ok: false,
                message,
            },
        },
    }
}

/// Save the configuration. Every field is validated first and the whole save is
/// refused if any of them is wrong, so a half-valid configuration can never be
/// persisted and then acted on.
#[tauri::command]
async fn config_set(next: Paths) -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let shared = shared()?;
        let mut failures = Vec::new();
        for (field, kind) in paths::FIELDS {
            let value = next.field(field);
            if let Err(message) = paths::check(value, *kind) {
                if !message.is_empty() {
                    failures.push(format!("{field}: {message}"));
                }
            }
        }
        if !failures.is_empty() {
            let result = ActionResult::fail(format!("Not saved. {}", failures.join("; ")));
            lock(&shared).record_action(&result);
            return Ok::<ActionResult, String>(result);
        }

        let mut core = lock(&shared);
        core.apply_paths(next);
        let config = core.settings.paths.clone();
        let ready = config.launch_ready();
        core.note(
            if ready { "ok" } else { "warn" },
            if ready {
                format!("Installation configured for {}.", config.served_name())
            } else {
                "Configuration saved, but the launch paths are still incomplete.".to_string()
            },
        );
        let result = ActionResult::ok(if ready {
            "Configuration saved"
        } else {
            "Configuration saved; some paths still need attention"
        });
        core.record_action(&result);
        Ok::<ActionResult, String>(result)
    })
    .await
    .map_err(|error| error.to_string())?
}

/// Import paths from a V1-shaped `config.json`, then save them like typed input.
#[tauri::command]
async fn config_import(legacy_path: String) -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let shared = shared()?;
        if let Err(message) = paths::check(&legacy_path, paths::Kind::File) {
            let result = ActionResult::fail(format!("Not imported: legacy config {message}"));
            lock(&shared).record_action(&result);
            return Ok::<ActionResult, String>(result);
        }
        match paths::import_legacy(std::path::Path::new(&legacy_path)) {
            Ok(imported) => {
                let mut core = lock(&shared);
                core.apply_paths(imported);
                let ready = core.settings.paths.launch_ready();
                core.note(
                    "info",
                    format!(
                        "Imported paths from {legacy_path}{}.",
                        if ready { "" } else { " (launch paths incomplete)" }
                    ),
                );
                let result = ActionResult::ok("Paths imported from the legacy configuration");
                core.record_action(&result);
                Ok::<ActionResult, String>(result)
            }
            Err(message) => {
                let result = ActionResult::fail(format!("Not imported: {message}"));
                lock(&shared).record_action(&result);
                Ok::<ActionResult, String>(result)
            }
        }
    })
    .await
    .map_err(|error| error.to_string())?
}

/// Whether inference starts with the application. Refused on battery by the
/// launch gate rather than by this flag, which is only intent.
#[tauri::command]
async fn autostart_set(enabled: bool) -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let shared = shared()?;
        let mut core = lock(&shared);
        core.settings.autostart_inference = enabled;
        core.persist_settings();
        let on_battery = core.power.on_battery();
        let suffix = if enabled && on_battery {
            " On battery it will not run; plug in for it to take effect."
        } else {
            ""
        };
        core.note(
            "info",
            format!(
                "Start inference with the application: {}.{suffix}",
                if enabled { "on" } else { "off" }
            ),
        );
        let result = ActionResult::ok(format!(
            "Autostart {}{suffix}",
            if enabled { "on" } else { "off" }
        ));
        core.record_action(&result);
        Ok::<ActionResult, String>(result)
    })
    .await
    .map_err(|error| error.to_string())?
}

// ------------------------------------------------------------ upstream releases

/// Ask GitHub for the upstream release list. `force` is the manual button; without
/// it the once-a-day window applies and the cached result is returned.
#[tauri::command]
async fn upstream_check(force: Option<bool>) -> Result<UpstreamView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let revision = {
            let shared = shared()?;
            let core = lock(&shared);
            core.installed.revision.clone()
        };
        // Blocking network work happens with no lock held.
        let view = upstream::check(force.unwrap_or(false), &revision);
        let shared = shared()?;
        let mut core = lock(&shared);
        core.apply_upstream(view.clone());
        Ok::<UpstreamView, String>(view)
    })
    .await
    .map_err(|error| error.to_string())?
}

/// Open a link in the default browser.
///
/// Restricted to the upstream project on github.com: a command that opens whatever
/// it is handed is a way to launch anything on the machine.
#[tauri::command]
fn open_external(url: String) -> Validation {
    if !url_allowed(&url) {
        return Validation {
            ok: false,
            message: "only links to the upstream project on github.com can be opened".to_string(),
        };
    }
    match open_url(&url) {
        true => Validation {
            ok: true,
            message: String::new(),
        },
        false => Validation {
            ok: false,
            message: "the system could not open a browser".to_string(),
        },
    }
}

/// `https://github.com/devjeonghwan/vllm-for-rtx-spark[/...]` and nothing else.
fn url_allowed(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    if rest.contains(char::is_whitespace) || rest.contains(['"', '\\', '\'']) {
        return false;
    }
    let Some((host, path)) = rest.split_once('/') else {
        return false;
    };
    if !host.eq_ignore_ascii_case("github.com") {
        return false;
    }
    let path = format!("/{path}").to_lowercase();
    if path.split('/').any(|segment| segment == "..") {
        return false;
    }
    path == "/devjeonghwan/vllm-for-rtx-spark"
        || path.starts_with("/devjeonghwan/vllm-for-rtx-spark/")
}

#[cfg(windows)]
fn open_url(url: &str) -> bool {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let verb: Vec<u16> = "open\0".encode_utf16().collect();
    let target: Vec<u16> = url.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let result = ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            target.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        );
        !result.is_null() && result as isize > 32
    }
}

#[cfg(not(windows))]
fn open_url(_url: &str) -> bool {
    false
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

/// The only automatic path that can start inference: an operator's saved
/// preference, applied once at launch, and only if power allows it.
///
/// The power gate is inside `vllm.start`, so this cannot bypass it even if the
/// preference says yes: on battery the request is refused and the reason is logged
/// where the operator will see it.
fn maybe_autostart(shared: Shared) {
    std::thread::spawn(move || {
        // Let the window paint first; a cold model load must not delay startup.
        std::thread::sleep(Duration::from_millis(1500));
        let wanted = {
            let Ok(mut core) = shared.lock() else {
                return;
            };
            if SHUTDOWN.load(Ordering::Relaxed) {
                return;
            }
            let wanted = core.settings.autostart_inference;
            if wanted {
                core.note(
                    "info",
                    "Autostart is on; checking the power source before starting inference.",
                );
            }
            wanted
        };
        if !wanted {
            return;
        }
        let mut core = match shared.lock() {
            Ok(core) => core,
            Err(_) => return,
        };
        let result = core.with_vllm(|vllm, events| vllm.start(LaunchIntent::Automatic, events));
        core.record_action(&result);
        drop(core);
        supervisor::request_probe();
    });
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
            config_detect,
            config_validate,
            config_set,
            config_import,
            autostart_set,
            upstream_check,
            open_external,
        ])
        .setup(move |app| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_theme(Some(tauri::Theme::Dark));
            }
            watch_visibility(app.handle().clone());
            supervisor::spawn(supervisor_shared.clone());
            // The daily update check runs on its own thread so a slow or absent
            // network can never delay a window.
            upstream::spawn(supervisor_shared.clone());
            maybe_autostart(supervisor_shared.clone());
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

// ------------------------------------------------------------------- unit tests

#[cfg(test)]
mod tests {
    use super::url_allowed;

    #[test]
    fn the_upstream_project_and_its_subpages_are_allowed() {
        for url in [
            "https://github.com/devjeonghwan/vllm-for-rtx-spark",
            "https://github.com/devjeonghwan/vllm-for-rtx-spark/releases",
            "https://github.com/devjeonghwan/vllm-for-rtx-spark/releases/tag/v0.2.0",
            "https://github.com/devjeonghwan/vllm-for-rtx-spark/commit/8b8a427a40",
            "https://GitHub.com/devjeonghwan/vllm-for-rtx-spark",
        ] {
            assert!(url_allowed(url), "{url} should be allowed");
        }
    }

    #[test]
    fn anything_that_is_not_the_upstream_project_is_refused() {
        for url in [
            "http://github.com/devjeonghwan/vllm-for-rtx-spark",
            "https://github.com/someone/else",
            "https://github.com/devjeonghwan/vllm-for-rtx-spark-evil",
            "https://evil.example/devjeonghwan/vllm-for-rtx-spark",
            "https://github.com.evil.example/devjeonghwan/vllm-for-rtx-spark",
            "https://github.com/devjeonghwan/vllm-for-rtx-spark/../../etc",
            "file:///C:/Windows/System32/calc.exe",
            "https://github.com/devjeonghwan/vllm-for-rtx-spark/releases\" onmouseover=x",
            "javascript:alert(1)",
            "https://api.github.com/repos/devjeonghwan/vllm-for-rtx-spark",
            "",
        ] {
            assert!(!url_allowed(url), "{url} should be refused");
        }
    }
}
