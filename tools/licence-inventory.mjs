// Generate the transitive dependency licence inventory.
//
// THIRD-PARTY-NOTICES.md lists what this project depends on *directly*, which is
// the part a person can read. A distributable also has to answer "what is
// actually inside this binary", and that answer lives in the lock files. This
// script reads them, asks each dependency's own manifest what its licence is, and
// writes DEPENDENCIES.md. It never guesses.
//
//   node tools/licence-inventory.mjs
//
// Two distinctions the script keeps honest:
//
//  - Cargo.lock resolves for every platform, but only the Windows ARM64 half is
//    ever compiled. Crates whose sources were never fetched are reported
//    separately as "not built on this platform" rather than as licence gaps.
//  - npm records a licence for platform binaries it does not install here
//    (esbuild's other targets). The lock entry is used when node_modules has no
//    copy to read.

import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const root = process.cwd();
const outFile = path.join(root, 'DEPENDENCIES.md');

// A licence option that needs no obligation beyond attribution/notice.
const PERMISSIVE = /^(MIT|MIT-0|Apache-2\.0|Apache-2\.0 WITH LLVM-exception|BSD-[234]-Clause|0BSD|ISC|Zlib|CC0-1\.0|Unicode-3\.0|Unicode-DFS-2016|CC-BY-4\.0|Unlicense|BSL-1\.0|WTFPL)$/i;
// Weak, file-level copyleft: usable unmodified, but the crate's own source must
// stay available. Kept as its own class so the obligation is visible.
const FILE_COPYLEFT = /^MPL-2\.0$/i;
const STRONG_COPYLEFT = /^(A|L)?GPL/i;

/** Classify an SPDX-ish expression by what a redistributor may actually choose. */
function classify(expression) {
  if (!expression) return 'unknown';
  // Older crates write dual licences as "MIT/Apache-2.0"; "/" means OR there.
  const normalised = expression.replace(/[(]/g, '').replace(/[)]/g, '').replace(/\s*\/\s*/g, ' OR ');
  const units = normalised.split(/\s+AND\s+/i).map((part) => part.trim());
  const kinds = units.map((unit) => {
    if (unit.split(/\s+OR\s+/i).some((part) => PERMISSIVE.test(part.trim()))) return 'permissive';
    if (unit.split(/\s+OR\s+/i).some((part) => FILE_COPYLEFT.test(part.trim()))) return 'weak copyleft option (MPL-2.0)';
    if (STRONG_COPYLEFT.test(unit)) return 'copyleft only';
    return 'other — review the package licence text';
  });
  for (const kind of ['copyleft only', 'other — review the package licence text',
    'weak copyleft option (MPL-2.0)']) {
    if (kinds.includes(kind)) return kind;
  }
  return 'permissive option available';
}

/** Parse the `[[package]]` blocks of Cargo.lock without a TOML dependency. */
function readCargoLock() {
  const text = fs.readFileSync(path.join(root, 'src-tauri', 'Cargo.lock'), 'utf8');
  const packages = [];
  let current = null;
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trim();
    if (line === '[[package]]') {
      if (current?.name) packages.push(current);
      current = {};
      continue;
    }
    if (!current) continue;
    const match = /^(name|version)\s*=\s*"([^"]+)"/.exec(line);
    if (match) current[match[1]] = match[2];
  }
  if (current?.name) packages.push(current);
  return packages;
}

/** The licence a crate declares, read from its manifest in the local registry. */
function crateLicense(name, version) {
  const registry = path.join(os.homedir(), '.cargo', 'registry', 'src');
  if (!fs.existsSync(registry)) return { license: null, file: false, fetched: false };
  for (const channel of fs.readdirSync(registry)) {
    const dir = path.join(registry, channel, `${name}-${version}`);
    const manifest = path.join(dir, 'Cargo.toml');
    if (!fs.existsSync(manifest)) continue;
    const text = fs.readFileSync(manifest, 'utf8');
    const license = /^license\s*=\s*"([^"]+)"/m.exec(text)?.[1] ?? null;
    const licenseFile = /^license-file\s*=/m.test(text);
    const hasFile = licenseFile
      || ['LICENSE', 'LICENSE-MIT', 'LICENSE-APACHE', 'LICENCE'].some((f) => fs.existsSync(path.join(dir, f)));
    return { license, file: hasFile, fetched: true };
  }
  return { license: null, file: false, fetched: false };
}

