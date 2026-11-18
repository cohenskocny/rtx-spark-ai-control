# RECOVERY-STATUS

Written so a future session can resume this project from disk, without the conversation.
Updated at the end of Phase 2.

**Date:** 2026-11-18 06:45 UTC (local 2026-11-18 00:45)

---

## 1. Bottom line

| Item | State |
| --- | --- |
| Tauri 2 + Rust + React, Windows ARM64 build | **PASS** — `npm run app:build` exit 0, 25.9–26.7 s incremental, no warnings |
| GUI launches from `run.ps1` | **PASS** — window renders live data from the production server |
| vLLM start / stop / restart / adopt | **PASS** — scripted against a mock on port 8011 (`tools/qa-lifecycle.py`, all PASS) |
| Failed launch handling | **PASS** — stage `exited with code 1`, traceback in the log tail, nothing left half-owned |
| Blackout Now / Stop | **PASS** — engine starts and stops, `state.json` correct, no orphan process |
| Automatic blackout after 180 s | **NOT TESTED** — the Windows session was locked for the whole run, and the validated engine stands down while locked |
| Input restores the desktop | **NOT TESTED** — same reason |
| Modern Standby absence during blackout | **NOT TESTED** — same reason |
| Follow Server automation | **PASS** — arm after hysteresis, stand down after grace, re-arm on input (mock server) |
| Settings persistence | **PASS** — `%APPDATA%` file round-trips across launches; UI prefs in localStorage |
| Icon (four-point glass spark) | **DONE** — ICO with 16/24/32/48/64/128/256, PNG set, sidebar mark, candidates kept in `assets/icon-candidates/` |
| Idle cost | **PASS** — 5.88 / 5.37 % of one core vs the 7.84 % Phase 1 baseline; no regression |
| Production server untouched | **PASS** — pid 10644 still serving `Qwen/Qwen3.8-Flash-Next`; V1 `config.json` and `runtime-state.json` MD5s unchanged |
| Git | **PASS** — baseline `b73bb82` plus Phase 2 commits (see §6) |

Read `TEST-RESULTS.md` for the item-by-item evidence, including the four unexecuted tests and
the exact commands to run them.

---

## 2. What Phase 2 added

**Backend (Rust)** — `vllm.rs` (launch/stop/restart/adopt, health, log parsing, readiness),
`blackout.rs` (V1 engine supervision, STOP sentinel, state file), `supervisor.rs` (1 Hz loop:
probes, idle anchoring, Follow Server hysteresis/grace, arm and stand-down), `events.rs`
(400-entry ring with repeat collapsing), `settings.rs` (`Settings` + `OwnedServer`, atomic
writes under `%APPDATA%`), `core.rs` (shared state, `ActionResult`, snapshots), plus
`winproc.rs` extensions: Toolhelp snapshots, ancestor walk, command lines, tree terminate,
`GetLastInputInfo`.

**Frontend (React)** — action runner with one in-flight call and toast feedback, cursor-based
event merge, full Server/Blackout/Settings/Logs pages, Home hero controls, `BrandMark` icon,
contrast bump for secondary labels, `span()` duration formatting.

**Not changed on purpose:** `blackout.py` (the validated engine), V1's `config.json`, the
optimized YAML, the chat template, the checkpoint, the NVIDIA driver and the ASUS profile.

---

## 3. Defects found and fixed during Phase 2

### 3.1 Adoption could climb into an unrelated Python parent

*Symptom.* `Take ownership` walked from the port listener up to any ancestor whose executable
name was `python.exe` / `pythonw.exe` / `*vllm*`, and Stop then terminated that tree. In a
test where the server had been started by a Python harness, adopting killed the harness — the
one thing V1's ownership model forbids.

*Fix.* `winproc::process_cmdline()` reads another process's command line through
`NtQueryInformationProcess` + `ReadProcessMemory` (PEB → `ProcessParameters` → `CommandLine`,
`raw-dylib` links, no new crates). `adopt()` now widens only while the ancestor's command line
names `vllm.exe` or `vllm.entrypoints`, and stops at the first ancestor that does not.

