# PUBLICATION AUDIT — RTX Spark AI Control v0.1.0

Independent-style pre-publication audit, performed 2026-11-18 against commit
`b923b73` plus the corrections this audit made. Everything below was executed and
its output read; where something could not be executed it says **NOT TESTED**
rather than assuming the previous audits were right.

**Verdict: not ready to publish as-is, but nothing found blocks it permanently.**
Four defects were found and fixed. Six items remain genuinely untested, one of
which (a second machine) cannot be closed on this hardware. Read §9 before
publishing.

Two corrections change the shipped binaries, so **the v0.1.0 artifacts were
rebuilt and the manifest and checksums updated**. The new installer supersedes
the one recorded in the previous manifest.

---

## 1. What was verified, and how

| Area | Result | Evidence |
| --- | --- | --- |
| Rust unit and integration tests | **PASS** — 50 passed, 0 failed | `cargo test --release` |
| Frontend unit tests | **PASS** — 24 passed, 0 failed | `npm run test:web` |
| TypeScript type check | **PASS**, no diagnostics | `tsc --noEmit` (inside `npm run build`) |
| Frontend production build | **PASS** | `vite build` |
| ARM64 production build + NSIS bundle | **PASS**, exit 0 | `npm run release:build` |
| Executable architecture | **PASS** — PE machine `0xAA64` | parsed from the PE header |
| Installer integrity | **PASS** | `sha256sum -c release/v0.1.0/SHA256SUMS.txt` both OK |
| Silent install into a throwaway directory | **PASS**, exit 0 | `validate-installer.ps1` |
| Installed file set | **PASS** — executable + uninstaller, nothing else | same |
| Installed executable hash | **PASS** — matches manifest row 3 | same |
| No unexpected service, driver, task, run key or shortcut | **PASS** | host state diffed before/after (§5) |
| Uninstall | **PASS**, exit 0; program, shortcut and registry entry gone | same |
| User settings survive uninstall | **PASS** | same |
| Single-instance behaviour | **PASS** — two launches, exactly 1 process, second exits | same |
| Configuration persistence | **PASS** — `settings.json`, `runtime-state.json`, `upstream-cache.json` written and re-read | same, plus `settings_and_paths_survive_a_save_and_load` |
| Setup without blackout dependencies | **PASS** | `a_vllm_only_configuration_saves_without_a_guard`, `an_unset_guard_reads_as_optional_not_as_a_defect`, 4 frontend `optional` tests |
| GitHub update check, mocked | **PASS** — 10 `upstream` tests: mock server, cache, offline, rate limit, stalled socket, junk payloads | `cargo test upstream::` |
| Battery-aware inference restrictions, mocked | **PASS** — 6 `power` tests: automatic never runs on battery, manual needs confirmation, AC allows, no battery reads as mains, unknown power blocks automatic, battery flag | `cargo test power::` |
| No model, vLLM server or CUDA work started | **PASS** | no `vllm` process, no benchmark run; the only Python process predates the session |
| Unsigned-installer SmartScreen warning appears | **NOT TESTED** — see §8 | would need a clean machine |
| Install on a clean second ARM64 machine | **NOT TESTED** — see §7 | no second host available |
| Install refusal on x64 Windows | **NOT TESTED** | no x64 host available |
| Update check against a real newly-published upstream release | **NOT TESTED** — mocks only | no such release exists |
| Battery gate refusing an actual manual start | **NOT TESTED** — needs a live server launch | out of scope by instruction |
| Automatic blackout from the installed package | **NOT TESTED** — was verified in the dev build previously | session was unlocked only briefly |
| GitHub Actions / CI | **NOT TESTED — no CI exists.** See §6 | `.github/` does not exist |

---

## 2. Release-binary privacy

### What was wrong

`rustc` embeds the absolute path of every source file it compiles, as the
location attached to a panic or assertion. `strip = true` in `Cargo.toml` removes
the symbol table, not those string literals. The shipped executable therefore
contained:

```
C:\Users\<builder>\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\url-2.5.8\src\parser.rs
```

