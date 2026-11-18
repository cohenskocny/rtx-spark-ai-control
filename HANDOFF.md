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
- QA tooling: `qa-lifecycle.py`, `qa-persistence.py`, `qa-blackout.py`, `qa-events.py`,
  `qa-shot*.py`, `shot-cdp.js`, `mock-vllm.py`.
- Docs: README (user-facing, verified claims), RECOVERY-STATUS.md, TEST-RESULTS.md,
  screenshots/ (baseline + phase2 sets).

## Remaining / NOT TESTED (run on an unlocked desktop)
1. Automatic blackout at the real 180 s idle (`python tools\qa-blackout.py` with 9222 CDP up).
2. Input restore after un-arming (the `blocked=False, reason=disarmed` check).
3. No Modern Standby entry during blackout coverage (`powercfg /requests` as admin).
4. Visible-window idle responsiveness; heavy-load check needs approval (queues work on the
   production server, pid 10644 — do not disrupt it).
5. Optional: a second `cargo test` while `target/` is warm.

Commands for all of these: `RECOVERY-STATUS.md` §8 and `TEST-RESULTS.md` §6.

## Key decisions
- Only verified capabilities are advertised in UI/docs.
- Blackout engine stands down while the Windows session is locked — cause of items 1–3 above.
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
