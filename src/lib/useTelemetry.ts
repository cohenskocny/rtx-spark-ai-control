import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { LogEvent, Snapshot, VisibilityEvent } from "./types";

const MAX_EVENTS = 80;

export interface TelemetryState {
  snap: Snapshot | null;
  error: string | null;
  events: LogEvent[];
  paused: boolean;
  pausedReason: string;
  lastAt: number | null;
  latencyMs: number | null;
  invertedPollMs: number;
  refresh: () => void;
}

function stamp(): string {
  const d = new Date();
  return d.toLocaleTimeString("en-GB", { hour12: false });
}

/**
 * Serial polling with a completion-scheduled timer: the next request is queued
 * only after the previous one resolves, so a slow poll can never stack. Polling
 * stops completely when the window is minimised or hidden (unless the operator
 * turns that off), and resumes with one immediate sample.
 */
export function useTelemetry(pollMs: number, pauseWhenHidden: boolean): TelemetryState {
  const [snap, setSnap] = useState<Snapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [events, setEvents] = useState<LogEvent[]>([]);
  const [paused, setPaused] = useState(false);
  const [pausedReason, setPausedReason] = useState("");
  const [lastAt, setLastAt] = useState<number | null>(null);
  const [latencyMs, setLatencyMs] = useState<number | null>(null);

  const timer = useRef<number | null>(null);
  const busy = useRef(false);
  const stopped = useRef(false);
  const eventId = useRef(0);
  const hidden = useRef(false);
  const prefs = useRef({ pollMs, pauseWhenHidden });
  prefs.current = { pollMs, pauseWhenHidden };

  const pushEvent = useCallback((kind: LogEvent["kind"], text: string) => {
    eventId.current += 1;
    const entry: LogEvent = { id: eventId.current, at: Date.now(), kind, text };
    setEvents((prev) => [entry, ...prev].slice(0, MAX_EVENTS));
  }, []);

  const schedule = useCallback((delay: number) => {
    if (timer.current !== null) window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => {
      void tick();
    }, delay);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const tick = useCallback(async () => {
    if (stopped.current || busy.current) return;
    if (hidden.current && prefs.current.pauseWhenHidden) {
      setPaused(true);
      setPausedReason("window hidden or minimised");
      return;
    }
    busy.current = true;
    const began = performance.now();
    try {
      const next = await invoke<Snapshot>("snapshot");
      setSnap(next);
      setError(null);
      setLastAt(Date.now());
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setError(message);
      pushEvent("error", `telemetry poll failed: ${message}`);
    } finally {
      setLatencyMs(Math.round(performance.now() - began));
      busy.current = false;
      if (!stopped.current) {
        setPaused(false);
        setPausedReason("");
        schedule(prefs.current.pollMs);
      }
    }
  }, [pushEvent, schedule]);

  // Visibility: Tauri reports minimise/restore; the DOM reports occlusion and
  // tab switches.
  useEffect(() => {
    pushEvent("info", "telemetry loop started");
    let unlisten: (() => void) | undefined;
    void listen<VisibilityEvent>("v2://visibility", (event) => {
      const payload = event.payload;
      const next = payload.minimized || !payload.visible;
      if (next !== hidden.current) {
        hidden.current = next;
        pushEvent("info", next ? "window hidden: polling suspended" : "window visible: polling resumed");
        if (!next) {
          schedule(120);
        }
      }
    }).then((fn) => {
      unlisten = fn;
    });

    const onDomVisibility = () => {
      const next = document.visibilityState !== "visible";
      if (next !== hidden.current) {
        hidden.current = next;
        if (!next) schedule(120);
      }
    };
    document.addEventListener("visibilitychange", onDomVisibility);

    return () => {
      stopped.current = true;
      if (timer.current !== null) window.clearTimeout(timer.current);
      unlisten?.();
      document.removeEventListener("visibilitychange", onDomVisibility);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Interval changes take effect immediately instead of after the current wait.
  useEffect(() => {
    if (!stopped.current && !paused) schedule(40);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pollMs]);

  // Transition events worth surfacing in the Logs page.
  const lastState = useRef<string>("");
  useEffect(() => {
    if (!snap) return;
    const state = snap.server.state;
    if (state !== lastState.current) {
      const kind =
        state === "ready" ? "ok" : state === "stopped" || state === "error" ? "warn" : "info";
      pushEvent(kind as LogEvent["kind"], `inference server: ${state}`);
      lastState.current = state;
    }
  }, [snap, pushEvent]);

  return {
    snap,
    error,
    events,
    paused,
    pausedReason,
    lastAt,
    latencyMs,
    invertedPollMs: pollMs,
    refresh: () => void tick(),
  };
}

export { stamp };
