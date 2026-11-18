# CHANGELOG — RTX Spark AI Control

Version numbers follow [Semantic Versioning](https://semver.org/). Entries are
grouped by release; everything below is local history, because **nothing has been
published yet** — there is no tag, no installer and no GitHub release.

Each item names the commit it came from, so a claim can be checked with
`git show <hash>`.

---

## [0.1.0] — Unreleased

The first version that is intended to be public. It has been built and tested on
the machine described in [README §3](README.md#3-requirements), and a Windows
ARM64 installer has been built and verified locally. Nothing has been published
or distributed.

### Added

- **Control center shell** — Tauri 2 + Rust + React, single window, seven pages
  (Home, Server, Blackout, Updates, Setup, Settings, Logs). `b73bb82`
- **Inference lifecycle** — start, stop, restart and adopt a vLLM server; ownership
  recorded as pid + creation time so a recycled pid is never stopped; readiness
  from `GET /v1/models` rather than a timer; the server's own output tail surfaced
  in the window. `b73bb82`, `fafdd7e`
- **Supervisor and automation** — health cadence, hysteresis before believing a
  recovery, grace window before giving up on a failure, Follow Server arming the
  blackout guard only while the server is genuinely healthy. `fafdd7e`
- **OLED blackout control** — Blackout Now / Stop Blackout around the validated
  `blackout.py` guard, automatic blackout after an idle threshold, cover selection
  (`internal` · `all` · `primary`), engine state read from its own `state.json`.
  `fafdd7e`
- **Telemetry** — GPU utilisation, VRAM, temperature, power, SM clock and throttle
  reasons from `nvidia-smi`, sampled on a configurable interval, with a bounded
  sparkline in the interface. `fafdd7e`
- **Portable configuration** — no machine paths in the build: every installation
  location is a validated field, discovery offers candidates, save is
  all-or-nothing, and a V1 `config.json` can be imported read-only. Paths are never
  interpolated into a shell command. `c60111c`
- **Battery-aware launch gating** — the power source is read every sample; on
  battery, saved autostart, Follow Server and recovery are all refused with the
  reason shown, and a manual launch requires an inline confirmation. Enforced in
  the backend, not in the buttons. `c60111c`, `4aa0d01`
- **Upstream awareness (Updates page)** — read-only reporting of the installed
  build (version, project build, source revision, wheel tag, interpreter) and the
  newest upstream release with notes and recent commits, plus a compatibility
  verdict kept separate from "there is something newer". One anonymous check per
  24 h, manual check with a 30-second floor, ETag revalidation, on-disk cache that
  survives a failed check, rate limits reported as their own state. `c60111c`,
  `80e7070`
- **Setup page** — every location configurable and validated as typed, discovery
  that suggests without applying, all-or-nothing save. `80e7070`
- **Frontend tests** — 20 assertions over the pure helpers (`format`, `prefs`,
  `present`) run by `node --test`, no additional dependency. `7f8c550`
- **QA tooling** — mock vLLM server, scripted lifecycle checks, CDP helpers, input
  and idle probes, window capture at logical scale, icon generator. `d5f0e4c`,
  `80e9990`, `978c04d`

### Changed

- `README.md`, `HANDOFF.md` and the documentation set were rewritten for a public
  reader, with tested and unverified behaviour kept apart by name.
- Presentation helpers moved out of `Updates.tsx` into `src/lib/present.ts` so the
  page's logic could be tested without a renderer. `45c12b1`
- Page captures were re-taken at logical size, including the new pages. `929b96a`

### Fixed

- **Idle cost** — an infinite CSS `breathe` animation forced the compositor to run
  at 60 Hz forever, and `backdrop-filter: blur()` on every card re-blurred each
  surface per paint. 85.2 % CPU idle → 7.84 % (1500×950). The pulse was replaced
  with a static halo. Measurements and conditions: [README §9](README.md#9-measured-results).
- **`npm run app:dev` died with `EBUSY`** — the Vite watcher descended into
  `src-tauri/target` while cargo executed a build script there, and the unhandled
  watcher error took the dev server down. `d70a37f`
- **Empty window in development** — React 18 StrictMode's mount/unmount/mount left
  the poller's `stopped` flag set for the real mount, so every page sat at
  "connecting" with no error. Release builds were unaffected. `6b39e1a`
- **Every HTTPS request failed with "no TLS backend is configured"** — `ureq` with
  `native-tls` needs the SChannel connector handed to its agent explicitly. Found
  by running the new page against the real machine; unit tests speak plain HTTP to
  a mock and never saw it. `6b39e1a`
- **Setup refused a complete configuration** — `python_exe` was required even
  though it is only a fallback for the guard and the version probe; it is now
  optional, and field labels read as prose ("an executable is required") instead of
  a Debug-lowered variant name. `6b39e1a`
- **A battery launch pretended to work** — the automatic path now refuses with the
  reason shown, and the manual path asks for confirmation instead of starting and
  hoping. `4aa0d01`
- **Setup refused a vLLM-only machine** — `blackout_engine` and `blackout_python`
  were required to save, so a machine without the OLED guard could not finish Setup
  at all. Blackout is optional now: blank saves, the Blackout page names the feature
  as unconfigured and disables its controls, and automation reports `unavailable`
  instead of retrying an absent guard on every tick. An optional field still means
  "may be empty", not "anything goes": a value that is present must exist, so a typo
  is reported. No interpreter is taken from `PATH` and nothing is installed on the
  user's behalf.

### Security

- The blackout guard is documented for what it is: a curtain over an unlocked
  desktop that holds a display request and paints the panel black. It does not hook
  or block input, and any input restores the desktop. Use `Win+L` for a real
  boundary. See [SECURITY.md](SECURITY.md).
- The only outbound link the interface can open is the upstream project on
  github.com; the URL is checked in Rust before the shell is asked for a browser.
- No listening socket, no elevation, no service, no registry writes. The inference
  installation is read and launched, never modified.

### Packaging

- **Windows ARM64 NSIS installer** — `RTX Spark AI Control_0.1.0_arm64-setup.exe`,
  1,543,842 bytes, per-user, no elevation. Chosen over MSI because it was already
  the configured target and installs without administrator rights. Build,
  checksums and the full verification matrix:
  [release/v0.1.0/RELEASE-MANIFEST.md](release/v0.1.0/RELEASE-MANIFEST.md).
- **Silent install and uninstall** verified twice in a throwaway directory: the
  uninstaller removes the program, the shortcut and the registry entry, and
  leaves the settings directory alone. The installer touches nothing outside its
  own directory.
- **Documentation screenshots** reduced from 20 PNGs (65 MB) to 7 JPEGs (1.2 MB)
  by `tools/optimize-shots.ps1`; originals stay out of the repository.
- **Transitive licence inventory** generated from both lock files into
  [DEPENDENCIES.md](DEPENDENCIES.md) by `tools/licence-inventory.mjs` — 300 Rust
  crates compiled for this target plus 128 JavaScript packages, every one with a
  permissive licence option, five MPL-2.0 and unmodified.
- **Blackout stays an external dependency** for v0.1.0: no Python runtime and no
  guard script are bundled. Setup completes without them.

### Not in this version

- A **published** release, a **signed** installer, a migration path for settings
  between published versions, and any installer test on a second machine.
- GitHub Actions or any continuous integration.
- Verification on Windows x64, on other GPU generations, or with other checkpoints.
  None of that has been run.
- Inference performance measurement while the window is visible: it requires
  generating real load, which needs the machine owner's approval.

---

## History before 0.1.0

Two internal phases preceded this version, neither released:

- **Phase 1** — a read-only dashboard: telemetry, server observation, blackout
  observation. Measured 7.84 % idle CPU against the previous controller's 64 %.
- **Phase 2** — the functional control center: supervisor, blackout control,
  settings persistence, event ring, scripted lifecycle tests. Evidence in
  [TEST-RESULTS.md](TEST-RESULTS.md).
- **Phase 3** — portability, battery gating, upstream awareness, Updates and Setup
  pages, and the frontend test suite. Evidence in [HANDOFF.md](HANDOFF.md).

Phase 1 and Phase 2 remain available in `git log` as `b73bb82`..`2e82152`; the
Phase 3 work starts at `ade8ca0`.
