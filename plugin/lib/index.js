import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

//#region lib/index.js
/**
* @aqian0/dsh-desktop-plugin — desktop-launch host plugin. The bundle
* patch adds one row that mounts after the web server binds; this plugin
* spawns the desktop shell binary attached to the loopback URL and ties the
* profile's lifetime to the window in both directions:
*
* - window closed by the user: the shell exits, and the plugin requests a
*   graceful app exit through the launcher-provided ctx.appExit (with a
*   bounded force-exit grace, since launcher-level handles can hold the
*   event loop after the tree disposes);
* - runtime dying first (signal, crash, kill -9): the shell's attach mode
*   watches the runtime — stdin EOF where the platform keeps the plugin's
*   pipe as fd 0, plus a parent-reparenting poll on unix (macOS rewires the
*   app's stdio during bootstrap, so the poll is the load-bearing link
*   there) — and exits with it. Tree disposal additionally SIGTERMs the
*   child: the root's dispose cascades through the loader to this row and
*   runs the effect cleanup (verified end to end on dsh).
*
* The shell binary resolves, in precedence order, from the row's `bin`
* config, DSH_DESKTOP_BIN, a binary bundled beside this package (future
* per-platform optionalDependencies), then `dsh-desktop` on PATH.
* @module @aqian0/dsh-desktop-plugin
*/
/** Stable Cordis plugin name. */
const name = "desktop-launch";
/** Services required: the bound web server and the launcher's exit request. */
const inject = ["webServer", "appExit"];
/** The canonical loopback host, matching the web-app's published URL. */
const LOOPBACK_HOST = "127.0.0.1";
/** The plugin package root, for locating a bundled shell binary. */
const PACKAGE_ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
/**
* Exit-request grace: ctx.appExit triggers the tree dispose (session flush)
* and sets process.exitCode, but launcher-level handles (the live patch
* watchers) can keep the event loop alive after dispose settles, which hangs
* the process. Mirror the harness's own bounded-shutdown grace: if the
* process is still alive this long after the request, force the exit.
*/
const EXIT_GRACE_MS = 5e3;

/**
* Resolve the desktop shell executable.
* @param config - the row's config object (may be undefined).
* @returns the executable path or name for spawn.
*/
function resolveBin(config) {
	const explicit = config?.bin;
	if (explicit) return explicit;
	const fromEnv = process.env.DSH_DESKTOP_BIN;
	if (fromEnv) return fromEnv;
	const bundled = join(PACKAGE_ROOT, "bin", process.platform === "win32" ? "dsh-desktop.exe" : "dsh-desktop");
	if (existsSync(bundled)) return bundled;
	return "dsh-desktop";
}

/**
* Mount the desktop shell opener.
* @param ctx - plugin context carrying the webServer and appExit services.
* @param config - the row's config object.
*/
function apply(ctx, config) {
	const settled = ctx.get("loader")?.await();
	let child = null;
	let failed = false;
	// Teardown link: when the tree disposes (a signal, or another app's exit
	// request), this effect cleanup closes the shell while the window is
	// still open — verified end to end: root disposal cascades through the
	// loader to this row (internal/status 2->5) and runs the cleanup. The
	// shell's own attach-mode watcher (stdin EOF plus unix reparenting) is
	// the parallel link for runtimes that die without disposing.
	ctx.effect(() => () => {
		if (!failed && child !== null && child.exitCode === null && child.signalCode === null) {
			child.kill("SIGTERM");
		}
	});
	const requestExit = (code) => {
		const exit = ctx.get("appExit");
		if (exit === void 0) process.exitCode = code;
		else exit(code);
		setTimeout(() => {
			process.stderr.write(`dsh: ${name}: still alive ${EXIT_GRACE_MS}ms after the exit request; forcing exit ${code}\n`);
			process.exit(code);
		}, EXIT_GRACE_MS).unref();
	};
	const launch = () => {
		const webServer = ctx.get("webServer");
		if (webServer === void 0 || webServer.port === void 0) {
			// No Web surface in this composition (--help, config dumps, or a
			// profile without dsh-web-app): nothing to attach, stay inert.
			return;
		}
		const url = `http://${LOOPBACK_HOST}:${String(webServer.port)}`;
		const bin = resolveBin(config);
		// stdin is a pipe the runtime holds: EOF (runtime death) makes the
		// shell exit from its own attach-mode watcher.
		child = spawn(bin, ["--attach", url], { stdio: ["pipe", "inherit", "inherit"] });
		child.once("error", (error) => {
			failed = true;
			process.stderr.write(`dsh: ${name}: failed to start ${bin}: ${error.message}\n`);
			requestExit(1);
		});
		child.once("exit", (code, signal) => {
			requestExit(code ?? (signal === null ? 0 : 1));
		});
	};
	if (settled === void 0) launch();
	else settled.then(launch, () => {});
}
//#endregion
export { apply, inject, name, resolveBin };
