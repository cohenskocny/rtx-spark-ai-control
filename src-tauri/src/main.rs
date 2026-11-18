//! RTX Spark AI Control V2 — read-only telemetry prototype.
//!
//! The backend is deliberately small: it collects a snapshot of the existing
//! environment (vLLM over loopback HTTP, GPU via nvidia-smi, V1 config and
//! blackout state on disk) and reports window visibility so the frontend can
//! stop polling when nobody is looking. It never starts, stops or reconfigures
//! anything.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod telemetry;
mod winproc;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::{Emitter, Manager};

struct AppState {
    started: Instant,
    seq: AtomicU64,
    collecting: AtomicBool,
    cached: Mutex<Option<telemetry::Snapshot>>,
}

impl AppState {
    fn new(started: Instant) -> Self {
        Self {
            started,
            seq: AtomicU64::new(0),
            collecting: AtomicBool::new(false),
            cached: Mutex::new(None),
        }
    }

    /// One collection at a time. A second caller gets the previous snapshot
    /// instead of queueing behind the first, so the UI can never stack requests.
    fn collect(&self) -> telemetry::Snapshot {
        if self.collecting.swap(true, Ordering::AcqRel) {
            if let Ok(guard) = self.cached.lock() {
                if let Some(snap) = guard.clone() {
                    return snap;
                }
            }
            return telemetry::Snapshot::default();
        }
        let seq = self.seq.fetch_add(1, Ordering::Relaxed) + 1;
        let snap = telemetry::collect(seq);
        if let Ok(mut guard) = self.cached.lock() {
            *guard = Some(snap.clone());
        }
        self.collecting.store(false, Ordering::Release);
        snap
    }
}

struct Holder(Arc<AppState>);

static SHUTDOWN: AtomicBool = AtomicBool::new(false);

#[tauri::command]
async fn snapshot(state: tauri::State<'_, Holder>) -> Result<telemetry::Snapshot, String> {
    let inner = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || inner.collect())
        .await
        .map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
struct ReadyReport {
    startup_ms: u128,
    arch: &'static str,
    version: &'static str,
}

/// Called once by the frontend after the first paint. Writes a small report next
/// to the process temp directory so startup time can be verified without a
/// console, and returns it for display in the UI.
#[tauri::command]
fn frontend_ready(state: tauri::State<'_, Holder>) -> ReadyReport {
    let ms = state.0.started.elapsed().as_millis();
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

/// One second of polling to notice minimise/restore. The check itself is two
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

    let started = Instant::now();

    tauri::Builder::default()
        .manage(Holder(Arc::new(AppState::new(started))))
        .invoke_handler(tauri::generate_handler![snapshot, frontend_ready])
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_theme(Some(tauri::Theme::Dark));
            }
            watch_visibility(app.handle().clone());
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
        .run(|_app, event| {
            if matches!(
                event,
                tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit
            ) {
                SHUTDOWN.store(true, Ordering::Relaxed);
            }
        });
}
