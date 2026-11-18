# CONTRIBUTING — RTX Spark AI Control

Thanks for looking. This file is about how the project is built and verified, not
about process theatre.

Two rules come before anything else:

1. **Never point the tooling at a working inference server unless the owner of the
   machine said so.** Starting, stopping or restarting it interrupts real work, and
   loading a model costs minutes and watts. Use the mock.
2. **Do not add an ambient animation or a polling loop that the hardware has to pay
   for.** The idle-cost measurements in [README §9](README.md#9-measured-results)
   are the reason this window is cheap; a 60 Hz CSS loop is how the previous
   controller reached 85 % CPU while idle.

---

## 1. Setup

| Tool | Version used here |
| --- | --- |
| Windows | 11, ARM64 |
| Rust | 1.98 `aarch64-pc-windows-msvc` (`rust-version = "1.77"` is the declared floor) |
| Node / npm | 26.13.1 / 11.20 |
| Build tools | Visual Studio 2022 Build Tools (ARM64 MSVC), Windows SDK 10.0.26100 |
| Python | Only for the QA scripts and the blackout engine |

```powershell
git clone https://github.com/cohenskocny/rtx-spark-ai-control.git
cd rtx-spark-ai-control
npm install
npm run app:dev      # Vite on 127.0.0.1:1420 + cargo dev build
```

## 2. What must pass before a commit

```powershell
cargo test --manifest-path src-tauri\Cargo.toml   # 46 tests
npm run test:web                                  # 20 tests, node --test
npx tsc --noEmit                                  # type check
npm run build                                     # bundle
```

`npm run build` runs the type check itself, so `npm run build` + both test
commands is the minimum. Do not claim a suite passes without the exit code in
front of you; that rule is the reason [TEST-RESULTS.md](TEST-RESULTS.md) quotes
commands rather than conclusions.

Frontend tests use `node --test` on the pure helpers in `src/lib/`. No test runner,
no DOM emulation, no new dependency. If a behaviour is worth testing, it belongs in
a pure function — that is why presentation logic lives in `src/lib/present.ts`
instead of inside JSX.

**Not run automatically, by design:** anything that needs a real GPU, real weights
or a real inference server. See §4.

## 3. Style

- **Rust**: `cargo fmt` defaults, no `unwrap()` on anything a user can influence,
  errors as `String` messages that read like sentences (they are shown verbatim in
  the window). Modules are named for what they own: `paths`, `power`, `vllm`,
  `blackout`, `supervisor`, `upstream`, `version`, `telemetry`, `winproc`.
- **TypeScript/React**: function components, no UI kit, CSS in `src/styles/`.
  Preference and presentation helpers stay pure and testable.
- **Comments** explain behaviour that a reader could not infer, and the *why* when
  a line looks odd (there are several: the StrictMode poller claim, the TLS
  connector, the `CREATE_NO_WINDOW` flags). A comment that narrates the diff does
  not belong in the source.
- **No dead abstractions.** If a wrapper exists, a call site needs it.
- **Text on screen** is written for an operator: say what happened and what it
  means, not "Error 0x57".

## 4. Testing against a mock, never the production server

```powershell
python tools\mock-vllm.py 8011          # stand-in for `vllm serve`
```

Then launch the application with the QA hooks pointed at it:

```powershell
$env:V2_TEST_PORT="8011"
$env:V2_TEST_VLLM_EXE="<python>\python.exe"
$env:V2_TEST_VLLM_ARGS="<repository>\tools\mock-vllm.py {port}"
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9222"
```

| Script | Purpose |
| --- | --- |
| `tools/mock-vllm.py` | Fake `vllm serve`: prints weight progress, serves `/v1/models`, holds the socket; `--fail` exits with a traceback |
| `tools/qa-lifecycle.py` | Drives Start / Restart / Stop / adopt / external detection over CDP and prints what the page rendered |
| `tools/cdp.py` | Evaluate an expression in the window over the loopback CDP endpoint |
| `tools/qa-input.py` | Inject real input, or read `GetLastInputInfo` idle time |
| `tools/shot.ps1` | Capture the window as a PNG at logical scale |
| `tools/measure.py` | CPU/RSS sampling for the idle-cost table |
| `tools/make-icon.py` | Regenerate the application icon set |

`mock-vllm.py`, `cdp.py`, `qa-input.py` and `qa-lifecycle.py` are QA aids, not part
of the shipped application.

**Blackout tests change the display.** Use a short `blackout_timeout` (45 s is
enough), set it back to 180 s afterwards, and do not run them on a machine someone
is using.

## 5. Commits

Conventional-commit style, scoped to the layer that changed:

```text
fix(dev): stop the Vite watcher from descending into src-tauri/target
feat(core): portable configuration, battery gating and upstream awareness
test(web): cover format, prefs and present helpers with node --test
docs(handoff): record the Phase 3 recovery, its tests and what remains
```

The body explains the defect and the fix, in prose, in the present tense. Commit
one logical change; a commit that mixes a refactor with a behaviour change is two
commits.

**No pushing, no publishing, no releases** from a working session. That is the
maintainer's call, made deliberately.

## 6. Documentation

Docs are part of the change, not a follow-up:

- A user-visible behaviour change updates [CONFIGURATION.md](CONFIGURATION.md) or
  [TROUBLESHOOTING.md](TROUBLESHOOTING.md) in the same commit.
- A measured number is quoted with its condition. "7.84 % idle" without
  "1500×950, animations and blur off" is not a measurement, it is a slogan.
- Anything unverified says so, in the same sentence. The three-way distinction
  **tested** / **measured once** / **unverified** is the standard in this
  repository: [TEST-RESULTS.md](TEST-RESULTS.md) is the evidence, and
  [README §3](README.md#3-requirements) is where the honest limits live.
- No personal paths, usernames, hostnames or tokens in docs, screenshots or
  committed output.
- Two generated files are never hand-edited: [DEPENDENCIES.md](DEPENDENCIES.md)
  comes from `node tools/licence-inventory.mjs` (run it after any dependency
  change), and the JPEGs under `screenshots/` come from
  `tools/optimize-shots.ps1 -Src <archive> -Out screenshots`. Keep the originals
  out of the repository — they are 65 MB and Git keeps every version of them.

## 7. Where to start

[HANDOFF.md](HANDOFF.md) states the current milestone and what remains. It is kept
current on purpose: if it disagrees with the code, that is a bug in the handoff,
worth reporting as one.
