/** The command surface of the Rust backend, one wrapper per action. */

import { invoke } from "@tauri-apps/api/core";
import type {
  ActionResult,
  Candidate,
  PathsConfig,
  Snapshot,
  UpstreamView,
  Validation,
} from "./types";

export function sample(lastEventId: number): Promise<Snapshot> {
  return invoke<Snapshot>("snapshot", { lastEventId });
}

export const actions = {
  /**
   * Start inference. `confirmBattery` is only ever true after the operator
   * acknowledged the battery prompt; the backend refuses a plain start on battery,
   * so the confirmation is not something the interface can skip by accident.
   */
  serverStart: (confirmBattery = false) =>
    invoke<ActionResult>("server_start", { confirmBattery }),
  serverStop: () => invoke<ActionResult>("server_stop"),
  serverRestart: (confirmBattery = false) =>
    invoke<ActionResult>("server_restart", { confirmBattery }),
  serverAdopt: () => invoke<ActionResult>("server_adopt"),
  blackoutNow: () => invoke<ActionResult>("blackout_now"),
  blackoutStop: () => invoke<ActionResult>("blackout_stop"),
  blackoutAuto: (enabled: boolean) => invoke<ActionResult>("blackout_auto", { enabled }),
  blackoutTimeout: (seconds: number) => invoke<ActionResult>("blackout_timeout", { seconds }),
  blackoutCover: (cover: string) => invoke<ActionResult>("blackout_cover", { cover }),
  followServer: (enabled: boolean) => invoke<ActionResult>("follow_server", { enabled }),
  telemetryInterval: (seconds: number) =>
    invoke<ActionResult>("telemetry_interval", { seconds }),
  autostartSet: (enabled: boolean) => invoke<ActionResult>("autostart_set", { enabled }),
};

export const config = {
  /** Bounded, read-only search for existing installations. */
  detect: () => invoke<Candidate[]>("config_detect"),
  /** Validate one field without saving anything. */
  validate: (field: string, value: string) =>
    invoke<Validation>("config_validate", { field, value }),
  /** Save every path at once; refused if any field is invalid. */
  save: (paths: PathsConfig) => invoke<ActionResult>("config_set", { next: paths }),
  importLegacy: (legacyPath: string) =>
    invoke<ActionResult>("config_import", { legacyPath }),
};

export const upstream = {
  /** The manual button. Without `force` the once-a-day window applies. */
  check: (force = true) => invoke<UpstreamView>("upstream_check", { force }),
  /** Opens a github.com link for this project in the default browser. */
  open: (url: string) => invoke<Validation>("open_external", { url }),
};

export function errorText(error: unknown): string {
  if (error instanceof Error) return error.message;
  return String(error);
}
