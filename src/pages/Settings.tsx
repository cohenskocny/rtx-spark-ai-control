import type { ReadyReport, Snapshot } from "../lib/types";
import type { Prefs } from "../lib/prefs";
import { Card, Row, SectionHead } from "../components/primitives";

export function Settings({
  snap,
  prefs,
  setPref,
  pollMs,
  latencyMs,
  paused,
  ready,
}: {
  snap: Snapshot | null;
  prefs: Prefs;
  setPref: (patch: Partial<Prefs>) => void;
  pollMs: number;
  latencyMs: number | null;
  paused: boolean;
  ready: ReadyReport | null;
}) {
  return (
    <div className="stack">
      <Card tint className="pad-lg">
        <SectionHead title="Dashboard behaviour" hint="applies immediately, persists locally" />

        <div className="setting">
          <div className="text">
            <span className="name">Telemetry interval</span>
            <span className="help">
              How often the backend samples the GPU, the API and the blackout state. The next
              sample is queued only after the previous one returns.
            </span>
          </div>
          <div className="segmented" role="group" aria-label="Telemetry interval">
            {[1000, 2000, 5000].map((ms) => (
              <button
                key={ms}
                type="button"
                data-on={pollMs === ms}
                onClick={() => setPref({ pollMs: ms })}
              >
                {ms / 1000}s
              </button>
            ))}
          </div>
        </div>

        <div className="setting">
          <div className="text">
            <span className="name">Suspend polling while hidden</span>
            <span className="help">
              Minimised or occluded windows stop sampling entirely. Restoring the window
              takes one fresh sample.
            </span>
          </div>
          <button
            type="button"
            className="switch"
            data-on={prefs.pauseWhenHidden}
            aria-label="Suspend polling while hidden"
            onClick={() => setPref({ pauseWhenHidden: !prefs.pauseWhenHidden })}
          />
        </div>

        <div className="setting">
          <div className="text">
            <span className="name">Reduce motion</span>
            <span className="help">
              Removes page transitions and the ambient pulse. The OS setting is honoured
              regardless of this switch.
            </span>
          </div>
          <button
            type="button"
            className="switch"
            data-on={prefs.reduceMotion}
            aria-label="Reduce motion"
            onClick={() => setPref({ reduceMotion: !prefs.reduceMotion })}
          />
        </div>

        <div className="setting">
          <div className="text">
            <span className="name">Remember last page</span>
            <span className="help">
              Reopening the application returns to the page shown when it last closed:
              currently <b>{prefs.page}</b>.
            </span>
          </div>
          <button
            type="button"
            className="switch"
            data-on
            aria-label="Remember last page"
            onClick={() => setPref({ page: "home" })}
          />
        </div>
      </Card>

      <div className="split-even">
        <Card className="pad-lg">
          <SectionHead title="This session" />
          <div className="rows">
            <Row k="Poll interval" v={`${pollMs / 1000} s`} />
            <Row k="Last poll" v={latencyMs === null ? "—" : `${latencyMs} ms round trip`} />
            <Row k="Polling" v={paused ? "suspended" : "active"} />
            <Row k="Samples" v={snap ? String(snap.seq) : "0"} />
            <Row k="Startup" v={ready ? `${ready.startup_ms} ms to first paint` : "—"} />
          </div>
        </Card>

        <Card className="pad-lg">
          <SectionHead title="Planned settings" hint="not implemented" />
          <div className="rows">
            <Row k="Launch at login" v={<span className="dim">Phase 3</span>} />
            <Row k="Tray icon" v={<span className="dim">Phase 3</span>} />
            <Row k="Accent palette" v={<span className="dim">Phase 3</span>} />
            <Row k="vLLM profiles" v={<span className="dim">Phase 2</span>} />
          </div>
          <div className="note" style={{ marginTop: 14 }}>
            Nothing in this application writes to the registry, creates a service, or adds a
            startup entry.
          </div>
        </Card>
      </div>

      <Card quiet className="pad">
        <div className="note">
          Configuration is read from <span className="mono">C:\Users\cohen\AI\RTX-Spark-AI-Control\config.json</span>{" "}
          and the blackout engine&apos;s <span className="mono">state.json</span>. Preferences
          above live in this window&apos;s local storage and are reset by clearing them.
        </div>
      </Card>
    </div>
  );
}
