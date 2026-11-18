# INSTALL — RTX Spark AI Control

> **No prebuilt installer is published.** This repository has not produced a
> release installer yet: packaging is the next milestone (see
> [HANDOFF.md](HANDOFF.md)). Everything below that is marked **tested** was run
> on the machine described in §1. Everything marked **unverified** has not been
> run.

---

## 1. What you need

**Tested configuration** — every measurement in this repository comes from this
machine:

| Component | Version verified |
| --- | --- |
| OS | Windows 11, ARM64 |
| GPU | NVIDIA RTX Spark N1X, 96 GB allocation from 128 GB unified memory |
| Driver | NVIDIA 621.14 |
| WebView2 Runtime | Microsoft Edge WebView2 157.0.4341.46 |
| Inference engine | native-vLLM `0.32.0+rtx_spark.0.2.0`, upstream commit `e9412963ff` |
| Model | `Qwen/Qwen3.8-Flash-Next`, FP4 checkpoint, MTP 4, FP8 KV cache, 409 600 context |
| Rust | 1.98, `aarch64-pc-windows-msvc` (`rust-version = "1.77"` is the declared floor) |
| Node / npm | Node 26.13.1, npm 11.20 |
| Build tools | Visual Studio 2022 Build Tools, ARM64 MSVC, Windows SDK 10.0.26100 |
| Python | Required only to run the blackout engine script |

**Unverified, expected to work through the configured paths:** Windows 11 x64,
other Blackwell-generation laptop GPUs, other builds of the same fork, other
checkpoints that speak the OpenAI-compatible API. Nothing on this list has been
run — see [README §3](README.md#3-requirements).

**The inference engine is not included.** Install native-vLLM from
<https://github.com/devjeonghwan/vllm-for-rtx-spark> first; this application
launches it, and never downloads, installs, upgrades or replaces it.

**The blackout engine is not included either.** The guard script (`blackout.py`)
is the validated controller from the previous version of this project and lives
outside this repository. Without it, everything except the Blackout page works;
that page reports the missing script instead of pretending.

## 2. Build from source

**Tested** (`npm run app:build`, exit 0; frontend build and `tsc --noEmit` clean):

```powershell
cd <repository>
npm install
npm run build       # typecheck + bundle the frontend into dist\
npm run app:build   # release binary, no installer bundle
```

The binary lands at:

```text
src-tauri\target\release\rtx-spark-ai-control-v2.exe
```

Run it directly; there is nothing to register and no service to install.

For a development window with hot reload (**tested**):

```powershell
npm run app:dev     # Vite on 127.0.0.1:1420 + cargo dev build
```

`npm run app:dev` must not be pointed at a production server: switching
inference off and on is a lifecycle action. Use the mock described in
[CONTRIBUTING.md](CONTRIBUTING.md).

### Per-user installer (unverified)

```powershell
npm run tauri build
```

`src-tauri/tauri.conf.json` asks for an NSIS target with
`installMode: "currentUser"`, so the result would be a per-user installer in
`src-tauri\target\release\bundle\nsis\`. **This command has not been run in this
repository** and no installer has been tested. Treat it as the packaging
milestone, not as a supported install path.

## 3. Connect an existing native-vLLM installation

The application ships with **no** machine paths. On first launch it opens with an
incomplete configuration and says so; the **Setup** page is where it becomes
usable.

| Field | What it points at | Required |
| --- | --- | --- |
| `vllm_root` | Root of the native-vLLM installation — the folder containing `build-info.json` | yes |
| `vllm_exe` | The `vllm.exe` launcher inside that installation's Python environment | yes |
| `model_checkpoint` | The local model checkpoint directory | yes |
| `launch_config` | The YAML serve configuration passed to `vllm serve --config` | yes |
| `chat_template` | The chat template `.jinja` passed to `--chat-template` | yes |
| `blackout_engine` | The `blackout.py` guard script | yes, for the Blackout page |
| `blackout_python` | The interpreter that runs the guard | yes, for the Blackout page |
| `python_exe` | The installation's own interpreter; only a fallback for the guard and the version probe | no |
| `logs_dir` | Where you want logs to live, if anywhere | no |
| `legacy_config` | A V1-style `config.json` to read paths from, once | no |

Plus three plain values: `api_host` (default `127.0.0.1`), `api_port` (default
`8000`) and `served_model` (display only).

Two ways to fill them in:

1. **"Find installations"** — a shallow search over a fixed set of roots
   (`%USERPROFILE%`, `%LOCALAPPDATA%`, `%PROGRAMFILES%`, `C:\`, and any folder
   already configured). It offers what it finds as suggestions. Clicking a
   suggestion fills a field; nothing is written until **Save configuration**
   succeeds, and that save is all-or-nothing.
2. **Type the paths.** Each field is validated as you type and reports its own
   verdict.

The launch command this application will use is:

```text
<vllm_exe> serve --config <launch_config> --chat-template <chat_template> --port <api_port>
```

It is passed as an argument vector, never through a shell. Readiness comes from
`GET http://<api_host>:<api_port>/v1/models`, not from a timer.

Nothing inside your installation is written to. Selecting a path records a
string; it does not create, move or repair anything. Full field semantics,
defaults and stored file locations are in [CONFIGURATION.md](CONFIGURATION.md).

## 4. First run

**Tested behaviour:**

1. The window opens even with an empty configuration, and the Setup page explains
   what is missing.
2. If a server is already listening on the configured port, it is reported as
   **external** and left alone. Adoption is a deliberate click; until then the
   application will not stop or restart it.
3. The top bar shows the power source. On battery, automatic launch paths are
   refused and a manual start asks for confirmation first.

## 5. Uninstall

**Tested:** stop the server from the Server page if this application started it,
then close the window. There is no service, no scheduled task, no autostart entry
and no registry write; closing the window stops whatever this application
started, including the blackout guard.

Then delete:

```text
%APPDATA%\com.rtxspark.aicontrol.v2\      settings, runtime state, update cache
<whichever folder you placed the exe in>  the application binary
```

If an installer is produced in the packaging milestone, its own uninstall entry
will remove the binary; the `%APPDATA%` folder above stays under your control
either way.

Deleting the `%APPDATA%` folder loses your configuration and the last successful
update check. It never touches your vLLM installation, its configuration, your
model files or the guard script.

## 6. Check the build yourself

**Tested on 2026-11-18:**

```powershell
cargo test --manifest-path src-tauri\Cargo.toml   # 46 passed, 0 failed
npm run test:web                                  # 20 passed, 0 failed
npx tsc --noEmit                                  # clean
npm run build                                     # 51 modules
```
