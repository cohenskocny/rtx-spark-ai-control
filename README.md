# RTX Spark AI Control V2

A native Windows ARM64 dashboard for the existing RTX Spark N1X inference environment:
Qwen Flash Next served by vLLM on `127.0.0.1:8000`, plus the validated V1 blackout engine.

This is **V2 of a separate application**. V1 (`C:\Users\cohen\AI\RTX-Spark-AI-Control`) is
untouched and remains the working controller. V2 currently **observes only**: it reads the
vLLM API, GPU telemetry via `nvidia-smi`, and the V1/blackout configuration and state files.
It does not start, stop or reconfigure anything, does not create display power requests, and
does not paint overlays.

```
Home  Server  Blackout  Settings  Logs
```

---

## 1. Stack and why

| Layer | Choice | Reason |
| --- | --- | --- |
| Shell | Tauri 2.12 | Native ARM64 binary, uses the WebView2 runtime already on the machine, no Chromium bundle |
| Backend | Rust (`aarch64-pc-windows-msvc`) | Process/window/GPU probing without spawning consoles |
| Frontend | React 18 + TypeScript + Vite 5 | Component model without a UI kit; CSS does the visual work |
| HTTP | `ureq` 2.12 (no default features) | Loopback HTTP only, small dependency surface |
| Windows APIs | `windows-sys` 0.59 | `GetExtendedTcpTable`, `OpenProcess`, `QueryFullProcessImageNameW` |

Rejected: Electron (150 MB+ runtime and an extra renderer per window), Tkinter (the V1
approach; 60 FPS repaint loop, 64 % of a core at idle), Avalonia (extra managed runtime
and XAML tooling for a dashboard that is mostly typography and gradient surfaces).

Toolchain used: Rust 1.98.0, Node 26.13.1, Visual Studio 2022 BuildTools with the ARM64
MSVC tools, Windows SDK 10.0.26100, WebView2 runtime 157.0.4341.46.

---

## 2. Architecture

```
React (WebView2)
  │  invoke('snapshot')            ── one in-flight call at a time, next queued on resolve
  ▼
Tauri command handler
  │  spawn_blocking
  ▼
telemetry::collect(seq)            ── single snapshot, no shared mutable state
  ├── HTTP GET /v1/models          ── ureq, 1.2 s timeout, loopback
  ├── nvidia-smi --query-gpu …     ── CREATE_NO_WINDOW, ~70–90 ms
  ├── GetExtendedTcpTable          ── listener pid on port 8000
  ├── OpenProcess + GetProcessTimes ── process name, uptime
  ├── V1 config.json               ── model + blackout + automation settings
  └── blackout state.json + STOP   ── engine phase, display request, last write
```

* **Polling is frontend-driven and serial.** A new `setTimeout` is only scheduled after
  `invoke` resolves, so a slow `nvidia-smi` cannot queue up work.
* **Visibility watcher** (Rust thread, 1 s): emits `v2://visibility` only when
  minimised/visible/focused changes. The frontend stops the loop while hidden and takes
  one fresh sample on restore.
* **Server state** is derived from three facts, not one: does something listen on 8000
  (TCP table), does `/v1/models` answer, and how long has that been true. A socket without
  an answer is `loading` for 15 minutes, then `error`.
* **GPU telemetry** is a single `nvidia-smi` call — utilisation, VRAM, temperature, power,
  SM clock and `clocks_throttle_reasons.active` (the same query V1 and the FN-Optimization
  benchmark scripts use, so numbers stay comparable).

### Source layout

```
src-tauri/src/main.rs        app state, snapshot command, visibility watcher, cleanup
src-tauri/src/telemetry.rs   snapshot model + collection (API, GPU, config, blackout)
src-tauri/src/winproc.rs     TCP listener lookup, process name/uptime
src/App.tsx                  page routing, ready report, status pill
src/lib/useTelemetry.ts      serial polling loop, event ring, visibility pause
src/pages/*.tsx              Home, Server, Blackout, Settings, Logs
src/components/*.tsx         shell, cards, pills, sparkline, rows, glyphs
src/styles/*.css             tokens/field, glass surfaces, layout + breakpoints
tools/                       measurement and capture tooling (not shipped)
```

---

## 3. Build and run (Windows ARM64)

```powershell
# once
winget install --id Rustlang.Rustup           # rustup target list --installed
rustup target add aarch64-pc-windows-msvc
npm install

# every build
cd C:\Users\cohen\AI\RTX-Spark-AI-Control-V2
npm run app:build      # tsc --noEmit && vite build && tauri build --no-bundle
.\run.ps1              # starts src-tauri\target\release\rtx-spark-ai-control-v2.exe
```

