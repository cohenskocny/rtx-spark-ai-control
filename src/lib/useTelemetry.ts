import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { errorText, sample } from "./api";
import type { LogEvent, Snapshot, VisibilityEvent } from "./types";

/** Newest-first, capped: the ring in Rust is bounded too, but the window can
 *  outlive several thousand lines and the Logs page only shows so many. */
const MAX_EVENTS = 240;

export interface TelemetryState {
  snap: Snapshot | null;
  error: string | null;
  events: LogEvent[];
  paused: boolean;
  pausedReason: string;
  lastAt: number | null;
  latencyMs: number | null;
  refresh: () => void;
}

function stamp(): string {
  const d = new Date();
  return d.toLocaleTimeString("en-GB", { hour12: false });
}

/**
 * Serial polling with a completion-scheduled timer: the next request is queued
 * only after the previous one resolves, so a slow poll can never stack. Polling
 * stops completely when the window is minimised or hidden, and resumes with one
 * immediate sample.
 *
 * Events come from the backend ring, which is the only place that sees process
 * lifetimes and blackout transitions. The cursor advances with what has been
 * received, so no line is delivered twice and none are missed while hidden.
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
  const cursor = useRef(0);
  const hidden = useRef(false);
  const prefs = useRef({ pollMs, pauseWhenHidden });
  prefs.current = { pollMs, pauseWhenHidden };

  const mergeEvents = useCallback((incoming: LogEvent[]) => {
    if (!incoming.length) return;
    setEvents((prev) => {
      const highest = incoming.reduce((max, e) => (e.id > max ? e.id : max), 0);
      if (highest > cursor.current) cursor.current = highest;
      // Backend order is oldest-first; the lists render newest-first.
      const fresh = incoming.filter((e) => !prev.some((p) => p.id === e.id)).reverse();
      if (!fresh.length) return prev;
      return [...fresh, ...prev].slice(0, MAX_EVENTS);
    });
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
      const next = await sample(cursor.current);
      setSnap(next);
      mergeEvents(next.events);
      setError(null);
      setLastAt(Date.now());
    } catch (err) {
      setError(errorText(err));
    } finally {
      setLatencyMs(Math.round(performance.now() - began));
      busy.current = false;
      if (!stopped.current) {
        setPaused(false);
        setPausedReason("");
        schedule(prefs.current.pollMs);
      }
    }
  }, [mergeEvents, schedule]);

  // Visibility: Tauri reports minimise/restore; the DOM reports occlusion and
  // tab switches.
  useEffect(() => {
    // The effect owns the poller, so it must claim it on every mount: React 18
    // StrictMode mounts, unmounts and mounts again in development, and the
    // first cleanup would otherwise leave `stopped` set for the real mount and
    // the window would sit at "connecting" forever with no error to show.
    stopped.current = false;
    busy.current = false;
    let unlisten: (() => void) | undefined;
    void listen<VisibilityEvent>("v2://visibility", (event) => {
      const payload = event.payload;
      const next = payload.minimized || !payload.visible;
      if (next !== hidden.current) {
        hidden.current = next;
        if (!next) schedule(120);
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
    schedule(0);

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

  return {
    snap,
    error,
    events,
    paused,
    pausedReason,
    lastAt,
    latencyMs,
    refresh: () => void tick(),
  };
}

export { stamp };
