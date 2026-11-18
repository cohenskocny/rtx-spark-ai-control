// Release build with build-host paths remapped out of the binary.
//
// rustc embeds the absolute path of every source file it compiles into the
// binary, as the location of a panic or assertion. `strip = true` in
// Cargo.toml removes symbols, not these string literals, so a naive build
// bakes the builder's profile directory into every dependency frame —
// `C:\Users\<name>\.cargo\registry\src\...` appears hundreds of times.
//
// `--remap-path-prefix` is the supported rustc setting for this. The prefixes
// are read from the environment at build time, so no machine-specific path is
// ever committed, and the remap is a no-op on a host whose paths already agree.
//
// Usage: node tools/build-release.mjs [tauri build args...]
//        npm run release:build

import { spawn } from "node:child_process";
import { homedir } from "node:os";
import { join } from "node:path";

const profile = process.env.USERPROFILE || process.env.HOME || homedir();
const cargoHome = process.env.CARGO_HOME || join(profile, ".cargo");

// rustc applies the *last* matching remapping, so the generic profile rule goes
// first and the specific cargo-home rule last. Verified against rustc 1.98.0:
// listing `.cargo` first leaves it inert and the path resolves through the
// profile rule instead.
const remaps = [
  [profile, "~"],
  [profile.replaceAll("\\", "/"), "~"],
  [cargoHome, "cargo"],
];

const remapFlags = remaps
  .filter(([from]) => from && from.length > 1)
  .map(([from, to]) => `--remap-path-prefix=${from}=${to}`);

const existing = (process.env.RUSTFLAGS || "").trim();
const rustflags = [existing, ...remapFlags].filter(Boolean).join(" ");

console.log("release build: remapping build-host paths");
for (const [from, to] of remaps) console.log(`  ${from} -> ${to}`);

const args = ["tauri", "build", ...process.argv.slice(2)];
console.log(`  $ npx ${args.join(" ")}`);

const child = spawn(process.platform === "win32" ? "npx.cmd" : "npx", args, {
  stdio: "inherit",
  env: { ...process.env, RUSTFLAGS: rustflags },
  shell: process.platform === "win32",
});

child.on("exit", (code) => process.exit(code ?? 1));
