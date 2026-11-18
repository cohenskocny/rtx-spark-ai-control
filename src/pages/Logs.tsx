import type { LogEvent, ReadyReport, Snapshot } from "../lib/types";
import { Card, EventList, Pill, Row, SectionHead } from "../components/primitives";
import { age, num } from "../lib/format";

export function Logs({
  snap,
  events,
  ready,
  latencyMs,
  paused,
  lastAt,
  pollMs,
}: {
  snap: Snapshot | null;
  events: LogEvent[];
  ready: ReadyReport | null;
  latencyMs: number | null;
  paused: boolean;
  lastAt: number | null;
  pollMs: number;
}) {
  const errors = events.filter((e) => e.kind === "error").length;
  const warnings = events.filter((e) => e.kind === "warn").length;
  const server = snap?.server;

  return (
    <div className="stack">
      <div className="kpis">
        <div className="glass kpi">
          <div className="label">Events</div>
          <div className="value tabular">
            {events.length}
            <span className="unit">kept</span>
          </div>
          <div className="foot">
            <span>errors</span>
            <span className="num">{errors}</span>
          </div>
        </div>
        <div className="glass kpi">
          <div className="label">Warnings</div>
          <div className="value tabular">
            {warnings}
            <span className="unit">total</span>
          </div>
          <div className="foot">
            <span>sampled</span>
            <span className="num">{age(lastAt)}</span>
          </div>
        </div>
        <div className="glass kpi">
          <div className="label">Round trip</div>
          <div className="value tabular">
            {latencyMs === null ? "—" : num(latencyMs, 0)}
            <span className="unit">ms</span>
          </div>
          <div className="foot">
            <span>every</span>
            <span className="num">{pollMs / 1000}s</span>
          </div>
        </div>
        <div className="glass kpi">
          <div className="label">Loop</div>
          <div className="value tabular" style={{ fontSize: 26 }}>
            {paused ? "paused" : "live"}
          </div>
          <div className="foot">
            <span>startup</span>
            <span className="num">{ready ? num(ready.startup_ms, 0) : "—"} ms</span>
          </div>
        </div>
      </div>

      <div className="split">
        <Card className="pad-lg">
          <SectionHead title="Session events" hint={`${events.length} recorded · newest first`} />
          <div style={{ maxHeight: 360, overflowY: "auto" }} className="scroll">
            <EventList events={events} limit={events.length} />
          </div>
          <hr className="hairline" />
          <SectionHead title="Server output" hint="tail of the managed process" />
          {server?.log_tail?.length ? (
            <div className="log-tail mono scroll" style={{ maxHeight: 200 }}>
              {server.log_tail.slice(-40).map((line, index) => (
                <div key={`${index}-${line.slice(0, 12)}`} className="log-line">
                  {line}
                </div>
              ))}
            </div>
          ) : (
            <div className="empty">
              {server?.listening ? "Started outside this application; its output is not captured." : "No server output."}
            </div>
          )}
        </Card>

        <Card className="pad-lg">
          <SectionHead title="Runtime" hint="live" />
          <div className="rows">
            <Row k="Host" v={snap?.host ?? "—"} mono />
            <Row k="App version" v={ready?.version ?? "2.0.0"} />
            <Row k="Target" v={ready?.arch ?? "aarch64"} mono />
            <Row k="Endpoint" v={server?.endpoint ?? "—"} mono />
            <Row k="Server state" v={`${server?.state ?? "unknown"} · ${server?.detail ?? "—"}`} />
            <Row
              k="Ownership"
              v={server?.owned ? (server.adopted ? "adopted" : "started here") : "external / idle"}
            />
            <Row
              k="Blackout"
              v={
                snap
                  ? `${snap.blackout.phase}${snap.blackout.active ? " · covering" : ""}${
                      snap.automation.enabled ? ` · auto ${snap.automation.phase}` : ""
                    }`
                  : "—"
              }
            />
            <Row k="Last action" v={snap?.last_action?.message ?? "—"} />
            <Row
              k="Backend notes"
              v={snap?.notes?.length ? snap.notes.join(" · ") : "none"}
            />
          </div>
          <hr className="hairline" />
          <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
            <Pill tone={server?.state === "ready" ? "live" : "idle"} small>
              server {server?.state ?? "unknown"}
            </Pill>
            <Pill tone={snap?.gpu.available ? "live" : "offline"} small>
              gpu {snap?.gpu.available ? "telemetry" : "unavailable"}
            </Pill>
            <Pill tone={snap?.blackout.running ? "live" : "ghost"} small>
              blackout {snap?.blackout.phase ?? "off"}
            </Pill>
            <Pill tone="ghost" small>
              400-line ring, in memory only
            </Pill>
          </div>
          <hr className="hairline" />
          <div className="note">
            Server lifecycle lines, blackout transitions, warnings and errors are recorded by the
            backend, not by this window, so nothing is lost while it is hidden. Identical lines
            within five seconds collapse into a repeat count.
          </div>
        </Card>
      </div>
    </div>
  );
}
