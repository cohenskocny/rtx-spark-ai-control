# TEST-RESULTS — V2 Phase 2 (functional control center)

Built and tested on 2026-11-18, 05:35–06:45 UTC (local 23:35–00:45, 2026-11-17/18).
Every row below was executed against the real application unless it says **NOT TESTED**.

| Item | Value |
| --- | --- |
| Binary | `src-tauri\target\release\rtx-spark-ai-control-v2.exe`, 4,923,392 B, built 00:40 local |
| Frontend bundle | 186.5 kB JS (58.1 kB gzip), 18.1 kB CSS |
| Target | `aarch64-pc-windows-msvc`, Windows 11 ARM64, WebView2 157.0.4341.46 |
| Model server | `Qwen/Qwen3.8-Flash-Next`, pid 10644, `127.0.0.1:8000`, MTP 4, FP8 KV, 409600 ctx |
| Session state during testing | **Windows session was locked** for the whole run (LogonUI pid 28344) |

The locked session matters: the validated blackout engine deliberately stands down while a
session is locked, so the overlay/activation rows below could not be executed and are marked
accordingly instead of being claimed.

---

## 1. Required verification, item by item

| # | Requirement | Verdict | Evidence |
| --- | --- | --- | --- |
| 1 | Builds and launches natively on Windows ARM64 | **PASS** | `npm run app:build` exit 0 four times this session (25.9–26.7 s incremental), `tsc --noEmit` clean, no compiler warnings; `run.ps1` launches the release binary; window serves the embedded frontend (`http://tauri.localhost/`) |
| 2 | Existing vLLM server detected correctly | **PASS** | Endpoint `http://127.0.0.1:8000/v1`, `GET /v1/models` 200 in 3–4 ms, listener pid 10644, uptime 9 h 6 m, ownership `external — left alone until adopted`, State `ready · healthy` |
| 3 | Start/Stop/Restart work with ownership safeguards | **PASS** | `tools/qa-lifecycle.py` against the mock: Start ready in **2.52 s**, Restart ready in **3.02 s** with a new pid, Stop left the port free; see §2 |
| 4 | Loading and ready states displayed accurately | **PASS** | During launch the Server page shows stage `weights`/`serving` with the percentage vLLM actually printed; an adopted server no longer shows a fabricated 100 %; ready caption is `Ready — /v1/models answered in 3 ms` |
| 5 | Blackout Now works | **PASS (start path), overlay NOT TESTED** | Clicking Blackout now started the engine (pid 34384, then 31884 in a later run); `state.json` updated within 1 s to `active:false, locked:true, display_request:true`; event "Blackout forced on (cover=internal, pid …)" reached the Logs page. The engine then refused to hold the display because the session was locked — correct V1 behaviour, not a defect |
| 6 | Stop Blackout works | **PASS** | STOP sentinel written, engine exited within 1 s, `state.json` = `{active:false, stopped:true}`, display request released, no `pythonw.exe` left behind, no `STOP` file left behind |
| 7 | Automatic blackout after ~180 s of inactivity | **NOT TESTED** | Blocked by the locked session. The engine's own earlier log (04:00 UTC) shows the mechanism working at the configured threshold: `idle trigger: effective_idle=180.1s threshold=180s system_idle=294.1s`. Re-run instructions in §6 |
| 8 | Mouse/keyboard input restores the desktop | **NOT TESTED** | Requires an unlocked session (same blocker) |
| 9 | Follow Server responds to health transitions | **PASS** | With QA settings `hysteresis_s=10, grace_s=20, poll_s=5` and the mock on 8011: phase `AUTO BLACKOUT · WAITING-SERVER` while down → `ARMED` with guard pid 37236 after the hysteresis → after stopping the server, stand-down at the grace deadline (`state.json` stopped 06:00:04, no guard process, phase back to `WAITING-SERVER`) |
| 10 | Settings persist between launches | **PASS** | Telemetry interval changed to 30 s → `%APPDATA%\com.rtxspark.aicontrol.v2\settings.json` written (`poll_s: 30`); after relaunch the 30 s segment renders `data-on="true"`; UI prefs (interval, reduce motion, polling-when-hidden) persist in `localStorage` |
| 11 | GPU telemetry accurate | **PASS** | App `0 %, 94.3 GiB of 96.0 GiB, 36 °C, 1.4 W, 916 MHz, reliability` vs `nvidia-smi` `4 %, 96425 MiB of 98304, 35.79 °C, 1.65 W, 923 MHz, 0x400` (sampled seconds apart) |
| 12 | Interface responsive during heavy inference | **NOT TESTED** | Deferred deliberately: generating load means sending requests to the production server, which needs the owner's approval. Command in §6 |
| 13 | No unnecessary Modern Standby during blackout | **NOT TESTED** | Requires an active, unlocked blackout |
| 14 | Qwen checkpoint and launch configuration unchanged | **PASS** | V1 `config.json` MD5 `7e2c65400c2011a80a373b4320f24cab`, V1 `runtime-state.json` MD5 `99914b932bd37a50b983c5e7c90ae93b` (2 bytes, `{}`), unchanged; the launch command is V1's verbatim (`--config …qwen3.8-flash-next-optimized.yaml --chat-template …nonleading-system.jinja --port 8000`); nothing in the checkpoint directory was written |
| 15 | Resource usage measured before and after | **PASS** | See §4 — Phase 2 idle is *lower* than the Phase 1 baseline |

