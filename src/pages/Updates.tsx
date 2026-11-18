import { useEffect, useRef, useState } from "react";
import type { Snapshot, UpstreamView } from "../lib/types";
import { Card, Pill, Row, SectionHead } from "../components/primitives";
import { age, clockTime } from "../lib/format";
import { errorText, upstream } from "../lib/api";

/** `2026-11-07T20:10:43Z` → `2026-11-07`; anything unparseable is shown as given. */
function day(iso: string): string {
  if (!iso) return "—";
  const at = new Date(iso);
  if (Number.isNaN(at.getTime())) return iso;
  return at.toISOString().slice(0, 10);
}

function stamp(ms: number | null): string {
  if (!ms) return "—";
  return `${day(new Date(ms).toISOString())} ${clockTime(ms)}`;
}

/** The check itself: never / ok / offline / rate-limited / error. */
function checkPill(status: string): {
  tone: "live" | "loading" | "offline" | "idle";
  text: string;
} {
  if (status === "ok") return { tone: "live", text: "reachable" };
  if (status === "offline") return { tone: "offline", text: "offline" };
  if (status === "rate-limited") return { tone: "loading", text: "rate limited" };
  if (status === "error") return { tone: "offline", text: "check failed" };
  return { tone: "idle", text: "not checked" };
}

/** What the newest upstream release means *for this machine*. */
function compatPill(status: string): {
  tone: "live" | "loading" | "offline" | "idle";
  text: string;
} {
  if (status === "current") return { tone: "live", text: "compatible" };
  if (status === "update-available") return { tone: "loading", text: "newer upstream" };
  if (status === "ahead") return { tone: "idle", text: "local ahead" };
  if (status === "unsupported-platform") return { tone: "offline", text: "not supported" };
  return { tone: "idle", text: "unknown" };
}

/**
 * Release notes arrive as GitHub-flavoured markdown. Rendering it as HTML would
 * mean shipping a renderer for text this page only ever reads, so the two things
 * that actually hurt legibility are folded away: inline links keep their label,
 * and code fences keep their content.
 */
