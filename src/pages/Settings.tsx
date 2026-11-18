import type { ReadyReport, Snapshot } from "../lib/types";
import type { Prefs } from "../lib/prefs";
import { Card, Row, SectionHead } from "../components/primitives";
import { actions } from "../lib/api";
import type { RunAction } from "../lib/useAction";

export function Settings({
  snap,
  prefs,
  setPref,
  latencyMs,
  paused,
  ready,
  run,
  pending,
}: {
  snap: Snapshot | null;
  prefs: Prefs;
  setPref: (patch: Partial<Prefs>) => void;
  latencyMs: number | null;
  paused: boolean;
  ready: ReadyReport | null;
  run: RunAction;
  pending: string | null;
}) {
  const settings = snap?.settings;
  const model = snap?.model;
  const automation = snap?.automation;
  const busy = pending !== null;

  return (
    <div className="stack">
      <Card tint className="pad-lg">
        <SectionHead title="Dashboard behaviour" hint="applies immediately, persists locally" />

        <div className="setting">
          <div className="text">
            <span className="name">Interface refresh</span>
            <span className="help">
              How often the window asks for a new sample. The next request is queued only after
              the previous one returns, so a slow sample never stacks.
            </span>
          </div>
          <div className="segmented" role="group" aria-label="Interface refresh">
            {[1000, 2000, 5000].map((ms) => (
              <button
                key={ms}
                type="button"
                data-on={prefs.pollMs === ms}
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
              Minimised or occluded windows stop sampling entirely. Blackout automation keeps
              running in the backend either way.
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
            <span className="name">Telemetry interval</span>
            <span className="help">
              How often the backend polls the inference server&apos;s health and drives Follow
              Server. Persisted in the application&apos;s own settings file.
            </span>
          </div>
          <div className="segmented" role="group" aria-label="Telemetry interval">
            {[5, 15, 30, 60].map((seconds) => (
              <button
                key={seconds}
                type="button"
                data-on={(settings?.poll_s ?? 15) === seconds}
                disabled={busy}
                onClick={() => void run("interval", () => actions.telemetryInterval(seconds))}
              >
                {seconds}s
              </button>
            ))}
          </div>
        </div>
      </Card>

      <div className="split-even">
        <Card className="pad-lg">
          <SectionHead title="Automation" hint="persisted · edit on the Blackout page" />
          <div className="setting">
            <div className="text">
              <span className="name">Start inference with the window</span>
              <span className="help">
                Off by default. When on, the window asks the backend to launch the server once at
                startup — except on battery, where the launch gate refuses regardless of this
                switch.
              </span>
            </div>
            <button
              type="button"
              className="switch"
              data-on={settings?.autostart_inference ?? false}
              aria-label="Start inference with the window"
              disabled={busy}
              onClick={() =>
                void run("autostart", () =>
                  actions.autostartSet(!(settings?.autostart_inference ?? false)),
                )
              }
            />
          </div>
          <div className="rows">
            <Row
              k="Automatic blackout"
              v={settings?.blackout_enabled ? "enabled" : "disabled"}
            />
            <Row k="Follow Server" v={settings?.follow_server ? "on" : "off"} />
            <Row k="Inactivity timeout" v={`${settings?.idle_seconds ?? 180} s`} />
            <Row k="Cover" v={settings?.cover ?? "internal"} />
            <Row k="Phase" v={automation?.phase ?? "—"} />
            <Row k="Health hysteresis" v={`${settings?.hysteresis_s ?? 60} s`} />
            <Row k="Failure grace" v={`${settings?.grace_s ?? 300} s`} />
            <Row k="Stop timeout" v={`${settings?.stop_timeout_s ?? 45} s`} />
          </div>
          <div className="note" style={{ marginTop: 14 }}>
            Stored in <span className="mono">%APPDATA%\com.rtxspark.aicontrol.v2\settings.json</span>{" "}
            and reloaded on the next launch. Nothing here creates a startup entry, a service or a
            scheduled task.
          </div>
        </Card>

        <Card className="pad-lg">
          <SectionHead title="Inference configuration" hint="selected on the Setup page" />
          <div className="rows">
            <Row k="Model" v={model?.served_name ?? "—"} mono />
            <Row k="Endpoint" v={model?.endpoint ?? "—"} mono />
            <Row k="Precision" v={`${model?.dtype ?? "—"} · KV ${model?.kv_cache_dtype ?? "—"}`} />
            <Row k="MTP depth" v={model?.mtp_depth ?? "—"} />
            <Row k="Context" v={`${model?.max_model_len ?? "—"} tokens`} />
            <Row k="Max sequences" v={model?.max_num_seqs ?? "—"} />
            <Row k="Prefix caching" v={model?.prefix_caching ? "enabled" : "disabled"} />
            <Row k="Config file" v={model?.config_path ?? "—"} mono title={model?.config_path} />
          </div>
        </Card>
      </div>

      <div className="split-even">
        <Card className="pad-lg">
          <SectionHead title="This session" />
          <div className="rows">
            <Row k="Interface refresh" v={`${prefs.pollMs / 1000} s`} />
            <Row k="Last round trip" v={latencyMs === null ? "—" : `${latencyMs} ms`} />
            <Row k="Polling" v={paused ? "suspended" : "active"} />
            <Row k="Samples" v={snap ? String(snap.seq) : "0"} />
            <Row k="Backend uptime" v={snap ? `${Math.round(snap.server_uptime_s / 1000)} s` : "—"} />
            <Row k="Startup" v={ready ? `${ready.startup_ms} ms to first paint` : "—"} />
            <Row k="Power" v={snap?.power.detail ?? "—"} />
            <Row k="Inference engine" v={snap?.installed.version || "not detected"} mono />
            <Row k="Compatibility" v={snap?.compat.status ?? "—"} />
          </div>
        </Card>

        <Card className="pad-lg">
          <SectionHead title="Deliberately absent" />
          <div className="rows">
            <Row k="Launch at login" v={<span className="dim">by design</span>} />
            <Row k="Background service" v={<span className="dim">by design</span>} />
            <Row k="Scheduled task" v={<span className="dim">by design</span>} />
            <Row k="Registry writes" v={<span className="dim">none</span>} />
          </div>
          <div className="note" style={{ marginTop: 14 }}>
            The control center is a window, not a daemon. Closing it stops the blackout guard it
            started and leaves the inference server running exactly as it was.
          </div>
        </Card>
      </div>

      <Card quiet className="pad">
        <div className="note">
          Installation paths live in <span className="mono">%APPDATA%\com.rtxspark.aicontrol.v2\settings.json</span>{" "}
          and are chosen on the <b>Setup</b> page. The blackout engine keeps its own{" "}
          <span className="mono">state.json</span> beside its script. Interface preferences above
          live in this window&apos;s local storage; the automation values live in the application&apos;s
          settings file.
        </div>
      </Card>
    </div>
  );
}
