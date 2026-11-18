# RECOVERY-STATUS

Written at the start of a new session after the previous one stalled during context
compaction. Purpose: state what is finished, what was broken, what changed, and how to
verify it without the old conversation.

**Date:** 2026-11-18 05:10-05:35 UTC (local 23:10-23:35, 2026-11-17)

---

## 1. Bottom line

| Item | State |
| --- | --- |
| Tauri 2 + Rust ARM64 app builds | **PASS** — `npm run app:build` exit 0, 20.9 s incremental |
| GUI launches from `run.ps1` | **PASS** — 391 ms to first paint, window renders live data |
| Frontend served from embedded `dist/` | **PASS** — page URL `http://tauri.localhost/`, not the dev URL |
| Live telemetry correctness | **PASS** — every field cross-checked against `nvidia-smi`, `/v1/models`, `config.json`, `state.json` |
| Idle cost | **PASS** — 7.65-10.43 % of one core across three runs; V1 was 63.8 % |
| Read-only guarantee | **PASS** — V1 config, V1 runtime state, blackout state unchanged (checksums below) |
| Duplicate launch | **was BROKEN — fixed this session** (see §4) |
| `runtime-state.json` capacity error | **diagnosed, not an app defect** (see §3) |
| Git | **not a repository** — no `.git`, so no history to recover; `.gitignore` exists |

The V2 read-only proof of concept is functionally complete. Nothing was re-installed, no
model or inference setting was touched.

---

## 2. What was already complete before this session

Recovered from `README.md`, `results/`, `screenshots/` and the mission log
`FN-Optimization/PROGRESS.md` (its V2 section was fully written, lines 932-1030):

* Full source: `src-tauri/src/{main,telemetry,winproc}.rs`, 13 frontend files, 3 stylesheets.
* Measurement harness `tools/` (`measure.py`, `contention.py`, `infer-probe.py`, `cdp.py`,
  `capture.py`, `winsize.py`) and their raw JSON in `results/`.
* 7 CDP screenshots at 150 % DPI in `screenshots/`, and a copy of everything in
  `FN-Optimization/results/v2-prototype/`.
* Progress and design documentation: project `README.md` (architecture, measurements,
  limitations, V1 migration plan, rollback) plus the V2 section of `FN-Optimization/PROGRESS.md`.

The previous session had reached its final step — it launched the release binary at
22:50:41 local and the session died immediately afterwards. No work item was left
half-written on disk; only verification and this report were missing.

---

## 3. The `stored JSON document is at capacity: runtime-state.json` error

**Not a defect in V1, V2 or the model server.** It is the harness's own session store
refusing a write, and it is what stalled the previous conversation.

* Message source: `%LOCALAPPDATA%\Programs\le-chat-desktop\resources\bin\vibe-app-server\
  _internal\mistralai_vibe_local_harness\vibe\_storage.py:5804`, raised as
  `HarnessStoreCapacityError` when a stored document exceeds `_MAX_DOCUMENT_BYTES`
  (line 162, 64 MiB).
* The `runtime-state.json` in the message is the **session document**
  `~/.vibe/logs/session/unified/<session-id>/generations/<gen>/runtime-state.json`, not any
  file belonging to this project. The session active during the stall
  (`34829a5d-e794-fe53-9888-a03aff8777a6`) holds generations of 21.4 MB and 10.9 MB — the
  conversation itself, grown past what the compaction rewrite could fit.
* The project file with the same name, `RTX-Spark-AI-Control/runtime-state.json`, is and was
  **2 bytes containing `{}`** (MD5 `99914b932bd37a50b983c5e7c90ae93b`), written by V1's
  `rtxcontrol/config.py`. It has nothing to do with the error.

Nothing was deleted, moved or rewritten to "fix" it — the correct response is a fresh
session with the state recorded on disk, which is what this file is for. Future sessions
should keep progress in project files (this one, `PROGRESS.md`) rather than relying on
conversation history.

---

## 4. Defect found and fixed this session: duplicate launch crashed

**Symptom.** Starting the app a second time exited with `0xC0000409`
(`STATUS_STACK_BUFFER_OVERRUN`, the code `panic = "abort"` produces) and wrote two
`BEX64` Windows Error Reporting entries at 23:18:35 and 23:18:43 local. Because the binary
is `windows_subsystem = "windows"`, the failure was completely silent: no window, no
console, no message. A second instance cannot share the WebView2 browser profile, so it
aborts inside the loader.

**Fix.** A per-session named mutex (`Local\RTXSparkAIControlV2`) is claimed at the top of
`main()`. On contention the second process restores the existing window and exits 0
instead of crashing — the same guarantee V1 gets from its single-instance mutex.

* `src-tauri/src/main.rs`: added `claim_single_instance()` and `focus_existing()`, called
  before any Tauri state is built. The mutex handle is deliberately never closed; Windows
  releases it when the owning process exits.
* `src-tauri/Cargo.toml`: added `Win32_Security` (needed to expose `CreateMutexW`) and
  `Win32_UI_WindowsAndMessaging` (`FindWindowW`, `ShowWindow`, `SetForegroundWindow`) to
  `windows-sys`. No new crates, no network access needed.

