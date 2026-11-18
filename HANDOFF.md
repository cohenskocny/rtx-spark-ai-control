# HANDOFF — RTX Spark AI Control V2

Updated: 2026-11-18 (release packaging complete)

## Status

Phase 3 implementation is complete and committed. The public documentation set is
committed. The Setup dependency defect is fixed. A Windows ARM64 installer for
**v0.1.0** has been built, installed, launched and uninstalled successfully on this
machine.

**The next milestone is publication** — code signing, a public repository, a
second-machine install test, then the tag. Nothing has been pushed, tagged or
released, and there is no GitHub repository yet.

Branch `master`. Baseline for Phase 3 was `ade8ca0`.

## Publication audit (2026-11-18, after `b923b73`)

An independent-style audit was run before publication. Full report:
[PUBLICATION-AUDIT.md](PUBLICATION-AUDIT.md). Four defects were found and fixed,
so **the v0.1.0 binaries were rebuilt and every hash re-recorded**:

1. **The executable carried the builder's account name.** 277 occurrences of
   `C:\Users\<builder>\.cargo\registry\...` from rustc panic locations — `strip`
   does not remove those. `tools/build-release.mjs` / `npm run release:build` now
   sets `--remap-path-prefix` from `USERPROFILE` and `CARGO_HOME` at build time,
   so the strings read `cargo\registry\...`. No behaviour change; panics still
   name crate, file and line.
2. **A personal path in `tools/contention.py`** named the builder's interpreter.
   Replaced with a description.
3. **`SHA256SUMS.txt` was CRLF**, so `sha256sum -c` could not verify it. Now LF;
   both entries report `OK`.
4. **No example configuration existed.** `CONFIGURATION.example.json` added, with
   a test that fails if it drifts from the schema.

Re-verified end to end: 50 Rust tests, 24 frontend tests, ARM64 production build,
silent install into a throwaway directory, two launches producing one process,
settings written and surviving uninstall, silent uninstall, and a full host-state
diff showing no new service, driver, task, run key or shortcut.

**Still not tested, and it matters:** the installer has never run on a second
machine. Everything here is one host. §6 of the audit lists the exact manual
validation that is still required.

