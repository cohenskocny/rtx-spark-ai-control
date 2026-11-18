# CONFIGURATION — RTX Spark AI Control

Two stores, both small and both local:

| Store | Location | Holds |
| --- | --- | --- |
| Backend settings | `%APPDATA%\com.rtxspark.aicontrol.v2\settings.json` | Paths, endpoint, automation timings, autostart flag |
| Interface preferences | `localStorage` key `rtx-v2.prefs.v1` in the webview | Poll interval, pause-when-hidden, reduce motion, last page |

Nothing is stored in the repository, in the installation directory or in the
registry. All backend writes are atomic (temporary file, then rename), so a crash
mid-write cannot leave a half document for the next launch to interpret.

Everything below describes the code as committed. Nothing in this file is a
promise about a future version; the schema number is the contract.

---

## 1. `settings.json` (schema 2)

| Key | Type | Default | Accepted range | Meaning |
| --- | --- | --- | --- | --- |
| `schema` | number | `2` | forced to `2` on write | Document version |
| `blackout_enabled` | bool | `false` | — | Automatic blackout on/off |
| `idle_seconds` | number | `180` | 5 – 7200 | Idle time before automatic blackout |
| `follow_server` | bool | `false` | — | Arm blackout only while the server is healthy |
| `cover` | string | `"internal"` | `internal` · `all` · `primary` | Which panel the guard covers; anything else falls back to `internal` |
| `grace_s` | number | `300` | 5 – 3600 | How long a failing health check is tolerated before standing down |
| `hysteresis_s` | number | `60` | 5 – 600 | How long a *new* health state must hold before it is believed |
| `poll_s` | number | `15` | 5 – 120 | Supervisor health-check interval |
| `stop_timeout_s` | number | `45` | 5 – 300 | Graceful stop window before the process tree is terminated |
| `autostart_inference` | bool | `false` | — | Start inference when the window launches. **Never honoured on battery** |
| `paths` | object | empty strings, `api_host` `127.0.0.1`, `api_port` `8000` | see §2 | Where the installation lives |

Out-of-range values are clamped rather than rejected; a value that could never be
executed is replaced with the default. A settings file written by an older build
keeps its known keys, because the whole struct deserialises with
`#[serde(default)]`.

## 2. `paths`

Every path field is validated by kind, both when it is typed and again before the
whole configuration is saved.

| Key | Kind | Required | Notes |
| --- | --- | --- | --- |
| `vllm_root` | folder | save | Must contain `build-info.json` for version reporting; the folder itself must exist |
| `vllm_exe` | executable | launch + save | The launcher inside the environment, not a shim on `PATH` |
| `python_exe` | optional executable | no | May be empty. Fallback interpreter for the guard and the version probe |
| `model_checkpoint` | folder | launch + save | Read only; the checkpoint is never written to |
| `launch_config` | file | launch + save | YAML passed as `--config` |
| `chat_template` | file | launch + save | Jinja passed as `--chat-template` |
| `blackout_engine` | optional file | no | `blackout.py`. Blank is a valid choice: the guard is an optional feature |
| `blackout_python` | optional executable | no | Interpreter that runs the guard; blank falls back to `python_exe` |
| `logs_dir` | optional folder | no | May be empty. Recorded only; the application keeps its event ring in memory |
| `legacy_config` | optional file | no | May be empty. V1-style `config.json`, read once on import |
| `api_host` | string | yes (defaulted) | Default `127.0.0.1`. Anything that is not hostname characters is stripped; a scheme or path cannot survive |
| `api_port` | number | yes (defaulted) | Default `8000`. `0` is replaced by the default |
| `served_model` | string | no | Display only; the server's own `/v1/models` answer is authoritative |

The four `required to launch` fields are what "launch-ready" means: `vllm_exe`,
`model_checkpoint`, `launch_config` and `chat_template`.

Six fields may be left empty: `python_exe`, `logs_dir`, `legacy_config`,
`served_model`, and both guard fields. Everything else must be present and valid
before **Save configuration** accepts the document.

**Blackout is optional.** A machine that only wants vLLM management saves with
`blackout_engine` and `blackout_python` blank; the Blackout page then says the
feature is not configured, its controls are disabled, and inference, telemetry and
the Updates page are untouched. Nothing is installed, downloaded or searched for on
your behalf, and no interpreter is taken from `PATH`.

An optional field means "may be empty", not "anything goes": a value that is
present must exist, so a typo is reported rather than quietly saved.

**Safety properties, by construction:**

- A path is only ever handed to `Command::new(exe).arg(..)` as a real argv
  element. No value is interpolated into a command line, so no field can become a
  shell injection.
- The character checks are defence in depth on top of that: a value that could
  never be executed cannot be silently accepted either.
- Selecting a path records a string. Nothing creates, moves, repairs or deletes
  anything inside an installation.
