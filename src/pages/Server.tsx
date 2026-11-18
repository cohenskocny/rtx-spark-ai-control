import type { Snapshot } from "../lib/types";
import { Card, Pill, Row, SectionHead } from "../components/primitives";
import { age, num, uptime } from "../lib/format";
import { actions } from "../lib/api";
import type { RunAction } from "../lib/useAction";

function tone(state: string | undefined) {
  if (state === "ready") return "live" as const;
  if (state === "loading" || state === "starting" || state === "stopping") return "loading" as const;
  if (state === "error") return "offline" as const;
  return "idle" as const;
}

export function Server({
  snap,
  lastAt,
  run,
  pending,
}: {
  snap: Snapshot | null;
  lastAt: number | null;
  run: RunAction;
  pending: string | null;
}) {
  const server = snap?.server;
  const model = snap?.model;
  const busy = pending !== null;
  const progress = server?.progress_pct;
  const busyStage =
    server?.state === "starting" || server?.state === "loading" || server?.state === "stopping";
  const tail = (server?.log_tail ?? []).slice(-12);

  return (
    <div className="stack">
      <Card tint className="pad-lg">
        <div className="card-head">
          <div>
            <span className="eyebrow">Inference server · OpenAI compatible</span>
            <h1 className="display-sm" style={{ marginTop: 12 }}>
              {server?.models?.[0] ?? model?.served_name ?? "Qwen Flash Next"}
            </h1>
          </div>
          <Pill tone={tone(server?.state)} pulse={server?.state === "ready"}>
            {server?.state ?? "unknown"}
          </Pill>
        </div>

        <div className="rows">
          <Row k="Endpoint" v={server?.endpoint ?? "—"} mono />
          <Row
            k="Health"
            v={
              server?.health_ok
                ? `GET /v1/models · 200 · ${num(server.latency_ms, 0)} ms`
                : (server?.health_error ?? "no response")
            }
          />
          <Row k="Listening" v={server?.listening ? "yes (loopback)" : "no"} />
          <Row
            k="Process"
            v={
              server?.pid
                ? `${server.process_name ?? "process"} · pid ${server.pid}`
                : "not found"
            }
            mono
          />
          <Row k="Uptime" v={uptime(server?.uptime_s)} />
          <Row
            k="Ownership"
            v={
              server?.owned
                ? server.adopted
                  ? "adopted from outside — Stop and Restart apply"
                  : "started by this application"
                : server?.listening
                  ? "external — left alone until adopted"
                  : "none"
            }
          />
          <Row k="Stage" v={server?.detail ?? "—"} />
          <Row k="Sampled" v={age(lastAt)} />
        </div>

        <div className="progress" data-active={busyStage}>
          <span style={{ width: `${busyStage ? Math.max(4, Math.min(100, progress ?? 12)) : 0}%` }} />
        </div>
        <div className="note" style={{ marginTop: 10 }}>
          {server?.last_error
            ? server.last_error
            : server?.state === "ready"
              ? `Ready — /v1/models answered in ${num(server?.latency_ms, 0)} ms${
                  server?.owned ? "." : " (started outside this application)."
                }`
              : progress != null
                ? `Weight loading ${num(progress, 0)}% — read from vLLM's own progress output.`
                : "Progress is only shown when vLLM prints a percentage; readiness always comes from /v1/models."}
        </div>
      </Card>

      <div className="split">
        <Card className="pad-lg">
          <SectionHead title="Lifecycle" hint="same launch command as V1" />
          <div className="list" style={{ marginBottom: 18 }}>
            <div className="list-item">
              <span className="k">Start</span>
              <button
                type="button"
                className="btn primary"
                disabled={!server?.can_start || busy}
                onClick={() => void run("start", actions.serverStart)}
              >
                {pending === "start" ? "Starting…" : "Start"}
              </button>
            </div>
            <div className="list-item">
              <span className="k">Stop</span>
              <button
                type="button"
                className="btn"
                disabled={!server?.can_stop || busy}
                onClick={() => void run("stop", actions.serverStop)}
              >
                {pending === "stop" ? "Stopping…" : "Stop"}
              </button>
            </div>
            <div className="list-item">
              <span className="k">Restart</span>
              <button
                type="button"
                className="btn"
                disabled={!server?.can_restart || busy}
                onClick={() => void run("restart", actions.serverRestart)}
              >
                {pending === "restart" ? "Restarting…" : "Restart"}
              </button>
            </div>
            <div className="list-item">
              <span className="k">Adopt</span>
              <button
                type="button"
                className="btn ghost"
                disabled={!server?.can_adopt || busy}
                onClick={() => void run("adopt", actions.serverAdopt)}
              >
                {server?.can_adopt ? "Take ownership" : "—"}
              </button>
            </div>
          </div>
          <div className="note">
            A server this application did not start is detected and shown, never terminated.
            Adopt records its pid and creation time, which is what makes Stop and Restart safe
            from then on. Restart waits for the supervised child to exit before launching again,
            and a failed launch leaves the error in the log rather than a half-started server.
          </div>
        </Card>

        <Card className="pad-lg">
          <SectionHead title="Server output" hint="last 12 lines" />
          {tail.length ? (
            <div className="log-tail mono scroll">
              {tail.map((line, index) => (
                <div key={`${index}-${line.slice(0, 12)}`} className="log-line">
                  {line}
                </div>
              ))}
            </div>
          ) : (
            <div className="empty">
              {server?.listening || server?.state === "ready"
                ? "No output captured — the server was started outside this application."
                : "Nothing running."}
            </div>
          )}
        </Card>
      </div>

      <Card className="pad-lg">
        <SectionHead title="Configuration in use" hint="paths read from V1, never written" />
        <div className="rows">
          <Row k="Model" v={model?.served_name ?? "—"} mono />
          <Row k="Checkpoint" v={model?.checkpoint ?? "—"} mono title={model?.checkpoint} />
          <Row k="vLLM config" v={model?.config_path ?? "—"} mono title={model?.config_path} />
          <Row k="Chat template" v={model?.chat_template ?? "—"} mono title={model?.chat_template} />
          <Row
            k="Precision"
            v={`${model?.dtype ?? "—"} · KV ${model?.kv_cache_dtype ?? "—"} · MTP ${model?.mtp_depth ?? "—"}`}
          />
          <Row k="Prefix caching" v={model?.prefix_caching ? "enabled" : "disabled"} />
          <Row
            k="Launch paths"
            v={server?.paths_ok ? "all present" : "missing — Start will report which"}
          />
          <Row
            k="Checkpoint present"
            v={model?.checkpoint_exists ? "yes" : "no — path missing"}
          />
        </div>
      </Card>
    </div>
  );
}
