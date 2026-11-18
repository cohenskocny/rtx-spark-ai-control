# Release manifest — RTX Spark AI Control v0.1.0

Everything in this file describes the artifacts that were actually built and
tested on 2026-11-18. Where a check could not be run on this hardware, it says
**NOT TESTED** instead of implying otherwise.

**Status: unreleased.** This is a locally validated build. Nothing has been
published, no GitHub repository exists for this project, and no release has been
uploaded anywhere.

---

## 0. Release summary

**Version 0.1.0 is an early public beta**, built by one developer and tested on
one machine. If this manifest is used as the release notes, keep the following
with it — it is the part a downloader needs before running anything.

| Item | Statement |
| --- | --- |
| Maturity | **Early public beta.** No stability or support promise: interface, settings schema and behaviour may change without a migration path |
| Supported platform | **Windows 11 ARM64.** ARM64-only package; it refuses to install on Windows x64. Other ARM64 Windows devices are untested |
| Tested hardware | **NVIDIA RTX Spark N1X** laptop — the only hardware family this has run on |
| Native vLLM | **External dependency.** [`devjeonghwan/vllm-for-rtx-spark`](https://github.com/devjeonghwan/vllm-for-rtx-spark) is the engine this application manages; it is not bundled, modified or redistributed here, and the application never installs it |
| Blackout engine | **External and optional.** No Python runtime or guard script is bundled; Setup completes without them (§5) |
| Signature | **Unsigned installer and executable.** No code-signing certificate exists, so there is no publisher to verify — use the SHA-256 in §2 |
| SmartScreen | **Warnings may occur.** Windows may show "Windows protected your PC" or an unknown-publisher prompt on first run |
| Test coverage | **One host, not multiple independent systems.** Every result in §6 comes from this laptop; nothing was run on a second machine or by an independent tester |

## 1. Identity

| Field | Value |
| --- | --- |
| Product name | RTX Spark AI Control |
| Version | 0.1.0 |
| Source commit | `6b07b84` — the last commit that changes application code. The packaging commits that follow it change documentation, screenshots and build scripts only, so the binary above is reproducible from that tree |
| Build date | 2026-11-18 |
| Build command | `npm run release:build` — `tools/build-release.mjs`, which sets `--remap-path-prefix` and then runs `tauri build` with the NSIS bundle. See §2a |
| Host | Windows 11 ARM64, `aarch64-pc-windows-msvc` |
| Rust | rustc 1.98.0 (88d9e12ae 2026-08-18), cargo 1.98.0 |
| Node | v26.13.1 |
| Tauri | 2.x CLI from `package.json`, bundler targets `nsis` |
| Code signing | None. The installer and executable are unsigned, so Windows SmartScreen will warn on first run. See §7 |

## 2. Artifacts

| Artifact | File name | Bytes | SHA-256 |
| --- | --- | --- | --- |
| Installer (distribute this) | `RTX Spark AI Control_0.1.0_arm64-setup.exe` | 1,543,540 | `39ec4b17a2baca830f930994889084e35e8793abeb18306cf25f3acb584503f6` |
| Executable, pre-bundle | `rtx-spark-ai-control-v2.exe` | 5,276,160 | `ee57faabd6047a4e38e13e0e6f2cac7ed0e3f5a1d28721b1d6f1340ddae43990` |
| Executable, as installed | `rtx-spark-ai-control-v2.exe` | 5,276,160 | `dcba0ea3804bcb1adf25fc188b564e5b98311dc4a6e21a88dbe56773618d63b0` |

The machine-readable copy of the first two is [SHA256SUMS.txt](SHA256SUMS.txt).

**Why the installed executable has a different hash.** Tauri's bundler rewrites a
three-byte marker inside the copied executable — `__TAURI_BUNDLE_TYPE_VAR_UNK`
becomes `__TAURI_BUNDLE_TYPE_VAR_NSS` at byte offset 4,950,334 — so the
application knows it was installed by the NSIS bundle rather than copied by hand.
Nothing else differs: same size, same code, three bytes. Verified by installing
and comparing byte by byte. Hash the *installer* to verify a
download; hash the *installed* executable only to compare against the third row.

Build origin: `src-tauri/target/release/` in the working tree.
Copies are kept in an archive directory outside the repository so that no binary
enters Git history.

### 2a. Build-host paths are remapped out of the binary

`rustc` records the absolute path of every source file it compiles as the
location of a panic or assertion. `strip = true` removes symbols, not those
string literals, so a plain `tauri build` on this host baked
`C:\Users\<builder>\.cargo\registry\src\...` into 277 dependency frames — the
builder's account name, visible to anyone running `strings` on the download.

`npm run release:build` (`tools/build-release.mjs`) now sets
`--remap-path-prefix` before invoking `tauri build`, so the same strings read
`cargo\registry\src\...`. The prefixes are read from `USERPROFILE` and
`CARGO_HOME` at build time, so no machine-specific path is committed and the
setting is a no-op on a host whose paths already agree. Panic messages still name
the crate and file, so the change costs nothing in diagnosis.

Verified after the rebuild: scanning the executable for `C:\Users`, the builder's
name, absolute drive paths, tokens, private keys, e-mail addresses and hostnames
returns no matches. Cost: 4,608 bytes of the executable and 302 of the installer.

## 3. Installer type — NSIS, not MSI

| Consideration | NSIS | MSI (WiX) |
| --- | --- | --- |
| Already configured and cached on this host | yes (`spec.bundle.targets = ["nsis"]`) | no, would have needed a WiX download |
| Elevation required | none in `currentUser` mode | per-machine installs expect an administrator |
| Silent install for scripted testing | `/S` and `/D=` | `msiexec /i /qn INSTALLDIR=` |
| Uninstall | `uninstall.exe`, registered in Add/Remove Programs | `msiexec /x` via the same registry entry |
| Size | 1.5 MB | larger, plus WiX toolchain |

NSIS was chosen because it was already the configured target, ships an installer
with no elevation prompt — matching an application that never needs administrator
rights — and can be installed and removed silently into a throwaway directory,
which is what the verification in §6 did.

Installer behaviour, as observed:

- Installs per user under `%LOCALAPPDATA%\RTX Spark AI Control` by default; `/D=<path>`
  overrides the location. No machine-wide files, no services, no drivers.
- Creates one Start Menu shortcut: `RTX Spark AI Control.lnk`.
- Adds one Add/Remove Programs entry: `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\RTX Spark AI Control`.
- Does not touch `%APPDATA%\com.rtxspark.aicontrol.v2` (the application's own
  settings directory) during install or uninstall.
- Uninstall removes the program directory, the shortcut and the registry entry,
  and leaves user settings in place.

Silent forms, as used for the verification runs:

```
"RTX Spark AI Control_0.1.0_arm64-setup.exe" /S /D=C:\path\to\install
"%LOCALAPPDATA%\RTX Spark AI Control\uninstall.exe" /S
```

`/D=` must be last and unquoted, which is an NSIS convention rather than a choice
made here.

## 4. What the application needs from the machine

| Requirement | Status |
| --- | --- |
| Windows 11 ARM64 | Target. Built and run there |
| WebView2 runtime | Used for the application window. Included with Windows 11; no separate installer is shipped |
| NVIDIA RTX Spark N1X | The tested hardware family |
| Other ARM64 Windows devices | **Unverified.** The application launches, but nothing about battery gating, GPU behaviour or display control was tested on other hardware |
| Windows on x64 | **Not supported.** The package is ARM64 only and will refuse to install on x64 |
| Native vLLM for RTX Spark | External, optional for the interface to run. Needed to start or monitor an inference server |
| Python | Not required unless the operator configures the optional OLED blackout engine |

## 5. Bundled and not bundled

Bundled in the 1.5 MB installer: the Rust executable, the compiled frontend
assets, the application icon set, and the NSIS uninstaller. That is all.

Not bundled, by decision:

- **Native vLLM** (`devjeonghwan/vllm-for-rtx-spark`) and any Python interpreter.
  Both are installed by the operator; the application launches what is already
  there and never installs anything.
- **The blackout engine** (`tools/`-external Python script driving the display
  API). It is configured as two optional paths in Setup. Shipping a Python
  runtime and an untested display-control script inside an installer would
  create a dependency that could not be verified here, so v0.1.0 keeps it
  external and optional. Users without it can complete Setup and use everything
  else.
- **Model checkpoints.** Tens of gigabytes, licensed separately, and the
  operator's own files.
- **CUDA, NVIDIA drivers, ASUS software.** Never shipped, never modified.
- **Screenshots.** Documentation only; see [screenshots/](../../screenshots).

## 6. Verification performed

Full environment and commands: [INSTALL.md](../../INSTALL.md) §6 and
[TEST-RESULTS.md](../../TEST-RESULTS.md). Installer runs used a throwaway
directory outside the repository, deleted afterwards.

| Check | Result |
| --- | --- |
| `cargo test` — 50 tests | PASS |
| `npm run test:web` — 24 tests | PASS |
| `npx tsc --noEmit` | PASS, no diagnostics |
| `npm run build` (frontend production build) | PASS |
| `npm run tauri build` (native ARM64 release + NSIS) | PASS, exit 0 |
| SHA-256 recorded for installer and executable | PASS |
| Silent install into a controlled directory | PASS, exit 0 |
| Installed files and hashes as expected | PASS |
| First run with no settings file → Setup page, blackout marked optional | PASS, screenshot captured |
| Second launch while running → still one process | PASS |
| vLLM-only settings, blackout paths empty → interface fully usable | PASS, screenshot captured |
| Settings file left unmodified by a read-only session | PASS |
| Application started no interpreter, no model, no GPU work | PASS (`python` count unchanged, `vllm` count 0) |
| Silent uninstall removes program, shortcut and registry entry | PASS, run twice |
| User data survives uninstall | PASS |
| Installing on a machine without vLLM | PASS — Setup reports the missing paths, nothing crashes |
| Automatic blackout with a real engine | PASS in the development build — armed via the interface, display request held, input restored ([TEST-RESULTS.md](../../TEST-RESULTS.md) §3b). **Not re-run from the installed package** |
| Battery gate visible in the installed build | PASS — the Setup page reported `ON BATTERY · LAUNCH BLOCKED` on this laptop's battery |
| Battery gate refusing an actual start | **NOT TESTED** — would need a live launch attempt and a real server, which is out of scope while on battery |
| Installer on a clean second machine | **NOT TESTED** — one host only |
| Installer on x64 Windows | **NOT TESTED** — the package is ARM64 only and is expected to refuse |
| Signature and SmartScreen reputation | **NOT TESTED** — the binaries are unsigned |
| Update check against a real new upstream release | **NOT TESTED** — only the cached/absent-path behaviour was exercised |

## 7. Security and distribution notes

- No telemetry, no analytics, no background service, no scheduled task.
- The API host defaults to `127.0.0.1`. The application talks to the inference
  server on loopback, and makes one outbound HTTPS request to the public GitHub
  API for the upstream release list: at most once per 24 hours from a background
  thread, plus whenever the Updates page asks. The response is cached in
  `upstream-cache.json` under the application's settings directory. The update
  checker is informational: it reports versions and links to the upstream
  project, and never downloads or installs anything.
- The application launches the vLLM executable path the operator configures. It
  does not search `PATH`, download executables, or elevate.
- The binaries are unsigned, so there is no signing identity to verify. A
  publisher signature is a prerequisite for distributing this to anyone else.
- `rustc` normally embeds the build machine's source paths in the executable —
  the `.cargo\registry\src\...` strings visible in a strings dump. Those are
  normal for a release build, but they would name the builder's account. The
  release build remaps them (§2a), so the shipped executable carries
  `cargo\registry\src\...` and no `C:\Users` path at all. Panic messages still
  name the crate, file and line. Nothing in the *repository* contains a personal
  path; see [SECURITY.md](../../SECURITY.md) §9.
- Vulnerability reporting, before any public repository exists, is described in
  [SECURITY.md](../../SECURITY.md).

## 8. Licence position

The application's own code is MIT — see [LICENSE](../../LICENSE). The complete
transitive inventory of what the executable and installer contain, generated from
the two lock files, is [DEPENDENCIES.md](../../DEPENDENCIES.md): 300 Rust crates
compiled for this target and 128 JavaScript packages, every one offering a
permissive licence option, five of them MPL-2.0 (file-level, unmodified). No GPL
or AGPL component is present. The external inference engine is Apache-2.0 and is
not redistributed.

## 9. Publication readiness

Ready locally, not ready publicly. Outstanding before anyone else should install
this:

1. **Code signing.** An unsigned installer from an unknown publisher is a real
   obstacle, not a formality.
2. **A public repository** with this history, so the source commit in §1 can be
   checked against the binary.
3. **A second-machine install test**, ideally on hardware other than the N1X and
   on a fresh Windows ARM64 image.
4. **The unverified rows in §6** — in particular automatic blackout with a real
   engine, and the battery gate observed on an actually-discharging battery.
5. **A decision on the blackout engine**: keep it external for v0.1.0 (current
   position) or build and test a bundled runtime later.
6. **GitHub private vulnerability reporting enabled** on the public repository,
   which cannot exist until the repository does.

Nothing in this build writes to, replaces or reconfigures vLLM, model checkpoints,
CUDA, NVIDIA drivers, ASUS software or Windows power settings. Installation is a
per-user file copy plus a shortcut.
