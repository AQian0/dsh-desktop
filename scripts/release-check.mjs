#!/usr/bin/env node
// Validates that every release manifest in the repo agrees with the version
// passed on the command line (locally) or `v<version>` from GitHub Actions.
//
// Usage: node scripts/release-check.mjs v0.2.0

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const version = process.argv[2]?.replace(/^v/, "");

if (!version || !/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version)) {
  console.error("usage: node scripts/release-check.mjs <v?version>");
  process.exit(1);
}

const read = (path) => readFileSync(join(repoRoot, path), "utf8");
const fail = (message) => {
  console.error(`release version mismatch: ${message}`);
  process.exit(1);
};

const jsonFiles = [
  "package.json",
  "plugin/package.json",
  "platforms/darwin-arm64/package.json",
  "platforms/darwin-x64/package.json",
  "platforms/linux-x64/package.json",
  "platforms/win32-x64/package.json",
  "src-tauri/tauri.conf.json"
];

for (const file of jsonFiles) {
  const pkg = JSON.parse(read(file));
  if (pkg.version !== version) {
    fail(`${file} is ${pkg.version}, expected ${version}`);
  }
}

const plugin = JSON.parse(read("plugin/package.json"));
for (const [dependency, range] of Object.entries(plugin.optionalDependencies ?? {})) {
  if (range !== version) {
    fail(`${dependency} optionalDependency is ${range}, expected ${version}`);
  }
}

const cargoToml = read("src-tauri/Cargo.toml");
const cargoVersion = cargoToml.match(/^version = "([^"]+)"$/m)?.[1];
if (cargoVersion !== version) {
  fail(`src-tauri/Cargo.toml is ${cargoVersion}, expected ${version}`);
}

const cargoLock = read("src-tauri/Cargo.lock");
const lockVersion = cargoLock.match(
  /name = "dsh-desktop"\nversion = "([^"]+)"/
)?.[1];
if (lockVersion !== version) {
  fail(`src-tauri/Cargo.lock dsh-desktop is ${lockVersion}, expected ${version}`);
}

console.log(`release version ${version} is consistent`);
