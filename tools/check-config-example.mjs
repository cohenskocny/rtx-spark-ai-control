// Check the example configuration that ships with the documentation.
//
// `cargo test` already round-trips CONFIGURATION.example.json through `Settings`
// and fails on schema drift (src-tauri/src/settings.rs). This script is the half
// that runs without a Rust build: it catches a truncated file, a dropped key, and
// — the reason it exists — a real machine path pasted into a file that is meant to
// be read as a template.
//
// Usage: node tools/check-config-example.mjs

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const file = "CONFIGURATION.example.json";

// Mirrors `struct Settings` and `struct Paths` in src-tauri/src/. Duplicated on
// purpose: if the schema gains a field the example is meant to show, this fails
// until the example gains it too.
const TOP_LEVEL = [
  "autostart_inference",
  "blackout_enabled",
  "cover",
  "follow_server",
  "grace_s",
  "hysteresis_s",
  "idle_seconds",
  "paths",
  "poll_s",
  "schema",
  "stop_timeout_s",
];

const PATH_KEYS = [
  "api_host",
  "api_port",
  "blackout_engine",
  "blackout_python",
  "chat_template",
  "launch_config",
  "legacy_config",
  "logs_dir",
  "model_checkpoint",
  "python_exe",
  "served_model",
  "vllm_exe",
  "vllm_root",
];

// The example must be usable: a reader copies it, fills in real locations, and
// expects the application to accept it. Empty installation paths would make the
// Setup page open instead.
const REQUIRED_PATHS = [
  "vllm_root",
  "vllm_exe",
  "python_exe",
  "model_checkpoint",
  "launch_config",
  "chat_template",
];

// A template path has to look like a template. Anything else is a host path.
const PLACEHOLDER_ROOTS = ["C:\\path\\to\\", "D:\\models\\"];

const raw = readFileSync(file, "utf8");
const example = JSON.parse(raw);

assert.ok(example && typeof example === "object", `${file}: not a JSON object`);

assert.deepEqual(
  Object.keys(example).sort(),
  TOP_LEVEL,
  `${file}: top-level keys do not match the Settings schema`,
);

assert.deepEqual(
  Object.keys(example.paths).sort(),
  PATH_KEYS,
  `${file}: paths keys do not match the Paths schema`,
);

assert.equal(example.schema, 2, `${file}: schema must be 2`);

for (const key of REQUIRED_PATHS) {
  assert.notEqual(example.paths[key], "", `${file}: paths.${key} must not be empty`);
}

// The optional feature is demonstrated as optional, not left half-configured.
assert.equal(example.paths.blackout_engine, "", `${file}: paths.blackout_engine must be empty`);
assert.equal(example.paths.blackout_python, "", `${file}: paths.blackout_python must be empty`);

assert.equal(example.paths.api_host, "127.0.0.1", `${file}: paths.api_host must stay on loopback`);
assert.ok(
  Number.isInteger(example.paths.api_port) && example.paths.api_port > 0 && example.paths.api_port < 65536,
  `${file}: paths.api_port must be a valid port number`,
);

for (const [key, value] of Object.entries(example.paths)) {
  if (typeof value !== "string" || value === "") continue;

  // Case-insensitive: Windows paths are not case-sensitive and neither is this.
  assert.ok(
    !/\\Users\\|\\ProgramData\\|\/home\//i.test(value),
    `${file}: paths.${key} contains a real user or system path`,
  );

  if (key === "api_host" || key === "served_model") continue;

  assert.ok(
    PLACEHOLDER_ROOTS.some((root) => value.startsWith(root)),
    `${file}: paths.${key} is not an obvious placeholder: ${value}`,
  );
}

console.log(
  `config example ok: ${file} — ${PATH_KEYS.length} path fields, ${raw.length} bytes, no host paths`,
);