**277 occurrences**, naming the builder's Windows account in every dependency
frame. The previous manifest recorded this as an accepted, normal consequence of
a release build (§7). It is normal for rustc, but it is not something a public
download needs to carry.

Note on method: the earlier `strings`-based check in this repository's history
would not have found these on a host without GNU binutils, because `strings` is
not installed in Git Bash here. This audit used a byte-level extractor that reads
ASCII and UTF-16LE runs directly from the file.

### The fix

`tools/build-release.mjs`, run as `npm run release:build`, sets
`--remap-path-prefix` before invoking `tauri build`:

| From | To |
| --- | --- |
| `$USERPROFILE` | `~` |
| `$CARGO_HOME` (default `$USERPROFILE\.cargo`) | `cargo` |

The prefixes are read from the environment at build time, so **no
machine-specific path is committed** and the setting is a no-op on a host whose
paths already agree.

One subtlety was measured rather than assumed: rustc applies the **last** matching
rule, not the longest. Listing `.cargo` before the profile root left it inert and
the paths resolved through the profile rule. The order in the script is the one
verified to produce `cargo\registry\src\...`.

### Effect

| Measure | Before | After |
| --- | --- | --- |
| `C:\Users\<builder>` occurrences in the executable | 277 | **0** |
| Absolute drive paths in the executable | 279 | 0 |
| Tokens, private keys, e-mail addresses, hostnames | 0 | 0 |
| Executable size | 5,280,768 | 5,276,160 |
| Installer size | 1,543,842 | 1,543,540 |

Behaviour is unchanged: panic messages still name the crate, file and line, they
are simply rooted at `cargo\registry\...` instead of an absolute path. Diagnosis
costs nothing. The installer was rescanned after the rebuild and contains no
build-host strings (its payload is compressed, so nothing of the executable's
`rdata` is visible in it either).

Residual strings in the executable, all intentional or harmless: the product
description, the crate name `rtx-spark-ai-control-v2`, the PDB **filename**
`rtx_spark_ai_control_v2.pdb` (a bare name in the PE debug directory, no path),
`AppData` (config-directory logic), and the User-Agent
`RTX-Spark-AI-Control/0.1.0 (+https://github.com/devjeonghwan/vllm-for-rtx-spark)`.

---

## 3. Repository and Git findings

### Clean

- **No credentials, tokens, keys or secrets** anywhere in the tree or in any
  commit. The application makes exactly one network call, an anonymous HTTPS GET
  to the public GitHub API; nothing to authenticate with.
- **No model checkpoints, virtual environments, build caches or logs tracked.**
  `.gitignore` excludes `node_modules/`, `dist/`, `src-tauri/target/`,
  `src-tauri/gen/`, `results/`, `logs/`, `*.log`, `shots/*.png`, `tools/ps/`.
- **No installer or executable in Git history.** Largest tracked file is a 4.8 MB
  PNG; total tracked size 7.41 MB across 111 files.
- **No personal paths in application source** (`src/`, `src-tauri/src/`).
- `src-tauri/capabilities/default.json` grants only `core:default` — no shell, FS
  or process plugin is exposed to the webview.

### Defect 1 — personal path in a tool script (FIXED)

`tools/contention.py:15` named the builder's interpreter absolutely:

```text
C:\Users\<builder>\AI\native-vLLM\.venv\Scripts\python.exe
```

This was the only personal path in the whole tree. Replaced with a description
that does not embed one.

### Defect 2 — build-host paths in the binaries (FIXED)

See §2.

### Finding 3 — Git author identity is a personal name and a placeholder e-mail

Every commit is authored and committed by `Cohen <cohen@localhost>`. Publishing
as-is means GitHub shows the commits as **unverified**, because `cohen@localhost`
is not a routable address. This is a choice, not a defect — but make it
deliberately:

```bash
git config user.name  "<public name>"
git config user.email "<you>@users.noreply.github.com"
```

