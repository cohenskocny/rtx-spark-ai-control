//! The automation supervisor: one thread, one tick a second.
//!
//! Follow Server, the idle clock and the guard lifetime all live here rather
//! than in the interface. A webview suspends timers while the window is hidden,
//! and hidden is exactly when blackout has to work, so the decisions cannot be
//! made by a JavaScript interval.
//!
//! Two rules shape the tick:
//!   * blocking work (the health probe, process termination) happens outside the
//!     state lock, so a snapshot can always be served;
//!   * the guard is only ever *armed* or *disarmed*. What happens once it runs —
//!     when the panel goes black, what counts as input, when the display request
//!     is held — belongs to the validated engine.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::core::{Core, Shared};
use crate::vllm::VllmManager;
use crate::winproc;

/// Set when a command changes the server's state and the next sample matters
/// now rather than in fifteen seconds.
static PROBE_NOW: AtomicBool = AtomicBool::new(false);

pub fn request_probe() {
    PROBE_NOW.store(true, Ordering::Relaxed);
}

pub fn spawn(shared: Shared) {
    let _ = std::thread::Builder::new()
        .name("v2-supervisor".to_string())
        .spawn(move || run(shared));
}

fn run(shared: Shared) {
    let mut next_probe = Instant::now();
    let mut last_health: Option<bool> = None;

    loop {
        if Instant::now() >= next_probe || PROBE_NOW.swap(false, Ordering::Relaxed) {
            // Blocking HTTP, deliberately without the lock held.
            let health = VllmManager::probe_health(VllmManager::port());
            let healthy = health.ok;
            let mut core = shared.lock().unwrap();
            core.with_vllm(|vllm, _events| vllm.apply_health(health));
            // A launch is the one moment readiness matters to a person watching
            // the window, so poll hard while a process is coming up or going
            // down and fall back to the operator's interval once it settles.
            let transient = core.with_vllm(|vllm, _events| {
                matches!(vllm.state.as_str(), "starting" | "loading" | "stopping")
            });
            let interval = if transient {
                3
            } else {
                core.settings.poll_s.clamp(5, 120)
            };
            next_probe = Instant::now() + Duration::from_secs(interval);
            if last_health != Some(healthy) {
                let text = match (last_health, healthy) {
                    (None, true) => "Inference server answered /v1/models.".to_string(),
                    (None, false) => "Inference server is not answering yet.".to_string(),
                    (Some(false), true) => "Inference server is healthy again.".to_string(),
                    _ => "Inference server stopped answering.".to_string(),
                };
                core.note(if healthy { "ok" } else { "warn" }, text);
                last_health = Some(healthy);
            }
        }

        {
            let mut core = shared.lock().unwrap();
            tick(&mut core);
        }

        std::thread::sleep(Duration::from_millis(1000));
    }
}