**Build through the Tauri CLI, not bare `cargo build`.** A `cargo build --release` binary
compiles and links, but it carries the dev URL and loads `http://localhost:1420`, which is
not running — the window comes up black. `tauri build` sets the flags that embed `dist/`.
This was the blank-window failure during development, and it is the first thing to check if
a rebuilt binary shows nothing.

Dev loop: `npm run dev` in one terminal, `npx tauri dev` in another.

**One instance at a time.** Starting the binary again while it runs is a no-op: the second
process finds the named mutex `Local\RTXSparkAIControlV2` already held, restores the window
that is already open and exits 0. Without that guard the second process aborts inside the
WebView2 loader (`0xC0000409`, silent — the binary has no console — logged as a `BEX64`
entry in Event Viewer) because two instances cannot share one WebView2 profile.

There is no installer, service, scheduled task or startup entry. Nothing is written outside
the application directory except `%TEMP%\rtx-v2-startup.json` (startup timing).

---

## 4. Measured results

Environment: Windows ARM64, RTX Spark N1X, 128 GB unified memory, NVIDIA driver 621.14,
ASUS Manual power profile, 96 GB GPU allocation, vLLM serving Qwen/Qwen3.8-Flash-Next
(pid 10644, `127.0.0.1:8000`, MTP 4, FP8 KV cache, prefix caching).

Harness: `tools/measure.py` walks the process tree (including `msedgewebview2.exe` children)
and reports CPU as **percent of one core**, the same unit V1's 64 % baseline used.

| Condition | CPU (mean) | CPU (median) | RSS | Notes |
| --- | --- | --- | --- | --- |
| V1 (baseline, from the earlier audit) | **64 %** | — | — | Tk repaint loop |
| V2 idle, 1500×950, 2 s poll, before fixes | 85.2 % | 85.6 % | 470 MiB | infinite `breathe` animation + blur on every card |
| V2 idle, animations disabled (probe) | 12.97 % | 12.05 % | 471 MiB | isolates the animation cost |
| V2 idle, animations + blur disabled (probe) | 7.86 % | 6.40 % | — | isolates the blur cost |
| **V2 idle, shipped build, 1500×950** | **7.84 %** | **7.00 %** | 451 MiB | 8.2× lower than V1 |
| V2 idle, minimised (polling suspended) | 0.47 % | 0.00 % | — | 7 processes alive, no sampling |
| V2 idle, 2400×1400 logical (3600×2100 px) | 11.38 % | 12.25 % | — | 2.56× the painted area |

Other measurements:

| Metric | Value | Method |
| --- | --- | --- |
| Startup to first paint | 535 ms first run, 388 ms warm | `frontend_ready` → `%TEMP%\rtx-v2-startup.json` |
| Release binary | 4.67 MB | `ls src-tauri\target\release\*.exe` |
| Frontend bundle | 173 kB JS (54.7 kB gzip), 16.9 kB CSS | `vite build` |
| API round trip | 13–18 ms mean | app's own `/v1/models` sampling |
| Idle GPU utilisation | 0.16 % mean, 1.52 W | `measure.py` |
| Processes in tree | 7 | Rust process + WebView2 |

**The idle-cost defect and its fix.** The first build measured 85 % of a core — worse than
V1. Two causes, both confirmed by probe rather than guessed:

1. An infinite CSS `breathe` animation on the "live" status dot. With `backdrop-filter`
   surfaces, that keeps the whole window compositing at 60 Hz forever. Disabling animations
   alone dropped idle CPU to 12.97 %.
2. `backdrop-filter: blur(26px)` on every card forced a full re-blur of each surface per
   sample. Removing it from cards (keeping it only on the rail, which sits over the
   brightest part of the field) took the shipped build to 7.84 %.

The pulse was replaced with a static halo ring. Visual difference: negligible — the field
behind the cards is a smooth gradient, so the blur was mostly invisible cost.

**Inference contention.** BF16 8192³ GEMM, 6 reps per condition, GUI closed vs visible,
interleaved (A/B/A/B):

| Condition | median TFLOPS | min | max | median SM clock |
| --- | --- | --- | --- | --- |
| GUI closed (a) | 83.64 | 82.47 | 86.15 | 2324 MHz |
| GUI visible (a) | 83.13 | 81.46 | 85.71 | 2320 MHz |
| GUI closed (b) | 80.82 | 75.80 | 83.41 | 2307 MHz |
| GUI visible (b) | 83.34 | 81.23 | 85.13 | 2327 MHz |

No measurable contention: visible averaged 83.24 TFLOPS, closed 82.23. The ±5 TFLOPS spread
is the machine's own clock behaviour, not the GUI. (This is the same platform whose clocks
collapse to ~750 MHz when the panel powers down — see FN-Optimization.)

Single-stream generation through the real vLLM server, 3 requests of 128 tokens each:

| Condition | tokens/s median |
| --- | --- |
| GUI visible | 43.55 |
| GUI closed | 39.84 |
| GUI visible (repeat) | 36.81 |