**Evidence after the fix.**

| Check | Result |
| --- | --- |
| Second launch while first is running | exits code **0**, first instance keeps running |
| Process count after two launches | **1** |
| Minimised first window after second launch | **restored** (`IsIconic` false) |
| New WER crash entries after the fix | **0** (both entries predate it, 23:18) |
| Rebuild | exit 0, 20.9 s, binary 4,674,048 B (was 4,672,000 B) |

**Known limit:** raising to the foreground is best-effort. Windows' foreground lock refuses
`SetForegroundWindow` when the caller was not itself started by the foreground process; the
restore from minimised works regardless. Taskbar click remains the reliable way to raise it.

---

## 5. Verification performed this session

Rebuilt from source rather than trusting the existing binary, then checked against the live
system. Frontend hashes are unchanged from the pre-recovery build (`index-Bvt_8JHK.js`
173.49 kB, `index-CfvcuKrH.css` 16.89 kB), so the fix touched no UI code.

| Check | Method | Result |
| --- | --- | --- |
| Build reproduces | `npm run app:build` | exit 0; `tsc --noEmit` clean |
| Embedded assets, not dev URL | CDP `/json` target | url `http://tauri.localhost/` |
| Home page | CDP DOM read | model ready, health 4 ms, uptime 8h 10m, pid 10644 `python.exe`, VRAM 93.7 of 96.0 GiB, 754 MHz, 1.9 W, reason `reliability` |
| Server page | CDP DOM read | `Qwen/Qwen3.8-Flash-Next`, listening yes (loopback), GET /v1/models 200 in 1 ms |
| Blackout page | CDP DOM read | phase `disabled`, display request not held, 180 s, follow-server off, cover internal, state written `2026-11-18T04:00:01Z` — matches `state.json` |
| Settings page | CDP DOM read | 2 s interval, last poll 75 ms, sampling active, startup 474 ms |
| Throttle decode | `format.ts` table vs `nvidia-smi` | raw `0x400` rendered `reliability` |
| GPU ground truth | `nvidia-smi` | 1 % util, 95 948 / 98 304 MiB, 50.3 °C, 2.18 W, 747 MHz, `0x400` |
| vLLM health | `GET /v1/models` | 200, `Qwen/Qwen3.8-Flash-Next`, max_model_len 409600 |
| Idle CPU, 1500x950, Home visible, 60 s | `tools/measure.py` x3 | mean 10.43 / 9.23 / **7.65 %** — bracketing the documented 7.84 %; no regression detectable, spread is machine noise (Brave and t3 also active) |
| RSS / process tree | `tools/measure.py` | 447.8 MiB, 7 processes |

**Untouched, verified by checksum and mtime:**

| File | Value |
| --- | --- |
| `RTX-Spark-AI-Control/config.json` | MD5 `7e2c65400c2011a80a373b4320f24cab` |
| `RTX-Spark-AI-Control/runtime-state.json` | MD5 `99914b932bd37a50b983c5e7c90ae93b`, 2 bytes, mtime 21:56 |
| `RTX-Spark-AI-Blackout/state.json` | MD5 `95a1141fdfe2ab20597bced376890fa9`, mtime 22:00 |
| vLLM | still serving on 127.0.0.1:8000, listener pid 10644 |
| Startup entries | no `HKCU\...\Run` value for V2 |

---

## 6. Open items

**Deliberately not implemented** (Phase 2, unchanged): Start / Stop / Restart, Blackout Now,
Follow Server, autostart, process ownership. Migration order is in `README.md` §7.

**Cosmetic, deliberately not touched** (no user-visible effect, and changing them would
invalidate the measured binary):

* `telemetry.rs`: `Snapshot.collect_duration_ms` is hard-coded to `0`; it is typed in
  `src/lib/types.ts` but never rendered.
* Dead code warnings: `telemetry::V1_ROOT` and `winproc::Listener::port` are unused.
* History and the event ring are in-memory only; exiting loses the sparkline.

**Recommended next:**

1. `git init` plus an initial commit — `.gitignore` already anticipates it, and there is
   currently no history to diff against after a stall like this one.
2. Phase 2 step 1: process ownership (spawn vLLM from
   `FN-Optimization/configs/final/qwen3.8-flash-next-optimized.yaml`, record the PID, refuse
   to stop anything V2 did not start).
3. If idle cost matters at 4K, revisit the remaining `backdrop-filter` on the rail only.

---

## 7. How to resume from a cold start

```powershell
cd C:\Users\cohen\AI\RTX-Spark-AI-Control-V2
.\run.ps1              # existing binary; add -Build to rebuild first
curl.exe http://127.0.0.1:8000/v1/models   # server health
```

`run.ps1` builds through `npm run app:build` (the Tauri CLI). Never `cargo build` alone:
the binary links but carries the dev URL `http://localhost:1420` and the window comes up
black. Diagnosis path for any blank window is: check the CDP target URL first.

Dev tooling, opt-in only:

```powershell
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=9222'
python tools/cdp.py eval "document.body.innerText"     # venv python with PIL: native-vLLM\.venv
python tools/measure.py --label x --seconds 60
```