Setting it now would leave the history mixed; rewriting history to change it
(`git rebase` / `filter-repo`) before the first push is the cleaner option and is
harmless while no remote exists.

### Finding 4 — a 4.8 MB QA capture is 65 % of the repository

`qa-screenshots/display-idle-visible-window.png` is 4,846,863 bytes. It **is**
referenced as evidence by `TEST-RESULTS.md` row 4, so it is deliberate, not
stray. Kept. If repository size matters, converting it to JPEG the way
`screenshots/` was converted would recover about 95 % of that without losing the
evidence. `assets/icon-candidates/` (10 PNGs, 195 KB contact sheet) is also
deliberate — design candidates produced by `tools/make-icon.py` and cited in
`RECOVERY-STATUS.md` and `TEST-RESULTS.md`.

### Finding 5 — the tracked screenshots are real captures

`screenshots/*.jpg` (8 files) are captures of this application's own window,
generated by `tools/shot.ps1` and downsized by `tools/optimize-shots.ps1`. They
show the application UI, the endpoint `127.0.0.1:8000`, and model names — no
filesystem paths, no account names, no credentials.

---

## 4. Licensing

| Item | Result |
| --- | --- |
| Project licence | **MIT**, `LICENSE`, "Copyright (c) 2026 RTX Spark AI Control contributors" |
| MIT scope stated | Yes — code, styles and generated assets only; third-party dependencies and the external engine are explicitly carved out |
| Attribution / notices | `THIRD-PARTY-NOTICES.md` — direct dependencies, external engine, interface assets |
| Machine-generated inventory | `DEPENDENCIES.md` from `tools/licence-inventory.mjs`, reading each package's own manifest |
| Rust packages | **456** in `Cargo.lock` — verified by counting `[[package]]` blocks; 300 compiled for ARM64, 156 other-platform-only |
| JavaScript packages | **128** in `package-lock.json` — verified |
| Packages with no licence field | 0 |
| Strong copyleft (GPL/AGPL) | **None.** No GPL- or AGPL-only dependency is present |
| Weak copyleft | **5 crates under MPL-2.0** — verified to be exactly `cssparser`, `cssparser-macros`, `dtoa-short`, `option-ext`, `selectors`. File-level copyleft, unmodified upstream sources, obligation satisfied by not modifying them |
| LGPL | `r-efi` offers `MIT OR Apache-2.0 OR LGPL-2.1-or-later`; the MIT option applies, so no LGPL obligation attaches |
| Bundled third-party code | None. The installer contains only this project's executable, frontend, icons and uninstaller |
| External engine | native-vLLM (`devjeonghwan/vllm-for-rtx-spark`, Apache-2.0) is a separate installation, never bundled or modified — so no NOTICE obligation is triggered here. The claim is recorded in `THIRD-PARTY-NOTICES.md` §3 and is **not independently re-verified by this audit** (no network access to the upstream licence file was used) |
| Original assets | Application icons, the icon candidate set and all screenshots are generated in-repository by `tools/make-icon.py`, `tools/shot.ps1`, `tools/optimize-shots.ps1`. No stock art, icon pack, font or theme is included; the visual language is original and no font is redistributed |

**Conclusion: the licence position is sound and unusually well documented.** The
only unverified assertion is the upstream engine's Apache-2.0 licence, which is
quoted from its repository metadata rather than re-read during this audit.

### Defect 6 — the checksum file could not be verified with `sha256sum -c` (FIXED)

`release/v0.1.0/SHA256SUMS.txt` was CRLF-terminated, so GNU `sha256sum` treated
the `\r` as part of the filename and reported both entries unreadable. A published
checksum file that fails its own standard verification command is a real usability
defect, not a nit. Rewritten with LF endings; `sha256sum -c` now reports
`OK` for both files.

---

## 5. Installer and host-integrity evidence

Silent install into `%LOCALAPPDATA%\Temp\rtx-audit-install`, then launch, then
silent uninstall. The whole host's service list, `Run`/`RunOnce` keys (HKCU and
HKLM), uninstall registry entries, scheduled tasks and Start Menu shortcuts were
captured before and after and diffed.

