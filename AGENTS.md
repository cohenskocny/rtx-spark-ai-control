# AGENTS.md — RTX Spark AI Control

Instructions for coding agents working **on this repository**. If you are trying
to *install* the application instead, use [AGENT-INSTALL.md](AGENT-INSTALL.md);
those are different jobs and this file is not an installation guide.

## 1. What this project is

A native Windows ARM64 control centre for a local **native-vLLM** inference
server on an NVIDIA RTX Spark laptop, with an optional OLED blackout guard. The
window starts, stops, restarts and supervises vLLM, watches GPU telemetry, and
arms the blackout guard after an idle timeout.

Version 0.1.0 is an early public beta, tested on one machine. Nothing here is a
compatibility promise.

## 2. Architecture

| Layer | Location | What it owns |
| --- | --- | --- |
| Shell | `src-tauri/` | Tauri 2 window, IPC commands, single-instance protection |
| Backend | `src-tauri/src/*.rs` | `paths` (configuration and validation), `vllm` (launch, stop, readiness), `supervisor` (health, hysteresis, grace), `blackout` (guard lifecycle), `power` (AC/battery gate), `telemetry` (`nvidia-smi` sampling), `upstream` (read-only GitHub release check), `version` (installed build from disk), `settings` (persisted state), `winproc` (process ownership by pid plus creation time) |
| Frontend | `src/` | React 18 function components, pages in `src/pages/`, pure helpers in `src/lib/` |
| Tests | `src-tauri/src/*.rs` (`#[cfg(test)]`), `tests/web/` (`node --test`) | Rust unit and integration tests, frontend helper tests |
| Tooling | `tools/` | Build, QA, licence inventory and the read-only installer checks |

The backend is authoritative: battery gating, path validation and process
ownership live in Rust, not in the interface.

## 3. Supported environment

| | |
| --- | --- |
| OS | **Windows 11 ARM64** only. The package is ARM64 and refuses to install on x64 |
| Tested hardware | NVIDIA RTX Spark N1X laptop, driver 621.14. Other ARM64 Windows devices are untested |
| Build host | Rust 1.98 `aarch64-pc-windows-msvc` (`rust-version = "1.77"` floor), Node 26.13, npm 11.20, VS 2022 ARM64 MSVC, Windows SDK 10.0.26100 |
| Runtime | Microsoft Edge WebView2, shipped with Windows 11 |
| Inference engine | native-vLLM `0.32.0+rtx_spark.0.2.0` from `devjeonghwan/vllm-for-rtx-spark`, installed by the operator |

Detecting architecture: use `Win32_Processor.Architecture` (12 = ARM64). Do not
use `$env:PROCESSOR_ARCHITECTURE` — on this platform a 64-bit x86 process under
emulation reports AMD64 and gives a false negative.

## 4. Build and test commands

```powershell
npm install
npm run app:dev        # development window: Vite on 127.0.0.1:1420 + cargo
npm run build          # tsc --noEmit && vite build
npm run test:web       # frontend unit tests (node --test, no extra dependency)
cargo test --manifest-path src-tauri\Cargo.toml
npm run release:build  # release binary + NSIS installer, build-host paths remapped
node tools/check-config-example.mjs      # example configuration still a template
powershell -File tools\test-install-scripts.ps1   # installer scenario tests
```

`npm run build` runs the type check itself. Both test commands plus the build are
the minimum before committing. CI (`.github/workflows/ci.yml`, `windows-11-arm`)
runs the same set; it has not been executed on GitHub yet because there is no
remote repository.

Use `npm run release:build` for anything intended to leave this machine: plain
`tauri build` bakes the builder's `C:\Users\...` paths into the executable as
panic-location strings.

## 5. External dependency boundaries

Never add functionality that installs, downloads, upgrades or reconfigures any
of these. They belong to the operator, and the application must keep working
when they are absent.

