# RTX Spark AI Control V2

A native Windows ARM64 control center for the existing RTX Spark N1X inference
environment: Qwen Flash Next served by vLLM on `127.0.0.1:8000`, plus the validated V1
blackout engine.

V2 now **manages** that environment — it can start, stop and restart vLLM, supervise the
blackout engine, and arm blackout automatically while the server is healthy. V1
(`C:\Users\cohen\AI\RTX-Spark-AI-Control`) is untouched and remains installed as the
fallback controller.

```
Home  Server  Blackout  Settings  Logs
```

---

## 1. What it does

| Area | Capability |
| --- | --- |
| Inference server | Start, Stop, Restart, and *adopt* a server it did not launch; live health, listener pid, uptime, model identity, endpoint, weight-loading progress, output tail |
| Ownership | Only processes this application started are ever stopped. Adoption records pid + creation time, so a stop can never hit a recycled pid |
| Blackout | Blackout Now, Stop Blackout, Automatic Blackout, inactivity timeout (180 s default), cover mode, display-request and session state |
| Follow Server | Arms blackout only while the server stays healthy (60 s hysteresis), stands it down after a 300 s failure grace, re-arms after input |
| Logs | Server lifecycle, blackout transitions, warnings and errors in a 400-entry ring with identical-line collapsing; the managed server's own output tail |
| Settings | UI preferences (persisted locally), backend telemetry cadence (persisted in `%APPDATA%`), read-only view of the inference and automation configuration |
| Telemetry | GPU utilisation, VRAM, temperature, power, SM clock, throttle reasons; API latency; process tree facts |

Deliberately absent: no autostart, no service, no scheduled task, no background daemon. The
app does what it is asked while it runs, and stops its own children when it exits.

**The blackout overlay is not a lock screen.** It is an ordinary topmost window on an
unlocked desktop that paints RGB(0,0,0) over the OLED panel and holds a display power
request. Windows authentication is unchanged; anyone in front of the machine can still reach
the desktop with an input event. Use `Win+L` when a real security boundary is needed.

---

## 2. Stack and why

| Layer | Choice | Reason |
| --- | --- | --- |
| Shell | Tauri 2.12 | Native ARM64 binary, uses the WebView2 runtime already on the machine, no Chromium bundle |
| Backend | Rust (`aarch64-pc-windows-msvc`) | Process/window/GPU probing and process supervision without spawning consoles |
| Frontend | React 18 + TypeScript + Vite 5 | Component model without a UI kit; CSS does the visual work |
| HTTP | `ureq` 2.12 (no default features) | Loopback HTTP only, small dependency surface |
| Windows APIs | `windows-sys` 0.59 | TCP table, process facts, Toolhelp snapshots, named mutex, input tick; command lines via `NtQueryInformationProcess` |

Toolchain: Rust 1.98.0, Node 26.13.1, Visual Studio 2022 BuildTools (ARM64 MSVC), Windows SDK
10.0.26100, WebView2 runtime 157.0.4341.46.

---

## 3. Architecture

```
React (WebView2)
  │  invoke('snapshot', {lastEventId})   ── one in-flight call, next queued on resolve
  ▼
Tauri commands ── spawn_blocking ──► Core (Mutex<State>)
  │                                   ├── vllm::Vllm      launch/stop/adopt/health/log tail
  │                                   ├── blackout::Engine V1 engine supervision
  │                                   ├── settings         Settings + OwnedServer, atomic JSON
  │                                   └── events::EventRing 400 entries, 5 s line collapse
  ▼
Supervisor thread (1 s tick)
  ├── health probe        GET /v1/models  (3 s while starting/loading/stopping, else poll_s)
  ├── input tick          GetLastInputInfo → idle anchor
  ├── Follow Server       60 s hysteresis, 300 s grace, arm/stand-down of the guard
  └── phase + reason      the text the Blackout page shows
  ▼
nvidia-smi --query-gpu …  (CREATE_NO_WINDOW, ~70–90 ms, on sample)
```