Spread (36.8–43.6) is larger than any difference between conditions; the GUI is not the
cause. Reported as-is rather than as a "win".

---

## 5. Visual design

Original cobalt/electric-blue interface influenced by the *visual language* of Nous Research
(large editorial type, generous spacing, monochrome technical texture) with no copied
marks, wording or layout.

* Static paint only: gradient field, halftone dot screen masked to the upper-left, SVG film
  grain at 5 % opacity, one blurred aurora sheet, glass surfaces with a 1 px lit top edge.
  No ambient animation of any kind.
* The only motion is a 260 ms page fade-in and 140–400 ms hover/meter transitions —
  event-driven, never continuous.
* `prefers-reduced-motion` and an in-app reduce-motion switch both disable motion entirely.
* Breakpoints: 1320 (hero stacks), 1120 (KPIs 2-up), 1040 (rail becomes a horizontal top
  bar), 760, 560. Verified at 980×660 and 2400×1400 logical.
* Fonts are the system UI stack (Segoe UI Variable) plus Cascadia Mono — no webfont
  download, no layout shift.

Screenshots in `screenshots/` are captured with CDP `Page.captureScreenshot` from the live
webview at its real device scale factor (150 % DPI), so they are the renderer's own output.
The native caption bar is not part of that surface and therefore not in the images.

---

## 6. Known limitations

* **Read-only.** No Start/Stop/Restart, no blackout activation, no Follow Server behaviour.
  Every such control is present in the UI but disabled and labelled `Phase 2`.
* **No process ownership yet.** V2 knows which PID owns port 8000 but must not touch it. The
  ownership model (only kill what this app spawned) arrives with lifecycle support.
* **`nvidia-smi` is one sample, ~70–90 ms**, at the poll interval. It cannot show clock
  transitions between samples; the FN-Optimization scripts exist for that.
* **History is in-memory and capped** (60 sparkline points, 80 events) and is lost on exit.
* **Loading vs error** uses a 15-minute threshold; a genuinely stuck server looks `loading`
  for that long before it reads `attention`.
* **Config is read, not validated.** Values come from V1's `config.json`; V2 displays what
  is there.
* **Raising the window is best-effort.** A duplicate launch always restores a minimised
  window, but Windows' foreground lock may refuse to push it in front when the caller was
  not itself started from the foreground. Clicking the taskbar button always works.
* **4K cost**: idle CPU rises to ~11 % of a core at 3600×2100 because each repaint covers
  2.56× the pixels. Still 5.6× below V1.
* **Debug flags are opt-in only.** The capture tooling needs
  `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`; the app never sets
  it and listens on nothing.

---

## 7. Migration plan for V1

Order chosen so that V1 stays usable as the fallback at every step:

1. **Read-only telemetry** — done (this build).
2. **Process ownership and lifecycle.** V2 spawns vLLM itself from the existing optimized
   YAML and records the PID; stop/restart only touch a PID V2 created. An already-running
   server started outside V2 is adopted read-only and cannot be stopped from the UI.
3. **Blackout supervision.** Wrap `blackout.py` as a supervised child, reuse its
   `--cover internal` and power-request behaviour unchanged, expose Blackout Now / Stop.
4. **Follow Server automation.** Port V1's 60 s health hysteresis and 300 s failure grace,
   with blackout released on confirmed shutdown.
5. **Cutover.** Only after 2–4 pass the acceptance tests does V2 become the primary UI; V1
   stays installed and unmodified.

## 8. Rollback / uninstall

Nothing outside the project directory was modified, so rollback is deletion:

```powershell
Stop-Process -Name rtx-spark-ai-control-v2 -ErrorAction SilentlyContinue
Remove-Item -Recurse -Force C:\Users\cohen\AI\RTX-Spark-AI-Control-V2
Remove-Item "$env:TEMP\rtx-v2-startup.json"
# optional: the browser profile Tauri created for this identifier
Remove-Item -Recurse -Force "$env:LOCALAPPDATA\com.rtxspark.aicontrol.v2"
```

vLLM, the model checkpoint, the NVIDIA driver, the ASUS profile, the 96 GB GPU allocation,
`FN-Optimization`, `RTX-Spark-AI-Blackout` and V1 are unaffected.

## 9. Tooling

| File | Purpose |
| --- | --- |
| `tools/measure.py` | idle CPU/RSS/GPU sampling for the whole process tree, optional minimize |
| `tools/contention.py` | BF16 8192³ GEMM probe with per-rep TFLOPS and SM clock |
| `tools/infer-probe.py` | single-stream generation throughput against the live server |
| `tools/cdp.py` | CDP eval/click/screenshot (needs `--remote-debugging-port=9222`) |
| `tools/capture.py` | `PrintWindow` window capture, for when the panel is lit |
| `tools/winsize.py` | resize the live window to a logical size |
