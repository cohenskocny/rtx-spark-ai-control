# AGENT-INSTALL — install RTX Spark AI Control with a coding agent

This file is an installation guide written for a coding agent, not for a person.
Copy the prompt between the markers below into Claude Code, Codex, OpenCode, T3
Code or any other agent that can read files and run read-only commands on your
machine. The agent reads it, asks you two questions, and then does the work with
your approval at each step that changes anything.

Two paths are offered, and you choose:

| Path | What it does | What it never does |
| --- | --- | --- |
| **A — GUI only** | Verifies the machine, finds the official release, checks its SHA-256, installs the GUI after your approval, connects an existing native-vLLM installation, and verifies the result **without loading a model** | Installs vLLM, Python, a driver, a model or a guard script |
| **B — Guided full setup** | Everything in A, plus hardware, driver and storage verification, and step-by-step help installing native vLLM from its official repository, with a separate approval for every dependency, model download and CUDA-related change | Decides your model, checkpoint, launch configuration or folder layout for you |

The agent is told, in the prompt, that it may not assume your checkpoint, model
name, launch YAML, chat template or filesystem layout — it has to ask.

**Where the pieces live**

- Application and releases: <https://github.com/cohenskocny/rtx-spark-ai-control>
- The inference engine this application manages, and does not include:
  <https://github.com/devjeonghwan/vllm-for-rtx-spark>
- Manual instructions, if you would rather not use an agent: [INSTALL.md](INSTALL.md)
- Every setting and path: [CONFIGURATION.md](CONFIGURATION.md)
- The read-only checks the prompt uses: `tools/check-requirements.ps1` and
  `tools/verify-install.ps1`

---

````text
===== PROMPT START — copy everything from here to PROMPT END =====

You are helping me install or configure RTX Spark AI Control v0.1.0 on this
Windows machine. Follow this brief exactly. Where it says ask, ask and wait for
my answer before doing anything else.

## Ground rules

1. Read-only until I approve something specific. Until then, run only commands
   that read: CIM queries, file listings, file hashes, a settings file, a
   process list. Do not install, download, write a setting, start a service or
   launch a server.
2. Do not assume anything about my machine. Do not assume a model name, a
   checkpoint path, a launch configuration, a chat template, a drive letter or
   a folder layout. If you need one, ask me, then verify what I gave you.
3. Do not install, upgrade, remove or reconfigure a GPU driver, the CUDA
   toolkit, cuDNN, a PyTorch build or any other CUDA-related software without
   my explicit approval for that exact action, after telling me what it changes
   and how to undo it.
4. Do not download model weights. If a step seems to need them, stop and ask.
5. Do not start inference to "check it works". Verifying the installation must
   not load a model. Starting a server is a separate action that needs my
   approval and AC power.
6. No secrets in output: no tokens, keys, passwords or personal paths beyond the
   ones needed for the task.
7. Report facts with numbers, and write "not checked" where you did not check.
   Never describe something as verified that you did not run.
8. Declining is a valid outcome. If I decline a step, stop that line of work,
   report what you did verify, and do not ask again unless I raise it.

## Step 0 — establish the environment (read-only)

Collect and record:

- Windows version: `Get-CimInstance Win32_OperatingSystem | Select-Object Caption, BuildNumber`.
  Windows 11 means build 22000 or newer.
- Architecture: `Get-CimInstance Win32_Processor | Select-Object Architecture`
  and `Get-CimInstance Win32_ComputerSystem | Select-Object SystemType`.
  Architecture code **12 is ARM64**. Do not decide this from
  `$env:PROCESSOR_ARCHITECTURE`: a 64-bit x86 process under emulation reports
  AMD64 on an ARM64 machine, and that is a false negative you must not act on.
- GPU and driver: `nvidia-smi --query-gpu=name,driver_version,memory.total --format=csv,noheader`
- WebView2 runtime: the `pv` value of the EdgeUpdate client key
  `{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}` under
  `HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients`,
  `HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients` or the `HKCU` equivalent.
