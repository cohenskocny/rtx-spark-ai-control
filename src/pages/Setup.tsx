import { useEffect, useRef, useState } from "react";
import type { Candidate, PathsConfig, Snapshot, Validation } from "../lib/types";
import { Card, Pill, Row, SectionHead } from "../components/primitives";
import { config, errorText } from "../lib/api";

/** Every text field of the configuration. `api_port` is edited as text and
 *  narrowed on save so a half-typed number never reaches the backend. */
type TextField = Exclude<keyof PathsConfig, "api_port">;

const LABELS: Record<string, { label: string; help: string }> = {
  vllm_root: {
    label: "Native-vLLM folder",
    help: "the environment that holds vllm/ and the launcher",
  },
  vllm_exe: {
    label: "vLLM executable",
    help: "vllm.exe inside that environment, used to start the server",
  },
  python_exe: {
    label: "Python executable",
    help: "the interpreter that owns the environment",
  },
  model_checkpoint: {
    label: "Model checkpoint",
    help: "the weights folder; read, never written",
  },
  launch_config: {
    label: "Launch configuration",
    help: "the JSON that describes the launch command",
  },
  chat_template: {
    label: "Chat template",
    help: "optional Jinja template for the served model",
  },
  blackout_engine: {
    label: "Blackout engine",
    help: "the sleep / OLED guard script",
  },
  blackout_python: {
    label: "Blackout interpreter",
    help: "the interpreter the blackout engine runs on",
  },
  logs_dir: {
    label: "Logs folder",
    help: "optional; where this application writes its logs",
  },
  legacy_config: {
    label: "Legacy V1 config",
    help: "optional; read once during import, never written",
  },
};

