import type { ReactNode } from "react";
import type { LogEvent } from "../lib/types";
import { clockTime, num } from "../lib/format";

export function Card({
  children,
  className = "",
  tint = false,
  quiet = false,
}: {
  children: ReactNode;
  className?: string;
  tint?: boolean;
  quiet?: boolean;
}) {
  return (
    <section
      className={`glass ${tint ? "glass-tint" : ""} ${quiet ? "glass-quiet" : ""} ${className}`}
    >
      {children}
    </section>
  );
}

export function Pill({
  tone,
  children,
  pulse = false,
  small = false,
}: {
  tone: "live" | "loading" | "offline" | "idle" | "ghost";
  children: ReactNode;
  pulse?: boolean;
  small?: boolean;
}) {
  return (
    <span className={`pill ${tone} ${small ? "sm" : ""} ${pulse ? "pulse" : ""}`}>
      <span className="dot" />
      {children}
    </span>
  );
}

export function Kpi({
  label,
  value,
  unit,
  foot,
  meter,
  meterTone = "",
}: {
  label: string;
  value: string;
  unit?: string;
  foot?: ReactNode;
  meter?: number;
  meterTone?: string;
}) {
  return (
    <div className="glass kpi">
      <div className="label">{label}</div>
      <div className="value tabular">
        {value}
        {unit ? <span className="unit">{unit}</span> : null}
      </div>
      {meter !== undefined ? (
        <div className={`meter ${meterTone}`}>
          <span style={{ width: `${Math.max(0, Math.min(100, meter))}%` }} />
        </div>
      ) : null}
      {foot ? <div className="foot">{foot}</div> : null}
    </div>
  );
}

export function Row({
  k,
  v,
  mono = false,
  title,
}: {
  k: string;
  v: ReactNode;
  mono?: boolean;
  title?: string;
}) {
  return (
    <div className="row">
      <div className="k">{k}</div>
      <div className={`v ${mono ? "mono" : ""}`} title={title}>
        {v}
      </div>
    </div>
  );
}

export function SectionHead({ title, hint }: { title: string; hint?: ReactNode }) {
  return (
    <div className="card-head">
      <h3 className="section-title">{title}</h3>
      {hint ? <span className="hint">{hint}</span> : null}
    </div>
  );
}

export function Sparkline({
  values,
  max = 100,
  id = "spark",
  unit = "",
}: {
  values: number[];
  max?: number;
  id?: string;
  unit?: string;
}) {
  const width = 320;
  const height = 64;
  const clamped = values.map((v) => Math.max(0, Math.min(max, v)));
  const step = values.length > 1 ? width / (values.length - 1) : width;
  const points = clamped.map((v, i) => {
    const x = i * step;
    const y = height - (v / max) * (height - 8) - 4;
    return `${x.toFixed(1)},${y.toFixed(1)}`;
  });
  const line = points.join(" ");
  const area = `0,${height} ${line} ${width},${height}`;
  const latest = clamped.length ? clamped[clamped.length - 1] : 0;
  const peak = clamped.length ? Math.max(...clamped) : 0;
  const mean = clamped.length ? clamped.reduce((a, b) => a + b, 0) / clamped.length : 0;

  return (
    <>
      <div className="spark-frame">
        <svg
          className="spark"
          viewBox={`0 0 ${width} ${height}`}
          preserveAspectRatio="none"
          aria-hidden="true"
        >
          <defs>
            <linearGradient id={`${id}-fill`} x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor="rgba(120, 168, 255, 0.34)" />
              <stop offset="100%" stopColor="rgba(120, 168, 255, 0)" />
            </linearGradient>
          </defs>
          {[0.25, 0.5, 0.75].map((f) => (
            <line
              key={f}
              className="spark-guide"
              x1="0"
              x2={width}
              y1={(height * f).toFixed(1)}
              y2={(height * f).toFixed(1)}
            />
          ))}
          {values.length > 1 ? (
            <>
              <polygon points={area} fill={`url(#${id}-fill)`} />
              <polyline points={line} />
            </>
          ) : null}
        </svg>
        <div className="spark-axis" aria-hidden="true">
          <span>{max}</span>
          <span>0</span>
        </div>
      </div>
      <div className="spark-stats">
        <div>
          <span className="k">now</span>
          <b>{num(latest, 0)}{unit}</b>
        </div>
        <div>
          <span className="k">peak</span>
          <b>{num(peak, 0)}{unit}</b>
        </div>
        <div>
          <span className="k">{values.length > 1 ? `mean of ${values.length}` : "mean"}</span>
          <b>{num(mean, 0)}{unit}</b>
        </div>
      </div>
    </>
  );
}

export function EventList({ events, limit = 5 }: { events: LogEvent[]; limit?: number }) {
  const slice = events.slice(0, limit);
  if (!slice.length) {
    return <div className="empty">No events yet</div>;
  }
  return (
    <div className="events">
      {slice.map((event) => (
        <div className="event" data-kind={event.kind} key={event.id}>
          <span className="t">{clockTime(event.at)}</span>
          <span>
            {event.text}
            {event.repeat > 1 ? ` ×${event.repeat}` : ""}
          </span>
        </div>
      ))}
    </div>
  );
}

const glyphs: Record<string, ReactNode> = {
  home: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.4">
      <path d="M4 10.6 12 4l8 6.6V20a1 1 0 0 1-1 1h-4.6v-6h-4.8v6H5a1 1 0 0 1-1-1v-9.4Z" />
    </svg>
  ),
  server: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.4">
      <rect x="3.5" y="4" width="17" height="6" rx="1.6" />
      <rect x="3.5" y="14" width="17" height="6" rx="1.6" />
      <circle cx="7" cy="7" r="0.9" fill="currentColor" stroke="none" />
      <circle cx="7" cy="17" r="0.9" fill="currentColor" stroke="none" />
    </svg>
  ),
  blackout: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.4">
      <rect x="3" y="4.5" width="18" height="12" rx="1.8" />
      <path d="M8 20h8" />
      <path d="M9 8.6h6M9 11.4h4" opacity="0.55" />
    </svg>
  ),
  settings: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.4">
      <path d="M4 7h9M17 7h3M4 12h3M11 12h9M4 17h11M19 17h1" />
      <circle cx="15" cy="7" r="1.8" />
      <circle cx="9" cy="12" r="1.8" />
      <circle cx="17" cy="17" r="1.8" />
    </svg>
  ),
  logs: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.4">
      <path d="M6 3.5h9L19 8v12.5H6z" />
      <path d="M14.6 3.6V8H19" />
      <path d="M9 12h7M9 15h5" opacity="0.7" />
    </svg>
  ),
  updates: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.4">
      <path d="M20.4 13.2a8.4 8.4 0 1 1-1.2-7" />
      <path d="M20.6 3.6v4.6h-4.6" />
      <circle cx="12" cy="13.2" r="2.2" />
    </svg>
  ),
  setup: (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.4">
      <path d="M3.6 7.4a1.6 1.6 0 0 1 1.6-1.6h3.3l1.8 2.1h8.5a1.6 1.6 0 0 1 1.6 1.6v8.1a1.6 1.6 0 0 1-1.6 1.6H5.2a1.6 1.6 0 0 1-1.6-1.6z" />
      <path d="M8.8 13.5 11 15.7l4.2-4.3" />
    </svg>
  ),
};

export function Glyph({ name }: { name: string }) {
  return (
    <span className="glyph" aria-hidden="true">
      {glyphs[name] ?? glyphs.home}
    </span>
  );
}
