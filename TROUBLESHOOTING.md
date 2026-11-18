# TROUBLESHOOTING — RTX Spark AI Control

Symptoms first, cause second. Every message quoted below is a real string in the
committed code, so you can search for it.

---

## Setup and configuration

### "an executable is required" / "a folder is required" and the save is refused

The save is all-or-nothing: every non-optional field is validated first, and one
failure refuses the whole document. Only `python_exe`, `logs_dir`,
`legacy_config` and `served_model` may be left empty — see
[CONFIGURATION.md §2](CONFIGURATION.md#2-paths). Read the per-field message; it
names the field that failed.

### "must be a full path, such as C:\folder\file"

Relative paths are rejected. So are paths containing `..`, paths longer than 512
characters, paths with characters Windows does not allow, and reserved device
names (`CON`, `PRN`, `AUX`, `NUL`, `COM1`…).

### The Blackout page is disabled and says "optional"

That is correct: you configured no guard. Blackout needs `blackout_engine` and an
interpreter, and it is the only feature that does. Leave both fields empty and the
rest of the application — lifecycle, telemetry, Updates, Logs — works unchanged.
Setting them later enables the page on the next save; nothing needs restarting.

### "Blackout is configured but the guard script or interpreter does not validate"

A path is set but does not resolve: the script was moved or deleted, or the
interpreter is not an `.exe`. Blank means "not using this feature"; a value that is
present must exist, so a typo is caught at save time. Fix the field, or clear both
to leave the feature off.

### "Find installations" finds nothing

The search is deliberately shallow — two levels under `%USERPROFILE%`,
`%LOCALAPPDATA%`, `C:\` and `D:\`, with `node_modules`, `.git`, `target`,
`windows`, `$recycle.bin` and `appdata` skipped. An installation outside those
roots, or deeper than two levels, must be typed in. Type it: validation runs per
field while you type, so you get the same verdict without a scan.

### The window opens with nothing configured

That is the designed first run. It is not a crash, and no default paths are
invented — a public build contains no machine's directory layout. Go to Setup.

## Server lifecycle

### A running server is shown as "external" and the Stop button will not touch it

Only processes this application started are ever stopped. Adoption records the pid
*and* its creation time, so a recycled pid cannot be mistaken for the one you
adopted. Press **Adopt** on the Server page if you want control of a server you
started yourself; until then it is reported and left alone — including on battery,
where its power state is reported as it is.

### "A server is already listening on port 8000; not starting a second one"

Something already answers on the configured port. Either use the running server
(Adopt) or stop it yourself. The application will not kill a listener it did not
start, and will not start a second one that would fail to bind.

### "This application's server is still starting"

A start is already in flight. Wait for the state to settle; the readiness probe is
`GET /v1/models`, not a timer.

### Stop takes up to 45 seconds

`stop_timeout_s` (default 45) is a graceful window before the process tree is
terminated. A server that ignores the first request is stopped at the deadline
rather than left half-alive. Raising it allows slower shutdowns; lowering it risks
a killed process mid-write.

### Restart while on battery is refused

Restart is stop-then-start, so it is gated like a start. Expect either
`Automatic launch is disabled on battery power. Plug in and try again, or start it
by hand.` for an automatic path, or
`This laptop is on battery power. Loading the model now will drain it quickly;
confirm to launch anyway.` for a manual one that needs the inline confirmation.

## Blackout

### The page shows the guard running but `active` stays false, `locked` true

By design. While the Windows session is locked the engine removes its overlay and
drops the display request, then re-takes it on unlock — a lock screen must not sit
lit for hours. `state.json` reporting `locked: true` is the engine telling the
truth, not a failure. Unlock and press Blackout now again, or let automatic
blackout arm after the idle threshold.

### Nothing happens at all when I press Blackout now

Check, in this order:

1. `blackout_engine` points at an existing `blackout.py`, and `blackout_python` at
   an interpreter that can run it.
2. `state.json` sits beside the script and is fresh. The application treats a file
   written within the last 4 seconds as the engine's live state; a stale file means
   the engine is gone.
3. The Logs page: the guard's start is recorded with its pid, and a refusal is
   recorded with the engine's own words.

### A `STOP` file is left beside the script

Stop Blackout writes a `STOP` sentinel and the guard removes it as it exits. A file
left behind means the guard did not exit cleanly. It is harmless — a guard started
later consumes it — but a guard that is still running while a new one starts is
prevented by the engine's own single-instance mutex, and the Logs page says so.

### Two guards, or a guard that will not die

The engine enforces a single instance through its own named mutex. If you have
edited or replaced the script, that protection is whatever the script provides; the
application only starts, stops and observes it.

### The panel goes dark but the machine is still awake and warm

That is the point of the guard's display request: it holds
`PowerSetRequest(PowerRequestDisplayRequired)` together with
`SetThreadExecutionState(ES_DISPLAY_REQUIRED)`, so Windows' 30-second blank timer
never fires and Modern Standby is not entered. Without a held request the panel
blanks, the machine drops into Modern Standby and the GPU is capped at roughly
7 W — which is exactly what an OLED "black screen" cannot prevent on its own.

### Modern Standby: what actually happens here

This machine has no S1/S2/S3 sleep states; it has **S0 Low Power Idle** (Modern
Standby) only. The panel blanks 30 seconds after the last input
(`VIDEOCONLOCK`), Windows enters Modern Standby, and GPU clocks fall. `powercfg
/requests` would show the held display request, but it needs an elevated prompt;
the guard's own log lines are the practical check, and that is what the test
evidence in [TEST-RESULTS.md](TEST-RESULTS.md) uses.

### Input wakes the desktop instead of being swallowed

Correct, and deliberate: **no input is hooked, blocked, swallowed, logged or
replayed**. The guard paints the panel black and holds the display request; any
real input removes the overlay immediately. It is a curtain over an unlocked
desktop, not a lock — see [SECURITY.md](SECURITY.md).

## Updates page

| What you see | What it means |
| --- | --- |
| `not checked` | No check has completed yet. The backend checks once every 24 h; **Check for Updates** forces one, with a 30-second floor |
| `reachable` | GitHub answered |
| `offline` | Network, DNS or timeout. The previously cached answer stays visible |
| `rate limited` | GitHub returned `403`/`429` with a reset time. Unauthenticated limit is 60 requests an hour; the reset is shown |
| `check failed` | A response arrived but could not be used (unexpected status or malformed body) |

The cache lives at `%APPDATA%\com.rtxspark.aicontrol.v2\upstream-cache.json`. A
failed check never clears it. Deleting the file only costs you the cached answer,
not any quota.

### The installed version reads "unavailable"

Version reporting is read-only file reading, in this order:
`<vllm_root>\build-info.json`, then
`<environment>\Lib\site-packages\vllm-<version>.dist-info\METADATA`. A build that
ships neither — or a `vllm_root` that does not contain `build-info.json` — reports
"unavailable" with a note rather than guessing. Nothing is imported from the
environment to find out.

## Interface

### GPU figures are blank or say `nvidia-smi unavailable`

Telemetry shells out to `nvidia-smi`. If it is not on `PATH` — or the driver's
`nvidia-smi` is missing — the GPU block reports the error instead of showing
zeroes. The rest of the window works; a missing GPU does not stop the server
controls.

### Every page sits at "connecting" and no error appears

React 18 mounts, unmounts and remounts effects in development. A poller that set a
`stopped` flag on its first cleanup and never cleared it would leave the real mount
with a dead poller — an empty window with nothing to report. The fix claims the
poller on entry (`src/lib/useTelemetry.ts`); if you see this again, look there
first. A release build was never affected.

### A second launch does nothing

The window claims a named single-instance mutex at startup. The second process
exits rather than opening a duplicate window against the same server. Use the
window that is already open.

### Settings do not survive a relaunch

Backend settings go to `%APPDATA%\com.rtxspark.aicontrol.v2\settings.json`, written
atomically (temp file, then rename). If the folder is unwritable — a redirected
profile, a policy, a full disk — the save is refused. Interface preferences are
separate and live in the webview's `localStorage`; a storage that throws is caught
and defaults are used, so a locked-down profile degrades to defaults rather than a
broken window.

## Development

### `npm run app:dev` dies with `EBUSY: resource busy or locked, watch ...`

`cargo` executes build scripts from `src-tauri\target`, and Windows locks a running
image against a watcher opening it. Vite's watcher emitted the OS error as an
unhandled event and the dev server exited. `vite.config.ts` now ignores the Rust
build tree and the other non-source folders. If the error returns, check that the
`watch.ignored` list is still in place; do not delete `src-tauri\target` while a
build is running.

### `no TLS backend is configured`

`ureq` with `native-tls` needs the SChannel connector handed to the agent
explicitly; without it every `https` request fails at run time. Unit tests do not
catch this, because they speak plain HTTP to a local mock. If you touch the HTTP
client, keep the connector.