export function Setup({ snap }: { snap: Snapshot | null }) {
  const saved = snap?.config.paths ?? null;
  const fields = snap?.config.fields ?? [];

  const [draft, setDraft] = useState<PathsConfig | null>(saved);
  const [portText, setPortText] = useState(String(saved?.api_port ?? 8000));
  const [checks, setChecks] = useState<Record<string, Validation>>({});
  const [candidates, setCandidates] = useState<Candidate[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [note, setNote] = useState<{ ok: boolean; text: string } | null>(null);
  const [dirty, setDirty] = useState(false);
  const [importPath, setImportPath] = useState("");
  const timers = useRef<Record<string, number>>({});

  // Follow the saved configuration until the operator starts editing; after that
  // the draft is theirs and a poll must not overwrite it mid-keystroke.
  useEffect(() => {
    if (dirty || !saved) return;
    setDraft(saved);
    setPortText(String(saved.api_port));
  }, [saved, dirty]);

  async function validateField(field: string, value: string) {
    try {
      const verdict = await config.validate(field, value);
      setChecks((prev) => ({ ...prev, [field]: verdict }));
    } catch {
      /* the save path is where hard failures surface */
    }
  }

  function setField(field: TextField, value: string) {
    setDirty(true);
    setDraft((prev) => (prev ? { ...prev, [field]: value } : prev));
    window.clearTimeout(timers.current[field]);
    timers.current[field] = window.setTimeout(() => void validateField(field, value), 320);
  }

  async function detect() {
    if (busy) return;
    setBusy("detect");
    setNote(null);
    try {
      const found = await config.detect();
      setCandidates(found);
      setNote({
        ok: true,
        text: found.length
          ? `${found.length} location${found.length === 1 ? "" : "s"} found. Nothing was changed.`
          : "Nothing found in the usual places. Type a path or use Browse in the installer copy.",
      });
    } catch (failure) {
      setNote({ ok: false, text: errorText(failure) });
    } finally {
      setBusy(null);
    }
  }

  async function save() {
    if (!draft || busy) return;
    setBusy("save");
    setNote(null);
    const port = Number.parseInt(portText, 10);
    const next: PathsConfig = {
      ...draft,
      api_port: Number.isFinite(port) ? port : 8000,
    };
    try {
      const result = await config.save(next);
      setNote({ ok: result.ok, text: result.message });
      if (result.ok) {
        setDirty(false);
        setChecks({});
      }
      const found = result.ok ? await config.detect().catch(() => []) : [];
      if (found.length) setCandidates(found);
    } catch (failure) {
      setNote({ ok: false, text: errorText(failure) });
    } finally {
      setBusy(null);
    }
  }

  async function importLegacy() {
    if (busy) return;
    setBusy("import");
    setNote(null);
    try {
      const target = (importPath || draft?.legacy_config || "").trim();
      const result = await config.importLegacy(target);
      setNote({ ok: result.ok, text: result.message });
      if (result.ok) setDirty(false);
    } catch (failure) {
      setNote({ ok: false, text: errorText(failure) });
    } finally {
      setBusy(null);
    }
  }

  const missingLaunch = snap?.config.missing_launch ?? [];
  const missingBlackout = snap?.config.missing_blackout ?? [];
  const ready = snap?.config.launch_ready ?? false;
  const onBattery = snap?.power.online === false;

  return (
    <div className="stack">
      <Card tint className="pad-lg">
        <SectionHead
          title="Installation"
          hint="points at what you already have · nothing is installed"
        />
        <div className="hero-meta" style={{ marginBottom: 12 }}>
          <Pill tone={ready ? "live" : "loading"}>
            {ready ? "launch paths complete" : "setup needed"}
          </Pill>
          <Pill tone={snap?.config.blackout_ready ? "ghost" : "idle"} small>
            {snap?.config.blackout_ready ? "blackout ready" : "blackout incomplete"}
          </Pill>
          {onBattery ? (
            <Pill tone="loading" small>
              on battery · launch blocked
            </Pill>
          ) : null}
        </div>
        <div className="rows">
          <Row k="Endpoint" v={snap?.config.endpoint ?? "—"} mono />
          <Row k="Missing" v={missingLaunch.length ? missingLaunch.join(", ") : "nothing"} />
          <Row
            k="Blackout missing"
            v={missingBlackout.length ? missingBlackout.join(", ") : "nothing"}
          />
        </div>
        <div className="hero-actions" style={{ marginTop: 16 }}>
          <button type="button" className="btn primary" disabled={busy !== null} onClick={() => void detect()}>
            {busy === "detect" ? "Searching…" : "Find installations"}
          </button>
          <div className="spacer" />
          <button type="button" className="btn ghost sm" onClick={() => void save()} disabled={busy !== null || !draft}>
            {busy === "save" ? "Saving…" : "Save configuration"}
          </button>
        </div>
        {note ? (
          <div className="note" style={{ marginTop: 12 }}>
            {note.text}
          </div>
        ) : null}
      </Card>

      <Card className="pad-lg">
        <SectionHead
          title="Paths"
          hint={dirty ? "unsaved changes" : "matches the saved configuration"}
        />
        <div className="fieldgrid">
          {fields.map((field) => {
            const meta = LABELS[field.field] ?? { label: field.field, help: "" };
            const check = checks[field.field];
            const value = draft ? String((draft as unknown as Record<string, unknown>)[field.field] ?? "") : field.value;
            // An empty optional field is a valid answer, not a failure.
            const optionalEmpty = field.kind.startsWith("optional") && !value.trim();
            const ok = optionalEmpty ? true : check ? check.ok : field.ok;
            const message = optionalEmpty
              ? "not set (optional)"
              : check
                ? check.message
                : field.message;
            const suggested = (candidates ?? []).filter((item) => item.field === field.field);
            return (
              <div className="fieldrow" key={field.field}>
                <div className="head">
                  <span className="name">{meta.label}</span>
                  <span className="kind">{field.kind}</span>
                </div>
                <input
                  className="input mono"
                  type="text"
                  spellCheck={false}
                  autoComplete="off"
                  value={value}
                  title={value}
                  placeholder={field.kind.startsWith("optional") ? "optional" : "required"}
                  onChange={(event) => setField(field.field as TextField, event.target.value)}
                  onBlur={(event) => void validateField(field.field, event.target.value)}
                />
                <div className="status" data-ok={ok ? "yes" : "no"}>
                  <span className="led" data-on={ok} />
                  {message || (ok ? "found" : meta.help)}
                </div>
                {suggested.length ? (
                  <div className="candidates">
                    {suggested.slice(0, 3).map((item) => (
                      <button
                        key={`${item.field}-${item.path}`}
                        type="button"
                        className="candidate"
                        title={item.path}
                        onClick={() => {
                          setField(field.field as TextField, item.path);
                          void validateField(field.field, item.path);
                        }}
                      >
                        <span className="mono">{item.path}</span>
                        <span className="why">{item.note}</span>
                      </button>
                    ))}
                  </div>
                ) : null}
              </div>
            );
          })}
        </div>
      </Card>

      <div className="split-even">
        <Card className="pad-lg">
          <SectionHead title="Endpoint" hint="where the server is expected to listen" />
          <div className="fieldrow">
            <div className="head">
              <span className="name">Host</span>
              <span className="kind">address</span>
            </div>
            <input
              className="input mono"
              type="text"
              spellCheck={false}
              autoComplete="off"
              value={draft?.api_host ?? ""}
              onChange={(event) => setField("api_host", event.target.value)}
            />
          </div>
          <div className="fieldrow">
            <div className="head">
              <span className="name">Port</span>
              <span className="kind">1–65535</span>
            </div>
            <input
              className="input mono"
              type="text"
              inputMode="numeric"
              autoComplete="off"
              value={portText}
              onChange={(event) => {
                setDirty(true);
                setPortText(event.target.value.replace(/[^0-9]/g, "").slice(0, 5));
              }}
            />
            <div className="status" data-ok="yes">
              <span className="led" data-on />
              saved endpoint becomes {snap?.config.paths.api_host || "127.0.0.1"}:{portText || "8000"}
            </div>
          </div>
          <div className="fieldrow">
            <div className="head">
              <span className="name">Served model name</span>
              <span className="kind">label</span>
            </div>
            <input
              className="input mono"
              type="text"
              spellCheck={false}
              autoComplete="off"
              placeholder="defaults to the checkpoint folder name"
              value={draft?.served_model ?? ""}
              onChange={(event) => setField("served_model", event.target.value)}
            />
          </div>
          <div className="note" style={{ marginTop: 12 }}>
            The host and port decide which server this window talks to. It never binds a port of
            its own and never starts a second server on a different one.
          </div>
        </Card>

        <Card className="pad-lg">
          <SectionHead title="Import an existing V1 setup" hint="read once, never written" />
          <div className="fieldrow">
            <div className="head">
              <span className="name">V1 configuration</span>
              <span className="kind">json</span>
            </div>
            <input
              className="input mono"
              type="text"
              spellCheck={false}
              autoComplete="off"
              placeholder={draft?.legacy_config || "C:\\path\\to\\config.json"}
              value={importPath}
              onChange={(event) => setImportPath(event.target.value)}
            />
          </div>
          <div className="hero-actions" style={{ marginTop: 4 }}>
            <button
              type="button"
              className="btn ghost sm"
              disabled={busy !== null}
              onClick={() => void importLegacy()}
            >
              {busy === "import" ? "Importing…" : "Import paths"}
            </button>
          </div>
          <div className="note" style={{ marginTop: 12 }}>
            Only the documented keys are read: model folder, launcher, blackout script, endpoint.
            The file itself is never modified, and any personal value it contains is stored in your
            own configuration rather than in the application.
          </div>
        </Card>
      </div>

      <Card quiet className="pad">
        <div className="note">
          Selecting a path only records it. Nothing under the native-vLLM installation, the model
          checkpoint or the blackout engine is created, moved or rewritten.
        </div>
      </Card>
    </div>
  );
}
