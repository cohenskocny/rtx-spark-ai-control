import type { LogEvent, Snapshot } from "../lib/types";
import { Kpi, Card, Pill, Row, SectionHead, Sparkline, EventList } from "../components/primitives";
import { age, context, gib, mhz, num, pct, throttleReasons, uptime, watts } from "../lib/format";

function stateTone(state: string | undefined): "live" | "loading" | "offline" | "idle" {
  if (state === "ready") return "live";
  if (state === "loading") return "loading";
  if (state === "error") return "offline";
  return "idle";
}

function stateLabel(state: string | undefined): string {
  if (state === "ready") return "Model ready";
  if (state === "loading") return "Loading";
  if (state === "error") return "Attention";
  return "Stopped";
}

export function Home({
  snap,
  events,
  history,
  lastAt,
}: {
  snap: Snapshot | null;
  events: LogEvent[];
  history: number[];
  lastAt: number | null;
}) {
  const gpu = snap?.gpu;
  const model = snap?.model;
  const server = snap?.server;
  const blackout = snap?.blackout;
  const reasons = throttleReasons(gpu?.throttle);

  const vramPct =
    gpu?.mem_used_mib && gpu?.mem_total_mib
      ? (gpu.mem_used_mib / gpu.mem_total_mib) * 100
      : undefined;

  return (
    <div className="stack">
      <div className="hero">
        <Card tint className="pad-lg hero-card">
          <div className="hero-title">
            <span className="eyebrow">Local inference · {snap?.host ?? "RTX Spark N1X"}</span>
            <h1 className="model">
              Qwen <em>Flash Next</em>
            </h1>
            <div className="hero-meta">
              <Pill tone={stateTone(server?.state)} pulse={server?.state === "ready"}>
                {stateLabel(server?.state)}
              </Pill>
              <span className="tag">{model?.dtype ?? "—"}</span>
              <span className="tag">
                MTP {model?.mtp_depth ?? "—"}
              </span>
              <span className="tag">{context(model?.max_model_len)} ctx</span>
            </div>
          </div>

          <div className="hero-foot">
            <span className="endpoint">
              <span
                className={`led ${
                  server?.state === "ready" ? "" : server?.state === "loading" ? "warm" : "off"
                }`}
              />
              {model?.endpoint ?? "http://127.0.0.1:8000/v1"}
            </span>
            <span className="note">
              Read-only telemetry · sampled {age(lastAt)}
            </span>
          </div>
        </Card>

        <Card className="pad-lg">
          <SectionHead title="Inference" hint={gpu?.name?.split("(")[0] ?? "GPU"} />
          <div className="rows">
            <Row
              k="Health"
              v={
                server?.health_ok ? (
                  <strong>{num(server.latency_ms, 0)} ms</strong>
                ) : (
                  <span className="dim">{server?.error ?? "no response"}</span>
                )
              }
            />
            <Row k="Uptime" v={<strong>{uptime(server?.uptime_s)}</strong>} />
            <Row
              k="Process"
              v={server?.pid ? `${server.process_name} · ${server.pid}` : "not listening"}
              mono
            />
            <Row k="Model id" v={server?.models?.[0] ?? model?.served_name ?? "—"} />
          </div>
        </Card>
      </div>

      <div className="kpis">
        <Kpi
          label="GPU utilisation"
          value={num(gpu?.util_pct, 0)}
          unit="%"
          meter={gpu?.util_pct ?? 0}
          foot={
            <>
              <span>sm clock</span>
              <span className="num">{mhz(gpu?.clock_mhz)}</span>
            </>
          }
        />
        <Kpi
          label="VRAM"
          value={gib(gpu?.mem_used_mib).replace(" GiB", "")}
          unit="GiB"
          meter={vramPct ?? 0}
          foot={
            <>
              <span>of {gib(gpu?.mem_total_mib)}</span>
              <span className="num">{pct(vramPct, 0)}</span>
            </>
          }
        />
        <Kpi
          label="Temperature"
          value={num(gpu?.temp_c, 0)}
          unit="°C"
          meter={Math.min(100, ((gpu?.temp_c ?? 0) / 100) * 100)}
          meterTone={(gpu?.temp_c ?? 0) > 80 ? "hot" : (gpu?.temp_c ?? 0) > 65 ? "warm" : ""}
          foot={
            <>
              <span>board</span>
              <span className="num">{watts(gpu?.power_w)}</span>
            </>
          }
        />
        <Kpi
          label="Power draw"
          value={num(gpu?.power_w, 1)}
          unit="W"
          meter={Math.min(100, ((gpu?.power_w ?? 0) / 140) * 100)}
          foot={
            <>
              <span>telemetry</span>
              <span className="num">{gpu?.available ? "live" : "unavailable"}</span>
            </>
          }
        />
      </div>

      <div className="split">
        <Card className="pad-lg">
          <SectionHead title="Signal" hint="GPU utilisation, last 60 samples" />
          <Sparkline values={history} max={100} id="home" unit="%" />
          <hr className="hairline" />
          <div className="rows">
            <Row
              k="Clock reasons"
              v={
                reasons.length ? (
                  <span className="dim">{reasons.join(" · ")}</span>
                ) : (
                  <strong>none active</strong>
                )
              }
            />
            <Row
              k="Collector"
              v={
                gpu?.available
                  ? `${num(gpu.collected_ms, 0)} ms nvidia-smi`
                  : (gpu?.error ?? "no sample")
              }
              mono
            />
          </div>
        </Card>

        <Card className="pad-lg">
          <SectionHead title="Model" hint="from V1 configuration" />
          <div className="rows">
            <Row k="Checkpoint" v={model?.checkpoint?.split("\\").pop() ?? "—"} mono title={model?.checkpoint} />
            <Row k="Precision" v={`${model?.dtype ?? "—"} · KV ${model?.kv_cache_dtype ?? "—"}`} />
            <Row k="Context" v={`${context(model?.max_model_len)} tokens`} />
            <Row
              k="Memory target"
              v={model?.gpu_memory_utilization ? pct(model.gpu_memory_utilization * 100, 0) : "—"}
            />
            <Row k="Prefix cache" v={model?.prefix_caching ? "enabled" : "disabled"} />
          </div>
        </Card>
      </div>

      <div className="split-even">
        <Card className="pad-lg">
          <SectionHead title="Blackout" hint="V1 engine, watched only" />
          <div className="rows">
            <Row
              k="Engine"
              v={
                blackout?.engine_active ? (
                  <strong>active · display held</strong>
                ) : (
                  <span className="dim">idle</span>
                )
              }
            />
            <Row k="Auto timeout" v={`${blackout?.idle_seconds ?? 180} s`} />
            <Row k="Follow server" v={blackout?.follow_server ? "on" : "off"} />
            <Row k="Cover" v={blackout?.cover ?? "internal"} />
          </div>
        </Card>

        <Card className="pad-lg">
          <SectionHead title="Recent events" hint="this session" />
          <EventList events={events} limit={5} />
        </Card>
      </div>

      {snap?.notes?.length ? (
        <Card quiet className="pad">
          <div className="note">{snap.notes.join(" · ")}</div>
        </Card>
      ) : null}
    </div>
  );
}
