import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Shell } from "./components/Shell";
import { Home } from "./pages/Home";
import { Server } from "./pages/Server";
import { Blackout } from "./pages/Blackout";
import { Settings } from "./pages/Settings";
import { Logs } from "./pages/Logs";
import { loadPrefs, savePrefs, type Prefs } from "./lib/prefs";
import { useTelemetry } from "./lib/useTelemetry";
import { useAction } from "./lib/useAction";
import type { ReadyReport } from "./lib/types";
import { clockTime } from "./lib/format";

export default function App() {
  const [prefs, setPrefs] = useState<Prefs>(() => loadPrefs());
  const [ready, setReady] = useState<ReadyReport | null>(null);
  const [history, setHistory] = useState<number[]>([]);
  const telemetry = useTelemetry(prefs.pollMs, prefs.pauseWhenHidden);
  const { pending, note, run, dismiss } = useAction(telemetry.refresh);
  const lastSeq = useRef(0);

  const setPref = useCallback((patch: Partial<Prefs>) => {
    setPrefs((prev) => {
      const next = { ...prev, ...patch };
      savePrefs(next);
      return next;
    });
  }, []);

  useEffect(() => {
    invoke<ReadyReport>("frontend_ready")
      .then(setReady)
      .catch(() => setReady(null));
  }, []);

  useEffect(() => {
    document.documentElement.dataset.reduceMotion = String(prefs.reduceMotion);
  }, [prefs.reduceMotion]);

  // A note that never leaves is clutter; six seconds is enough to read it.
  useEffect(() => {
    if (!note) return;
    const timer = window.setTimeout(dismiss, 6000);
    return () => window.clearTimeout(timer);
  }, [note, dismiss]);

  // One point per received sample (not per render), capped so the trace keeps a
  // fixed cost no matter how long the window stays open.
  useEffect(() => {
    const snap = telemetry.snap;
    if (!snap || snap.seq === lastSeq.current) return;
    lastSeq.current = snap.seq;
    setHistory((prev) => {
      const next = [...prev, snap.gpu.util_pct ?? 0];
      return next.length > 60 ? next.slice(next.length - 60) : next;
    });
  }, [telemetry.snap]);

  const { snap, paused, lastAt, latencyMs } = telemetry;

  const statusPill = useMemo(() => {
    const state = snap?.server.state;
    if (!snap) return { tone: "idle" as const, text: "connecting", pulse: true };
    if (state === "ready") return { tone: "live" as const, text: "model ready", pulse: true };
    if (state === "loading") return { tone: "loading" as const, text: "loading", pulse: true };
    if (state === "starting") return { tone: "loading" as const, text: "starting", pulse: true };
    if (state === "error") return { tone: "offline" as const, text: "attention", pulse: false };
    if (state === "external") return { tone: "idle" as const, text: "external", pulse: false };
    if (state === "stopping") return { tone: "loading" as const, text: "stopping", pulse: true };
    return { tone: "idle" as const, text: "stopped", pulse: false };
  }, [snap]);

  const topRight = (
    <>
      <span className={`pill ${statusPill.tone} ${statusPill.pulse ? "pulse" : ""}`}>
        <span className="dot" />
        {telemetry.error ? "telemetry error" : statusPill.text}
      </span>
      {snap?.automation.enabled ? (
        <span className="pill ghost sm" title={snap.automation.reason}>
          auto blackout · {snap.automation.phase}
        </span>
      ) : null}
      <span className="pill ghost sm" title="Last successful sample">
        {paused ? "paused" : `sampled ${clockTime(lastAt)}`}
      </span>
      <button
        type="button"
        className="btn ghost"
        onClick={telemetry.refresh}
        title="Sample once now"
      >
        Refresh
      </button>
    </>
  );

  const foot = (
    <>
      <div className="line">
        <span>Endpoint</span>
        <b className="mono">127.0.0.1:{snap?.server.port ?? 8000}</b>
      </div>
      <div className="line">
        <span>Poll</span>
        <b>
          {prefs.pollMs / 1000}s · {latencyMs === null ? "—" : `${latencyMs}ms`}
        </b>
      </div>
      <div className="line">
        <span>Server</span>
        <b>{snap?.server.owned ? "managed here" : snap?.server.listening ? "external" : "idle"}</b>
      </div>
    </>
  );

  return (
    <>
      <Shell
        page={prefs.page}
        onNavigate={(id) => setPref({ page: id })}
        topRight={topRight}
        foot={foot}
      >
        {prefs.page === "home" && (
          <Home
            snap={snap}
            events={telemetry.events}
            history={history}
            lastAt={lastAt}
            run={run}
            pending={pending}
          />
        )}
        {prefs.page === "server" && (
          <Server snap={snap} lastAt={lastAt} run={run} pending={pending} />
        )}
        {prefs.page === "blackout" && <Blackout snap={snap} run={run} pending={pending} />}
        {prefs.page === "settings" && (
          <Settings
            snap={snap}
            prefs={prefs}
            setPref={setPref}
            latencyMs={latencyMs}
            paused={paused}
            ready={ready}
            run={run}
            pending={pending}
          />
        )}
        {prefs.page === "logs" && (
          <Logs
            snap={snap}
            events={telemetry.events}
            ready={ready}
            latencyMs={latencyMs}
            paused={paused}
            lastAt={lastAt}
            pollMs={prefs.pollMs}
          />
        )}
      </Shell>
      {note ? (
        <div className={`toast ${note.ok ? "ok" : "fail"}`} role="status">
          <span>{note.text}</span>
          <button type="button" className="btn ghost sm" onClick={dismiss}>
            Dismiss
          </button>
        </div>
      ) : null}
    </>
  );
}
