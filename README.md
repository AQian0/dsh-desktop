# dsh-desktop

English | [中文](README.zh.md)

A Tauri 2 desktop shell for the DeepSeek Harness Web surface. The dsh plugin
`@aqian0/dsh-desktop-plugin` launches `dsh-desktop --attach http://127.0.0.1:<port>`
once the Web runtime binds its loopback port, and opens the Web app in a single
desktop window. The runtime remains the parent process; closing the window exits
the profile, and the window never outlives the runtime.

## Prerequisites

- [`dsh`](https://github.com/deepseek-ai/deepseek-harness) CLI:
  `npm i -g @deepseek-ai/dsh`
- For source installs: Node.js + pnpm, Rust, and the
  [Tauri platform prerequisites](https://tauri.app/start/prerequisites/).

## Installation

### Quick install as a dsh plugin

```sh
dsh plugin --profile desktop add @aqian0/dsh-desktop-plugin
dsh --profile desktop
```

The plugin package ships per-platform prebuilt shell binaries through
optionalDependencies, so no Rust toolchain is required.

If the profile was newly created and has no Web bundle, set
`dsh.profile.bundles` in `~/.dsh/profiles/desktop/package.json` to:

```json
["@deepseek-ai/dsh-base", "@deepseek-ai/dsh-web-app", "@aqian0/dsh-desktop-plugin"]
```

`@deepseek-ai/dsh-web-app` resolves from the installed `dsh`; do not add it from
npm.

### Manual install from source

```sh
git clone https://github.com/aqian0/dsh-desktop.git
cd dsh-desktop
pnpm install
pnpm shell:build
pnpm plugin:install   # adds ./plugin to the desktop profile and configures bundles
DSH_DESKTOP_BIN=src-tauri/target/debug/dsh-desktop dsh --profile desktop
```

`pnpm plugin:install` is idempotent; after pulling changes re-run
`pnpm shell:build && pnpm plugin:install`.

## Build and packaging

```sh
pnpm build:no-bundle              # release binary
pnpm build                        # release binary + platform installers
pnpm package:current -- --build   # package plugin + current-platform binary for npm
pnpm plugin:smoke                 # GUI-free lifetime smoke test
```

## License

MIT — see [LICENSE](LICENSE). The app icon under `icons-src/` and
`src-tauri/icons/` is derived from the DeepSeek Harness Web favicon and is used
under the same MIT license.
