# HANDOFF — RTX Spark AI Control V2

Updated: 2026-11-18 (Phase 3 recovery — frontend committed)

## Status
**Phase 3 in progress, committed.** Phase 2 remains complete and operational.
Branch `master`. Nothing has been pushed and no release has been published.

The previous session hit its session-storage limit while preparing the frontend
commits. The code it had written was already on disk and is committed here
unchanged; this session added no features.

Phase 3 commits on top of the Phase 2 baseline `ade8ca0`:

| Commit | What |
| --- | --- |
| `d70a37f` | Vite watcher no longer walks `src-tauri/target` |
| `c60111c` | portable configuration, battery gating, upstream awareness |
| `6b39e1a` | dev poller / TLS / interpreter fixes |
| `80e7070` | Updates and Setup pages |
| `4aa0d01` | battery launch confirmation instead of a false start |
| `5add06f` | version 0.1.0 |
| `45c12b1` | presentation helpers moved out of `Updates.tsx` into `lib/present.ts` |
| `7f8c550` | frontend unit tests (`node --test`), `npm run test:web` |
| `978c04d` | `tools/shot.ps1`, webview capture over loopback CDP |
| `929b96a` | refreshed page captures plus the four Phase 3 screenshots |

## Done in this phase
- **Updates page** (`src/pages/Updates.tsx`): installed build, source revision,
  wheel tag, interpreter, newest upstream release with notes, 12 recent commits,
  compatibility verdict, and the state of the check (reachable / offline /
  rate limited / never). Manual "Check for Updates"; daily cadence in the
  backend. Links go through `open_external`, which only accepts the upstream
  github.com prefix. The pure helpers behind it live in `src/lib/present.ts` so
  they can be tested without a renderer.
- **Setup page** (`src/pages/Setup.tsx`): every location is a validated field,
  discovery offers candidates without applying them, save is all-or-nothing,
  V1 `config.json` import is read-only. Paths live in
  `%APPDATA%\com.rtxspark.aicontrol.v2\settings.json` (schema 2), never in the
  repository.
- **Battery gating** visible in the top bar and enforced in `vllm::start`:
  automatic paths never launch on battery; a manual launch needs the inline
  confirmation (`confirmBattery`).
- **Dev workflow fixed**: Vite no longer walks `src-tauri/target`
  (`d70a37f`), and the StrictMode poller bug that left a dev window empty is
  fixed (`6b39e1a`). `npm run app:dev` now builds and runs with a live UI.
- **Upstream check verified live**: `0.32.0+rtx_spark.0.2.0`, revision
  `e9412963ff`, newest release `v0.2.0`, verdict "compatible", quota reported.
- **Frontend tests**: 20 assertions over `format`, `prefs` and `present` —
  formatting units, preference round-trip, a storage that throws and one that
  is corrupt, and the Updates page's status/timestamp/quota/notes behaviour.

## Verified in the recovery session (2026-11-18)
- `cargo test --manifest-path src-tauri\Cargo.toml`: **46 passed, 0 failed**,
  no warnings (2.7 s).
- `npm run test:web`: **20 passed, 0 failed** (~220 ms).
- `npx tsc --noEmit`: clean. `npm run build`: 51 modules, 204.97 kB JS
  (62.93 kB gzip) / 21.33 kB CSS.
- Raw output, gitignored: `logs/verify/cargo-test.log`, `web-tests.log`,
  `tsc.log`, `build.log`.
- **No inference engine was started, stopped, restarted or loaded.** No CUDA
  benchmark, no inference request, no power-setting change. The machine stayed
  on battery throughout.

## React polling fix (reviewed here, unchanged)
`src/lib/useTelemetry.ts` — the polling effect claims the poller on every mount
(`stopped.current = false; busy.current = false`) with a comment stating why:
React 18 StrictMode mounts, unmounts and mounts again in development, and the
first cleanup would otherwise leave `stopped` set for the real mount, giving a
window that sits at "connecting" with no error. Cleanup sets `stopped` and
clears the timer; the request's `finally` reschedules only while `!stopped`.
Schedule is completion-driven, so a slow poll cannot stack, and the timer is
cleared before every new one.

One accepted nuance: an in-flight sample from the discarded first mount can
resolve after the second mount and replace its timer. Polling continues and no
second loop is created (the old timer is cleared first), so it is left as is
rather than papered over.

## Deliberately left uncommitted
`README.md` is rewritten for a 0.1.0 public release and links to eight documents
that do not exist yet: `INSTALL.md`, `CONFIGURATION.md`, `TROUBLESHOOTING.md`,
`SECURITY.md`, `CONTRIBUTING.md`, `CHANGELOG.md`, `THIRD-PARTY-NOTICES.md`,
`LICENSE`. Public-release documentation was out of scope for this session, so
the file stays modified and untracked-in-diff. Commit it together with those
documents, or drop it with `git checkout -- README.md`.

## Still open (Phase 3 remainder)
1. Release documentation set (the eight files above, plus issue/PR templates),
   then commit the README rewrite.
2. Release build + SHA-256, GitHub Actions, NSIS packaging.
3. Repository audit: credentials, personal paths, weights, logs, licensing.
4. `screenshots/` is ~65 MB, most of it Phase 2 duplicates; the Phase 3
   captures add ~8.6 MB. Needs an owner decision before publishing — they are
   isolated in `929b96a` and can be dropped on their own.
5. Deferred, needs AC power and approval: anything that loads a model (the
   three-request inference probe, real-server lifecycle).
6. Not measured: foreground idle cost for the two new pages. Every Phase 3
   sample is startup-inclusive or occluded, since the window could not be
   brought to the foreground from the session's shell.

## Rules that still hold
- No vLLM start/stop/restart/load, no CUDA benchmark, no inference request.
  Use the mock server (`tools/mock-vllm.py`) for anything that needs a server.
- Never modify the production model, the native-vLLM environment, the
  checkpoint, the NVIDIA driver, ASUS settings or the OLED power configuration.
- No push, no public repository, no release without explicit approval.

## Next commands
```powershell
cd C:\Users\cohen\AI\RTX-Spark-AI-Control-V2
npm run test:web                       # frontend unit tests
npm run build                          # frontend typecheck + bundle
cargo test --manifest-path src-tauri\Cargo.toml
npm run app:dev                        # development window (CDP on 9222 optional)
powershell -File tools\shot.ps1 -Out screenshots\x.png -Page Updates -Scale 0.667
```