| Observation | Result |
| --- | --- |
| Installer exit code | 0 |
| Files installed | `rtx-spark-ai-control-v2.exe` (5,276,160 B) and `uninstall.exe` (79,462 B) |
| Extra executables, DLLs or drivers | none |
| Services created | **none** |
| Drivers created | none |
| Scheduled tasks created | none (199 before, 199 after) |
| `Run`/`RunOnce` entries created | none (8 before, 8 after) |
| Uninstall registry entries | one, `HKCU\...\Uninstall\RTX Spark AI Control`; removed on uninstall (244 before, 244 after) |
| Start Menu shortcuts | one `.lnk`; removed on uninstall (57 before, 57 after) |
| Version resource | `0.1.0` / product `0.1.0`, company `rtxspark`, description `RTX Spark AI Control` |
| Bundler byte patch | exactly 3 bytes differ from the pre-bundle executable: `UNK` → `NSS` at offset 4,950,334; same length, same size |
| Uninstall exit code | 0; program directory, shortcut and registry entry gone |
| Settings kept after uninstall | yes |
| Processes left after uninstall | 0 |

The only host-state difference anywhere in the diff was an unrelated Windows
service (`ClipSVC`) changing from Running to Stopped during the window. Nothing
the installer did.

---

## 6. CI and external validation

### There is no CI

`.github/` does not exist. There is no workflow, no pipeline, no scheduled run
and no signing step. `HANDOFF.md:151` and `CHANGELOG.md:135` already say so, and
that honesty is correct — but it means §1's results are only reproducible by hand
on this machine.

**Untested assumptions that a workflow would otherwise cover:**

1. That the build is reproducible on a *different* machine. Nothing pins a
   toolchain (`rust-toolchain.toml` is absent), so "rustc 1.98.0" is a recording of
   this host, not an enforced requirement.
2. That `npm ci` from `package-lock.json` alone is sufficient (the tree here has a
   populated `node_modules/`, so a clean install was never exercised).
3. That the `--remap-path-prefix` sanitization holds on a host whose
   `CARGO_HOME` or profile path differs — the logic is path-derived and tested
   locally, but a second environment would be the real check.
4. That the 50 Rust tests pass on a clean checkout without the existing
   `src-tauri/target/` cache.
5. Platform-specific test behaviour: `upstream` tests bind loopback ports and set
   process-wide environment variables under a mutex. That works here; a CI runner
   would confirm it is not host-dependent.
6. That the ARM64 NSIS bundle succeeds without the cached `nsis` toolchain.

A minimal honest workflow would run `npm ci`, `npm run test:web`,
`npm run build`, and `cargo test` on `windows-11-arm`, and nothing else. Do not
add a release-upload or signing step until §10's ordering is decided.

### Second-machine installation: NOT PERFORMED

No isolated Windows ARM64 environment is available to this session. Only the
original laptop was used, as in every previous round. **No claim of
second-machine compatibility is made anywhere in this document.** The strongest
statement supported by the evidence is: *the ARM64 installer installs, launches,
supports a second-instance refusal, writes its own settings and uninstalls
cleanly on the one host it was built on.*

**Manual validation still required before publishing as a working release:**

1. Clean Windows 11 ARM64 machine (or VM) with no prior install, no vLLM and no
   Python. Run the installer; expect SmartScreen to warn (record the exact prompt
   text), then confirm the Setup page appears with blackout marked optional.
2. Confirm the installed executable's SHA-256 matches manifest row 3.
3. Confirm no service, driver, scheduled task or `Run` entry appears. The
   `state.ps1` / `validate-installer.ps1` pair in this audit does exactly this and
   can be reused.
4. Launch twice; confirm one process.
5. Uninstall; confirm program, shortcut and registry entry are gone and settings
   remain.
6. On a Windows 11 ARM64 machine **without** the WebView2 runtime, confirm the
   failure is a clear message rather than a blank window. Windows 11 ships
   WebView2, so this tests the documented dependency, not the common path.