---

## 2. Server lifecycle, scripted (`tools/qa-lifecycle.py`)

Mock server (`tools/mock-vllm.py`, port 8011) so the production server is never touched.
V2 ran with the documented QA hooks (`V2_TEST_PORT=8011`, `V2_TEST_VLLM_EXE`,
`V2_TEST_VLLM_ARGS`).

```
PASS Start reaches ready                    2.52s healthy python.exe · pid 32332
PASS Restart replaces the process           3.02s python.exe · pid 34384
PASS Stop releases the port                 port_open=False state=no response
PASS External server detected               external — left alone until adopted python.exe · pid 32488
PASS Adopt takes ownership                  adopted from outside — Stop and Restart apply
PASS Adopted process can be stopped         listener_alive=False port_open=False
PASS Stop leaves the test harness alone     this process (pid 6744) survived the stop
```

Failed launch (`V2_TEST_VLLM_ARGS="… mock-vllm.py {port} --fail"`):

| Observation | Value |
| --- | --- |
| Server page stage | `exited with code 1` |
| Process row | `not found`, ownership `none` (no half-owned zombie) |
| Logs page | `vLLM: Traceback (most recent call last):`, `vLLM: RuntimeError: failed to load weights from checkpoint`, `vLLM exited unexpectedly with code 1` |

Unexpected shutdown (mock stopped outside V2 while adopted): the supervisor reports
`Server pid 32120 exited.` and returns to a stopped state without touching anything else.

### Defect found and fixed during this testing

Adoption originally climbed from the port listener to **any** ancestor named
`python.exe`/`pythonw.exe`/`*vllm*`. Pressing *Take ownership* in a test whose mock was
started by a Python harness therefore adopted the harness itself, and *Stop* killed it —
proving the rule could terminate an unrelated process, which V1 never allowed.

* `winproc.rs`: added `process_cmdline()` (PEB → `RTL_USER_PROCESS_PARAMETERS.CommandLine`
  via `NtQueryInformationProcess` + `ReadProcessMemory`, `raw-dylib` links, no new crates).
* `vllm.rs`: the climb now continues only while the ancestor's command line names
  `vllm.exe` or `vllm.entrypoints`, and stops at the first ancestor that does not.
* Verified: the harness survived, the adopted listener was the only thing stopped, and the
  production listener (whose command line contains `vllm.exe serve …`) still resolves to the
  outermost `vllm.exe` when adopted.

---

## 3. Blackout and automation

| Observation | Value |
| --- | --- |
| Manual start | guard pid 34384 / 31884; `state.json` written at ≥1 Hz |
| Display request while locked | `display_request: true`, `display_held: false` — the engine refuses to hold the panel for a locked session (OLED protection) |
| Manual stop | `{active:false, stopped:true}`, request released, engine process gone, no `STOP` file left |
| Follow Server arm | `WAITING-SERVER` → `ARMED` after the configured hysteresis, guard started, `Guard process: managed here · pid 37236` |
| Follow Server stand-down | after the grace window with the server down: guard stopped, phase `WAITING-SERVER`, message `Waiting for the inference server to stay healthy.` |
| Inactivity timeout | `180 s` default rendered and persisted; QA values (10 s / 20 s) only ever set through `%APPDATA%\…\settings.json` and restored to defaults afterwards |
| Idle clock | `IDLE 9s of 180s · 171s to activation`, counted from arm time, matching `idle_s` in `state.json` |
| Engine safety | duplicate guard refused (exit 3 on the named mutex) — inherited from the V1 engine, unchanged |