| Boundary | Rule |
| --- | --- |
| native-vLLM (`devjeonghwan/vllm-for-rtx-spark`) | External. Never bundled, never installed, never modified. The Updates page is informational and read-only |
| Model checkpoints and weights | The operator's files. Never downloaded, never written, never moved |
| NVIDIA driver, CUDA toolkit, cuDNN, PyTorch builds | Never installed or changed by this project |
| Blackout guard script and Python | External and optional. Configured as paths only; no runtime is shipped and none is installed |
| WebView2 | Shipped with Windows; repaired by the operator, never by this project |
| Services, scheduled tasks, Run entries, drivers | This application installs none. Do not add any |

For a server in tests, use `tools/mock-vllm.py`, never a real installation.

## 6. Configuration paths

| Store | Location |
| --- | --- |
| Backend settings (schema 2) | `%APPDATA%\com.rtxspark.aicontrol.v2\settings.json` |
| Runtime ownership record | `%APPDATA%\com.rtxspark.aicontrol.v2\runtime-state.json` |
| Update cache | `%APPDATA%\com.rtxspark.aicontrol.v2\upstream-cache.json` |
| Interface preferences | `localStorage` key `rtx-v2.prefs.v1` |
| Installer location | `%LOCALAPPDATA%\RTX Spark AI Control\rtx-spark-ai-control-v2.exe` |
| Test-only overrides | `V2_TEST_DATA_DIR`, `V2_TEST_GITHUB_API`, `V2_TEST_CACHE_PATH` |

Setup requires five paths — `vllm_root`, `vllm_exe`, `model_checkpoint`,
`launch_config`, `chat_template` — and validates them all-or-nothing. The guard
paths, Python, logs directory and legacy import are optional. Full semantics:
[CONFIGURATION.md](CONFIGURATION.md). Never write inside an installation, a
checkpoint or the registry.

## 7. Safety requirements

1. **No inference in a working session.** Do not start, stop, restart or adopt a
   real server, and never load a model. If a test needs a server, run
   `tools/mock-vllm.py`.
2. **No CUDA workloads.** No benchmark, no `nvidia-smi` reset, no driver or power
   setting change, no ASUS or OLED configuration change.
3. **Read-only installer tooling stays read-only.** `tools/check-requirements.ps1`
   and `tools/verify-install.ps1` may query, hash, list and connect; they may not
   install, download, write settings or start anything. Their exit codes are
   documented in each header and asserted by `tools/test-install-scripts.ps1`.
4. **No publishing from a working session.** No remote, no push, no tag, no
   release without the maintainer's explicit instruction.
5. **No personal data in the tree** — no user names, absolute host paths (use
   `C:\path\to\...`), hostnames, tokens or screenshots of real conversations.
6. **Tests and docs tell the truth.** A claim is *tested*, *measured once* or
   *unverified*; the word appears in the sentence with the number. If you did not
   run it, say so.

## 8. Contribution guidelines

- Conventional commits, scoped to the layer: `fix(dev): ...`, `feat(core): ...`,
  `docs(handoff): ...`. One logical change per commit; prose body, present tense.
- Match the surrounding style: Rust with `cargo fmt` defaults and sentence-shaped
  error messages, React function components with CSS in `src/styles/`, PowerShell
  with comment-based help and `$ErrorActionPreference = 'Stop'`.
- Documentation is part of the change: a user-visible behaviour change updates
  `CONFIGURATION.md` or `TROUBLESHOOTING.md` in the same commit, and a new
  measured number is quoted with its condition.
- Generated files are not hand-edited: `DEPENDENCIES.md` comes from
  `node tools/licence-inventory.mjs`, the JPEGs in `screenshots/` from
  `tools/optimize-shots.ps1`.
- Keep the idle cost. No ambient animation loop, no new polling the hardware has
  to pay for; the figures in [README.md](README.md) §9 are the reason the window
  is cheap.
- Update [HANDOFF.md](HANDOFF.md) when the milestone changes; if it disagrees with
  the code, that is a bug.