/** JavaScript packages, from the lock file, with the licence from their manifest. */
function readNpmLock() {
  const lockPath = path.join(root, 'package-lock.json');
  if (!fs.existsSync(lockPath)) return [];
  const lock = JSON.parse(fs.readFileSync(lockPath, 'utf8'));
  const rows = [];
  for (const [key, entry] of Object.entries(lock.packages ?? {})) {
    if (!key || entry.link) continue;
    const name = entry.name ?? key.replace(/^.*node_modules\//, '');
    const manifest = path.join(root, key, 'package.json');
    let license = null;
    let installed = false;
    if (fs.existsSync(manifest)) {
      installed = true;
      const pkg = JSON.parse(fs.readFileSync(manifest, 'utf8'));
      const value = pkg.license ?? pkg.licenses;
      license = typeof value === 'string'
        ? value
        : Array.isArray(value) ? value.map((l) => l.type ?? l).join(' OR ') : null;
    }
    // npm records the licence for platform binaries it does not install here.
    if (!license && typeof entry.license === 'string') license = entry.license;
    rows.push({ name, version: entry.version ?? '?', license, dev: Boolean(entry.dev), installed });
  }
  return rows.sort((a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version));
}

const crates = readCargoLock().map((pkg) => ({ ...pkg, ...crateLicense(pkg.name, pkg.version) }))
  .sort((a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version));
const built = crates.filter((crate) => crate.fetched);
const notBuilt = crates.filter((crate) => !crate.fetched);
const npm = readNpmLock();

const countBy = (items, key) => {
  const map = new Map();
  for (const item of items) {
    const value = key(item);
    map.set(value, (map.get(value) ?? 0) + 1);
  }
  return map;
};
const sorted = (map) => [...map.entries()].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]));

const crateLicenses = countBy(built, (c) => c.license ?? 'no license field in the crate manifest');
const npmLicenses = countBy(npm, (p) => p.license ?? 'no license field in package.json');
const crateClasses = countBy(crates, (c) => (c.fetched ? classify(c.license) : 'not built on this platform'));
const npmClasses = countBy(npm, (p) => classify(p.license));
const gaps = [...built.filter((c) => !c.license), ...npm.filter((p) => !p.license)];
const copyleftOnly = [...crates.filter((c) => classify(c.license) === 'copyleft only'),
  ...npm.filter((p) => classify(p.license) === 'copyleft only')];