Design rules that keep it cheap and safe:

* **Polling is frontend-driven and serial.** A new `setTimeout` is scheduled only after
  `invoke` resolves, so a slow `nvidia-smi` cannot queue work.
* **The backend owns state, the frontend owns presentation.** Events travel by cursor
  (`lastEventId`), so a hidden or restarted window never replays the ring.
* **Stop is two-phase.** The decision (which pid, still the same process?) is made under the
  lock; the terminate call happens outside it, then the result is applied. The UI never
  blocks on a kill.
* **Ownership is proven, not assumed.** A server is stopped only if V2 launched it, or if the
  operator adopted it and the recorded pid + creation ticks still match.
* **Server state comes from three facts**, not one: does something listen on the port, does
  `/v1/models` answer, and is the listener ours.

### Source layout

```
src-tauri/src/main.rs        commands, single-instance guard, shutdown cleanup
src-tauri/src/core.rs        shared state, ActionResult, settings persistence, snapshots
src-tauri/src/vllm.rs        launch/stop/restart/adopt, health, log parsing, readiness
src-tauri/src/blackout.rs    V1 engine supervision, STOP sentinel, state.json
src-tauri/src/supervisor.rs  1 Hz loop: probes, idle anchoring, Follow Server automation
src-tauri/src/settings.rs    Settings + OwnedServer, atomic writes under %APPDATA%
src-tauri/src/events.rs      capped event ring with repeat collapsing
src-tauri/src/telemetry.rs   GPU, model, system snapshot
src-tauri/src/winproc.rs     TCP table, process facts, command lines, tree terminate, input tick
src/App.tsx                  routing, action runner, status pill, toast
src/lib/{api,types,useTelemetry,useAction,format}.ts
src/pages/*.tsx              Home, Server, Blackout, Settings, Logs
src/components/*.tsx         shell, brand mark, cards, pills, sparkline, rows, event list
src/styles/*.css             tokens/field, glass surfaces, layout + breakpoints
tools/                       measurement, capture, QA tooling (never shipped)
```

---

## 4. Build and run (Windows ARM64)

```powershell
# once
winget install --id Rustlang.Rustup
rustup target add aarch64-pc-windows-msvc
npm install

# every build
cd C:\Users\cohen\AI\RTX-Spark-AI-Control-V2
npm run app:build      # tsc --noEmit && vite build && tauri build --no-bundle
.\run.ps1              # starts src-tauri\target\release\rtx-spark-ai-control-v2.exe
```

**Build through the Tauri CLI, not bare `cargo build`.** A `cargo build --release` binary
compiles and links but carries the dev URL (`http://localhost:1420`), which is not running,
so the window comes up black. `tauri build` embeds `dist/`.

**One instance at a time.** A second launch finds the named mutex
`Local\RTXSparkAIControlV2` held, restores the existing window and exits 0.

There is no installer, service, scheduled task or startup entry.

---

## 5. Using it

### Home

Model status, the primary Start/Stop/Restart controls, current model and endpoint, GPU
telemetry with a 60-sample sparkline, blackout and automation status, and the most recent
meaningful events.

### Server

Start, Stop, Restart and Take ownership, with health, listener pid, uptime, ownership, stage
and the last 12 lines of the managed process's output. When a launch fails the stage reads
`exited with code N` and the traceback is in the log tail and the Logs page — nothing is left
half-started.

* **Existing server?** V2 detects it, shows it as `external — left alone until adopted`, and
  disables Stop and Restart. *Take ownership* records its pid and creation time; from then on
  Stop and Restart apply to that exact process, and nothing else.
* **Progress** is only shown when vLLM prints a percentage (weight-loading bars). Readiness
  always comes from `/v1/models`, never from a percentage.

### Blackout

