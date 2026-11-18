//! Power source awareness.
//!
//! Loading a 96 GB model is the most expensive thing this machine can be asked
//! to do, and on battery it is the wrong thing to do silently. This module reads
//! the Windows power status and answers one question for every launch path:
//! *may inference start right now?*
//!
//! The rules, in order of authority:
//!
//!   1. On battery, nothing starts automatically — not a saved autostart flag,
//!      not Follow Server automation, not recovery. No override exists.
//!   2. On battery, a person may still start it by hand, but only after
//!      confirming: the interface has to pass `ManualConfirmed` for that, and a
//!      plain `Manual` intent is refused with a message that says why.
//!   3. On AC — or on a desktop with no battery at all — the operator's own
//!      configuration decides, exactly as before.
//!   4. If the power status cannot be read, the automatic paths are refused and
//!      manual ones need confirmation: an unreadable status must not silently
//!      become permission.
//!
//! Nothing here stops, suspends or kills anything. An inference server that is
//! already running is reported as it is, even on battery.

use std::time::Instant;

use serde::Serialize;

#[cfg(windows)]
use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

const PERCENT_UNKNOWN: u8 = 255;
const NO_BATTERY_FLAG: u8 = 128;

#[derive(Serialize, Clone, Debug)]
pub struct PowerView {
    /// `Some(true)` on mains, `Some(false)` on battery, `None` when unreadable.
    pub online: Option<bool>,
    /// `ac` · `battery` · `no-battery` · `unknown`
    pub source: String,
    pub percent: Option<u8>,
    pub battery_saver: Option<bool>,
    pub seconds_left: Option<u32>,
    pub sampled_ms: u128,
    /// Plain-language summary the interface can print without recomputing it.
    pub detail: String,
}

impl Default for PowerView {
    fn default() -> Self {
        Self {
            online: None,
            source: "unknown".to_string(),
            percent: None,
            battery_saver: None,
            seconds_left: None,
            sampled_ms: 0,
            detail: "Power source not read yet".to_string(),
        }
    }
}

impl PowerView {
    pub fn on_battery(&self) -> bool {
        self.source == "battery"
    }

    pub fn known(&self) -> bool {
        self.source != "unknown"
    }
}

/// Sample the power source. Cheap enough to call on every snapshot: one
/// non-blocking call into the kernel.
pub fn sample() -> PowerView {
    let began = Instant::now();
    #[cfg(windows)]
    {
        // The Win32 struct has no `Default` in windows-sys; zero is the value the
        // API overwrites, and zero also reads as "unknown" if the call fails.
        let mut raw: SYSTEM_POWER_STATUS = unsafe { std::mem::zeroed() };
        if unsafe { GetSystemPowerStatus(&mut raw) } == 0 {
            return unavailable(began.elapsed().as_millis());
        }
        from_status(&raw, began.elapsed().as_millis())
    }
    #[cfg(not(windows))]
    {
        unavailable(began.elapsed().as_millis())
    }
}

fn unavailable(sampled_ms: u128) -> PowerView {
    PowerView {
        sampled_ms,
        detail: "Power source unavailable; automatic launches stay disabled".to_string(),
        ..Default::default()
    }
}

#[cfg(windows)]
fn from_status(raw: &SYSTEM_POWER_STATUS, sampled_ms: u128) -> PowerView {
    let online = match raw.ACLineStatus {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    };
    let no_battery = raw.BatteryFlag == NO_BATTERY_FLAG || raw.BatteryFlag == 255;
    let source = if no_battery {
        "no-battery"
    } else {
        match online {
            Some(true) => "ac",
            Some(false) => "battery",
            None => "unknown",
        }
    };
    let percent = if raw.BatteryLifePercent == PERCENT_UNKNOWN {
        None
    } else {
        Some(raw.BatteryLifePercent.min(100))
    };
    let seconds_left = if raw.BatteryLifeTime == u32::MAX || raw.BatteryLifeTime == 0 {
        None
    } else {
        Some(raw.BatteryLifeTime)
    };
    let battery_saver = if no_battery {
        None
    } else {
        Some(raw.SystemStatusFlag == 1)
    };

    let detail = match (source, percent) {
        ("battery", Some(pct)) => format!("On battery · {pct}%"),
        ("battery", None) => "On battery".to_string(),
        ("ac", Some(pct)) => format!("On AC power · {pct}%"),
        ("ac", None) => "On AC power".to_string(),
        ("no-battery", _) => "Mains power, no battery reported".to_string(),
        _ => "Power source unknown".to_string(),
    };

    PowerView {
        online,
        source: source.to_string(),
        percent,
        battery_saver,
        seconds_left,
        sampled_ms,
        detail,
    }
}