- Free space on the drive you would install to.
- Power source: AC or battery (the `System.Windows.Forms.PowerStatus` API, or
  `Win32_Battery`).

If this is not Windows 11 ARM64, stop here and say so. v0.1.0 ships an
ARM64-only package that refuses to install on x64, there is no x64 build, and no
workaround should be attempted.

If the repository is available locally, the same checks are scripted:
`powershell -File tools\check-requirements.ps1` (add `-RequireInference` if this
machine is meant to serve, `-SkipNetwork` if it is offline). Exit codes:
0 every required check passed, 1 a required check failed, 2 unsupported
platform, 3 the checks could not run. The script installs nothing.

## Step 1 — ask me which path I want

Ask this as one question with two options, and wait:

- **A. GUI only** — install the application, connect an existing native-vLLM
  installation, verify it without loading a model.
- **B. Guided full setup** — everything in A, plus hardware, driver and storage
  verification and help installing native vLLM from its official repository,
  with a separate approval for every dependency, model download and
  CUDA-related change.

Also ask, once: do I already have native vLLM installed, or should the answer to
that be discovered? Then follow the matching path below.

## Path A — GUI only

### A1. Find the official release (read-only)

The only official source is <https://github.com/cohenskocny/rtx-spark-ai-control>
and its releases page, <https://github.com/cohenskocny/rtx-spark-ai-control/releases>.
The v0.1.0 installer asset is named `RTX-Spark-AI-Control_0.1.0_arm64-setup.exe`.
Use the name exactly as it appears on the release page: GitHub rewrites spaces in
uploaded asset names, so the published file has hyphens where a locally built
one has spaces. The bytes are identical. Because `SHA256SUMS.txt` lists the
published name, `sha256sum -c SHA256SUMS.txt` works on the files as downloaded —
do not rename anything to make a check pass.

- Do not download an installer from anywhere else: not a mirror, not a file
  host, not a search result, not an attachment.
- If no release is published yet, say so plainly and stop this path. Offer the
  source build in INSTALL.md section 2 instead. Do not substitute a build you
  find elsewhere.
- Read the expected SHA-256 from the release's own `SHA256SUMS.txt` asset. That
  asset is authoritative: it describes the exact file attached to the release.
- The in-repository record is `release/v0.1.0/RELEASE-MANIFEST.md` section 2.
  It currently states
  `39ec4b17a2baca830f930994889084e35e8793abeb18306cf25f3acb584503f6`
  for the v0.1.0 installer — the build-time value from the 0.1.0 build. If the
  artifacts are rebuilt before publication, that number changes and the manifest
  is regenerated with it. So treat the release asset as the source of truth, and
  a repository value that disagrees with it as a stale build record, not as
  evidence of tampering. Say which of the two you compared against.
- If you cannot fetch a published hash, say the file is unverified instead of
  trusting a hash I paste — and say why.

### A2. Verify the file before anything runs (read-only)

Hash the file you downloaded:

    Get-FileHash '<installer>' -Algorithm SHA256

Compare it with the published value, case-insensitively. Then, if the repository
is available, run `powershell -File tools\verify-install.ps1 -InstallerPath '<installer>'`
and read the exit code: 0 verified, 1 a required check failed (including a
checksum mismatch), 2 unsupported platform, 3 not installed, 4 the checks could
not run.

- **Match:** continue, and show me the two hashes side by side.
- **Mismatch:** stop. Do not run the file, do not "try it anyway", do not delete
  it silently. Tell me the two values, and suggest downloading again from the
  official release page only.
- **No published hash:** treat the file as unverified and tell me so before I
  decide.

### A3. Ask for approval to install (blocking)

Before running the installer, tell me in one short block:

- the file name, its size and its SHA-256;
- that it installs per user into `%LOCALAPPDATA%\RTX Spark AI Control` with no
  administrator rights, no service, no scheduled task, no Run entry and no
  driver;
