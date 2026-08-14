# dsh-desktop

English | [中文](README.zh.md)

A Tauri 2 desktop shell for the DeepSeek Harness Web surface. The shell spawns one harness runtime (`dsh web` by default), waits for the loopback URL the runtime prints on stdout, and opens a single webview window on that URL. It contributes no application UI: the window renders the product's own Web frontend, served by the runtime over HTTP on 127.0.0.1.

This project is **powered by DeepSeek Harness** (`dsh`) — it launches and supervises the runtime, which is a separate project. The codebase is **written AI-natively**.

## How it works

On startup the shell launches the command selected by `DSH_BIN` plus `DSH_ARGS` (default `dsh web`), forces `--host 127.0.0.1`, and appends `--port <port>` when `DSH_PORT` is set. It scans the child's stdout for a loopback URL (`dsh web: http://127.0.0.1:<port>`) and opens the webview window only once that URL appears; child stdout lines are echoed with a `[dsh]` prefix and stderr is inherited. If the runtime exits before publishing a URL, or no URL appears within 60 seconds, the shell opens a static error window instead.

Closing the last window terminates the runtime: SIGTERM first so the harness drains (session flush, terminal restore), then SIGKILL after a five-second grace window; on Windows the shell kills the process tree. If the runtime dies on its own, the shell closes.

The Web trust fence needs no configuration here: the window navigates to the loopback origin, so every request presents a loopback `Host`.

## Design notes

**Why a shell, not a second UI.** The Web host deliberately serves browsers only — a webview loading the built `dist` over `file://` would not be same-origin with the runtime's `/api` — so the shell supervises the existing runtime over its own loopback HTTP: one process, one window, and no second copy of the UI. No Tauri API is exposed to the page, so no capability grants are needed.

**Alternatives rejected.** Loading `dist` over `file://` (breaks the same-origin trust model and duplicates the connection layer); a fixed port plus TCP polling (the printed URL line is the runtime's own single source of truth); a Cordis UI bundle inside the runtime (window lifecycle and process supervision are launcher concerns, kept out of the runtime so each side stays independently patchable); bundling the runtime into the app (deferred to the harness's single-file distribution work).

**Verification.** `cargo test` pins URL scanning (LAN suffixes and portless URLs are ignored), env-command assembly, spawn–terminate–reap, and early-exit reporting. The assembled smoke runs the debug binary against a source-launched `dsh web` on a fresh port and checks the published URL, a 200 with `window.__DSH_BOOT__` injected, and that SIGTERM to the shell releases the port.

## Prerequisites

- A DeepSeek Harness runtime — one of:
  - an installed `dsh` CLI (`npm i -g @deepseek-ai/dsh`), used by the default `dsh web` command; or
  - a source checkout of [deepseek-harness](https://github.com/deepseek-ai/deepseek-harness) with built Web artifacts (`pnpm run build`), for the source-checkout command below.
- Node.js and pnpm, for this project's scripts and the Tauri CLI.
- The Rust toolchain (via [rustup](https://rustup.rs/)), for building the shell.
- Tauri's platform dependencies: Xcode Command Line Tools on macOS, WebView2 plus the MSVC build tools on Windows, and webkit2gtk-4.1 (or the distro equivalent) on Linux — see [Tauri's prerequisites](https://tauri.app/start/prerequisites/).

## Running

```sh
pnpm install   # installs the Tauri CLI
pnpm dev       # builds the shell in dev mode and launches it
```

`pnpm dev` spawns `dsh web` from PATH and opens the window once the runtime prints its URL — no window appears before that. The runtime's stdout is echoed on the launching terminal with a `[dsh]` prefix; watch that terminal if the window is slow to appear.

To develop against a harness source checkout instead:

```sh
DSH_BIN=node \
  DSH_ARGS="--import tsx/esm <checkout>/apps/cli/src/bin.ts web" \
  DSH_PORT=3180 \
  pnpm dev
```

`DSH_PORT` defaults to the harness default (3080); pick another port when something already serves 3080.

## Building a release bundle

```sh
pnpm build             # release build plus platform installers
pnpm build:no-bundle   # release binary only
```

Installers land under `src-tauri/target/release/bundle` (`.app`/`.dmg` on macOS, `.msi`/`.exe` on Windows, `.deb`/`.rpm`/`AppImage` on Linux). The bundles are unsigned — see [Known Limitations](#known-limitations-and-deferred-work).

## Configuration

| Variable | Default | Meaning |
|---|---|---|
| `DSH_BIN` | `dsh` | The executable to spawn. |
| `DSH_ARGS` | `web` | Arguments appended after `DSH_BIN`, split on whitespace. |
| `DSH_PORT` | unset | When set, `--port <value>` is appended. |

The split-on-whitespace parsing cannot quote arguments; point `DSH_BIN` at a wrapper script when an argument contains spaces.

## Troubleshooting

- **The window shows the static error page** — the runtime failed before publishing its URL, or the 60-second wait timed out. Launch from a terminal and read the runtime's stderr.
- **Build fails with `failed to read plugin permissions … No such file or directory`** — the repository was moved or renamed after a previous build, leaving stale absolute paths in the build cache. Run `cargo clean` in `src-tauri` (or delete `src-tauri/target`) and retry.
- **Port 3080 is already in use** — set `DSH_PORT` to a free port.
- **`dsh: command not found`** — install the CLI (`npm i -g @deepseek-ai/dsh`) or point `DSH_BIN` at your runtime.

## Known Limitations and Deferred Work

- **The runtime is not bundled** — the shell spawns a separately installed or built harness; a single-app distribution that embeds the runtime is deferred work.
- **The shutdown ladder is untested on Windows** — SIGTERM draining is Unix-only; Windows uses `taskkill /T /F`.
- **No macOS code signing or notarization** — `pnpm build` produces unsigned bundles; distributing them outside local use needs signing credentials.
- **Error details stay on stderr** — the failure window is static; launch from a terminal to read the runtime's diagnostics.

## License and Notices

MIT License — see [LICENSE](LICENSE).

### DeepSeek Harness notice

This project is powered by [DeepSeek Harness](https://github.com/deepseek-ai/deepseek-harness) (`dsh`): the shell launches and supervises a dsh runtime as a child process. The runtime is a separate project distributed under the MIT License, Copyright (c) 2026 DeepSeek; it is not bundled in or distributed with this repository.

The application icon under `icons-src/app-icon.svg` and `src-tauri/icons/` is derived from the DeepSeek Harness Web favicon glyph ([apps/web/public/favicon.svg](https://github.com/deepseek-ai/deepseek-harness/blob/master/apps/web/public/favicon.svg)), used under that same MIT License.

The full MIT permission notice lives in the [harness repository](https://github.com/deepseek-ai/deepseek-harness).
