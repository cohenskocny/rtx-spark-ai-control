import type { Snapshot } from "../lib/types";
import { Card, Pill, Row, SectionHead } from "../components/primitives";
import { num, span } from "../lib/format";
import { blackoutAvailability, blackoutBlockedReason } from "../lib/optional";
import { actions } from "../lib/api";
import type { RunAction } from "../lib/useAction";

export function Blackout({
  snap,
  run,
  pending,
}: {
  snap: Snapshot | null;
  run: RunAction;
  pending: string | null;
}) {
  const blackout = snap?.blackout;
  const automation = snap?.automation;
  const settings = snap?.settings;
  const active = Boolean(blackout?.active);
  const running = Boolean(blackout?.running);
  const busy = pending !== null;
  // The guard is optional: when it is not configured, or the configured paths no
  // longer validate, every control here explains itself instead of failing.
  const availability = blackoutAvailability(
    Boolean(snap?.config.blackout_ready),
    snap?.config.missing_blackout ?? [],
  );
  const blocked = blackoutBlockedReason(availability);

  const timeouts = Array.from(
    new Set([30, 60, 180, 300, 900, settings?.idle_seconds ?? 180]),
  ).sort((a, b) => a - b);

  const phaseTone = active ? ("live" as const) : running ? ("loading" as const) : ("idle" as const);

  return (
    <div className="stack">
      <Card tint className="pad-lg">
        <div className="card-head">
          <div>
            <span className="eyebrow">OLED blackout · validated V1 engine</span>
            <h1 className="display-sm" style={{ marginTop: 12 }}>
              {!availability.available
                ? "Blackout is an optional extra"
                : active
                  ? "Panel held awake"
                  : running
                    ? "Armed, waiting for idle"
                    : "Blackout standing by"}
            </h1>
          </div>
          <Pill tone={phaseTone} pulse={active}>
            {blackout?.phase ?? "off"}
          </Pill>
        </div>

        <div className="hero-actions">
          <button
            type="button"
            className="btn primary"
            disabled={busy || !availability.available}
            onClick={() => void run("now", actions.blackoutNow)}
            title={blocked ?? "Cover the panel immediately, in RGB(0,0,0), without waiting for idle"}
          >
            {pending === "now" ? "Covering…" : "Blackout now"}
          </button>
          <button
            type="button"
            className="btn"
            disabled={busy || !running}
            onClick={() => void run("halt", actions.blackoutStop)}
            title="Ask the guard to exit; the display request is released with it"
          >
            {pending === "halt" ? "Stopping…" : "Stop blackout"}
          </button>
          <span className="spacer" />
          <span className="note">
            {active
              ? `Covering ${blackout?.cover ?? "internal"} · display request ${
                  blackout?.display_held ? "held" : "not held"
                }`
              : running
                ? `Activates after ${settings?.idle_seconds ?? 180}s without input`
                : "No guard process is running"}
          </span>
        </div>

        {blocked ? (
          <div className="note" style={{ marginTop: 12 }}>
            {blocked}
          </div>
        ) : null}

        <div className="rows">
          <Row
            k="Guard process"
            v={
              running
                ? `${blackout?.owned ? "managed here" : "external"} · pid ${blackout?.pid ?? "—"}`
                : "not running"
            }
            mono
          />
          <Row
            k="Display request"
            v={
              blackout?.display_held ? (
                <strong>held · panel cannot blank</strong>
              ) : blackout?.display_request ? (
                blackout.locked ? "enabled · released because the session is locked" : "enabled · not held yet"
              ) : (
                "off"
              )
            }
          />
          <Row
            k="Idle"
            v={
              blackout?.idle_s != null
                ? `${num(blackout.idle_s, 0)}s of ${settings?.idle_seconds ?? 180}s${
                    blackout.idle_remaining_s != null
                      ? ` · ${num(blackout.idle_remaining_s, 0)}s to activation`
                      : ""
                  }`
                : "—"
            }
          />
          <Row k="Session locked" v={blackout?.locked ? "yes — guard stands down" : "no"} />
          <Row k="STOP sentinel" v={blackout?.stop_file_present ? "present" : "absent"} />
          <Row
            k="State file"
            v={blackout?.updated_utc || "no state file"}
            mono
            title={blackout?.state_path}
          />
          {blackout?.error ? <Row k="Last error" v={<span className="dim">{blackout.error}</span>} /> : null}
        </div>
      </Card>

      <div className="split">
        <Card className="pad-lg">
          <SectionHead title="Automation" hint={automation?.reason ?? "—"} />

          <div className="setting">
            <div className="text">
              <span className="name">Automatic blackout</span>
              <span className="help">
                Arms the guard while this application runs. It activates only after a full
                inactivity interval — never after a few seconds.
              </span>
            </div>
            <button
              type="button"
              className="switch"
              data-on={settings?.blackout_enabled ?? false}
              aria-label="Automatic blackout"
              disabled={busy || !availability.available}
              title={blocked ?? "Arm the guard automatically"}
              onClick={() =>
                void run("auto", () => actions.blackoutAuto(!(settings?.blackout_enabled ?? false)))
              }
            />
          </div>

          <div className="setting">
            <div className="text">
              <span className="name">Follow Server</span>
              <span className="help">
                Arm only while the inference server answers. The state must hold for{" "}
                {span(automation?.hysteresis_s ?? 60)} before it is believed, and a failing check
                is tolerated for {span(automation?.grace_s ?? 300)}.
              </span>
            </div>
            <button
              type="button"
              className="switch"
              data-on={automation?.follow_server ?? false}
              aria-label="Follow server"
              disabled={busy || !availability.available}
              title={blocked ?? "Arm the guard only while the server is healthy"}
              onClick={() =>
                void run("follow", () => actions.followServer(!(automation?.follow_server ?? false)))
              }
            />
          </div>

          <div className="setting">
            <div className="text">
              <span className="name">Inactivity timeout</span>
              <span className="help">
                Restarting a running guard is required for a new timeout to take effect; it is
                done automatically and the guard keeps its state.
              </span>
            </div>
            <div className="segmented" role="group" aria-label="Inactivity timeout">
              {timeouts.map((seconds) => (
                <button
                  key={seconds}
                  type="button"
                  data-on={(settings?.idle_seconds ?? 180) === seconds}
                  disabled={busy}
                  onClick={() => void run("timeout", () => actions.blackoutTimeout(seconds))}
                >
                  {seconds >= 60 ? `${seconds / 60}m` : `${seconds}s`}
                </button>
              ))}
            </div>
          </div>

          <div className="setting">
            <div className="text">
              <span className="name">Cover</span>
              <span className="help">
                Internal covers only the OLED panel, which is what keeps Modern Standby away
                without blacking out external displays.
              </span>
            </div>
            <div className="segmented" role="group" aria-label="Cover">
              {["internal", "primary", "all"].map((option) => (
                <button
                  key={option}
                  type="button"
                  data-on={(settings?.cover ?? "internal") === option}
                  disabled={busy}
                  onClick={() => void run("cover", () => actions.blackoutCover(option))}
                >
                  {option}
                </button>
              ))}
            </div>
          </div>
        </Card>

        <Card className="pad-lg">
          <SectionHead title="What this does, and what it is not" />
          <div className="note" style={{ display: "grid", gap: 12 }}>
            <p style={{ margin: 0 }}>
              While the panel is covered, Windows still receives{" "}
              <span className="mono">ES_DISPLAY_REQUIRED</span> and a display power request, so
              the machine never enters Modern Standby and CUDA keeps its clocks. The overlay is
              pure RGB(0,0,0).
            </p>
            <p style={{ margin: 0 }}>
              The first keyboard or mouse event restores the desktop immediately. Sessions that
              lock release the request by design.
            </p>
            <p style={{ margin: 0 }}>
              This is <b>not a lock screen</b>. The overlay is an ordinary window on an unlocked
              desktop: it hides the display, not the session. Windows authentication is unchanged,
              and anything that needs a secure barrier should lock the workstation.
            </p>
          </div>
          <hr className="hairline" />
          <div className="rows">
            <Row k="Grace after failure" v={span(automation?.grace_s ?? 300)} />
            <Row k="Health hysteresis" v={span(automation?.hysteresis_s ?? 60)} />
            <Row k="Health poll" v={span(automation?.poll_s ?? 15)} />
            <Row k="Stop timeout" v={span(settings?.stop_timeout_s ?? 45)} />
          </div>
        </Card>
      </div>
    </div>
  );
}