- Saving is all-or-nothing: every field is validated first, and any failure
  refuses the entire save with a per-field message.

## 3. Discovery

"Find installations" walks a fixed allowlist of roots — `%USERPROFILE%`,
`%LOCALAPPDATA%`, `C:\` and `D:\` — two levels deep with a hard entry cap, and
filters every hit through the same validator the form uses. `node_modules`,
`.git`, `target`, `windows`, `$recycle.bin` and `appdata` are skipped, so a scan is
cheap and offers nothing absurd. Model folders are offered where a `config.json`
sits beside safetensors weights, since a checkpoint has no naming convention.

Candidates are **suggestions only**. They are returned to the page, never applied
by the backend.

## 4. V1 `config.json` import

`config_import` reads a V1-style `config.json` and maps its documented keys onto
the fields above. It is read-only: the file is opened, parsed and left exactly as
it was. Unknown keys are ignored, and an unreadable or malformed file reports why
instead of half-applying.

## 5. Interface preferences (`localStorage`)

| Key | Values | Default | Effect |
| --- | --- | --- | --- |
| `pollMs` | 1000 · 2000 · 5000 | `2000` | Snapshot poll interval |
| `pauseWhenHidden` | bool | `true` | Stop polling entirely when the window is minimised or occluded |
| `reduceMotion` | bool | `false` | Disables non-essential animation |
| `page` | `home` · `server` · `blackout` · `updates` · `setup` · `settings` · `logs` | `home` | Last page, restored on relaunch |

Invalid or corrupt values fall back to the defaults; a storage that throws (for
example when writes are blocked) is caught, so the window still opens with
defaults rather than failing. Twenty unit tests cover these paths —
`npm run test:web`.

## 6. Files this application writes

| File | Written when | Purpose |
| --- | --- | --- |
| `%APPDATA%\com.rtxspark.aicontrol.v2\settings.json` | Any setting or path change | Documented above |
| `%APPDATA%\com.rtxspark.aicontrol.v2\runtime-state.json` | Ownership and lifecycle changes | Which process this application started (pid plus creation time), so a recycled pid is never mistaken for its own |
| `%APPDATA%\com.rtxspark.aicontrol.v2\upstream-cache.json` | After a successful update check | Last release list, commit list, ETag and timestamps |
| `%TEMP%\rtx-v2-startup.json` | Every launch | Startup report used by the QA scripts (`startup_ms`, `arch`, `version`) |
| `<engine folder>\STOP` | When you press Stop Blackout | Sentinel asking the guard to exit; removed by the guard |
| `<engine folder>\state.json` | Continuously, by the guard itself | Guard's own state, read by this application |

Nothing else. No registry keys, no autostart entry, no scheduled task, no service.

**Test-only overrides** (environment variables, used by the QA scripts and unit
tests, not part of a normal install): `V2_TEST_DATA_DIR` relocates the
`%APPDATA%` folder, `V2_TEST_GITHUB_API` replaces the API base URL,
`V2_TEST_CACHE_PATH` relocates the update cache.

## 7. Endpoints and network traffic

| Destination | When | Content |
| --- | --- | --- |
| `http://<api_host>:<api_port>/v1/models` | Health probe, every `poll_s` seconds | Readiness, served model name, latency |
| `https://api.github.com/repos/devjeonghwan/vllm-for-rtx-spark/...` | Once per 24 h, or when you press Check for Updates (30 s floor) | Releases and recent commits |

The application opens no listening socket. The development tooling does — `npm run
dev` serves Vite on `127.0.0.1:1420`, and an optional CDP endpoint on 9222 is
enabled by an environment variable for QA — but neither exists in a release build.
The update check is anonymous: no
token, no account, no identifier beyond a `User-Agent` naming the application and
the upstream project. GitHub's unauthenticated limit is 60 requests an hour, and
`403`/`429` with a reset time is surfaced as a state, not as an error. Responses
are cached and revalidated with `If-None-Match`, so an unchanged answer costs a
`304` rather than the payload; a failed check keeps the previous answer visible.

The window's CSP is `default-src 'self'` with inline styles allowed and no remote
script, style or image origin. The only outbound link the interface can open is
`https://github.com/devjeonghwan/vllm-for-rtx-spark...`, enforced in Rust before
the shell is asked to open a browser.

## 8. Timings that interact

- `poll_s` drives health, so it also bounds how quickly Follow Server reacts.
- `hysteresis_s` delays believing a *recovery*; `grace_s` delays giving up on a
  *failure*. Both exist so one slow answer cannot flap the guard.
- `idle_seconds` is the guard's own threshold, passed to it on the command line;
  changing it while the guard runs restarts the guard with the new value.
- `pollMs` in the interface is independent: it changes how often the window reads
  the snapshot, not how often the backend checks the server.
