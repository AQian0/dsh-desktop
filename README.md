# dsh-desktop

English | [中文](README.zh.md)

A Tauri 2 desktop shell for the DeepSeek Harness Web surface. It **neither spawns nor supervises a runtime**: the desktop profile's plugin (this repository's `plugin/`, package `@aqian0/dsh-desktop-plugin`) launches this binary as `dsh-desktop --attach http://127.0.0.1:<port>` once the Web runtime binds its loopback port, and the shell opens a single webview window on that URL. It contributes no application UI: the window renders the product's own Web frontend, served by the runtime over HTTP on 127.0.0.1.

This project is **powered by DeepSeek Harness** (`dsh`) — the runtime is the parent process and the shell is the launched side; the runtime is a separate project. The codebase is **written AI-natively**.

## Quick start

```sh
pnpm install                 # installs dependencies (the Tauri CLI)
pnpm shell:build             # builds the debug shell binary (src-tauri/target/debug/dsh-desktop)
pnpm plugin:install          # installs the desktop profile (this plugin included; safe to re-run)
pnpm plugin:smoke            # (optional) two-way lifetime smoke, no GUI
pnpm plugin:run              # boots dsh --profile desktop and opens the desktop window
```

To use another port: `dsh --profile desktop --port 3081` (any web-profile flag works).
After pulling changes, re-run `pnpm shell:build && pnpm plugin:install` (the install is idempotent).

## How it works

The plugin's `desktop-launch` row spawns this binary with the loopback URL once the web server binds, then ties the lifetime in both directions:

