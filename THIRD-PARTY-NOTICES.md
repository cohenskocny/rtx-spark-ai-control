# THIRD-PARTY NOTICES — RTX Spark AI Control

This file records what this repository depends on and which licence each
dependency carries. Nothing here re-licenses anybody else's work, and no licence
text is reproduced: read the licence files shipped inside each package, or the
links the projects publish.

The application's own code, styles and generated icons are MIT — see
[LICENSE](LICENSE).

---

## 1. Runtime dependencies (Rust)

Direct dependencies from [`src-tauri/Cargo.toml`](src-tauri/Cargo.toml). The version
column is what this project's lock file resolves for the direct dependency; the
licence column is the `license` field of that crate's own manifest, read from the
local registry. A crate that appears at several versions transitively is listed
once, at the version this project requests — several `windows-sys` and `windows-*`
versions exist under Tauri's own graph.

| Crate | Version | Licence (as declared by the crate) | Used for |
| --- | --- | --- | --- |
| `tauri` | 2.12.6 | Apache-2.0 OR MIT | Application shell, IPC, window |
| `tauri-build` | 2.7.5 | Apache-2.0 OR MIT | Build script (build-time only) |
| `serde` | 1.0.229 | MIT OR Apache-2.0 | Response and settings serialisation |
| `serde_json` | 1.0.154 | MIT OR Apache-2.0 | JSON documents on disk and over IPC |
| `ureq` | 2.12.1 | MIT OR Apache-2.0 | Loopback HTTP and the GitHub check (`native-tls` feature) |
| `windows-sys` | 0.59 | MIT OR Apache-2.0 | Win32 calls: TCP table, process facts, power state, input tick, mutex |

`ureq` with `native-tls` resolves at run time to Windows SChannel (schannel), a
component of the operating system rather than a redistributed library. The
`ring`/`rustls` path is deliberately not used: it needs a C/asm toolchain that
does not exist for Windows ARM64 without clang.

**Transitive crates.** `Cargo.lock` pins 456 packages in total, including the
Tauri dependency graph. Each is under its own licence, generally
`MIT OR Apache-2.0`. This file does not enumerate them because a hand-written
list drifts; generate one from the lock file with a tool such as `cargo-about` or
`cargo-deny` when a release ships. **Not yet generated — see the release
packaging milestone in [HANDOFF.md](HANDOFF.md).**

## 2. Build and test dependencies (JavaScript)

From [`package.json`](package.json) and `package-lock.json`; licences read from
each package's own `package.json`.

| Package | Resolved licence | Used for |
| --- | --- | --- |
| `@tauri-apps/api` | Apache-2.0 OR MIT | IPC and event bindings in the frontend |
| `react`, `react-dom` | MIT | Interface |
| `@tauri-apps/cli` | Apache-2.0 OR MIT | `npm run tauri`, `npm run app:dev`, `app:build` (build time) |
| `vite` | MIT | Dev server and bundler (build time) |
| `@vitejs/plugin-react` | MIT | JSX transform (build time) |
| `typescript` | Apache-2.0 | Type checking (build time) |
| `@types/react`, `@types/react-dom` | DefinitelyTyped (MIT per package manifest) | Type declarations (build time) |

`package-lock.json` covers the transitive set; the same caveat as above applies.

## 3. The inference engine — external, not bundled

| Project | Licence | Relationship |
| --- | --- | --- |
| [native-vLLM / `devjeonghwan/vllm-for-rtx-spark`](https://github.com/devjeonghwan/vllm-for-rtx-spark) | Apache-2.0 (verified from the repository's licence metadata) | The engine this application launches. Installed separately by the operator. Not bundled, not modified, not redistributed |
| [vLLM](https://github.com/vllm-project/vllm) | Apache-2.0 | Upstream project the above is built from. Not bundled |

Consequences worth being explicit about:

- Nothing of either project's code, binaries or weights is stored in this
  repository, so Apache-2.0's NOTICE obligations are not triggered by this
  repository. If a future installer ever embedded the engine, that would change
  and a NOTICE file plus licence text would have to ship with it.
- Model weights are the operator's own files and are never redistributed here.
- The Updates page names the upstream project and links to it. Naming a project
  and linking to its public pages is not a licence grant or an endorsement.

## 4. Interface assets

| Asset | Origin | Licence |
| --- | --- | --- |
| `src-tauri/icons/*` (application icon and Windows asset set) | Drawn by [`tools/make-icon.py`](tools/make-icon.py) in this repository | MIT, as part of this work |
| `assets/icon-candidates/*` | Same generator's comparison output | MIT |
| `screenshots/*` | Captures of this application's own window, produced by [`tools/shot.ps1`](tools/shot.ps1) | MIT, as part of this work |
| Fonts | None bundled. The interface asks for `Segoe UI Variable`, `Cascadia Mono` and fallbacks already present in Windows | Not redistributed |
| Visual language | Original: a cobalt/pale-blue surface written for this project. No third-party theme, icon set, illustration or template is included | MIT |

No stock art, no icon packs, no purchased fonts, no third-party UI kit.

## 5. Tooling scripts

The scripts under `tools/` are part of this repository and are MIT. They are
development and QA aids, not part of the shipped application. Two of them drive
Microsoft Windows components at run time (WebView2's loopback CDP endpoint and
`SendInput`); nothing from Microsoft is redistributed.

## 6. If you believe something is missing or wrong

Open an issue with the file and the claim you are disputing. A licence claim that
cannot be traced to a dependency's own manifest or repository metadata should be
treated as an error in this file, not as a grant.