*Verified.* The harness survived; the adopted listener was the only process stopped; the
production listener's command line (`… vllm.exe serve …`) still resolves to the outermost
`vllm.exe`, which is the intended root for a stop.

### 3.2 A fabricated "Weight loading 100 %"

*Symptom.* An already-running external server showed a full progress bar and
"Weight loading 100 %" because a healthy probe saturated the progress field.

*Fix.* `apply_health` only saturates an *observed* progress value
(`progress_pct = progress_pct.map(|_| 100.0)`); the bar is active only while starting, loading
or stopping; the caption reads `Ready — /v1/models answered in N ms`.

### 3.3 Follow Server logged an event while switched off

*Symptom.* "Inference server healthy for 60 s; Follow Server may arm blackout" appeared in the
Logs page with Follow Server off (the health state is tracked regardless, for the phase text).

*Fix.* The hysteresis is still tracked, but the event is only recorded when Follow Server is on.

### 3.4 Copy defects

"tolerated for 0 minutes" for a 20 s grace window, and a display-request row that said
"released while idle" even when the real reason was a locked session. Both corrected
(`span()` helper; explicit locked wording).

---

## 4. How to resume

```powershell
cd C:\Users\cohen\AI\RTX-Spark-AI-Control-V2
.\run.ps1              # existing binary; add -Build to rebuild first
curl.exe http://127.0.0.1:8000/v1/models   # server health
git log --oneline      # baseline + Phase 2 commits
```

Dev tooling is opt-in only:

```powershell
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=9222'
python tools\cdp.py eval "document.body.innerText"   # venv python: native-vLLM\.venv
python tools\shots.py screenshots
python tools\measure.py --label x --seconds 60
```

Never `cargo build` alone: the binary links but carries the dev URL and the window comes up
black. Diagnosis for a blank window is always "check the CDP target URL first".

---

## 5. Open items (next session)

1. **The four unexecuted display tests.** They need an unlocked desktop:
   automatic blackout at 180 s, input restore, `powercfg /requests` during coverage, and a
   visible-window idle measurement. Commands are in `TEST-RESULTS.md` §6. Nothing in the code
   needs changing first.
2. **Responsiveness under heavy inference.** Deliberately deferred: generating load means
   queueing real work on the production server, which needs the owner's approval
   (`tools/infer-probe.py`).
3. **Adoption of a bare `python -m vllm…` server** stops the listener only, not an unknown
   parent. Documented in `README.md` §9; acceptable as designed, but worth revisiting if the
   operator switches to module-style launches.
4. **Optional Phase 1 leftovers, still untouched:** `Snapshot.collect_duration_ms` is hard-coded
   to `0` (typed in `types.ts`, never rendered), and the sparkline/event history is in-memory
   by design.

---

## 6. Git

Repository was initialised during this work; identity is local only
(`Cohen <cohen@localhost>`).

| Commit | Content |
| --- | --- |
| `b73bb82` | Baseline: validated read-only V2 prototype (Tauri 2 + Rust + React, ARM64) |
| Phase 2 code | vLLM lifecycle, blackout integration, Follow Server automation, event ring, settings persistence, branded icon, page rebuilds |
| Phase 2 docs | `TEST-RESULTS.md`, this file, `README.md`, `FN-Optimization/PROGRESS.md` |

---

## 7. `stored JSON document is at capacity: runtime-state.json`

That error, which stalled the earlier conversation, is **not** a defect in V1, V2 or the model
server. It came from the agent harness's own session store
(`HarnessStoreCapacityError`, `_MAX_DOCUMENT_BYTES` = 64 MiB) refusing a write to the session
document `~/.vibe/logs/session/unified/<session-id>/generations/<gen>/runtime-state.json`.
The project file with the same name, `RTX-Spark-AI-Control/runtime-state.json`, is 2 bytes
containing `{}` (MD5 `99914b932bd37a50b983c5e7c90ae93b`) and has nothing to do with it.

Nothing was deleted or rewritten to "fix" it; the response is keeping state in project files —
this one, `TEST-RESULTS.md`, `README.md` and `FN-Optimization/PROGRESS.md`.
