/**
 * Presentation helpers for the Updates page.
 *
 * They live here, away from the JSX, because they are the part of that page
 * worth testing directly: how a status becomes a pill, how a timestamp becomes
 * a date, and how GitHub's markdown is reduced to something readable.
 */

export type Tone = "live" | "loading" | "offline" | "idle";

/** `2026-11-07T20:10:43Z` → `2026-11-07`; anything unparseable is shown as given. */
export function day(iso: string): string {
  if (!iso) return "—";
  const at = new Date(iso);
  if (Number.isNaN(at.getTime())) return iso;
  return at.toISOString().slice(0, 10);
}

/** An epoch in milliseconds as `date time`; zero means "nothing recorded". */
export function stamp(ms: number | null | undefined): string {
  if (!ms) return "—";
  const at = new Date(ms);
  if (Number.isNaN(at.getTime())) return "—";
  const time = at.toLocaleTimeString("en-GB", { hour12: false });
  return `${at.toISOString().slice(0, 10)} ${time}`;
}

/** The check itself: never / ok / offline / rate-limited / error. */
export function checkPill(status: string): { tone: Tone; text: string } {
  if (status === "ok") return { tone: "live", text: "reachable" };
  if (status === "offline") return { tone: "offline", text: "offline" };
  if (status === "rate-limited") return { tone: "loading", text: "rate limited" };
  if (status === "error") return { tone: "offline", text: "check failed" };
  return { tone: "idle", text: "not checked" };
}

/**
 * What the newest upstream release means *for this machine*. Newer activity is
 * deliberately not the same answer as "verified": only the tested baseline is.
 */
export function compatPill(status: string): { tone: Tone; text: string } {
  if (status === "current") return { tone: "live", text: "compatible" };
  if (status === "update-available") return { tone: "loading", text: "newer upstream" };
  if (status === "ahead") return { tone: "idle", text: "local ahead" };
  if (status === "unsupported-platform") return { tone: "offline", text: "not supported" };
  return { tone: "idle", text: "unknown" };
}

/**
 * Release notes arrive as GitHub-flavoured markdown. Rendering it as HTML would
 * mean shipping a renderer for text this page only ever reads, so the things
 * that actually hurt legibility are folded away: inline links keep their label,
 * emphasis markers go, and blank-line runs collapse.
 */
export function readableNotes(raw: string): string {
  return raw
    .replace(/```[a-zA-Z]*\n?/g, "")
    .replace(/\[([^\]]+)\]\([^)]+\)/g, "$1")
    .replace(/\*\*([^*]+)\*\*/g, "$1")
    .replace(/^\s*Full Changelog:.*$/gim, "")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}

/** `remaining of limit left` for the GitHub quota row. */
export function quota(remaining: number | null, limit: number | null): string {
  if (remaining === null || remaining === undefined) return "—";
  return `${remaining} of ${limit ?? "?"} left`;
}