Blackout Now, Stop Blackout, the Automatic Blackout switch, the Follow Server switch, the
inactivity timeout (30 s, 1 m, 3 m, 5 m, 15 m), the cover mode (internal / primary / all) and
live engine state: guard pid and owner, display request, idle countdown, session lock state,
STOP sentinel, state-file timestamp.

What it preserves from the validated engine: internal OLED-only coverage by default, pure
RGB(0,0,0) output, an `ES_DISPLAY_REQUIRED` display power request for as long as the panel
is covered (so the machine does not drop into Modern Standby and CUDA keeps its clocks), and
immediate recovery on the first input event. Sessions that lock release the request by
design — the engine refuses to hold a panel for a locked session.

### Follow Server

1. Server healthy (started here or adopted, or merely detected).
2. `Follow Server` on + `Automatic blackout` on → once the server has been healthy for the
   hysteresis window, V2 arms the guard.
3. After a full inactivity interval (180 s by default) the panel is covered.
4. The first mouse or keyboard event restores the desktop and re-anchors the idle clock.
5. If the server stops answering for longer than the grace window, V2 stands the guard down
   and disarms; brief probe failures do not flash the panel.
6. Manual Blackout Now works regardless of Follow Server.

### Settings

Interface refresh cadence, suspend-polling-while-hidden, reduce motion (the OS setting is
honoured regardless), backend telemetry interval, and read-only views of the automation and
inference configuration.

### Logs

Session events (server lifecycle, blackout transitions, warnings, errors) with repeat counts,
the managed server's output tail, and a runtime card: host, app version, target, endpoint,
server state, ownership, blackout state, last action, backend notes.

---

## 6. Files V2 writes

| Path | What |
| --- | --- |
| `%APPDATA%\com.rtxspark.aicontrol.v2\settings.json` | blackout/automation preferences and poll cadence |
| `%APPDATA%\com.rtxspark.aicontrol.v2\runtime-state.json` | the owned/adopted server record (pid + creation ticks), so ownership survives a restart |
| `%TEMP%\rtx-v2-startup.json` | startup timing, used by the footer counters |
| `RTX-Spark-AI-Blackout\state.json`, `STOP` | written by the V1 engine, read (and the sentinel written) by V2 |
| Browser profile | `%LOCALAPPDATA%\com.rtxspark.aicontrol.v2` (localStorage: UI prefs, last page) |

Nothing else is touched. V1's `config.json` and `runtime-state.json` are read-only to V2.

---

## 7. Measured results

Environment: Windows ARM64, RTX Spark N1X, 128 GB unified memory, NVIDIA driver 621.14,
ASUS Manual power profile, 96 GB GPU allocation, vLLM serving Qwen/Qwen3.8-Flash-Next
(MTP 4, FP8 KV cache, prefix caching, 409600 ctx).

Harness: `tools/measure.py` walks the process tree (including `msedgewebview2.exe` children)
and reports CPU as **percent of one core**, the unit V1's 64 % baseline used.

| Condition | CPU (mean) | Median | RSS | Notes |
| --- | --- | --- | --- | --- |
| V1 (baseline, earlier audit) | **64 %** | — | — | Tk repaint loop |
| V2 idle, before fixes | 85.2 % | 85.6 % | 470 MiB | infinite `breathe` animation + blur on every card |
| V2 idle, animations off (probe) | 12.97 % | 12.05 % | 471 MiB | isolates the animation cost |
| V2 idle, animations + blur off (probe) | 7.86 % | 6.40 % | — | isolates the blur cost |
| **V2 Phase 1, shipped, 1500×950** | **7.84 %** | 7.00 % | 451 MiB | 8.2× below V1 |
| V2 idle, minimised (polling suspended) | 0.47 % | 0.00 % | — | 7 processes alive, no sampling |
| V2 idle, 2400×1400 logical | 11.38 % | 12.25 % | — | 2.56× the painted area |
| **V2 Phase 2, full control center** | **5.88 / 5.37 %** | 4.20 % | 451–460 MiB | supervisor + events added, no regression |