function readableNotes(raw: string): string {
  return raw
    .replace(/```[a-zA-Z]*\n?/g, "")
    .replace(/\[([^\]]+)\]\([^)]+\)/g, "$1")
    .replace(/\*\*([^*]+)\*\*/g, "$1")
    .replace(/^\s*Full Changelog:.*$/gim, "")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}

export function Updates({ snap, refresh }: { snap: Snapshot | null; refresh: () => void }) {
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const asked = useRef(false);

  const view: UpstreamView | null = snap?.upstream ?? null;
  const installed = snap?.installed ?? null;
  const compat = snap?.compat ?? null;

  // The backend keeps a once-a-day cadence of its own; this only makes the very
  // first visit produce a result instead of an empty panel.
  useEffect(() => {
    if (!view || asked.current) return;
    if (view.status !== "never") return;
    asked.current = true;
    void check(false);
  }, [view]);

  async function check(force: boolean) {
    if (busy) return;
    setBusy("check");
    setError(null);
    try {
      await upstream.check(force);
      refresh();
    } catch (failure) {
      setError(errorText(failure));
    } finally {
      setBusy(null);
    }
  }

  async function open(url: string) {
    setError(null);
    try {
      const verdict = await upstream.open(url);
      if (!verdict.ok) setError(verdict.message);
    } catch (failure) {
      setError(errorText(failure));
    }
  }

  const checked = checkPill(view?.status ?? "never");
  const verdict = compatPill(compat?.status ?? "unknown");
  const latest = view?.latest ?? null;

  return (
    <div className="stack">
      <Card tint className="pad-lg">
        <SectionHead
          title="Inference engine"
          hint="installed build · read-only"
        />
        <div className="hero-meta" style={{ marginBottom: 6 }}>
          <Pill tone={verdict.tone}>{verdict.text}</Pill>
          <Pill tone={checked.tone} small>
            {checked.text}
          </Pill>
          {view?.from_cache ? (
            <Pill tone="ghost" small>
              cached
            </Pill>
          ) : null}
        </div>
        <div className="rows">
          <Row
            k="Installed version"
            v={installed?.version || "not detected"}
            mono
            title={installed?.version}
          />
          <Row
            k="Project build"
            v={
              installed?.project_version
                ? `${installed.project_version} of native-vLLM`
                : "—"
            }
          />
          <Row
            k="Source revision"
            v={
              installed?.revision_short ? (
                installed.revision
              ) : (
                <span className="dim">not recorded by the build</span>
              )
            }
            mono
            title={installed?.revision || undefined}
          />
          <Row k="Read from" v={installed?.version_source ?? "—"} />
          <Row k="Wheel tag" v={installed?.wheel_tag || "—"} mono />
          <Row k="Python" v={installed?.python || "—"} mono title={installed?.python} />
          <Row k="Engine root" v={snap?.config.paths.vllm_root || "—"} mono title={snap?.config.paths.vllm_root} />
        </div>
        {installed?.notes?.length ? (
          <div className="note" style={{ marginTop: 12 }}>
            {installed.notes.join(" · ")}
          </div>
        ) : null}
        <div className="note" style={{ marginTop: 12 }}>
          native-vLLM is an external dependency. This control center reads its version and
          never installs, upgrades, rebuilds or replaces it.
        </div>
      </Card>

      <div className="split-even">
        <Card className="pad-lg">
          <SectionHead title="Compatibility" hint="new activity is not the same as verified" />
          <div className="rows">
            <Row k="Verdict" v={verdict.text} />
            <Row
              k="Verified"
              v={compat?.verified ? "tested configuration" : <span className="dim">unverified</span>}
            />
            <Row
              k="Tested with"
              v={
                compat
                  ? `${compat.tested_project} · vLLM ${compat.tested_upstream} · ${compat.tested_platform}`
                  : "—"
              }
            />
            <Row
              k="Upstream activity"
              v={
                view?.commits_ahead ? (
                  "newer commits on main, no release yet"
                ) : (
                  <span className="dim">nothing newer than the installed build</span>
                )
              }
            />
            <Row
              k="Installed revision"
              v={view?.installed_revision ? view.installed_revision.slice(0, 10) : "—"}
              mono
              title={view?.installed_revision}
            />
          </div>
          <div className="note" style={{ marginTop: 12 }}>
            {compat?.message ?? "No compatibility report yet."}
          </div>
        </Card>

        <Card className="pad-lg">
          <SectionHead title="Update check" hint="once a day, read-only" />
          <div className="rows">
            <Row k="State" v={view?.message || "Not checked yet"} />
            <Row k="Last attempt" v={view ? `${age(view.checked_ms)} · ${stamp(view.checked_ms)}` : "—"} />
            <Row
              k="Last success"
              v={view?.last_success_ms ? `${age(view.last_success_ms)} · ${stamp(view.last_success_ms)}` : "never"}
            />
            <Row k="Next automatic" v={view?.next_auto_ms ? stamp(view.next_auto_ms) : "on next launch"} />
            <Row
              k="GitHub quota"
              v={
                view?.rate.remaining === null || view?.rate.remaining === undefined
                  ? "—"
                  : `${view.rate.remaining} of ${view.rate.limit ?? "?"} left`
              }
            />
          </div>
          <div className="note" style={{ marginTop: 12 }}>
            One anonymous request per day against the public GitHub API, cached to disk. Offline
            and rate-limited checks keep the previous answer instead of clearing it.
          </div>
          <div className="hero-actions" style={{ marginTop: 16 }}>
            <button
              type="button"
              className="btn primary"
              disabled={busy !== null}
              onClick={() => void check(true)}
            >
              {busy === "check" ? "Checking…" : "Check for Updates"}
            </button>
            <div className="spacer" />
            <button
              type="button"
              className="btn ghost sm"
              onClick={() => void open(view?.repo_url ?? "")}
            >
              Project
            </button>
            <button
              type="button"
              className="btn ghost sm"
              onClick={() => void open(view?.releases_url ?? "")}
            >
              Releases
            </button>
          </div>
        </Card>
      </div>

      {error ? (
        <Card quiet className="pad">
          <div className="note">
            <span className="dim">Update check error:</span> {error}
          </div>
        </Card>
      ) : null}

      <div className="split-even">
        <Card className="pad-lg">
          <SectionHead
            title="Latest upstream release"
            hint={latest ? `${day(latest.published_at)} · ${latest.tag}` : "none"}
          />
          {latest ? (
            <>
              <div className="hero-meta" style={{ marginBottom: 10 }}>
                <Pill tone={latest.prerelease ? "loading" : "ghost"} small>
                  {latest.prerelease ? "pre-release" : "release"}
                </Pill>
                <span className="note">{latest.name || latest.tag}</span>
              </div>
              <div className="rows">
                <Row k="Tag" v={latest.tag} mono />
                <Row k="Published" v={day(latest.published_at)} />
                <Row k="Assets" v={String(latest.assets)} />
              </div>
              <div className="release-notes" role="region" aria-label="Release notes">
                {readableNotes(latest.notes) || "This release has no notes."}
              </div>
              <div className="hero-actions" style={{ marginTop: 12 }}>
                <button
                  type="button"
                  className="btn ghost sm"
                  onClick={() => void open(latest.html_url)}
                >
                  Open on GitHub
                </button>
              </div>
            </>
          ) : (
            <div className="empty">No release information yet</div>
          )}
        </Card>

        <Card className="pad-lg">
          <SectionHead
            title="Recent upstream changes"
            hint={view?.commits.length ? `${view.commits.length} commits` : "none"}
          />
          {view?.commits.length ? (
            <div className="news">
              {view.commits.map((commit) => (
                <button
                  key={commit.sha}
                  type="button"
                  className="news-item"
                  title={`${commit.sha} · ${commit.date}`}
                  onClick={() => void open(commit.url)}
                >
                  <span className="sha mono">{commit.short}</span>
                  <span className="title">{commit.title}</span>
                  <span className="when">{day(commit.date)}</span>
                </button>
              ))}
            </div>
          ) : (
            <div className="empty">No commits recorded yet</div>
          )}
        </Card>
      </div>

      <Card quiet className="pad">
        <div className="note">
          Upstream notifications describe <b>native-vLLM</b>, not this application. Its own
          releases are a separate stream; nothing on this page downloads or installs either.
        </div>
      </Card>
    </div>
  );
}
