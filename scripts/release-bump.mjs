#!/usr/bin/env node
// Bumps every version manifest in the repo to the same release version.
//
// Usage: node scripts/release-bump.mjs 0.2.0
//
// The release workflow runs scripts/release-check.mjs against the pushed
// `v<version>` tag before building, so package.json, the plugin, the
// per-platform optionalDependencies, src-tauri/tauri.conf.json, Cargo.toml
// and Cargo.lock must all agree.

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const version = process.argv[2];

if (!version || !/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version)) {
  console.error("usage: node scripts/release-bump.mjs <version>");
  process.exit(1);
}

const read = (path) => readFileSync(join(repoRoot, path), "utf8");
const write = (path, contents) => writeFileSync(join(repoRoot, path), contents);

const jsonFiles = [
  "package.json",
  "plugin/package.json",
  "platforms/darwin-arm64/package.json",
  "platforms/darwin-x64/package.json",
  "platforms/linux-x64/package.json",
  "platforms/win32-x64/package.json",
  "src-tauri/tauri.conf.json"
];

// Replace only the version strings so unrelated formatting in the existing
// manifests is preserved.
for (const file of jsonFiles) {
  const currentVersion = JSON.parse(read(file)).version;
  if (currentVersion !== version) {
    const next = read(file).replace(
      `"version": "${currentVersion}"`,
      `"version": "${version}"`
    );
    write(file, next);
  }
}

// Optional platform packages are always published at the plugin version so
// every optionalDependency range in the plugin manifest resolves exactly.
const pluginPath = "plugin/package.json";
const plugin = JSON.parse(read(pluginPath));
const pluginText = read(pluginPath);
let pluginNext = pluginText;
for (const [dependency, currentVersion] of Object.entries(
  plugin.optionalDependencies ?? {}
)) {
  if (currentVersion !== version) {
    pluginNext = pluginNext.replace(
      `"${dependency}": "${currentVersion}"`,
      `"${dependency}": "${version}"`
    );
  }
}
if (pluginNext !== pluginText) {
  write(pluginPath, pluginNext);
}

const cargoTomlPath = "src-tauri/Cargo.toml";
const cargoToml = read(cargoTomlPath);
const cargoVersion = cargoToml.match(/^version = "([^"]+)"$/m)?.[1];
if (!cargoVersion) {
  throw new Error(`package version not found in ${cargoTomlPath}`);
}
if (cargoVersion !== version) {
  write(
    cargoTomlPath,
    cargoToml.replace(
      /^version = "[^"]+"$/m,
      `version = "${version}"`
    )
  );
}

const cargoLockPath = "src-tauri/Cargo.lock";
const cargoLock = read(cargoLockPath);
const lockVersion = cargoLock.match(
  /name = "dsh-desktop"\nversion = "([^"]+)"/
)?.[1];
if (!lockVersion) {
  throw new Error(`dsh-desktop version not found in ${cargoLockPath}`);
}
if (lockVersion !== version) {
  write(
    cargoLockPath,
    cargoLock.replace(
      /(name = "dsh-desktop"\nversion = )"[^"]+"/,
      `$1"${version}"`
    )
  );
}

console.log(`bumped release version to ${version}`);
