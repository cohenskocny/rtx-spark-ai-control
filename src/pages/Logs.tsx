import type { LogEvent, ReadyReport, Snapshot } from "../lib/types";
import { Card, EventList, Pill, Row, SectionHead } from "../components/primitives";
import { age, num } from "../lib/format";

export function Logs({
  snap,
  events,
  ready,
  pollMs,
  latencyMs,
  paused,
  lastAt,
}: {
  snap: Snapshot | null;
  events: LogEvent[];
  ready: ReadyReport | null;
  pollMs: number;
  latencyMs: number | null;
  paused: boolean;
  lastAt: number | null;
}) {
  const errors = events.filter((e) => e.kind === "error").length;

  return (
    <div className="stack">
      <div className="kpis">
        <div className="glass kpi">
          <div className="label">Poll interval</div>
          <div className="value tabular">
            {pollMs / 1000}
            <span className="unit">s</span>
          </div>
          <div className="foot">
            <span>samples</span>
            <span className="num">{snap?.seq ?? 0}</span>
          </div>
        </div>
        <div className="glass kpi">
          <div className="label">Round trip</div>
          <div className="value tabular">
            {latencyMs === null ? "—" : num(latencyMs, 0)}
            <span className="unit">ms</span>
          </div>
          <div className="foot">
            <span>includes</span>
            <span className="num">nvidia-smi</span>
          </div>
        </div>
        <div className="glass kpi">
          <div className="label">Startup</div>
          <div className="value tabular">
            {ready ? num(ready.startup_ms, 0) : "—"}
            <span className="unit">ms</span>
          </div>
          <div className="foot">
            <span>first paint</span>
            <span className="num">{ready?.arch ?? "—"}</span>
          </div>
        </div>
        <div className="glass kpi">
          <div className="label">Loop</div>
          <div className="value tabular" style={{ fontSize: 26 }}>
            {paused ? "paused" : "live"}
          </div>
          <div className="foot">
            <span>errors</span>
            <span className="num">{errors}</span>
          </div>
        </div>
      </div>

      <div className="split">
        <Card className="pad-lg">
          <SectionHead
            title="Session events"
            hint={`${events.length} recorded · newest first`}
          />
          <div style={{ maxHeight: 420, overflowY: "auto" }} className="scroll">
            <EventList events={events} limit={events.length} />
          </div>
        </Card>

        <Card className="pad-lg">
          <SectionHead title="Runtime" hint="read-only" />
          <div className="rows">
            <Row k="Host" v={snap?.host ?? "—"} mono />
            <Row k="App version" v={ready?.version ?? "2.0.0"} />
            <Row k="Target" v={ready?.arch ?? "aarch64"} mono />
            <Row k="Last sample" v={age(lastAt)} />
            <Row
              k="Endpoint"
              v={snap?.server.endpoint ?? "—"}
              mono
            />
            <Row
              k="Backend notes"
              v={snap?.notes?.length ? snap.notes.join(" · ") : "none"}
            />
          </div>
          <hr className="hairline" />
          <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
            <Pill tone={snap?.server.state === "ready" ? "live" : "idle"} small>
              server {snap?.server.state ?? "unknown"}
            </Pill>
            <Pill tone={snap?.gpu.available ? "live" : "offline"} small>
              gpu {snap?.gpu.available ? "telemetry" : "unavailable"}
            </Pill>
            <Pill tone="ghost" small>
              events kept in memory only
            </Pill>
          </div>
        </Card>
      </div>
    </div>
  );
}
