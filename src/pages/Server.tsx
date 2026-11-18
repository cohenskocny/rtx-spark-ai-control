import type { Snapshot } from "../lib/types";
import { Card, Pill, Row, SectionHead } from "../components/primitives";
import { age, num, uptime } from "../lib/format";

function tone(state: string | undefined) {
  if (state === "ready") return "live" as const;
  if (state === "loading") return "loading" as const;
  if (state === "error") return "offline" as const;
  return "idle" as const;
}

export function Server({ snap, lastAt }: { snap: Snapshot | null; lastAt: number | null }) {
  const server = snap?.server;
  const model = snap?.model;

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
                : (server?.error ?? "no response")
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
          <Row k="Sampled" v={age(lastAt)} />
        </div>
      </Card>

      <div className="split">
        <Card className="pad-lg">
          <SectionHead title="Detected models" hint="advertised by the API" />
          {server?.models?.length ? (
            <div className="rows">
              {server.models.map((id) => (
                <Row key={id} k="model" v={<strong>{id}</strong>} mono />
              ))}
            </div>
          ) : (
            <div className="empty">The API is not advertising models yet</div>
          )}
          <hr className="hairline" />
          <div className="note">
            Loading and ready are distinguished by the socket plus a passing{" "}
            <span className="mono">/v1/models</span> call. A bound socket with no answer is
            reported as <em>loading</em> until it has been in that state for more than
            15 minutes, then as <em>attention</em>.
          </div>
        </Card>

        <Card className="pad-lg">
          <SectionHead title="Lifecycle" hint="disabled in this phase" />
          <div className="list" style={{ marginBottom: 18 }}>
            <div className="list-item">
              <span className="k">Start</span>
              <button className="btn" disabled aria-disabled="true">
                Not in V2 yet
              </button>
            </div>
            <div className="list-item">
              <span className="k">Stop</span>
              <button className="btn" disabled aria-disabled="true">
                Not in V2 yet
              </button>
            </div>
            <div className="list-item">
              <span className="k">Restart</span>
              <button className="btn" disabled aria-disabled="true">
                Not in V2 yet
              </button>
            </div>
          </div>
          <div className="note">
            V2 observes only. Process ownership, start/stop and restart are Phase 2 work and
            will reuse V1&apos;s ownership rules — a server V2 did not spawn must never be
            terminated by accident.
          </div>
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
            k="Checkpoint present"
            v={model?.checkpoint_exists ? "yes" : "no — path missing"}
          />
        </div>
      </Card>
    </div>
  );
}
