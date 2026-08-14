// Installs this repository's plugin bundle into the `desktop` dsh profile.
//
// Steps:
//   1. `dsh plugin --profile desktop add <repo>/plugin` — initializes the
//      profile when missing and appends @deepseek-ai/dsh-desktop-plugin to
//      dsh.profile.bundles (it declares dsh.bundle).
//   2. Ensure dsh.profile.bundles is exactly
//      [base, web-app, dsh-desktop-plugin]: @deepseek-ai/dsh-web-app is
//      listed but never installed as a dependency — bundle resolution tries
//      the running dsh installation first, so the web surface always matches
//      the installed dsh version instead of a registry snapshot.

import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const pluginDir = join(repoRoot, "plugin");
const dshHome = process.env.DSH_HOME ?? join(homedir(), ".dsh");
const profileDir = join(dshHome, "profiles", "desktop");
const manifestPath = join(profileDir, "package.json");

const BUNDLES = [
  "@deepseek-ai/dsh-base",
  "@deepseek-ai/dsh-web-app",
  "@deepseek-ai/dsh-desktop-plugin"
];

execFileSync("dsh", ["plugin", "--profile", "desktop", "add", pluginDir], {
  cwd: repoRoot,
  stdio: "inherit"
});

const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
manifest.dsh = manifest.dsh ?? {};
manifest.dsh.profile = manifest.dsh.profile ?? {};
manifest.dsh.profile.bundles = BUNDLES;
writeFileSync(manifestPath, JSON.stringify(manifest, null, 2) + "\n");

console.error(
  `dsh: desktop profile bundles: ${BUNDLES.join(", ")}`
);
if (!existsSync(join(profileDir, "cordis.patch.yml"))) {
  console.error("dsh: warning: profile cordis.patch.yml missing; dsh plugin should have created it");
}
