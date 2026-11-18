# HANDOFF — RTX Spark AI Control V2

Updated: 2026-11-18 (public documentation milestone complete)

## Status

Phase 3 implementation is complete and committed. The public documentation set is
committed. **The next milestone is release packaging** — installer, generated
licence inventory, CI, repository audit and publishing. Nothing has been pushed,
tagged or released, and there is no GitHub repository yet.

Branch `master`. Baseline for Phase 3 was `ade8ca0`.

| Commit | What |
| --- | --- |
| `d70a37f` | Vite watcher no longer walks `src-tauri/target` |
| `c60111c` | portable configuration, battery gating, upstream awareness |
| `6b39e1a` | dev poller, TLS connector, optional interpreter |
| `80e7070` | Updates and Setup pages |
| `4aa0d01` | battery launch confirmation instead of a false start |
| `5add06f` | version 0.1.0 |
| `45c12b1` | presentation helpers moved to `lib/present.ts` |
| `7f8c550` | frontend unit tests (`npm run test:web`) |
| `978c04d` | `tools/shot.ps1` webview capture |
| `929b96a` | refreshed captures plus the four Phase 3 screenshots |
| `60ba062` | Phase 3 recovery handoff |
| `b144ce8` | **documentation set for 0.1.0** |

## Done in the documentation milestone

- **The eight promised documents exist**: `INSTALL.md`, `CONFIGURATION.md`,
  `TROUBLESHOOTING.md`, `SECURITY.md`, `CONTRIBUTING.md`, `CHANGELOG.md`,
  `THIRD-PARTY-NOTICES.md`, `LICENSE`. Every relative link and anchor across the
  twelve Markdown files resolves; every referenced repository path exists.
- **Licence: MIT**, for this application's own code, styles and generated icons.
  Compatibility was checked rather than assumed: every direct Rust and JavaScript
  dependency declares MIT or Apache-2.0 (`tauri`, `tauri-build`, `serde`,
  `serde_json`, `ureq`, `windows-sys`, `@tauri-apps/api`, `@tauri-apps/cli`,
  `react`, `react-dom`, `vite`, `@vitejs/plugin-react`, `typescript`). Upstream
  native-vLLM is Apache-2.0 and external — not bundled, not modified, so no NOTICE
  obligation attaches to this repository. No permission, licence text or notice is
  invented anywhere; `THIRD-PARTY-NOTICES.md` quotes what each manifest says and
  marks the transitive inventory as still to be generated.
- **README corrected against the source.** It had claimed a prebuilt installer that
  does not exist, called the blackout guard a PowerShell script, claimed the guard
  **suppresses input** (it does not — its own source says nothing is hooked or
  blocked, and any input removes the overlay), listed "log files" on the Logs page
  (it shows the event ring and the server's output tail), and called 0.1.0 a first
  public release when nothing is published. All five are fixed, and
  `SECURITY.md` now describes a curtain over an unlocked desktop rather than
  implying a lock.
- **Tested and unverified are separated by name** throughout: README §3 lists the
  untested platform/model combinations, §9 labels every measurement with its
  condition, `INSTALL.md` marks the NSIS installer path unverified, `CHANGELOG.md`
  carries a "Not in this version" section.
- **Private information removed**: the working directory and a local git identity
  were scrubbed from `HANDOFF.md`, `RECOVERY-STATUS.md` and `TEST-RESULTS.md`. The
  source tree contained none — no credentials, tokens or personal paths.
- **Battery, OLED, Modern Standby, the security trade-off and the battery gate**
  are documented where a reader will look for them: README §6–§8,
  `SECURITY.md` §3 and §7, `TROUBLESHOOTING.md`.

### Found while documenting, documented rather than fixed

- The Setup save requires **every** non-optional field, including
  `blackout_engine` and `blackout_python`. A machine with a working vLLM but no
  guard script cannot complete that save. Recorded in `CONFIGURATION.md` §2 and
  `TROUBLESHOOTING.md` as current behaviour. Relaxing it is an application change,
  so it was not made in a documentation session.
- `logs_dir` is stored but nothing writes there; the event ring lives in memory.
  Documented as such in `CONFIGURATION.md` §6.

## Verified in this session

- `npm run test:web`: **20 passed, 0 failed**. `npx tsc --noEmit`: clean.
- Link and anchor validation across all twelve Markdown files: **all resolve**.
  Referenced repository paths: **all exist**.
- Rust suite **not re-run**: no Rust code changed in this session; it passed 46/46
  at `60ba062`.
- **No inference engine was started, stopped, restarted or loaded.** No CUDA work,
  no inference request, no power-setting change; the machine stayed on battery.

## Next milestone: release packaging

In order, because each step depends on the one above:

1. **Third-party licence inventory.** Generate the transitive list from
   `Cargo.lock` (456 packages) and `package-lock.json` with `cargo-about` or
   `cargo-deny` plus a JS licence checker, and bundle the licence texts a
   distributable should carry. `THIRD-PARTY-NOTICES.md` currently covers direct
   dependencies only, and says so.
2. **Release build and installer.** `npm run tauri build` → per-user NSIS
   (`installMode: "currentUser"`, already configured). Then record the binary size
   and SHA-256, and actually install, launch and uninstall it on a clean profile.
   The installer path is **unverified today**; `INSTALL.md` and `README.md` say so.
3. **Continuous integration.** A Windows ARM64 workflow that runs `cargo test`,
   `npm run test:web`, `tsc --noEmit` and the frontend build. Confirm runner
   availability for `windows-11-arm` before writing the badge.
4. **Repository audit.** Secrets and personal-path scan, `.gitignore` review
   (`logs/`, `dist/`, `src-tauri/target/`, `results/`, `qa-screenshots`), and the
   size decision in §"Open decisions".
5. **Publish.** Create the repository, tag `v0.1.0`, and use `CHANGELOG.md` as the
   release notes. Remove the "unreleased" wording from `README.md`, `CHANGELOG.md`
   and `SECURITY.md` in the same commit that tags.
6. **Then** the deferred verification: foreground idle cost for the two new pages,
   the three-request inference probe and real-server lifecycle on AC power, and
   Windows x64.

## Open decisions for the owner

- **`screenshots/` is ~65 MB**, most of it Phase 2 duplicates at full device scale.
  The four Phase 3 captures add ~8.6 MB. Publishing that is a choice about
  repository size; the Phase 3 set is isolated in `929b96a` if you want it out.
- **The Setup save requiring guard paths** — documented, not changed. Decide
  whether it should be relaxed before 0.1.0 is published.
- **Security contact.** `SECURITY.md` §9 deliberately says no address is published
  yet. Pick a channel (private advisory, or an address you are willing to make
  public) when the repository goes up.
- **Supported-versions policy** — currently "the only line is 0.1.0, and it is
  unpublished".

## Rules that still hold

- No vLLM start/stop/restart/load, no CUDA benchmark, no inference request. Use
  `tools/mock-vllm.py` for anything that needs a server.
- Never modify the production model, the native-vLLM environment, the checkpoint,
  the NVIDIA driver, ASUS settings or the OLED power configuration.
- No push, no public repository, no release without explicit approval.

## Next commands

```powershell
cd <repository>
npm run test:web                       # 20 frontend tests
npm run build                          # typecheck + bundle
cargo test --manifest-path src-tauri\Cargo.toml   # 46 Rust tests
npm run app:dev                        # development window (CDP on 9222 optional)
powershell -File tools\shot.ps1 -Out screenshots\x.png -Page Updates -Scale 0.667
```
