# SECURITY — RTX Spark AI Control

## What this program is

A local, single-user Windows desktop application that starts, stops and observes a
vLLM server on the same machine, and runs an OLED guard script. It has no server
component, opens no listening socket, has no account system, no telemetry, no
analytics and no remote configuration.

**It is not a security boundary.** Read §3 before relying on the blackout guard for
anything that matters.

## 1. What it touches

| Asset | Access | Detail |
| --- | --- | --- |
| Your vLLM installation | read + launch | Reads `build-info.json` and `dist-info`; launches `vllm.exe serve` with your configured files. Never writes, installs, upgrades or repairs anything inside it |
| Model checkpoint | read | Passed to the engine as a path. Never written |
| Blackout guard script | read + launch | Launches `blackout.py`, writes a `STOP` sentinel beside it to ask it to exit |
| `%APPDATA%\com.rtxspark.aicontrol.v2` | read + write | Settings, ownership record, update cache |
| `%TEMP%\rtx-v2-startup.json` | write | Startup report for QA scripts |
| Network | outbound only | Loopback HTTP to the configured endpoint; HTTPS to `api.github.com` |

No registry writes. No service, scheduled task, autostart entry or driver. No
elevation: the application never asks for administrator rights, and no code path
attempts to raise its own privileges.

## 2. Process ownership

The one place this program can destroy something is a stop, so ownership is the
control that matters:

- A server is only stopped if **this application started it**, recorded in
  `runtime-state.json` as pid *plus* process creation time.
- A recycled pid cannot therefore be mistaken for the recorded one; the creation
  time must match.
- A server that was already running is reported as **external** and left alone
  until you explicitly adopt it — including while on battery, where it is neither
  stopped nor restarted because of the power state.
- Stops are graceful (`stop_timeout_s`, default 45 s) before the tree is terminated,
  so a server is not killed mid-write unless it ignores the first request.

## 3. The blackout guard: a curtain, not a lock — read this

The guard exists to keep an OLED panel out of a powered-on black state and to stop
the machine dropping into Modern Standby while it is idle. That is a power and
burn-in feature. It is **not** an access control, and it cannot become one:

- It paints the panel black and holds a display power request. It does **not**
  hook, block, swallow, log or replay input. Any real input removes the overlay
  immediately — that behaviour is a feature of the guard and is tested.
- Anyone in front of an unlocked machine can therefore reach the desktop. Nothing
  in the window changes that, and nothing in the window claims otherwise.
- Windows authentication is untouched. Use `Win+L` — or the Windows lock policy you
  already have — when you need a real boundary.
- While the Windows session is locked, the guard deliberately removes its overlay
  and drops the display request, then re-takes them on unlock. A locked session with
  a lit panel is not a state this program will maintain.

**The trade-off, stated plainly:** an idle machine that is hidden from view is more
comfortable and less conspicuous, and a hidden desktop is not a protected desktop.
If your threat model includes a person with physical access, the guard buys you
nothing; the Windows lock does.

## 4. Inputs this program trusts

| Input | Trust | Mitigation |
| --- | --- | --- |
| Paths from Setup | Operator-supplied, may be any absolute path | Validated (kind, existence, absolute, no `..`, length, reserved names, illegal characters); handed to `Command::new(exe).arg(..)` as argv elements, never interpolated into a shell command |
| `api_host` | Treated as a hostname | Reduced to hostname characters; a scheme, path or whitespace cannot survive |
| The API response at the configured endpoint | Semi-trusted | Parsed defensively into typed structures; a malformed answer is an error state, not a panic |
| `state.json` from the guard | Semi-trusted, freshness-checked | Only treated as live if written in the last 4 seconds |
| `build-info.json`, `dist-info` | Semi-trusted | Read as files; invalid JSON is a note, not a crash. Nothing is imported or executed |
| GitHub API responses | Untrusted (external) | Bounded read with timeouts; status codes handled as states; content rendered as text, never as HTML |
| Links clicked in the window | Untrusted | `open_external` refuses anything that is not `https://github.com/devjeonghwan/vllm-for-rtx-spark…`; quotes, backslashes and whitespace are rejected |

Release notes from GitHub are markdown from an external source. The page reduces
them to plain text (links keep their label, code fences are stripped, blank runs
collapsed). There is no HTML renderer in the window for them.

## 5. Network behaviour

- **No listener in a release build.** Nothing binds a port; the single-instance
  mutex is a kernel object, not a socket. Development tooling is the exception and
  exists only while it runs: Vite on `127.0.0.1:1420`, and an optional loopback CDP
  endpoint on 9222 that has to be enabled with `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`.
- **Outbound loopback HTTP** to `http://<api_host>:<api_port>/v1/models` for health,
  on `poll_s` seconds. Plain HTTP because the engine serves loopback plain HTTP; on
  a non-loopback `api_host` that traffic is unencrypted — keep it on loopback.
- **Outbound HTTPS** to `api.github.com`, once per 24 h, or on demand with a
  30-second floor. Anonymous: no token, no account. The `User-Agent` names the
  application and the upstream project. Responses are cached to disk and
  revalidated with `If-None-Match`.
- The webview's CSP is `default-src 'self'`, inline styles only, no remote script,
  style or image origin, and `connect-src 'self' ipc: http://ipc.localhost`.
- The Tauri capability set is `core:default` for the `main` window only. There is no
  filesystem or shell plugin exposed to the frontend: every privileged action is a
  named command implemented in Rust.

## 6. Data handling

- No credentials are stored. There is nothing to store: no token is used anywhere.
- The update cache and settings contain paths, release metadata and timings — no
  personal identifiers, no machine identifiers, no telemetry.
- Nothing is sent anywhere except the two HTTP calls above. Re-read §5 if that
  sentence surprises you: the only third party that ever sees this application is
  GitHub, once a day, asking about a public repository.

## 7. Battery-aware launch restrictions

Not a security control, a hardware one: loading a model on battery drains it fast
and the operator may not be watching. The gate lives in the Rust backend
(`power.rs`), not in the buttons, so every startup path obeys it:

| Path | On battery |
| --- | --- |
| Saved autostart | Refused, reason shown |
| Follow Server re-arm | Refused |
| Recovery after a failure | Refused |
| Manual Start / Restart | Allowed only after an inline confirmation |

A server that is already running is reported as it is — never stopped or restarted
because of the power state. On AC, or on a desktop with no battery, the operator's
own settings apply unchanged.

## 8. What is out of scope

- Protecting a machine from someone with physical access, or from an unlocked
  session. See §3.
- Protecting the inference server itself, its model weights or its API. The engine
  is a separate program with its own exposure; this window neither hardens nor
  proxies it.
- Multi-user or remote deployments. One window, one interactive session, one
  machine.
- Side channels, supply-chain attacks on your own installation, and the security of
  Windows, the NVIDIA driver or the OLED firmware.

## 9. Reporting a vulnerability

This repository has no published security contact yet, and inventing an address
here would be worse than saying so. When the repository is published, report
through a private security advisory on it; until then, contact the maintainer
privately through whatever channel you were given the project by.

Please include the version (Server page, or
`%TEMP%\rtx-v2-startup.json`), the Windows build, and the exact message from the
Logs page. Do not include model weights, tokens or personal paths.

## 10. Supported versions

Only the current `0.1.0` line is being worked on, and it is not yet published — see
[HANDOFF.md](HANDOFF.md). There is no long-term support commitment and no backport
policy, because there has been no release to backport to.
