// Packages the plugin plus the current platform's shell-binary package:
//   - locates (or with --build, builds) the release shell binary,
//   - copies it into platforms/<platform>-<arch>/bin/,
//   - npm-packs the plugin and the platform package into dist/.
//
// The tarballs are what npm publish ships: publish the plugin and every
// platform package together, all at the same version, so the plugin's
// optionalDependencies resolve for every supported target.

import { execFileSync } from "node:child_process";
import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  writeFileSync
} from "node:fs";
import { arch, platform } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const slug = `${platform()}-${arch()}`;
const binName = platform() === "win32" ? "dsh-desktop.exe" : "dsh-desktop";
const releaseBin = join(repoRoot, "src-tauri", "target", "release", binName);
const platformDir = join(repoRoot, "platforms", slug);
const distDir = join(repoRoot, "dist");

if (!existsSync(join(platformDir, "package.json"))) {
  throw new Error(`no platform package for ${slug}; create platforms/${slug}/package.json first`);
}

if (!existsSync(releaseBin)) {
  if (process.argv.includes("--build")) {
    execFileSync("cargo", ["build", "--release", "--manifest-path", join(repoRoot, "src-tauri", "Cargo.toml")], { stdio: "inherit" });
  } else {
    throw new Error(`release binary missing at ${releaseBin}; run pnpm package:current -- --build, or build it first`);
  }
}

// Copy the binary into the platform package.
mkdirSync(join(platformDir, "bin"), { recursive: true });
copyFileSync(releaseBin, join(platformDir, "bin", binName));
if (platform() !== "win32") chmodSync(join(platformDir, "bin", binName), 0o755);

// Version sync: platform packages always ship at the plugin's version.
const pluginVersion = JSON.parse(readFileSync(join(repoRoot, "plugin", "package.json"), "utf8")).version;
const platformPkgPath = join(platformDir, "package.json");
const platformPkg = JSON.parse(readFileSync(platformPkgPath, "utf8"));
if (platformPkg.version !== pluginVersion) {
  platformPkg.version = pluginVersion;
  writeFileSync(platformPkgPath, JSON.stringify(platformPkg, null, 2) + "\n");
}

// Pack both into dist/.
mkdirSync(distDir, { recursive: true });
const pack = (dir) => {
  // npm is a .cmd shim on Windows, which execFile cannot run without a shell
  // (rejected outright on current Node), so route through the shell there.
  const lines = execFileSync("npm", ["pack", "--pack-destination", distDir], {
    cwd: dir,
    encoding: "utf8",
    shell: platform() === "win32"
  }).trim().split("\n");
  return join(distDir, lines[lines.length - 1]);
};
const pluginTarball = pack(join(repoRoot, "plugin"));
const platformTarball = pack(platformDir);
console.log(`packed ${pluginTarball}`);
console.log(`packed ${platformTarball}`);
console.log("publish together at the same version: npm publish <plugin tgz> <platform tgz> ...");
