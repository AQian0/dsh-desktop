// Lifecycle smoke for the installed desktop profile, no GUI required:
// exercises both lifetime directions with fake shell binaries.
//
// Test A (window close -> profile exit): the fake shell exits 0 immediately;
// the profile must exit on its own with code 0 (graceful appExit plus the
// bounded force-exit grace).
// Test B (runtime death -> window close): the fake shell sleeps and records
// its pid; SIGTERM to the dsh process must dispose the tree, run this
// plugin's ctx.effect cleanup, and kill the fake shell.
//
// Run: pnpm plugin:smoke  (requires the desktop profile installed:
// pnpm plugin:install)

import { spawn } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:net";
import { homedir, tmpdir } from "node:os";
import { join } from "node:path";

const DSH_BIN = process.env.DSH_BIN ?? "dsh";
const PROFILE = "desktop";
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

function freePort() {
  return new Promise((resolve, reject) => {
    const server = createServer();
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const port = server.address().port;
      server.close(() => resolve(port));
    });
  });
}

async function waitFor(predicate, timeoutMs, what) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (predicate()) return;
    await sleep(200);
  }
  throw new Error("timed out waiting for " + what);
}

function isAlive(pid) {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

// The installed profile must exist and carry the plugin bundle.
const dshHome = process.env.DSH_HOME ?? join(homedir(), ".dsh");
const manifestPath = join(dshHome, "profiles", PROFILE, "package.json");
if (!existsSync(manifestPath)) {
  throw new Error("profile " + PROFILE + " is not installed; run pnpm plugin:install first");
}
const bundles = JSON.parse(readFileSync(manifestPath, "utf8")).dsh?.profile?.bundles ?? [];
if (!bundles.includes("@deepseek-ai/dsh-desktop-plugin")) {
  throw new Error("profile " + PROFILE + " bundles lack @deepseek-ai/dsh-desktop-plugin; run pnpm plugin:install first");
}

const dir = mkdtempSync(join(tmpdir(), "dsh-desktop-smoke-"));
const exitingShell = join(dir, "exiting-shell.sh");
writeFileSync(exitingShell, "#!/bin/sh\nexit 0\n", { mode: 0o755 });
const sleeperPidFile = join(dir, "sleeper.pid");
const sleeperShell = join(dir, "sleeper-shell.sh");
writeFileSync(
  sleeperShell,
  "#!/bin/sh\necho \"$$\" > " + JSON.stringify(sleeperPidFile) + "\nsleep 300\n",
  { mode: 0o755 }
);

function spawnDsh(port, shellBin) {
  const child = spawn(DSH_BIN, ["--profile", PROFILE, "--port", String(port)], {
    env: { ...process.env, DSH_DESKTOP_BIN: shellBin },
    stdio: ["ignore", "pipe", "pipe"]
  });
  let output = "";
  child.stdout.on("data", (chunk) => { output += chunk; });
  child.stderr.on("data", (chunk) => { output += chunk; });
  const waitExit = (timeoutMs) =>
    new Promise((resolve) => {
      const timer = setTimeout(() => resolve("timeout"), timeoutMs);
      child.once("exit", (code) => {
        clearTimeout(timer);
        resolve("code=" + code);
      });
    });
  return { child, getOutput: () => output, waitExit };
}

try {
  // Test A: the window closes (fake shell exits 0) -> the profile exits 0.
  {
    const port = await freePort();
    const run = spawnDsh(port, exitingShell);
    const outcome = await run.waitExit(25000);
    if (outcome !== "code=0") {
      throw new Error("Test A failed (" + outcome + "):\n" + run.getOutput());
    }
    console.log("Test A (window close -> profile exit 0): ok");
  }

  // Test B: the runtime dies first (SIGTERM) -> tree disposal kills the shell.
  {
    const port = await freePort();
    const run = spawnDsh(port, sleeperShell);
    await waitFor(() => existsSync(sleeperPidFile), 20000, "sleeper shell spawn");
    const sleeperPid = Number(readFileSync(sleeperPidFile, "utf8").trim());
    run.child.kill("SIGTERM");
    const outcome = await run.waitExit(15000);
    if (outcome !== "code=0") {
      throw new Error("Test B dsh exit failed (" + outcome + "):\n" + run.getOutput());
    }
    await waitFor(() => !isAlive(sleeperPid), 5000, "sleeper shell death");
    console.log("Test B (runtime death -> tree disposal kills shell): ok");
  }
} finally {
  rmSync(dir, { recursive: true, force: true });
}

console.log("lifecycle smoke passed");
