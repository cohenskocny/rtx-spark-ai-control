/** The command surface of the Rust backend, one wrapper per action. */

import { invoke } from "@tauri-apps/api/core";
import type { ActionResult, Snapshot } from "./types";

export function sample(lastEventId: number): Promise<Snapshot> {
  return invoke<Snapshot>("snapshot", { lastEventId });
}

export const actions = {
  serverStart: () => invoke<ActionResult>("server_start"),
  serverStop: () => invoke<ActionResult>("server_stop"),
  serverRestart: () => invoke<ActionResult>("server_restart"),
  serverAdopt: () => invoke<ActionResult>("server_adopt"),
  blackoutNow: () => invoke<ActionResult>("blackout_now"),
  blackoutStop: () => invoke<ActionResult>("blackout_stop"),
  blackoutAuto: (enabled: boolean) => invoke<ActionResult>("blackout_auto", { enabled }),
  blackoutTimeout: (seconds: number) => invoke<ActionResult>("blackout_timeout", { seconds }),
  blackoutCover: (cover: string) => invoke<ActionResult>("blackout_cover", { cover }),
  followServer: (enabled: boolean) => invoke<ActionResult>("follow_server", { enabled }),
  telemetryInterval: (seconds: number) =>
    invoke<ActionResult>("telemetry_interval", { seconds }),
};

export function errorText(error: unknown): string {
  if (error instanceof Error) return error.message;
  return String(error);
}
