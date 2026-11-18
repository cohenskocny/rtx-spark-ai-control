import type { Snapshot } from "../lib/types";
import { Card, Pill, Row, SectionHead } from "../components/primitives";

export function Blackout({ snap }: { snap: Snapshot | null }) {
  const blackout = snap?.blackout;
  const active = Boolean(blackout?.engine_active);

  return (
    <div className="stack">
      <Card tint className="pad-lg">
        <div className="card-head">
          <div>
            <span className="eyebrow">OLED blackout · V1 engine</span>
            <h1 className="display-sm" style={{ marginTop: 12 }}>
              {active ? "Panel held awake" : "Blackout standing by"}
            </h1>
          </div>
          <Pill tone={active ? "live" : "idle"} pulse={active}>
            {active ? "active" : "idle"}
          </Pill>
        </div>

        <div className="rows">
          <Row
            k="Display request"
            v={blackout?.display_held ? <strong>held</strong> : "not held"}
          />
          <Row k="Phase" v={blackout?.phase ?? "unknown"} />
          <Row k="Auto blackout" v={blackout?.enabled ? "enabled" : "disabled"} />
          <Row k="Inactivity timeout" v={`${blackout?.idle_seconds ?? 180} s`} />
          <Row k="Follow server" v={blackout?.follow_server ? "on" : "off"} />
          <Row k="Cover" v={blackout?.cover ?? "internal"} />
          <Row k="STOP sentinel" v={blackout?.stop_file_present ? "present" : "absent"} />
          <Row
            k="State file written"
            v={blackout?.updated_utc ? `${blackout.updated_utc}${blackout.reason ? ` · ${blackout.reason}` : ""}` : "no state file"}
            mono
            title={blackout?.state_path}
          />
        </div>
      </Card>

      <div className="split">
        <Card className="pad-lg">
          <SectionHead title="What V2 does, and deliberately does not" />
          <div className="note" style={{ display: "grid", gap: 12 }}>
            <p style={{ margin: 0 }}>
              V2 reads the engine&apos;s own state file and shows what it finds. It does not
              create a display power request, does not paint an overlay, and does not start
              or stop the engine.
            </p>
            <p style={{ margin: 0 }}>
              That is intentional for this phase: the blackout engine is the validated V1
              implementation, and a second controller racing it is the fastest way to leave a
              panel covered or a power request leaked.
            </p>
            <p style={{ margin: 0 }}>
              When blackout is active, CUDA runs at full clock while the panel stays dark.
              Sessions that lock release the request by design — this is not a lock screen,
              and V2 does not pretend otherwise.
            </p>
          </div>
        </Card>

        <Card className="pad-lg">
          <SectionHead title="Controls" hint="Phase 2" />
          <div className="list">
            <div className="list-item">
              <span className="k">Blackout now</span>
              <button className="btn" disabled aria-disabled="true">
                Phase 2
              </button>
            </div>
            <div className="list-item">
              <span className="k">Stop blackout</span>
              <button className="btn" disabled aria-disabled="true">
                Phase 2
              </button>
            </div>
            <div className="list-item">
              <span className="k">Automatic blackout</span>
              <button className="switch" data-on={blackout?.enabled ?? false} disabled aria-disabled="true" />
            </div>
          </div>
          <hr className="hairline" />
          <div className="note">
            Migration order: read-only telemetry (done) → process ownership and lifecycle →
            blackout supervision → Follow Server automation. Each step keeps V1 available as
            the fallback.
          </div>
        </Card>
      </div>
    </div>
  );
}