**Not verified (locked session):** overlay visibility, RGB(0,0,0) output, input restore,
Modern Standby absence, real 180 s activation.

---

## 4. Resource measurements

Harness `tools/measure.py`, same method as the Phase 1 baseline (process tree including
WebView2 children, CPU as percent of one core, 55 samples over 60 s, Home page visible).

| Run | CPU mean | CPU median | CPU max | RSS mean | Processes | GPU util | GPU power | API latency |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Phase 1 baseline (`bench-v2-visible-final`) | 7.84 % | 7.00 % | 23.8 % | 450.7 MiB | 7 | 0.16 % | 1.52 W | 18.1 ms |
| Phase 1 spread (`recovery-spread-a/b`) | 9.23 / 7.65 % | — | 43.7 / 24.5 % | 454 MiB | 7 | — | — | — |
| **Phase 2 (`bench-v2-phase2-home`)** | **5.88 %** | 4.20 % | 17.6 % | 450.8 MiB | 7 | 0.05 % | 1.55 W | 18.1 ms |
| **Phase 2 (`bench-v2-phase2-home-b`)** | **5.37 %** | — | — | 459.7 MiB | 7 | 0.07 % | 1.57 W | 18.4 ms |

Interpretation: adding the supervisor thread (1 Hz, three cheap Win32 calls plus a health
probe at 15 s), the event ring and the action plumbing did not raise idle cost; the measured
mean is 1.8–2.5 points *below* the Phase 1 baseline, which is inside the machine's own
run-to-run spread (7.65–10.43 % for the same Phase 1 build). No regression to explain.

Caveats: the desktop was locked during these runs (the WebView2 surface still reported
`document.visibilityState === "visible"`, so polling ran at the configured 2 s). A visible,
unlocked run is recommended before quoting these numbers elsewhere.

---

## 5. Other checks

| Check | Result |
| --- | --- |
| Single instance | second launch exits **0**, exactly **1** process, first window restored |
| Duplicate/orphan processes | after every stop: no `pythonw.exe` engine, no mock, no leftover `STOP` file |
| Pre-existing processes untouched | V1's `pythonw.exe app.py` (pids 1308, 10108, parent 34060) and production vLLM (10644/26076/29148/13260/22636) survived every run |
| Autostart | no `HKCU\…\Run` value for V2, no service, no scheduled task (registry read after the work) |
| Logs page | server lifecycle, blackout transitions, errors, warnings with a repeat count (`×N`) for identical lines inside 5 s; 400-entry in-memory ring |
| Screenshots | `screenshots/phase2-0{1..5}-<page>.png` plus `-b.png` for pages taller than the window, captured from the live webview at 150 % DPI |
| Icon | ICO contains 16, 24, 32, 48, 64, 128, 256 (32 bpp each); candidates in `assets/icon-candidates/` with a contact sheet |

---

## 6. How to run the tests that were not executed

All four need an **unlocked** desktop; none of them needs the production server and only the
last one touches it at all.

```powershell
cd C:\Users\cohen\AI\RTX-Spark-AI-Control-V2

# 7/8/13 — automatic blackout, input restore, Modern Standby
# 1. start the app, Blackout page, enable "Automatic blackout"
# 2. leave the mouse and keyboard untouched:
#      watch IDLE climb to 180 s, then the panel covers and state.json shows active:true
# 3. move the mouse: the desktop returns within ~100 ms and IDLE restarts from 0
# 4. confirm no Modern Standby:   powercfg /requests
#      expect a SYSTEM display request from the guard; no "away mode" entries

# 12 — responsiveness under heavy inference (needs the owner's OK: it queues real work)
python tools\infer-probe.py --requests 3 --tokens 128
# while that runs, watch the footer "Poll … ms" figure and page transitions

# 4 (repeat) — visible, unlocked idle cost, comparable with the Phase 1 table
python tools\measure.py --label v2-phase2-visible --seconds 60
python tools\measure.py --label v2-phase2-minimized --seconds 60 --minimize
```

---

## 7. Deliberately not done

* No autostart, service, scheduled task or Windows startup shortcut.
* No change to the production server, checkpoint, YAML, chat template, NVIDIA driver, ASUS
  power profile or GPU allocation.
* No rewrite of `blackout.py`; V2 supervises it exactly as V1 does.
* The QA environment hooks (`V2_TEST_VLLM_EXE`, `V2_TEST_VLLM_ARGS`, `V2_TEST_PORT`) exist so
  lifecycle tests never point at port 8000; with them unset the app uses V1's configuration.