/// Why a launch would be started. The distinction is what lets a hand-started
/// launch proceed on battery while every automatic path stays closed.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LaunchIntent {
    /// A person pressed Start or Restart.
    Manual,
    /// A person pressed Start and then confirmed on battery.
    ManualConfirmed,
    /// A saved setting, Follow Server, or recovery decided to start it.
    Automatic,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct GateBlock {
    /// Stable identifier so the interface can offer the confirmation prompt.
    pub code: String,
    pub message: String,
}

impl GateBlock {
    fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
        }
    }
}

/// Decide whether inference may start, given a power reading and an intent.
///
/// Pure, so the whole matrix is unit-testable without a battery or a GPU.
pub fn gate_for(intent: LaunchIntent, power: &PowerView) -> Result<(), GateBlock> {
    match (intent, power.on_battery(), power.known()) {
        // 1. Battery blocks every automatic path, unconditionally.
        (LaunchIntent::Automatic, true, _) => Err(GateBlock::new(
            "battery_blocks_automatic",
            "Automatic launch is disabled on battery power. Plug in and try again, or start it by hand.",
        )),
        // 2. A hand-started launch needs an explicit confirmation on battery.
        (LaunchIntent::Manual, true, _) => Err(GateBlock::new(
            "battery_confirmation_required",
            "This laptop is on battery power. Loading the model now will drain it quickly; confirm to launch anyway.",
        )),
        // 3. Unreadable status is not permission.
        (LaunchIntent::Automatic, false, false) => Err(GateBlock::new(
            "power_unknown",
            "The power source could not be read, so automatic launch stays disabled.",
        )),
        (LaunchIntent::Manual, false, false) => Err(GateBlock::new(
            "power_unknown_confirmation",
            "The power source could not be read. Confirm to launch anyway.",
        )),
        // 4. AC, no battery, or an explicit confirmation.
        _ => Ok(()),
    }
}

/// The gate used by the live application.
pub fn gate(intent: LaunchIntent) -> Result<PowerView, GateBlock> {
    let power = sample();
    gate_for(intent, &power).map(|_| power)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn power(source: &str) -> PowerView {
        let online = match source {
            "ac" | "no-battery" => Some(true),
            "battery" => Some(false),
            _ => None,
        };
        PowerView {
            online,
            source: source.to_string(),
            percent: if source == "battery" { Some(96) } else { None },
            ..Default::default()
        }
    }

    #[test]
    fn automatic_never_runs_on_battery() {
        let block = gate_for(LaunchIntent::Automatic, &power("battery")).unwrap_err();
        assert_eq!(block.code, "battery_blocks_automatic");
    }

    #[test]
    fn manual_on_battery_needs_confirmation() {
        let block = gate_for(LaunchIntent::Manual, &power("battery")).unwrap_err();
        assert_eq!(block.code, "battery_confirmation_required");
        assert!(gate_for(LaunchIntent::ManualConfirmed, &power("battery")).is_ok());
    }

    #[test]
    fn ac_allows_every_intent() {
        for intent in [
            LaunchIntent::Manual,
            LaunchIntent::ManualConfirmed,
            LaunchIntent::Automatic,
        ] {
            assert!(gate_for(intent, &power("ac")).is_ok(), "{intent:?} refused on AC");
        }
    }

    #[test]
    fn a_desktop_without_a_battery_is_treated_as_mains() {
        assert!(gate_for(LaunchIntent::Automatic, &power("no-battery")).is_ok());
    }

    #[test]
    fn unknown_power_blocks_automatic_and_prompts_manual() {
        let block = gate_for(LaunchIntent::Automatic, &power("unknown")).unwrap_err();
        assert_eq!(block.code, "power_unknown");
        let block = gate_for(LaunchIntent::Manual, &power("unknown")).unwrap_err();
        assert_eq!(block.code, "power_unknown_confirmation");
        assert!(gate_for(LaunchIntent::ManualConfirmed, &power("unknown")).is_ok());
    }

    #[test]
    fn battery_reading_flags_battery_clearly() {
        let view = power("battery");
        assert!(view.on_battery());
        assert!(view.known());
        assert!(!power("ac").on_battery());
    }
}