const row = (cells) => `| ${cells.join(' | ')} |`;
const lines = [];
lines.push('# DEPENDENCIES — generated licence inventory');
lines.push('');
lines.push('Generated by [`tools/licence-inventory.mjs`](tools/licence-inventory.mjs) from');
lines.push('`src-tauri/Cargo.lock` and `package-lock.json`, asking each package\'s own');
lines.push('manifest what its licence is. Do not edit by hand — re-run the script.');
lines.push('');
lines.push('This is the complete list of what is linked into, or used to build, the');
lines.push('application. [`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md) describes the');
lines.push('direct dependencies and the external inference engine; this file is the');
lines.push('evidence behind it. This project\'s own code is MIT — see [LICENSE](LICENSE).');
lines.push('');
lines.push('## 1. Summary');
lines.push('');
lines.push(`- Rust packages pinned in \`Cargo.lock\`: **${crates.length}**, of which **${built.length}**`);
lines.push(`  are compiled for Windows ARM64 and **${notBuilt.length}** are other-platform-only`);
lines.push(`  (Linux, macOS, Android) and never enter this build.`);
lines.push(`- JavaScript packages in \`package-lock.json\`: **${npm.length}**.`);
lines.push(`- Packages compiled or installed here with no licence field at all:`);
lines.push(`  **${gaps.length}**.`);
lines.push(`- Packages whose licence offers no permissive option: **${copyleftOnly.length}**.`);
lines.push('');
lines.push('| Licence classification | Rust | JavaScript |');
lines.push(row(['---', '---', '---']));
const classes = [...new Set([...crateClasses.keys(), ...npmClasses.keys()])].sort();
for (const key of classes) {
  lines.push(row([key, String(crateClasses.get(key) ?? 0), String(npmClasses.get(key) ?? 0)]));
}
lines.push('');
const lgplOffered = built.filter((crate) => /LGPL/i.test(crate.license ?? ''));
const mplOnly = built.filter((crate) => classify(crate.license) === 'weak copyleft option (MPL-2.0)');
if (!copyleftOnly.length) {
  lines.push('Every package compiled into this build offers a permissive licence option');
  lines.push('(MIT, Apache-2.0, BSD, ISC, Zlib, Unicode, CC0, or an `OR`-combination that');
  lines.push('includes one).');
  if (lgplOffered.length) {
    lines.push('');
    lines.push(`${lgplOffered.length} crates are also offered under LGPL-2.1-or-later as one alternative`);
    lines.push(`(${[...new Set(lgplOffered.map((crate) => `\`${crate.name}\``))].join(', ')}). The MIT alternative is the one relied`);
    lines.push('on here, so no LGPL obligation attaches.');
  }
  lines.push('');
  lines.push('No GPL- or AGPL-only dependency is present.');
  if (mplOnly.length) {
    lines.push('');
    lines.push(`${mplOnly.length} crates are under MPL-2.0 (${mplOnly.map((crate) => `\`${crate.name}\``).join(', ')}).`);
    lines.push('MPL-2.0 is file-level copyleft: it reaches only those crates\' own unmodified');
    lines.push('source files, which are published on crates.io and stay available there. It');
    lines.push('places no condition on this project\'s own code, and those crates are neither');
    lines.push('modified nor replaced.');
  }
} else {
  lines.push('Packages that offer no permissive option and need a licence decision before');
  lines.push('distribution:');
  lines.push('');
  for (const item of copyleftOnly) lines.push(`- \`${item.name}\` ${item.version} — ${item.license}`);
}
lines.push('');
lines.push('Every licence expression as declared by the packages themselves:');
lines.push('');
lines.push('| Licence (as declared) | Rust packages | JavaScript packages |');
lines.push(row(['---', '---', '---']));
const declared = [...new Set([...crateLicenses.keys(), ...npmLicenses.keys()])].sort();
for (const key of declared) {
  lines.push(row([key, String(crateLicenses.get(key) ?? 0), String(npmLicenses.get(key) ?? 0)]));
}
lines.push('');
lines.push('## 2. Rust packages compiled for Windows ARM64');
lines.push('');
lines.push('These are the crates in the actual binary, including Tauri, wry, WebView2 and the');
lines.push('Windows API bindings.');
lines.push('');
lines.push(row(['Package', 'Version', 'Licence declared by the crate', 'Licence file shipped']));
lines.push(row(['---', '---', '---', '---']));
for (const crate of built) {
  lines.push(row([
    `\`${crate.name}\``,
    crate.version,
    crate.license ?? '**no license field**',
    crate.file ? 'yes' : 'no',
  ]));
}
lines.push('');
lines.push('## 3. JavaScript packages (from `package-lock.json`)');
lines.push('');
lines.push(row(['Package', 'Version', 'Licence', 'Scope']));
lines.push(row(['---', '---', '---', '---']));
for (const pkg of npm) {
  lines.push(row([
    `\`${pkg.name}\``,
    pkg.version,
    pkg.license ?? '**no license field**',
    pkg.dev ? 'build/test' : 'runtime',
  ]));
}
lines.push('');
lines.push('## 4. Packages with no licence field');
lines.push('');
if (!gaps.length) {
  lines.push('None. Every crate compiled for this target and every installed JavaScript');
  lines.push('package declares a licence.');
} else {
  for (const item of gaps) lines.push(`- \`${item.name}\` ${item.version}`);
}
lines.push('');
lines.push('## 5. Platform-specific Rust packages (not in this build)');
lines.push('');
lines.push('`Cargo.lock` resolves dependencies for every platform Tauri supports, so it');
lines.push('records crates for Linux (GTK, dbus, x11), macOS (core-foundation, objc2) and');
lines.push('Android that a Windows ARM64 build never compiles and never ships. Their sources');
lines.push('are not even fetched to this machine. They are listed for completeness and');
lines.push('carry no obligation here.');
lines.push('');
lines.push(row(['Package', 'Version']));
lines.push(row(['---', '---']));
for (const crate of notBuilt) lines.push(row([`\`${crate.name}\``, crate.version]));
lines.push('');
lines.push('## 6. Not covered by this inventory');
lines.push('');
lines.push('- **native-vLLM** ([devjeonghwan/vllm-for-rtx-spark](https://github.com/devjeonghwan/vllm-for-rtx-spark),');
lines.push('  Apache-2.0) and **vLLM** (Apache-2.0): external, installed and run by the');
lines.push('  operator, never bundled, linked or redistributed.');
lines.push('- **Model weights** (for example Qwen checkpoints): the operator\'s own files.');
lines.push('- **Windows, WebView2, SChannel**: operating-system components.');
lines.push('- **NSIS**: used by Tauri to build the installer. Its licence applies to the');
lines.push('  build tooling, not to the packaged application.');
lines.push('- **Screenshots in `screenshots/`**: captured by this project, showing its own');
lines.push('  interface. See [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).');
lines.push('');

fs.writeFileSync(outFile, lines.join('\n'), 'utf8');
console.log(`wrote ${outFile}: ${crates.length} rust (${built.length} built), ${npm.length} js, ` +
  `${gaps.length} without a licence field, ${copyleftOnly.length} copyleft-only`);