- **the window closes** (the user closes the last window): the shell process exits and the plugin requests a graceful profile exit through `ctx.appExit` (session flush happens during the harness's tree disposal; the plugin carries a bounded 5-second force-exit grace against launcher-level handles holding the event loop);
- **the runtime dies first** (signal, crash, kill -9): three parallel links close the window — the plugin's `ctx.effect` cleanup SIGTERMs the shell as the tree disposes, the shell's own stdin pipe EOF, and the unix parent-reparenting poll (macOS rewires the app's stdio during bootstrap, so the poll is the load-bearing link there). The window never outlives its runtime.

The Web trust fence needs no configuration here: the window navigates to the loopback origin, so every request presents a loopback `Host`.

## Design notes

**Why a shell, not a second UI.** The Web host deliberately serves browsers only — a webview loading the built `dist` over `file://` would not be same-origin with the runtime's `/api` — so the shell attaches to the existing runtime over its own loopback HTTP: one process, one window, and no second copy of the UI. No Tauri API is exposed to the page, so no capability grants are needed.

**The runtime is the parent; the window lifecycle stays inside the shell.** `plugin/` packages the shell as an installable dsh bundle: the desktop profile's bundles list `@deepseek-ai/dsh-base`, `@deepseek-ai/dsh-web-app`, and this plugin in order, so it fully inherits the Web profile. The plugin spawns the shell once the web server binds, the closing window requests a profile exit, and tree disposal closes the window in return. Window lifecycle and parent-death detection stay inside the shell binary; the runtime merely gains a launcher role.

**Alternatives rejected.** Loading `dist` over `file://` (breaks the same-origin trust model and duplicates the connection layer); a Cordis UI bundle inside the runtime (the window lifecycle belongs to the shell, not the runtime; keeping it out lets each side stay independently patchable); the shell spawning and supervising the runtime instead (the old supervisor shape: the runtime became a child, so the exit ladder and stdout URL scanning lived in the shell — with the plugin shape the runtime is naturally the parent and all of that machinery was deleted); bundling the runtime into the app (deferred to the harness's single-file distribution work).

**Verification.** `cargo test` pins attach-argument parsing (absent, non-loopback, portless, malformed) and loopback URL token scanning (LAN suffixes and portless URLs are ignored); `pnpm plugin:smoke` regression-tests both lifetime directions GUI-free (window close -> the profile exits with code 0; SIGTERM to dsh -> tree disposal -> the plugin's effect cleanup kills the shell); a real-binary smoke verifies the shell exits with its parent (within a second on macOS).

## Prerequisites

- An installed `dsh` CLI (`npm i -g @deepseek-ai/dsh`) with the desktop profile installed as described below.
- Node.js and pnpm, for this project's scripts and the Tauri CLI.
- The Rust toolchain (via [rustup](https://rustup.rs/)), for building the shell.
- Tauri's platform dependencies: Xcode Command Line Tools on macOS, WebView2 plus the MSVC build tools on Windows, and webkit2gtk-4.1 (or the distro equivalent) on Linux — see [Tauri's prerequisites](https://tauri.app/start/prerequisites/).

## Installing as a dsh profile plugin

The repository root's `plugin/` directory is an installable bundle package (`@aqian0/dsh-desktop-plugin`): its `package.json` declares `dsh.bundle.patch`, its `cordis.patch.yml` adds one `desktop-launch` row, and the host plugin spawns the shell once the web server binds, tying the profile's lifetime to the window (closing the window exits, tree disposal closes the window). dsh has no explicit profile inheritance; inheriting the web profile is bundle composition:

```sh
pnpm plugin:install   # runs dsh plugin --profile desktop add ./plugin, then lists web-app in bundles
pnpm plugin:smoke     # GUI-free two-way lifetime smoke: window close -> profile exit; runtime death -> shell close
pnpm plugin:run       # equivalent to dsh --profile desktop
```

The resulting `~/.dsh/profiles/desktop/package.json` lists `dsh.profile.bundles` as `["@deepseek-ai/dsh-base", "@deepseek-ai/dsh-web-app", "@aqian0/dsh-desktop-plugin"]`; the config tree stacks in order, last write wins. `@deepseek-ai/dsh-web-app` is listed in bundles but never installed as a profile dependency - bundle resolution tries the running dsh installation first, so the Web surface always matches the installed dsh version. Shell-binary resolution order and distribution plans live in `plugin/README.md`.

## Running (source-checkout development)

```sh
pnpm install        # installs the Tauri CLI
pnpm shell:build    # builds the debug shell binary

# point the shell at the debug binary, launched by the desktop profile
DSH_DESKTOP_BIN=src-tauri/target/debug/dsh-desktop dsh --profile desktop
```

If the profile installed by `pnpm plugin:install` cannot find the shell on PATH, pin the binary by
writing `config: { bin: <absolute path> }` for the `desktop-launch` row in
`~/.dsh/profiles/desktop/cordis.patch.yml`. Running the binary directly without a valid `--attach`
prints guidance and shows the static error window: the shell can only be launched by the profile
plugin.

## Building a release bundle

```sh
pnpm build             # release build plus platform installers
pnpm build:no-bundle   # release binary only
```

Installers land under `src-tauri/target/release/bundle` (`.app`/`.dmg` on macOS, `.msi`/`.exe` on Windows, `.deb`/`.rpm`/`AppImage` on Linux). The bundles are unsigned — see [Known Limitations](#known-limitations-and-deferred-work).

## Packaging for npm (install and run)

The plugin ships the shell binary as per-platform optionalDependencies, so users need only
`dsh plugin --profile desktop add @aqian0/dsh-desktop-plugin` — no environment variables:

```sh
pnpm package:current -- --build   # release build, then copies the binary into
                                  # platforms/<current-platform>-<arch>/bin/ and packs two
                                  # tarballs into dist/: the plugin + the current platform package
```

Publish the plugin and ALL platform packages (the 4 targets under `platforms/`) together, at the
same version:

```sh
npm publish dist/<plugin tgz> dist/<platform tgz> ...
```

Binaries for other platforms must be built on their own OS (or by a CI matrix).

## Configuration

| Variable | Default | Meaning |
|---|---|---|
| `DSH_DESKTOP_BIN` | unset | The plugin's path to the shell executable; unset falls back to the row's `bin` config, the per-platform bundled binary, or PATH. |

## Troubleshooting

- **The window shows the static error page** — the binary was run directly without a valid `--attach <url>`. Launch from a terminal to read the stderr hint; the normal entry is `dsh --profile desktop` (the plugin launches the shell).
- **Build fails with `failed to read plugin permissions … No such file or directory`** — the repository was moved or renamed after a previous build, leaving stale absolute paths in the build cache. Run `cargo clean` in `src-tauri` (or delete `src-tauri/target`) and retry.
- **`dsh: command not found`** — install the CLI (`npm i -g @deepseek-ai/dsh`).
- **`pnpm plugin:smoke` says the profile is not installed** — run `pnpm plugin:install` first.

## Known Limitations and Deferred Work

- **The runtime is not bundled** — the shell is attach-only and depends on a separately installed dsh with the desktop profile; a single-app distribution that embeds the runtime is deferred work.
- **No macOS code signing or notarization** — `pnpm build` produces unsigned bundles; distributing them outside local use needs signing credentials.
- **Error details stay on stderr** — the failure window is static; launch from a terminal to read the diagnostics.
- **Multi-platform binaries need a per-OS build matrix** — packaging the current platform is implemented (`pnpm package:current`); tarballs for the other platforms must be built on their own OS (or by a CI matrix).

## License and Notices

MIT License — see [LICENSE](LICENSE).

### DeepSeek Harness notice

This project is powered by [DeepSeek Harness](https://github.com/deepseek-ai/deepseek-harness) (`dsh`): the shell is launched by a dsh runtime through the desktop profile plugin and attaches to the runtime's own loopback Web service; it neither bundles nor distributes the runtime. The runtime is a separate project distributed under the MIT License, Copyright (c) 2026 DeepSeek.

The application icon under `icons-src/app-icon.svg` and `src-tauri/icons/` is derived from the DeepSeek Harness Web favicon glyph ([apps/web/public/favicon.svg](https://github.com/deepseek-ai/deepseek-harness/blob/master/apps/web/public/favicon.svg)), used under that same MIT License.

The full MIT permission notice lives in the [harness repository](https://github.com/deepseek-ai/deepseek-harness).
