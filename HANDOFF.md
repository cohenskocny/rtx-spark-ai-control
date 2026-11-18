# HANDOFF — RTX Spark AI Control V2

Updated: 2026-11-18 (Phase 2 closeout)

## Status
**Phase 2: COMPLETE.** Code implemented, verified, and committed on `master`.
Phase 3 not started. See `RECOVERY-STATUS.md` for the recovery history and
`TEST-RESULTS.md` for item-by-item evidence.

## Completed (Phase 2)
- Rust backend modules: `core.rs`, `vllm.rs` (start/stop/restart/adopt, supervisor,
  log ring), `blackout.rs` (validated overlay + input suppression), `settings.rs`,
  `events.rs` (Tauri event bridge), `lib.rs`/`main.rs` wiring.
- Frontend: `src/lib/api.ts` + `useAction.ts`, Server / Blackout / Settings / Logs tabs,
  idle cost display, Follow Server automation.
- Verification: `npm run app:build` PASS; `tools/qa-lifecycle.py` PASS against the mock
  on 127.0.0.1:8011 (start/stop/restart/adopt/failure path); blackout Now/Stop with
  `state.json` checks; settings persistence; GPU telemetry cross-checked vs `nvidia-smi`;
  single-instance check; V1 files and production server untouched.
- QA tooling: `tools/qa-lifecycle.py`, `tools/mock-vllm.py`, `tools/cdp.py`,
  `tools/qa-input.py` (SendInput injection + `GetLastInputInfo` idle reads),
  `tools/shots.py`, `tools/make-icon.py`. (Other qa-* scripts from earlier sessions
  were transient and were not preserved; their evidence lives in TEST-RESULTS.md.)
- Docs: README (user-facing, verified claims), RECOVERY-STATUS.md, TEST-RESULTS.md,
  screenshots/ (baseline + phase2 sets), qa-screenshots/ (display/idle test evidence).

## Remaining / NOT TESTED
1. Heavy-load responsiveness check — needs the owner's approval (queues real work on the
   production server, pid 10644 — do not disrupt it). `python tools\infer-probe.py ...`
2. Optional: a second `cargo test` while `target/` is warm.

Closed 2026-11-18 (unlocked desktop, TEST-RESULTS.md §3b): automatic blackout at idle
(fired at a 45 s threshold), input restore via injected SendInput, standby protection
(`ES_DISPLAY_REQUIRED`/`PowerRequestDisplayRequired` held; note: this machine has only
S0 Low Power Idle — "no Modern Standby" was the wrong check), visible-window idle behaviour.

## Key decisions
- Only verified capabilities are advertised in UI/docs.
- Blackout engine stands down while the Windows session is locked (OLED protection) —
  verified behaviour, relevant when running idle tests.
- Build with `npm run app:build` (bare `cargo build` ships the dev URL → black window;
  if blank, check the CDP target URL first).
- Runtime state is kept in project files, not the harness session document (the earlier
  "stored JSON document is at capacity" error was the harness's runtime-state.json
  64 MiB per-document limit, not this app — RECOVERY-STATUS.md §7).

## Protected state (never modify/restart)
- vLLM pid 10644 on 127.0.0.1:8000 (`Qwen/Qwen3.8-Flash-Next`, MTP 4, FP8 KV, 409 600 ctx).
- V1 `config.json` md5 `7e2c65400c2011a80a373b4320f24cab`; V1 `runtime-state.json`
  md5 `99914b932bd37a50b983c5e7c90ae93b`; Tauri `settings.json` md5
  `85974323a65807d93062d9e172900391`; NVIDIA settings.

## Next commands
```powershell
cd C:\Users\cohen\AI\RTX-Spark-AI-Control-V2
npm run app:build          # release build (frontend + cargo)
npx vite build && npm run typecheck   # frontend only
python tools\qa-lifecycle.py          # lifecycle suite (mock on 8011)
```
