# dsh-desktop

English | [中文](README.zh.md)

A Tauri 2 desktop shell for the DeepSeek Harness Web surface. The dsh plugin
`@aqian0/dsh-desktop-plugin` launches `dsh-desktop --attach <url>` with the Web
runtime's authenticated loopback URL and opens the Web app in a single desktop
window. The runtime remains the parent process; closing the window exits
the profile, and the window never outlives the runtime.

## Prerequisites

- [`dsh`](https://github.com/deepseek-ai/deepseek-harness) CLI 0.1.2-rc.1 or newer:
  `npm i -g @deepseek-ai/dsh@next`
- For source installs: Node.js + pnpm, Rust, and the
  [Tauri platform prerequisites](https://tauri.app/start/prerequisites/).

## Installation

### Quick install as a dsh plugin

```sh
dsh plugin --profile desktop add @aqian0/dsh-desktop-plugin
dsh --profile desktop
```

The plugin package ships per-platform prebuilt shell binaries through
optionalDependencies, so no Rust toolchain is required. Its bundle layer also
pins `web-runtime.openBrowser` to `false`, so `dsh --profile desktop` opens
only the desktop window and never hands the URL to the system default browser.
Inside the shell, same-origin Web app routes keep navigating in the window,
while links to other origins and `mailto:`/`tel:` links are handed to the
system default browser or app. The initial URL carries a one-time process token
that the Web runtime exchanges for its session cookie.

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

## Desktop startup and input focus

macOS, Windows, and Linux share one startup sequence:

1. Create the native window hidden, without requesting initial window/WebView focus.
2. Wait for Tauri `Ready` and the next event-loop drain, then request show once.
3. Once the window reports visible and not minimized, request native-window focus
   and embedded-WebView focus once, then retire the startup handler.

On Linux, GTK applies show asynchronously. The handler restores GTK focusability
before show and waits for visibility on subsequent event-loop drains rather than
issuing a focus request that the still-hidden window would ignore. On macOS this
also separates window presentation from AppKit's launch callback; WKWebView may
still activate the application during construction.

Startup does not wait for page loading and applies to the startup-error page too.
A close/destroy event, an observed minimized state, or a main-window blur event
while waiting for visibility cancels pending focus. The shell does not run a focus
timer or re-trigger startup on page reloads, app switches, or display changes.
Activation remains subject to Windows foreground rules and Linux window-manager/
compositor policy, including Wayland;
a successful API call does not guarantee foreground activation.

Rust source changes do not update an installed prebuilt binary. To test locally,
run from the project root (macOS/Linux):

```sh
pnpm shell:build
DSH_DESKTOP_BIN="$PWD/src-tauri/target/debug/dsh-desktop" dsh --profile desktop
```

Windows PowerShell:

```powershell
pnpm shell:build
$env:DSH_DESKTOP_BIN = Join-Path $PWD "src-tauri/target/debug/dsh-desktop.exe"
dsh --profile desktop
```

Run the platform-independent startup state tests with
`cargo test --locked --manifest-path src-tauri/Cargo.toml`. The package/release
matrices also run these tests on each native build runner before packaging.

Manual regression checks (require each native desktop; unit tests are not enough):

- Repeat cold launches on primary and secondary displays with mixed scaling;
  cover macOS Spaces, Windows foreground restrictions, and Linux X11/Wayland.
  When activation is granted, buttons and text input should work immediately,
  without first clicking another display.
- Switch to another app, reload the page, and minimize or close the window during
  startup and afterwards; it must not keep reclaiming focus or restoring itself.
- Verify the window still appears with a slow/unavailable Web page and when
  launched without `--attach` (the startup-error page).
- Check single-display startup and both lifetime directions: closing the window
  exits the profile, and stopping the runtime closes the window.

## License

MIT — see [LICENSE](LICENSE). The app icon under `icons-src/` and
`src-tauri/icons/` is derived from the DeepSeek Harness Web favicon and is used
under the same MIT license.