- that the binaries are unsigned, so Windows SmartScreen may warn, and that
  "More info → Run anyway" is my decision to make, not yours;
- what uninstalling does, and what it leaves behind (the settings directory).

Then ask, and wait. Only an explicit yes lets you run it. Prefer the interactive
installer; `'/S /D=<folder>'` is available for a silent install into a folder I
name, but do not silence my decision — the approval is what gets silenced, not
the prompts.

### A4. Connect my existing native-vLLM installation (read-only discovery)

Look, read-only, for an installation: a folder containing `build-info.json`,
with `vllm.exe` inside its Python environment (`<root>\.venv\Scripts\vllm.exe` or
`<root>\venv\Scripts\vllm.exe`). Report what you found with its version from
`build-info.json`. If you find none, say so; Path B covers installing one.

The fields the application needs are the user's, not yours:

| Field | Required | What it points at |
| --- | --- | --- |
| `vllm_root` | yes | folder containing `build-info.json` |
| `vllm_exe` | yes | `vllm.exe` inside that installation |
| `model_checkpoint` | yes | the checkpoint directory |
| `launch_config` | yes | YAML passed to `vllm serve --config` |
| `chat_template` | yes | Jinja template passed to `--chat-template` |
| `blackout_engine`, `blackout_python` | no | the optional OLED guard script and its interpreter |
| `python_exe`, `logs_dir`, `legacy_config` | no | fallbacks and imports |
| `api_host`, `api_port` | defaults | `127.0.0.1`, `8000` |

Ask me for the checkpoint, launch configuration and chat template. Do not infer
them from a model name, a previous installation, or a guess about my layout. If
I do not have a launch configuration or a template, say which one is missing
rather than filling it in.

Prefer the window's **Setup** page: it validates every field and saves
all-or-nothing. If I ask you to write
`%APPDATA%\com.rtxspark.aicontrol.v2\settings.json` instead, start from
`CONFIGURATION.example.json`, keep `schema: 2`, write only the fields above, and
never write inside the vLLM installation, the checkpoint or the model files.

### A5. Verify without loading a model

- `powershell -File tools\verify-install.ps1` — expected exit 0 when installed
  and configured. Warnings are normal and harmless: an unconfigured optional
  blackout guard (`skip`), a battery-powered laptop, an offline host.
- Launch the window once, without pressing Start, and confirm it renders and
  reports the configuration. Read `%TEMP%\rtx-v2-startup.json` for the first-paint
  figure instead of claiming one.
- Report the `vllm` and `python` process counts before and after. Both should be
  unchanged, and no model should have been loaded.

## Path B — guided full setup

Do A1, A2, A3, A4 and A5 in order, with the additions below. Each addition adds
an approval gate, not a shortcut.

### B1. Hardware, driver and storage (read-only)

- **GPU:** name, driver version and memory from `nvidia-smi`. The tested family
  is the NVIDIA RTX Spark N1X. Any other GPU is unverified: say that, do not
  claim support and do not refuse outright — the application will still run, and
  telemetry may not.
- **Driver:** 621.14 is the version this release was tested with. Do **not**
  update, roll back or reinstall a driver. If it is missing or older, report it
  and ask me whether to treat that as a blocker. A driver change is a separate,
  explicitly approved action outside this guide.
- **Storage:** compare my checkpoint's size on disk with free space on the drive
  it lives on. The application itself is under 6 MB; a checkpoint is tens of
  gigabytes, and vLLM's build needs room of its own. Report the numbers.
- **WebView2:** present or not, with its version. Do not install or repair it
  without approval.

If the repository is available:
`powershell -File tools\check-requirements.ps1 -RequireInference`.

### B2. Install the GUI

Repeat A1 to A3 exactly. The approval gate is not waived because the setup is
"full".

### B3. Install native vLLM, from its official repository only