fn tick(core: &mut Core) {
    // 1. Process reality first: a child that exited, or an adopted process that
    //    vanished, must be reflected before any decision is taken.
    core.with_vllm(|vllm, events| vllm.refresh(events));
    core.with_blackout(|blackout, events| blackout.refresh(events));

    // 2. Input re-anchors the idle clock. `GetLastInputInfo` is the same source
    //    the engine uses, so the two never disagree about what "idle" means.
    let tick_count = winproc::input_tick();
    if tick_count != core.auto.last_tick {
        core.auto.last_tick = tick_count;
        core.auto.anchor = Instant::now();
        if core.auto.suspended {
            core.auto.suspended = false;
            core.note(
                "info",
                "Activity detected; automatic blackout may arm again after a full idle interval.",
            );
        }
    }
    core.auto.idle_s = core.auto.anchor.elapsed().as_secs_f64();

    // 3. Follow Server hysteresis: a state must hold before it is believed, and
    //    a failing check is tolerated for the grace window. Together these keep a
    //    single dropped probe from flashing the panel.
    let now = Instant::now();
    let healthy = core.with_vllm(|vllm, _events| vllm.health.ok);
    let hysteresis = Duration::from_secs(core.settings.hysteresis_s);
    let grace = Duration::from_secs(core.settings.grace_s);
    let was_ready = core.auto.server_ok;
    if healthy {
        core.auto.fail_since = None;
        let since = *core.auto.ok_since.get_or_insert(now);
        if !core.auto.server_ok && now.duration_since(since) >= hysteresis {
            core.auto.server_ok = true;
        }
    } else {
        core.auto.ok_since = None;
        let since = *core.auto.fail_since.get_or_insert(now);
        if core.auto.server_ok && now.duration_since(since) >= grace {
            core.auto.server_ok = false;
        }
    }
    if core.auto.server_ok != was_ready {
        // Health is tracked even with Follow Server off, because the phase text
        // reports it; the event is only worth recording when it drives the guard.
        let following = core.settings.follow_server;
        if core.auto.server_ok {
            if following {
                core.note(
                    "ok",
                    format!(
                        "Inference server healthy for {}s; Follow Server may arm blackout.",
                        core.settings.hysteresis_s
                    ),
                );
            }
        } else if following {
            core.note(
                "warn",
                format!(
                    "Inference server unhealthy for {}s; Follow Server stands blackout down.",
                    core.settings.grace_s
                ),
            );
        }
    }

    // 4. Arm or stand down the guard. Only a guard this application armed for
    //    automation is ever stopped here; one the operator forced keeps running.
    let settings = core.settings.clone();
    let present = core.with_blackout(|blackout, _events| blackout.engine_present());
    let owner = core.blackout.mode.clone();
    // Blackout is optional. With no usable guard there is nothing to arm, and
    // retrying every tick would only repeat the same failure into the event
    // ring, so automation stands down and the reason says so.
    let can_guard = settings.paths.blackout_ready();
    let want = can_guard
        && settings.blackout_enabled
        && !core.auto.suspended
        && (!settings.follow_server || core.auto.server_ok);

    let phase_before = core.auto.phase.clone();
    if want && !present {
        let result = core.with_blackout(|blackout, events| {
            blackout.start(false, "auto", &settings, events)
        });
        if !result.ok {
            core.auto.reason = result.message;
        }
    } else if !want && present && owner == "auto" {
        let result = core.with_blackout(|blackout, events| blackout.stop(events));
        if !result.ok {
            core.auto.reason = result.message;
        }
    }

    // 5. Describe the state for the interface. The guard's own `state.json` adds
    //    the armed/active distinction; this is the automation's view of it.
    let present = core.with_blackout(|blackout, _events| blackout.engine_present());
    core.auto.armed = want && present;
    let (phase, reason) = if !settings.blackout_enabled {
        ("disabled", "Automatic blackout is off.".to_string())
    } else if !can_guard {
        (
            "unavailable",
            "Blackout is optional and no guard is configured; vLLM management does not need one."
                .to_string(),
        )
    } else if core.auto.suspended {
        (
            "paused",
            "Stopped by hand; resumes after input and a full idle interval.".to_string(),
        )
    } else if settings.follow_server && !core.auto.server_ok {
        (
            "waiting-server",
            "Waiting for the inference server to stay healthy.".to_string(),
        )
    } else if present {
        (
            "armed",
            if settings.follow_server {
                "Following the inference server.".to_string()
            } else {
                format!(
                    "Armed after {}s without input.",
                    settings.idle_seconds
                )
            },
        )
    } else {
        ("starting", "Starting the blackout guard.".to_string())
    };
    core.auto.phase = phase.to_string();
    core.auto.reason = reason.clone();
    if phase_before != phase {
        core.note("blackout", format!("Automation phase {phase}: {reason}"));
    }
}

/// Stop a guard this application started, at shutdown. Bounded, because exit
/// must not hang on a reluctant child.
pub fn shutdown(shared: &Arc<std::sync::Mutex<Core>>) {
    let Ok(mut core) = shared.lock() else {
        return;
    };
    let owned = core.with_blackout(|blackout, _events| blackout.owns());
    if owned {
        core.with_blackout(|blackout, events| blackout.stop(events));
    }
}