7. On an x64 machine, confirm the ARM64 package refuses to install.

---

## 7. Known limitations to publish alongside v0.1.0

These are all already stated in the documentation; they are repeated here so the
release decision sees them together.

- **One test host, one hardware family.** Windows 11 ARM64 on an NVIDIA RTX Spark
  laptop. Battery gating, GPU telemetry and display control were never observed on
  other ARM64 hardware.
- **x64 is unsupported.** The package is ARM64 and is expected to refuse.
- **Unsigned.** See §8.
- **native-vLLM is external.** The application never installs, upgrades or
  repairs it, and the Updates page is information only.
- **The OLED blackout engine is optional and external.** Requires a script and a
  Python interpreter the operator supplies. The application works fully without
  it, and this was the defect fixed in `6b07b84`.
- **The battery gate was never observed refusing a real launch.** The mock tests
  cover the decision logic; the end-to-end refusal needs a live server attempt.
- **The update check was never exercised against a genuinely newer upstream
  release.** Only cached, absent, offline, rate-limited and malformed responses,
  all mocked.
- **Documented measurements come from this machine** (`results/`, which is
  gitignored). They are reproducible with the `tools/` scripts, not portable
  claims.

---

## 8. Publishing an unsigned installer

The installer and executable carry **no Authenticode signature**. Consequences:

- Windows SmartScreen will show "Windows protected your PC" on first run for most
  users, with the publisher shown as unknown. Some users will stop there.
- No publisher identity can be verified. The SHA-256 in `SHA256SUMS.txt` is the
  only integrity check available, and it is only trustworthy if the user got it
  from a channel they already trust.
- Windows Defender may treat a low-prevalence, unsigned, self-extracting installer
  with suspicion. Nothing here suggests a false positive, but it is **NOT TESTED**
  on a clean machine.

**Assessment: publishing v0.1.0 unsigned is reasonable, provided it is labelled as
such.** The mitigation is disclosure, not silence:

1. State "unsigned, expect a SmartScreen warning" in the README download section
   and in the release notes, in those words. The documentation already does this
   in `INSTALL.md` and the manifest; the release notes and README download block
   must repeat it, because that is where a user arrives.
2. Publish `SHA256SUMS.txt` next to the installer and give both a
   `Get-FileHash` and a `sha256sum -c` recipe (`INSTALL.md` already has the
   PowerShell form; the LF fix in §4 makes the GNU form work).
3. Say plainly that the build is reproducible by the user from source, and how.
4. Do not describe the binary as "verified" anywhere.

If a code-signing certificate is obtained later, re-sign and re-issue the manifest
rather than retrofitting a claim onto the existing hashes.

---

## 9. Outstanding release blockers

Nothing here is unfixable. Ordered by what actually stops a release.

| # | Blocker | Severity | Status |
| --- | --- | --- | --- |
| 1 | No remote repository exists, and nothing has ever been pushed. `git remote -v` is empty, `git tag` is empty | Blocking by definition | For the maintainer |
| 2 | Binaries unsigned; SmartScreen warning not observed on a clean machine | High — user trust | Documented in §8; accepted risk |
| 3 | Installer never run on a second machine or clean image | High — the only real compatibility evidence | **Open**, cannot be closed here (§6) |
| 4 | No CI. Every result in §1 is manual and host-specific | Medium | **Open** (§6 lists the six assumptions) |
| 5 | Git author identity is `Cohen <cohen@localhost>`; commits will show unverified | Medium — cosmetic but permanent once pushed | Decision needed (§3 finding 3) |
| 6 | SmartScreen behaviour on a clean machine for an unsigned low-prevalence installer | Medium | **NOT TESTED** |
| 7 | Automatic blackout and the battery refusal were not re-run from the installed package | Low — tested in the dev build and at unit level | **Open** |
| 8 | `qa-screenshots/display-idle-visible-window.png` is 4.8 MB, 65 % of repository size | Low — cosmetic | Accepted, with the JPEG recommendation in §3 |

