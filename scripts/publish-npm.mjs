#!/usr/bin/env node
// Publishes one npm tarball idempotently.
//
// Usage:
//   node scripts/publish-npm.mjs <platform-slug|plugin> <dist-dir> <v0.2.0>
//
// Publishing uses npm trusted publishing (OIDC) configured in the GitHub
// Actions workflow, so no npm token is needed. If the exact package version
// already exists on npm, the script skips publishing, which makes the
// release workflow safe to re-run.

import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { resolve } from "node:path";

const [target, distDir, ref] = process.argv.slice(2);
const version = ref?.replace(/^v/, "");

if (!target || !distDir || !version) {
  console.error("usage: node scripts/publish-npm.mjs <target> <dist-dir> <v?version>");
  process.exit(1);
}
if (!existsSync(distDir)) {
  console.error(`dist dir does not exist: ${distDir}`);
  process.exit(1);
}

const name =
  target === "plugin"
    ? "@aqian0/dsh-desktop-plugin"
    : `@aqian0/dsh-desktop-plugin-${target}`;
const tarball =
  target === "plugin"
    ? `aqian0-dsh-desktop-plugin-${version}.tgz`
    : `aqian0-dsh-desktop-plugin-${target}-${version}.tgz`;
const tarballPath = resolve(distDir, tarball);

if (!existsSync(tarballPath)) {
  console.error(`expected tarball not found: ${tarballPath}`);
  process.exit(1);
}

const spec = `${name}@${version}`;
const alreadyPublished = (() => {
  try {
    execFileSync("npm", ["view", spec, "version"], { stdio: "ignore" });
    return true;
  } catch {
    return false;
  }
})();

if (alreadyPublished) {
  console.log(`${spec} is already on npm; skipping publish`);
} else {
  console.log(`publishing ${tarballPath} as ${spec}`);
  execFileSync(
    "npm",
    ["publish", tarballPath, "--access", "public"],
    { stdio: "inherit" }
  );
}
