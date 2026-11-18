# HANDOFF — RTX Spark AI Control V2

Updated: 2026-11-18 (repository published to GitHub; first Windows ARM64 CI run passed)

## Status

Phase 3 implementation is complete and committed. The public documentation set is
committed. The Setup dependency defect is fixed. A Windows ARM64 installer for
**v0.1.0** has been built, installed, launched and uninstalled successfully on this
machine.

**The next milestone is release** — code signing, a second-machine install test,
then the tag. The repository is public and CI runs green; **no tag has been
created, no release has been drafted on GitHub, and no installer has been
uploaded.**

## Publication state

| Item | State |
| --- | --- |
| Repository | <https://github.com/cohenskocny/rtx-spark-ai-control> (public) |
| Remote | `origin`, HTTPS, tracks `master` |
| Pushed commit | `c861aa23c126d1666aea39223ddecb3d8d4454c2` |
| CI | workflow `ci`, run [44398122737](https://github.com/cohenskocny/rtx-spark-ai-control/actions/runs/44398122737), **success** on `c861aa2` |
| CI steps | all green: checkout, Node 26.13.1, Rust 1.98.0 aarch64-pc-windows-msvc, `npm ci` from the lock file, 24 frontend tests, `tsc --noEmit` + build, 50 Rust tests, config-example validation |
| Security | private vulnerability reporting enabled; Actions default token permissions `read`, PR approval by Actions off |
| Release workflows | none exist — nothing can publish itself |
| Tags / releases | none |
| Code signing | none — installer and executable are unsigned |

The Git author identity for commits after this point is `Cohen Skocny
<117538498+cohenskocny@users.noreply.github.com>`, set with `git config --local`
only. The 33 pre-existing commits still carry `Cohen <cohen@localhost>`; that is a
deliberate open decision compared in [PUBLICATION-AUDIT.md](PUBLICATION-AUDIT.md)
§3 finding 3, and **no history was rewritten**. Global Git settings are untouched.

## AI-assisted installation milestone (2026-11-18)

Scope was documentation and read-only tooling. **No application code changed**:
no Rust, no React, no `tauri.conf.json`, no `package.json` dependency, no
behaviour.

| Added | What it is |
| --- | --- |
| [AGENT-INSTALL.md](AGENT-INSTALL.md) | One standalone prompt for Claude Code, Codex, OpenCode, T3 Code or any agent. Two paths: GUI-only, and guided full setup. Approval gates before every change, SHA-256 verification of the installer, native-vLLM discovery and configuration, and explicit instructions not to assume a model, checkpoint or folder layout |
| [AGENTS.md](AGENTS.md) | Repository guide for agents: architecture, supported environment, build and test commands, external dependency boundaries, configuration paths, safety requirements, contribution rules |
| [tools/check-requirements.ps1](tools/check-requirements.ps1) | Read-only machine check. Exit 0 all required checks passed, 1 a required check failed, 2 unsupported platform, 3 the checks could not run |
| [tools/verify-install.ps1](tools/verify-install.ps1) | Read-only install check, including an installer SHA-256 comparison against `release/v0.1.0/SHA256SUMS.txt`. Exit 0 verified, 1 required check failed, 2 unsupported, 3 not installed, 4 could not run |
| [tools/lib/install-checks.ps1](tools/lib/install-checks.ps1) | The verdict functions and the probes behind both scripts. Judgements are pure functions so scenarios are testable without hardware |
| [tools/test-install-scripts.ps1](tools/test-install-scripts.ps1) | 30 scenario tests: ARM64 detection, unsupported systems, missing GPU, driver and WebView2, absent native-vLLM, existing native-vLLM, incomplete installation, absent blackout engine, battery power, offline host, checksum mismatch, and a user who declines to install |

Documentation and metadata updates in the same commit: README gained an **Install
with AI Agent** section near the top, two rows in its documentation table and a
clone URL in Development; `INSTALL.md` points at the guide; `package.json` carries
`repository`, `homepage` and `bugs`; `CONTRIBUTING.md`, `RELEASE-MANIFEST.md` §9,
`PUBLICATION-AUDIT.md` §10 and the CI header now name
<https://github.com/cohenskocny/rtx-spark-ai-control>.

**Verified in this session**

| Check | Result |
| --- | --- |
| `powershell -File tools\test-install-scripts.ps1` | **30 scenarios passed, 0 failed**, including an end-to-end `-Json` run of both scripts |
| `tools\check-requirements.ps1` on this laptop | exit 0 — Windows 11 build 28000, ARM64 via `Win32_Processor` 12, N1X, driver 621.14, WebView2 157.0.4341.60, native-vLLM `0.32.0+rtx_spark.0.2.0` found at its root, battery warning, network reachable |
| `tools\verify-install.ps1` on this laptop (nothing installed) | exit 3, one required failure — "no application found" — settings file hash and mtime unchanged, no program directory created |
| Relative links and anchors | 16 Markdown files scanned, none broken |
| PowerShell parse of all three scripts and the library | clean |
| Any inference, CUDA work or model load | **none** — no server was started or stopped |

**Limits of that verification**

- The x64, Windows 10, missing-driver, missing-WebView2, offline and
  checksum-mismatch scenarios are covered with fabricated fact sets, not with
  real machines. That is deliberate: the point is that the logic is exercised
  without a second host.
- The download path in `AGENT-INSTALL.md` has **not** been exercised end to end,
  because no release is published. Today the guide's "no release yet" branch is
  the one that applies; the checksum path was exercised with the recorded v0.1.0
  values and with deliberate mismatches.
- No coding agent was driven through the prompt by hand. The wording is asserted
  by test, not by an agent transcript.
- `Get-ScheduledTask` matching is deliberately narrow: NVIDIA's own
  `RTXSparkFirstRunNotification_…` task is reported as an ignored vendor task, not
  as a footprint finding.

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
| `RTX Spark AI Control_0.1.0_arm64-setup.exe` | 1,543,540 | `39ec4b17a2baca830f930994889084e35e8793abeb18306cf25f3acb584503f6` |
| `rtx-spark-ai-control-v2.exe` (pre-bundle) | 5,276,160 | `ee57faabd6047a4e38e13e0e6f2cac7ed0e3f5a1d28721b1d6f1340ddae43990` |
| `rtx-spark-ai-control-v2.exe` (as installed) | 5,276,160 | `dcba0ea3804bcb1adf25fc188b564e5b98311dc4a6e21a88dbe56773618d63b0` |

These are the hashes of the **rebuilt** artifacts, the same values as
[release/v0.1.0/RELEASE-MANIFEST.md](release/v0.1.0/RELEASE-MANIFEST.md) §2. The
first build's hashes (1,543,842 / `50c785…` / `5,280,768` / `61ffb34…`) are
superseded: the publication audit added `--remap-path-prefix` and the binaries
were rebuilt, so anything quoting them is out of date.

**NSIS over MSI**, because it was already the configured target, installs per user
with no elevation prompt, and can be driven silently for testing. The installed
executable differs from the build-tree one by three bytes at offset 4,950,334:
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
- **CI** — `.github/workflows/ci.yml` exists, but no GitHub Actions run has ever
  happened: there is no remote repository. The commands it runs were executed by
  hand on this host instead.

## Session: CI and publication preparation (2026-11-18)

Scope was CI, publication wording and this commit only — no application changes.

### 1. GitHub Actions

`.github/workflows/ci.yml`, one job on `windows-11-arm`:

| Step | What runs |
| --- | --- |
| checkout | `actions/checkout@v7` |
| Node.js 26.13.1 | `actions/setup-node@v7` with the npm cache |
| Rust 1.98.0 + `aarch64-pc-windows-msvc` | `rustup toolchain install 1.98.0 --profile minimal --target …` |
| install | `npm ci` |
| frontend tests | `npm run test:web` |
| type check and build | `npm run build` (`tsc --noEmit && vite build`) |
| Rust tests | `cargo test --manifest-path src-tauri/Cargo.toml --locked` |
| example configuration | `node tools/check-config-example.mjs` |

Runner choice: `windows-11-arm` is GitHub's standard GitHub-hosted Windows 11
ARM64 image (4 vCPU / 16 GB public, 2 vCPU / 8 GB private). Checked against the
hosted-runner reference on 2026-11-18, which lists it for both public and private
repositories: it began in 2025 as a public-preview label for public repositories
and is no longer limited to the preview. No other hosted label is ARM64 Windows.

Triggers: pushes to `master`, pull requests, manual dispatch. `permissions:
contents: read`, concurrency group per ref, 45-minute timeout. No signing,
bundling, upload, tag or release step, as instructed.

`tools/check-config-example.mjs` is new: it compares the example's key sets with
the schema, requires the placeholder paths, and fails if a real host path is pasted
into a template. `cargo test` already round-trips the same file through `Settings`;
this is the version that runs without a Rust build.

### 2. Publication privacy

- Author identity is still `Cohen <cohen@localhost>` on all 33 commits. Three
  options — keep it, use the GitHub noreply address from now on, or rewrite the
  history — are compared in [PUBLICATION-AUDIT.md](PUBLICATION-AUDIT.md) §3
  finding 3. **Nothing was rewritten; that needs separate approval.**
- Re-scanned the tracked tree for the audit's §3 findings: no credentials, tokens,
  keys or private keys; no personal or absolute host path beyond deliberate
  placeholders (`C:\path\to\…`, and one generic `C:\Users\…` inside a sentence
  about rustc). `tools/contention.py` is still free of the builder's interpreter
  path. **The findings remain valid.**

### 3. Release wording

`README.md` gained a "Status: v0.1.0, early public beta" table, `CHANGELOG.md`
gained release notes for 0.1.0, and `RELEASE-MANIFEST.md` gained §0 Release
summary. All three now state, in the same words: early public beta; Windows 11
ARM64 is the supported platform; the NVIDIA RTX Spark N1X is the tested hardware
family; native vLLM is an external dependency; the blackout engine is external and
optional; the installer is unsigned; SmartScreen warnings may occur; testing was
performed on one host, not several independent systems. Attribution to
`devjeonghwan/vllm-for-rtx-spark` is kept in README §7 and §12 and in the manifest.

### 4. Verified locally in this session (nothing was pushed)

| Check | Result |
| --- | --- |
| YAML parse of the workflow (`yaml valid`) | PASS, exit 0 |
| `npm run test:web` | PASS, 24 tests, 0 failures |
| `npm run build` | PASS, 52 modules, no diagnostics |
| `cargo test --manifest-path src-tauri/Cargo.toml --locked` | PASS, 50 tests, 0 failures |
| `node tools/check-config-example.mjs` | PASS — 13 path fields, no host paths |
| Any GitHub Actions run | **NOT RUN — no remote repository exists** |

No vLLM start or stop, no model load, no CUDA work, no power-setting change.

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
4. **Continuous integration — DONE, green.** `.github/workflows/ci.yml`
   runs `npm ci`, `npm run test:web`, `npm run build`, `cargo test --locked` and
   the example-configuration check on `windows-11-arm`, with Node.js 26.13.1 and
   Rust 1.98.0 pinned. It deliberately does not sign, bundle, upload or publish.
   First run [44398122737](https://github.com/cohenskocny/rtx-spark-ai-control/actions/runs/44398122737)
   passed on `c861aa2`; the clean `npm ci` that was expected to break first did
   not. A badge is now defensible.
5. **Repository audit and creation — DONE.** Secret and personal-path scan of the
   full history, `.gitignore` review, public repository created, `master` pushed,
   **Settings → Code security → Private vulnerability reporting** enabled, and
   workflow token permissions reduced to read-only.
6. **Publish — NOT STARTED, needs the owner.** Tag `v0.1.0`, attach the installer
   and `SHA256SUMS.txt`, use `CHANGELOG.md` as release notes, and remove the
   "unreleased" wording from `README.md`, `CHANGELOG.md` and `SECURITY.md` in the
   same commit that tags. Nothing has been tagged, drafted or uploaded.
7. **Then** the deferred verification: foreground idle cost for the new pages, the
   three-request inference probe and a real-server lifecycle on AC power, and
   Windows x64 behaviour.

## Open decisions for the owner

- **Signing certificate**: which publisher identity, and whether to buy one or
  ship unsigned with a documented hash-only check.
- **Git author identity**: keep `Cohen <cohen@localhost>`, switch to a GitHub
  noreply address for future commits, or rewrite the history. The third option
  invalidates every recorded SHA and needs explicit approval. The author **name**
  is now `Cohen Skocny` (local config, this session); the **e-mail** is unchanged
  until a GitHub noreply address is verified.
- **Screenshot size**: resolved for the tracked set (1.2 MB). Full-resolution
  originals stay outside the repository by choice.
- **Security contact**: `SECURITY.md` §9 now names GitHub private vulnerability
  reporting as the channel and deliberately publishes no address. Private
  vulnerability reporting is enabled on the published repository.
- **Supported-versions policy**: currently "the only line is 0.1.0, and it is
  unreleased".

## Rules that still hold

- No vLLM start/stop/restart/load, no CUDA benchmark, no inference request. Use
  `tools/mock-vllm.py` for anything that needs a server.
- Never modify the production model, the native-vLLM environment, the checkpoint,
  the NVIDIA driver, ASUS settings or the OLED power configuration.
- No release, tag, installer upload or publish without explicit approval. The
  repository itself is now public and CI is expected to stay green there.
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
