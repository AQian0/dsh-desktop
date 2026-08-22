# @aqian0/dsh-desktop-plugin

Desktop shell bundle for DeepSeek Harness: opens the Web surface in a Tauri
webview window. Installed as a profile plugin, `dsh --profile desktop` boots
the Web runtime and opens the desktop window once it binds the loopback port;
closing the window exits the profile gracefully.

## Install

```sh
# Add this plugin (local path or the published package); a missing profile is
# initialized from the dsh-base template first
dsh plugin --profile desktop add /path/to/dsh-desktop/plugin

# Then set dsh.profile.bundles in ~/.dsh/profiles/desktop/package.json to
# ["@deepseek-ai/dsh-base", "@deepseek-ai/dsh-web-app", "@aqian0/dsh-desktop-plugin"]
# The repository script does both steps for you: pnpm plugin:install

# Boot: config tree = dsh-base -> dsh-web-app -> dsh-desktop-plugin -> cordis.patch.yml
dsh --profile desktop
```

`@deepseek-ai/dsh-web-app` is listed in bundles but **never installed as a
profile dependency**: bundle resolution tries the running dsh installation
first, so the Web surface always matches the installed dsh version (the same
mechanism as the shipped web template). Do not
`dsh plugin add @deepseek-ai/dsh-web-app` - the registry's latest tag points
at an old version with unpublished dependencies.

Equivalently, write `dsh.profile.bundles` as
`["@deepseek-ai/dsh-base", "@deepseek-ai/dsh-web-app", "@aqian0/dsh-desktop-plugin"]`
directly - dsh has no explicit profile inheritance; inheriting a profile is
bundle composition.

The desktop bundle pins the `web-runtime` row's `openBrowser` to `false`: the
desktop window replaces the browser handoff, so `dsh --profile desktop` no
longer hands the URL to the system default browser. Passing `--no-open` is
still accepted but is now redundant. Inside the window, same-origin Web app
routes stay in the shell; external `http(s)`/`mailto:`/`tel:` links are handed
to the system default application.

**Verification**: the repository ships a GUI-free smoke, `pnpm plugin:smoke` -
it first asserts that the composed profile pins `web-runtime.openBrowser` to
`false` (no default-browser handoff), then exercises the window-close
direction (the fake shell exits 0, the profile exits 0 on its own) and the
runtime-death direction (SIGTERM to dsh, tree disposal, the plugin's
`ctx.effect` cleanup kills the fake shell).

## Shell binary resolution

The desktop-launch row resolves the shell executable in this order:

1. the row's `bin` config (pin it by writing `config: { bin: ... }` for the
   `desktop-launch` row in `~/.dsh/profiles/desktop/cordis.patch.yml`);
2. the `DSH_DESKTOP_BIN` environment variable (source-checkout development);
3. the per-platform bundled binary: `bin/dsh-desktop` inside
   `@aqian0/dsh-desktop-plugin-<platform>-<arch>` (this package's
   optionalDependency) - registry installs work out of the box;
4. a `bin/dsh-desktop` shipped beside this package (local packaging);
5. `dsh-desktop` on PATH.

Resolution failures fail loud: a stderr message and exit code 1.

## Packaging and publishing

```sh
pnpm package:current -- --build   # release build, then copies the binary into the
                                  # current platform's package and packs two tarballs into
                                  # dist/: the plugin + the current platform package
```

Publish the plugin and ALL platform packages (the 4 targets under `platforms/`)
together, at the same version, so the plugin's optionalDependencies resolve on
every supported platform; tarballs for other platforms must be built on their
own OS (or by a CI matrix).

## Relationship to the shell binary

The shell binary (`src-tauri`) is attach-only, with a single boot shape:

`dsh-desktop --attach http://127.0.0.1:<port>`

It opens the window only; it neither spawns nor supervises any runtime (the
runtime is the parent process). The lifetime is tied in both directions:

- closing the window exits the process and this plugin requests a graceful
  profile exit (with a bounded 5-second force-exit grace);
- when the runtime dies first (signal, crash), three parallel links close
  the shell: this plugin's `ctx.effect` cleanup SIGTERMs it as the tree
  disposes (the root fiber's dispose cascades through the loader to this
  row - verified end to end), the shell's own stdin-pipe EOF, and the unix
  parent-reparenting poll (macOS rewires the app's stdio during bootstrap,
  so the poll is the load-bearing link there). The window never outlives
  its runtime.

Running the binary directly without a valid `--attach` prints guidance and
shows the static error window. The window and URL-parsing logic live in the
shell; this plugin only owns the launch timing and the exit request. No UI
is duplicated.