New artifact hashes (the previous manifest's hashes are superseded):

| Artifact | SHA-256 |
| --- | --- |
| `RTX Spark AI Control_0.1.0_arm64-setup.exe` | `39ec4b17a2baca830f930994889084e35e8793abeb18306cf25f3acb584503f6` |
| `rtx-spark-ai-control-v2.exe` | `ee57faabd6047a4e38e13e0e6f2cac7ed0e3f5a1d28721b1d6f1340ddae43990` |


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
| `b144ce8` | documentation set for 0.1.0 |
| `cb10292` | handoff pointed at release packaging |
| `6b07b84` | **blackout became optional; Setup no longer blocks on it** |
| `b091934` | 20 PNG screenshots replaced by 8 optimised JPEGs |
| `9fc2a4c` | generated transitive licence inventory |
| `22b901b` | v0.1.0 installer manifest, checksums and verification record |

## Done in the packaging session

### 1. Setup dependency defect (committed separately, `6b07b84`)

The save required `blackout_engine` and `blackout_python` even for a user who only
wants vLLM management. Now:

- `paths.rs`: guard fields are optional kinds, and a new `IfSetFile` / `IfSetExe`
  pair means "may be empty, but a value that is present must exist" — optional
  stopped being "anything goes". `Paths::guard_interpreter()` falls back to
  `python_exe`. Three new Rust tests, including one that runs the exact
  `config_set` validation sweep over a vLLM-only configuration.
- `main.rs`: `blackout_unavailable()` gates `blackout_now`, `blackout_auto(true)`
  and `follow_server(true)`; disabling is always allowed. The message says vLLM
  management is unaffected and never suggests installing anything.
- `supervisor.rs`: arming is gated on `blackout_ready()`; automation reports an
  `"unavailable"` phase with a reason instead of retrying every tick.
- Frontend: `src/lib/optional.ts` (`blackoutAvailability`, `blackoutBlockedReason`)
  with four tests, `Blackout.tsx` headline and disabled controls, `Setup.tsx`
  optional labels and a three-state pill.
- Existing installations with both guard paths set behave exactly as before. No
  Python and no guard script is installed on the user's behalf, ever.

### 2. Release build and installer

`npm run tauri build`, exit 0, native `aarch64-pc-windows-msvc`.

| Artifact | Bytes | SHA-256 |
| --- | --- | --- |
| `RTX Spark AI Control_0.1.0_arm64-setup.exe` | 1,543,842 | `50c785282a92fed63e7f64d4c76aa3df234a1a0474131f1d424f46f1c50daa3f` |
| `rtx-spark-ai-control-v2.exe` (pre-bundle) | 5,280,768 | `61ffb34b9fb1f239d7184fc754f379996a159d6ad2d2b8e26208f20624fb517f` |
| `rtx-spark-ai-control-v2.exe` (as installed) | 5,280,768 | `c69baeb572760444092e58e86ff59a60c631a3d2cdacf87f806755b9b6cd759c` |

**NSIS over MSI**, because it was already the configured target, installs per user
with no elevation prompt, and can be driven silently for testing. The installed
executable differs from the build-tree one by three bytes at offset 4,953,567:
Tauri rewrites its `__TAURI_BUNDLE_TYPE_VAR_UNK` marker to `..._NSS` so the app
knows it came from a bundle. Both hashes are recorded so nobody has to guess.

Artifacts stay out of Git in an archive directory beside the repository. The
manifest and a machine-readable checksum file are in
[release/v0.1.0/](release/v0.1.0/RELEASE-MANIFEST.md).

**The blackout engine remains an optional external dependency.** Bundling a Python
runtime and an untested display-control script inside a 1.5 MB installer would
create a dependency that could not be verified here.

### 3. Installer verification

Run in a throwaway directory, twice installed and uninstalled:

- silent install with `/S /D=<dir>` → exit 0; nothing written outside that
  directory; Start Menu shortcut and one `HKCU` uninstall entry created
- first run with no settings file → Setup page, pill row `SETUP NEEDED` /
  `BLACKOUT OPTIONAL` / `ON BATTERY · LAUNCH BLOCKED`; bundled frontend renders
  (no dev-server URL, no blank window)
- second launch while running → still one process
- vLLM-only `settings.json` with both blackout paths empty → interface fully
  usable, Blackout page explains what is missing, settings file left untouched
- no interpreter and no model started: `python` process count unchanged, `vllm`
  count 0
- silent uninstall → program directory, shortcut and registry entry removed; user
  settings preserved

### 4. Repository hygiene

- Screenshots: 20 PNGs / 65 MB → 8 JPEGs / 1.2 MB, originals archived outside the
  repository, `tools/optimize-shots.ps1` does the conversion.
- Licence inventory: `node tools/licence-inventory.mjs` writes
  [DEPENDENCIES.md](DEPENDENCIES.md) from both lock files — 456 Rust packages (300
  compiled here, 156 other-platform-only) and 128 JavaScript packages. Every
  compiled dependency offers a permissive option; five crates are MPL-2.0 and
  unmodified; nothing is GPL or AGPL only.
- The release manifest had picked up the local build path and the test directory;
  both were genericised before committing. `logs/` stays gitignored.

### 5. Documentation updated for the package

`INSTALL.md` (build the installer, install, verify the hash, silent forms,
uninstall), `TROUBLESHOOTING.md` (new "Installer and uninstall" section),
`SECURITY.md` §9 (GitHub private vulnerability reporting, with no invented
address) and §10 (unsigned binaries), `CHANGELOG.md` (Packaging section, corrected
"Not in this version"), `CONTRIBUTING.md` (the two generated files), `README.md`
(installer exists but is unpublished; new Setup screenshot), `TEST-RESULTS.md`
(screenshot row), `THIRD-PARTY-NOTICES.md` (links the generated inventory).

## Verified in this session

- `cargo test --manifest-path src-tauri\Cargo.toml` — **49 passed, 0 failed**
- `npm run test:web` — **24 passed, 0 failed**
- `npx tsc --noEmit` — clean, no diagnostics
- `npm run build` — 52 modules, 473 ms
- `npm run tauri build` — exit 0, installer produced, hashes recorded
- Install / first run / single instance / vLLM-only config / uninstall — see §3
- Links and anchors across all 13 Markdown files, plus every `npm run` reference
  against `package.json` — all resolve
- **No inference engine was started, stopped, restarted or loaded.** No CUDA work,
  no inference request, no power-setting change, no driver or ASUS change; the
  machine stayed on battery throughout.

## NOT TESTED

- Installer on a **second machine** or a clean Windows ARM64 image — one host only.
- Installer on **x64 Windows** — the package is ARM64 only and is expected to
  refuse; that expectation was not run.
- The **battery gate refusing an actual start** — the gate was visible
  (`ON BATTERY · LAUNCH BLOCKED`) and is covered by unit tests, but no live launch
  was attempted on battery.
- **Automatic blackout from the installed package** — it passes in the development
  build (TEST-RESULTS §3b); the packaged build was not re-exercised with a guard.
- **Signature and SmartScreen reputation** — the binaries are unsigned.
- **Update check against a real new upstream release** — only cached and
  absent-cache behaviour was exercised.
- **CI** — no workflow exists.

## Next milestone: publication

> **Updated after the publication audit.** Read
> [PUBLICATION-AUDIT.md](PUBLICATION-AUDIT.md) before acting on the list below:
> it re-ran everything, found four defects, fixed them, and rebuilt the binaries,
> so the hashes in `release/v0.1.0/` are **new**. The list below is still the
> right order; §9 of the audit is the authoritative blocker list.

In order, because each step depends on the one above:

1. **Code signing.** An unsigned installer from an unknown publisher is the single
   biggest obstacle to anyone else installing this. Obtain a certificate, wire
   `bundle.windows.certificateThumbprint` (or a CI secret), rebuild, re-record the
   hashes, and confirm SmartScreen behaviour changes.
2. **Second-machine install test.** Fresh Windows 11 ARM64, no Rust toolchain, no
   vLLM: install, first run, configure against a mock, uninstall. Record it in
   `TEST-RESULTS.md` and the manifest's test matrix.
3. **Decide the blackout engine's future.** Current position: external and
   optional for v0.1.0. Bundling is a separate milestone that needs a tested
   runtime.
4. **Continuous integration.** A Windows ARM64 workflow running `cargo test`,
   `npm run test:web`, `tsc --noEmit`, the frontend build and, on tags, the Tauri
   bundle. Confirm `windows-11-arm` runner availability before writing a badge.
5. **Repository audit and creation.** Secret and personal-path scan of the full
   history (not just the working tree), `.gitignore` review, then create the
   repository and enable **Settings → Code security → Private vulnerability
   reporting**, which `SECURITY.md` §9 already tells reporters to use.
6. **Publish.** Tag `v0.1.0`, attach the installer and `SHA256SUMS.txt`, use
   `CHANGELOG.md` as release notes, and remove the "unreleased" wording from
   `README.md`, `CHANGELOG.md` and `SECURITY.md` in the same commit that tags.
7. **Then** the deferred verification: foreground idle cost for the new pages, the
   three-request inference probe and a real-server lifecycle on AC power, and
   Windows x64 behaviour.

## Open decisions for the owner

- **Signing certificate**: which publisher identity, and whether to buy one or
  ship unsigned with a documented hash-only check.
- **Screenshot size**: resolved for the tracked set (1.2 MB). Full-resolution
  originals stay outside the repository by choice.
- **Security contact**: `SECURITY.md` §9 now names GitHub private vulnerability
  reporting as the channel and deliberately publishes no address. Nothing to do
  until the repository exists.
- **Supported-versions policy**: currently "the only line is 0.1.0, and it is
  unpublished".

## Rules that still hold

- No vLLM start/stop/restart/load, no CUDA benchmark, no inference request. Use
  `tools/mock-vllm.py` for anything that needs a server.
- Never modify the production model, the native-vLLM environment, the checkpoint,
  the NVIDIA driver, ASUS settings or the OLED power configuration.
- No push, no public repository, no release without explicit approval.
- Generated files are not hand-edited: `DEPENDENCIES.md` comes from
  `tools/licence-inventory.mjs`, the JPEGs from `tools/optimize-shots.ps1`.

## Next commands

```powershell
cd <repository>
cargo test --manifest-path src-tauri\Cargo.toml   # 50 Rust tests
npm run test:web                                  # 24 frontend tests
npx tsc --noEmit                                  # typecheck only
npm run build                                     # frontend bundle
npm run release:build                             # release binary + NSIS, paths remapped
node tools\licence-inventory.mjs                  # regenerate DEPENDENCIES.md
powershell -File tools\shot.ps1 -Out screenshots\x.png -Page Updates -Scale 0.667
powershell -File tools\optimize-shots.ps1 -Src <archive> -Out screenshots
```

Use `npm run release:build` for anything you intend to publish. Plain
`tauri build` still works, but it bakes the builder's `C:\Users\...` paths into
the executable as panic-location strings; `release:build` remaps them. See
[PUBLICATION-AUDIT.md](PUBLICATION-AUDIT.md) §2.

The installed package can be smoke-tested without touching real settings or the
inference environment by pointing `V2_TEST_DATA_DIR` at a throwaway directory:

```powershell
$env:V2_TEST_DATA_DIR = "$env:TEMP\rtx-v2-smoke"
& "$env:LOCALAPPDATA\RTX Spark AI Control\rtx-spark-ai-control-v2.exe"
```
