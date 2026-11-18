/** Formatting helpers: every value rendered comes from the backend verbatim. */

export function num(value: number | null | undefined, digits = 0): string {
  if (value === null || value === undefined || Number.isNaN(value)) return "—";
  return value.toLocaleString("en-US", {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  });
}

export function pct(value: number | null | undefined, digits = 0): string {
  if (value === null || value === undefined) return "—";
  return `${num(value, digits)}%`;
}

/** MiB as GiB with one decimal, for VRAM. */
export function gib(mib: number | null | undefined): string {
  if (mib === null || mib === undefined) return "—";
  return `${num(mib / 1024, 1)} GiB`;
}

export function watts(value: number | null | undefined): string {
  if (value === null || value === undefined) return "—";
  return `${num(value, 1)} W`;
}

export function degrees(value: number | null | undefined): string {
  if (value === null || value === undefined) return "—";
  return `${num(value, 0)} °C`;
}

export function mhz(value: number | null | undefined): string {
  if (value === null || value === undefined) return "—";
  return `${num(value, 0)} MHz`;
}

export function context(tokens: number | null | undefined): string {
  if (!tokens) return "—";
  if (tokens >= 1000) return `${num(tokens / 1000, 0)}K`;
  return num(tokens, 0);
}

export function uptime(seconds: number | null | undefined): string {
  if (seconds === null || seconds === undefined) return "—";
  const s = Math.max(0, Math.floor(seconds));
  const d = Math.floor(s / 86400);
  const h = Math.floor((s % 86400) / 3600);
  const m = Math.floor((s % 3600) / 60);
  if (d > 0) return `${d}d ${h}h`;
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return `${m}m ${s % 60}s`;
  return `${s}s`;
}

export function clockTime(ms: number | null | undefined): string {
  if (!ms) return "—";
  return new Date(ms).toLocaleTimeString("en-GB", { hour12: false });
}

export function age(ms: number | null | undefined): string {
  if (!ms) return "never";
  const delta = Math.max(0, Date.now() - ms) / 1000;
  if (delta < 2) return "just now";
  if (delta < 60) return `${delta.toFixed(0)}s ago`;
  return `${(delta / 60).toFixed(1)}m ago`;
}

/** nvidia-smi reports the throttle field as a bitmask such as 0x0000000000000400. */
export function throttleReasons(raw: string | null | undefined): string[] {
  if (!raw) return [];
  const text = raw.trim();
  if (!text || text.toLowerCase().startsWith("0x0000000000000000")) return [];
  const mask = Number.parseInt(text.replace(/^0x/i, ""), 16);
  if (!Number.isFinite(mask) || mask === 0) return [];
  const bits: Array<[number, string]> = [
    [0x1, "gpu idle"],
    [0x2, "applications clocks"],
    [0x4, "sw power cap"],
    [0x8, "hw slowdown"],
    [0x10, "sync boost"],
    [0x20, "sw thermal slowdown"],
    [0x40, "hw thermal slowdown"],
    [0x80, "hw power brake"],
    [0x100, "display clock setting"],
    [0x400, "reliability"],
  ];
  const out = bits.filter(([bit]) => (mask & bit) !== 0).map(([, label]) => label);
  return out.length ? out : [text];
}