- The engine is <https://github.com/devjeonghwan/vllm-for-rtx-spark>. It is the
  only project this application manages and the only upstream this guide
  supports. Do not substitute another vLLM build, another fork, another branch
  or a prebuilt wheel from an unofficial source.
- Read that project's own README for the supported procedure. Summarise the exact
  commands for my machine, in order, with what each one installs and how much
  space and time it needs.
- Ask me where to put the checkout, and use that path. Do not assume a folder
  name.
- **Ask for explicit approval before each of these, separately:** installing
  packages or build dependencies, downloading wheels or model weights, and any
  change to CUDA-related software (CUDA toolkit, cuDNN, a PyTorch build, or the
  driver). Say what changes and how to reverse it.
- When it is built, confirm the installation is ARM64 (`wheel_tag` in
  `build-info.json` ends in `win_arm64`) and record where `build-info.json` and
  `vllm.exe` are. Those two paths are what the application needs.

### B4. Model and serve configuration

- I supply the checkpoint, the launch YAML and the chat template. Ask for each
  path; do not download weights unless I approve a specific source and size, and
  do not reuse a checkpoint from another machine without asking.
- Validate, read-only: the checkpoint directory exists and contains what I say it
  contains, the YAML parses if a parser is available, the template file exists.
  Report mismatches; change nothing.

### B5. Configure and validate

- Write or confirm the five required paths, the optional guard paths and the
  endpoint. Prefer the Setup page; if you write the file, follow A4's rules.
- Starting the server needs my approval and AC power. Readiness is
  `GET http://<api_host>:<api_port>/v1/models`, never a timer or a log line.
- If I decline to start a server, validate the configuration only, state clearly
  that no inference was started, and leave it to me.
- The optional blackout guard is not bundled and is never installed on my behalf.
  Configure it only if I already have a `blackout.py` and an interpreter for it.

### B6. Failures you must not "fix" on your own

- An installer that refuses to install: report the reason. Do not extract it by
  hand, patch it or bypass the architecture check.
- A checksum mismatch: stop, per A2.
- A missing dependency: report it and ask. Do not install it as a side effect of
  something else.
- No network: an offline install is fine if the file's hash verifies; skip the
  update check and say so. Nothing else in this guide needs the network.
- Any decline from me: stop, report what was verified, and do not retry.

## Report at the end

Use this shape, and keep it short:

    Machine:        Windows <build>, <architecture>, <GPU>, driver <version>
    Path:           A (GUI only) or B (guided full setup)
    Verified:       <what you actually ran, with the numbers it printed>
    Installed:      <exact files and folders you created, or "nothing">
    Configured:     <paths you recorded, or "not configured">
    Not done:       <the steps I declined or that did not apply>
    Next step:      <one sentence>

If something failed, the first line of the report says what failed, and the exit
code you saw.

===== PROMPT END =====
````

---

## Notes for the person using this file

- **Both paths are gated.** The agent may not install the GUI before showing you
  the hash and getting a yes, and may not install dependencies, download models
  or touch CUDA-related software at all without a separate yes for each.
- **Nothing is assumed about your installation.** Path A works with your existing
  native-vLLM setup wherever it lives; Path B asks where things should go.
- **Verification is scripted and read-only.** `tools/verify-install.ps1` checks
  the installation, the settings document, the host footprint (no service, no
  scheduled task, no Run entry) and the absence of a started server. It hashes,
  reads and reports; it never installs, downloads or starts anything.
- **Offline is supported.** The only step that needs the network is the download.
  Checks report an unreachable host as a warning, not a failure.
- **Declining is documented behaviour.** The not-installed exit code is 3 and the
  message says nothing was installed. An agent that keeps re-asking after a
  decline is not following this brief.
- **Keep it current.** Hashes and version numbers in this file mirror
  [release/v0.1.0/RELEASE-MANIFEST.md](release/v0.1.0/RELEASE-MANIFEST.md) §2. When
  a release is rebuilt, update this file in the same commit; the test suite checks
  that the safety wording is still here, not that the numbers are still right.