Other numbers:

| Metric | Value |
| --- | --- |
| Release binary | 4.92 MB |
| Frontend bundle | 186.5 kB JS (58.1 kB gzip), 18.1 kB CSS |
| API round trip | 18 ms mean at idle, 3–4 ms on the health probe |
| Idle GPU utilisation / power | 0.05–0.16 % / 1.55 W |
| Processes in tree | 7 |
| Mock start → ready | 2.52 s; restart → ready 3.02 s |

**The idle-cost defect and its fix (Phase 1).** Two causes, each confirmed by probe: an
infinite CSS `breathe` animation forced the compositor to run at 60 Hz forever (disabling
animation alone: 85.2 % → 12.97 %), and `backdrop-filter: blur(26px)` on every card re-blurred
each surface per paint (removing it from cards, keeping it only on the rail: → 7.84 %). The
pulse was replaced with a static halo ring; the visual difference is negligible.

**Inference contention (Phase 1).** BF16 8192³ GEMM, GUI closed vs visible, interleaved:
83.24 TFLOPS visible vs 82.23 closed — inside the machine's own ±5 TFLOPS spread. Generation
through the real server: 36.8–43.6 tok/s with a larger spread than any difference between
conditions.

---

## 8. Visual design

Original cobalt/electric-blue interface influenced by the *visual language* of Nous Research
(large editorial type, generous spacing, monochrome technical texture) with no copied marks,
wording or layout.

* Static paint only: gradient field, halftone dot screen masked to the upper-left, SVG film
  grain at 5 % opacity, one blurred aurora sheet, glass surfaces with a 1 px lit top edge.
* The only motion is a 260 ms page fade-in and 140–400 ms hover/meter transitions —
  event-driven, never continuous.
* `prefers-reduced-motion` and an in-app switch both disable motion entirely.
* Breakpoints: 1320, 1120, 1040 (rail becomes a top bar), 760, 560.
* Fonts: Segoe UI Variable + Cascadia Mono. No webfont download, no layout shift.
* Secondary label contrast was raised for Phase 2 (`--ink-dim` 0.66, `--ink-faint` 0.46) on
  the Settings and Blackout pages. No colour, layout or identity changes.

**Application icon.** A four-point spark of refractive cobalt glass with an electric-blue
inner glow, on a rounded cobalt tile — readable at 16 px, no branding borrowed from NVIDIA or
Nous Research. Source: `tools/make-icon.py`; outputs `src-tauri/icons/icon.ico`
(16/24/32/48/64/128/256 at 32 bpp) plus the PNG set Tauri expects. The same geometry is drawn
inline for the sidebar mark (`BrandMark` in `src/components/Shell.tsx`), so title bar, taskbar,
executable and sidebar agree. Candidate designs, at six sizes each, are kept in
`assets/icon-candidates/` with `contact-sheet.png`; `spark-glass` was chosen for small-size
legibility.

Screenshots in `screenshots/` are captured with CDP `Page.captureScreenshot` from the live
webview at its real device scale factor (150 % DPI). Pages taller than the window have a
`-b.png` companion captured after scrolling the content area. The native caption bar is not
part of that surface and is therefore not in the images.

---

## 9. Known limitations

* **The overlay is not a lock screen.** It hides the display, not the session.
* **Blackout needs an unlocked session.** The engine stands down while the session is locked;
  that is the validated V1 behaviour, and it is why the overlay tests in
  `TEST-RESULTS.md` are marked NOT TESTED (the test machine was locked).
* **Progress percentages depend on vLLM's output.** vLLM prints weight-loading bars; if a
  build does not, the Server page shows readiness only, never a guessed number.
* **`nvidia-smi` is one sample, ~70–90 ms**, at the poll interval. It cannot show clock
  transitions between samples.
* **Event ring and sparkline are in-memory** (400 events, 60 points) and are lost on exit.
  Nothing is written to disk for them on purpose.