**Fixed by this audit:** build-host paths and the builder's account name in the
binaries (§2); a personal path in `tools/contention.py` (§3); a checksum file that
could not be verified by its own standard tool (§4); a missing example
configuration, now shipped and covered by a test (§10).

---

## 10. Exact steps to publish v0.1.0

Assumes the corrections in this audit are committed on `master` and that you have
decided on §3 finding 5 and §8.

```bash
# 0. In the repository root, on the build host

# 1. Decide the public author identity (once, before the first push).
git config user.name  "<public name>"
git config user.email "<you>@users.noreply.github.com"

# 2. Optional but recommended while no remote exists: normalise the history so
#    every commit carries that identity.
#    git filter-repo --commit-callback 'commit.author_name = b"<public name>"; ...'

# 3. Confirm a clean tree and that the suite still passes from scratch.
git status --porcelain          # expect no output
npm run test:web                # expect 24 passing
npm run build                   # tsc --noEmit + vite build
cargo test --manifest-path src-tauri/Cargo.toml --release   # expect 50 passing

# 4. Rebuild from a clean target only if the recorded hashes must stay valid;
#    otherwise reuse the artifacts already verified in §1.
rm -rf src-tauri/target/release/bundle
npm run release:build

# 5. Re-verify the shipped artifacts against the manifest.
sha256sum -c release/v0.1.0/SHA256SUMS.txt
#    and rescan for build-host strings -- the audit's scan must still report 0.

# 6. Create the empty public repository on GitHub, then:
git remote add origin https://github.com/<owner>/<repo>.git
git push -u origin master

# 7. Enable, in the repository settings:
#    - Private vulnerability reporting (SECURITY.md assumes it)
#    - Issues
#    - Branch protection on master, once CI exists

# 8. Create the release. Draft it, attach both artifacts, then publish.
git tag -a v0.1.0 -m "RTX Spark AI Control v0.1.0"
git push origin v0.1.0
```

Release assets for `v0.1.0`:

| Asset | Path on the build host | SHA-256 |
| --- | --- | --- |
| `RTX Spark AI Control_0.1.0_arm64-setup.exe` | `src-tauri/target/release/bundle/nsis/` | `39ec4b17a2baca830f930994889084e35e8793abeb18306cf25f3acb584503f6` |
| `rtx-spark-ai-control-v2.exe` | `src-tauri/target/release/` | `ee57faabd6047a4e38e13e0e6f2cac7ed0e3f5a1d28721b1d6f1340ddae43990` |
| `SHA256SUMS.txt` | `release/v0.1.0/` | (text) |

Release notes must state: ARM64 only; unsigned, so SmartScreen will warn; vLLM is
an external dependency that is not bundled; the build was validated on one host
and no second-machine test has been performed.

**Do not** attach `qa-screenshots/`, `results/`, `logs/` or any model file.

---

## 11. Corrections made by this audit

| File | Change |
| --- | --- |
| `tools/build-release.mjs` | New. Remaps build-host paths out of the binary; `npm run release:build` |
| `package.json` | Adds the `release:build` script |
| `tools/contention.py` | Removes the builder's absolute interpreter path from a docstring |
| `CONFIGURATION.example.json` | New. Sanitized, loadable example configuration |
| `src-tauri/src/settings.rs` | New test: the shipped example parses and matches the schema key for key |
| `CONFIGURATION.md` | Points at the example and explains the test |
| `release/v0.1.0/RELEASE-MANIFEST.md` | New artifact sizes and hashes, the build command, §2a on path remapping, corrected bundler byte offset (4,950,334), private-path bullet rewritten, test count 50 |
| `release/v0.1.0/SHA256SUMS.txt` | LF endings so `sha256sum -c` verifies; hashes regenerated |
| `SECURITY.md`, `README.md`, other docs | No change required — none of them claimed the binary was path-free |

Both binaries were rebuilt after the change and re-verified. The installer
recorded in the previous manifest (`50c7852…`) is superseded by `39ec4b17…`.