* **Adoption is conservative.** V2 only widens ownership to a parent whose command line names
  `vllm.exe` or `vllm.entrypoints`. A server launched as a bare `python -m …` module is
  adopted as its listener alone; stopping it then takes that process, not an unknown parent.
* **Raising the window is best-effort** (Windows' foreground lock). A duplicate launch always
  restores a minimised window; the taskbar button always works.
* **4K cost**: idle CPU rises to ~11 % of a core at 3600×2100 because each repaint covers
  2.56× the pixels.
* **Debug flags are opt-in only.** CDP tooling needs
  `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`; the app never sets it
  and listens on nothing.

---

## 10. Testing

`TEST-RESULTS.md` holds the item-by-item evidence for Phase 2, including the tests that could
not be executed (`NOT TESTED`, with the reason and the exact commands to run them). The
scripted lifecycle test is repeatable:

```powershell
# V2 with the QA hooks, mock server on 8011 — production is never touched
$env:V2_TEST_PORT='8011'
$env:V2_TEST_VLLM_EXE='C:\Users\cohen\AI\native-vLLM\.venv\Scripts\python.exe'
$env:V2_TEST_VLLM_ARGS='C:\Users\cohen\AI\RTX-Spark-AI-Control-V2\tools\mock-vllm.py {port}'
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS='--remote-debugging-port=9222'
.\src-tauri\target\release\rtx-spark-ai-control-v2.exe
python tools\qa-lifecycle.py
```

Those three `V2_TEST_*` variables are QA hooks: with them unset the app reads V1's
`config.json` and launches exactly the optimized command it always has.

---

## 11. Rollback / uninstall

Nothing outside the project directory and `%APPDATA%` was modified, so rollback is deletion:

```powershell
Stop-Process -Name rtx-spark-ai-control-v2 -ErrorAction SilentlyContinue
# optional: remove the app's own state so a reinstall starts clean
Remove-Item -Recurse -Force "$env:APPDATA\com.rtxspark.aicontrol.v2"
Remove-Item -Recurse -Force "$env:LOCALAPPDATA\com.rtxspark.aicontrol.v2"
Remove-Item "$env:TEMP\rtx-v2-startup.json"
# then the project itself
Remove-Item -Recurse -Force C:\Users\cohen\AI\RTX-Spark-AI-Control-V2
```

V1 keeps working throughout: it never depended on V2, and V2 never writes V1's files. vLLM,
the model checkpoint, the NVIDIA driver, the ASUS profile, the 96 GB GPU allocation,
`FN-Optimization`, `RTX-Spark-AI-Blackout` and V1 are unaffected. If V2 was running with an
owned server, stop it from the UI first — closing the window stops the blackout engine it
armed, and leaves the inference server running on purpose (see `RunEvent::Exit` in
`main.rs`).

---

## 12. Tooling

| File | Purpose |
| --- | --- |
| `tools/measure.py` | idle CPU/RSS/GPU sampling for the whole process tree, optional minimize |
| `tools/contention.py` | BF16 8192³ GEMM probe with per-rep TFLOPS and SM clock |
| `tools/infer-probe.py` | single-stream generation throughput against the live server |
| `tools/mock-vllm.py` | stand-in `vllm serve` for lifecycle tests (progress bars, `/v1/models`, `--fail`) |
| `tools/qa-lifecycle.py` | scripted Start/Restart/Stop/adopt run against the mock, prints PASS/FAIL |
| `tools/cdp.py` | CDP eval/click/screenshot (needs `--remote-debugging-port=9222`) |
| `tools/shots.py` | captures every page, including the part below the fold |
| `tools/capture.py` | `PrintWindow` window capture, for when the panel is lit |
| `tools/winsize.py` | resize the live window to a logical size |
| `tools/make-icon.py` | generates the icon set and the candidate contact sheet |
| `tools/ps/*.ps1` | process listing and polite window close for QA |
