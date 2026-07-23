# `lingxia` — the LingXia CLI

What the `lingxia` command-line interface can **do** — each command's purpose, the
capability worth knowing, and when to reach for it. Assumes the CLI is installed
and you are inside a LingXia project.

`lingxia` owns the project lifecycle: scaffold, build, **start dev sessions**
(`lingxia dev`), package, sign, publish. Driving a *running* session is the
other binary's job — [`lxdev`](./lxdev.md). Signing setup, publishing, and
store submission live in [Distribution](./distribution.md) — read its signing
section before any device build or store packaging.

---

## `--help` is the source of truth for flags and values

This file teaches the **model** of `lingxia` — what each command is for and the
non-obvious behavior worth knowing. It deliberately reproduces **no** flags,
defaults, or value enums: the binary's own `--help` is exhaustive and always
matches the installed version, so read flags there.

```bash
lingxia --help               # the command list + global flags
lingxia <cmd> --help         # exact flags, defaults, and which are required
lingxia <cmd> <sub> --help   # e.g. lingxia auth apple login --help
```

**Platforms.** The platform-aware commands support `android`, `ios`, `macos`,
`harmony`, and `windows`. Which platforms a given run actually touches depends
on the project's `lingxia.yaml` and any `--platform` selection.

---

## Commands

### `lingxia new`

Scaffold a new LingXia project. Run it interactively to be prompted for project
type (a native host **app** or a standalone **lxapp**), target platforms, and
package id, or pass those up front to script it. Can also seed an app icon.

Custom React template precedence is explicit `--template <path>`, then
`~/.lingxia/templates/lxapp` when present, then the embedded template. The flag
implies `--project-type lxapp`. A custom root must contain `package.json` and
`lxapp.json`; it replaces the embedded template as one unit. Repository metadata
and generated build directories are not copied, and standard `{{...}}` scaffold
placeholders are expanded in text files.

```bash
lingxia new my-lxapp --template ../my-lxapp-template --yes
```

See `lingxia new --help` for the flags.

### `lingxia build`

Build the project. The key distinction to internalize: `--env`
(developer / preview / release) picks the **environment slot** — its package-id
suffixing and per-env server config — while `--release` picks the **compiler
profile**. They are independent; `lingxia build --env release --release` is the
shippable combination. Defaults and the per-env behavior are documented in
[App Project → Environment versions](../app/project.md#environment-versions).

Beyond plain compilation, `build` also drives the per-platform **packaging and
signing** steps when asked: a signed iOS IPA, a macOS DMG, a Windows MSIX
(optionally self-signed for local install/test), and the Android distribution
format (sideloadable APK vs a Google Play AAB). It can also build just the
native Rust library, reuse existing native binaries, inject optional native
features or a private provider crate for a single build, and select Android
ABIs / macOS arch. When a host project has `lingxia.yaml`, `build` additionally
prepares configured host assets; lxapp builds generate the Native client when
`lxapp.config.ts` declares `native`.

Flags: `lingxia build --help`. Platform signing setup: [Distribution → App signing](./distribution.md#app-signing).

### `lingxia clean`

Remove generated artifacts for the current project context (host outputs and
platform build directories in a host app; `dist/` and build caches in an
lxapp). Reach for it when a `lingxia.yaml` change seems ignored after a
rebuild.

### `lingxia package`

Package release artifacts for publishing or delivery. Always performs a release
package build. Like `build` it can choose the Android distribution format and
inject native features / a provider crate; publishable Android artifacts are
staged under `dist/android/` and macOS update zips under `dist/macos/`. Use it
when you want the staged, distributable outputs rather than a plain build.

See `lingxia package --help` for the flags.

### `lingxia dev`

Development mode for both app and lxapp projects. In an app project it builds,
installs, launches the host app, and starts a local dev websocket that `lxdev`
drives. In a standalone lxapp project it builds the lxapp and launches LingXia
Runner on macOS/Windows. Networking is handled per platform: Android and
Harmony get reverse port forwarding so the device reaches the local dev server;
iOS embeds a LAN dev websocket URL, so the iOS device must be able to reach the
host Mac over the local network.

`dev` also accepts an explicit Runner target, so the current directory remains
the owner of session state while the launched content lives elsewhere:

```bash
lingxia dev ../my-lxapp                  # build and run a standalone lxapp
lingxia dev http://127.0.0.1:5173        # run an existing web dev server
lingxia dev https://preview.example.com  # run a remote web target
```

An explicit path must be a standalone lxapp directory. An explicit URL must use
`http` or `https`; Runner mounts its managed browser in self mode, with an editable
URL field, page history, and its own tab group. The field accepts URLs, not
search queries. A URL target does not create a placeholder lxapp
and neither target directory needs `lingxia.yaml` — that file remains native
host configuration. `status`, `stop`, logs, and `.lingxia/` state are scoped to
the directory where `lingxia dev <target>` was invoked.

Override the display language for one Runner session when testing localization:

```bash
lingxia dev --display-language zh-CN  # explicit language
lingxia dev --display-language auto   # system locale
```

The override applies only to that process; it is not written to host settings
or lxapp storage.

`dev` chooses native targets from what it launches: Android uses the selected
device's reported ABI and macOS uses the host architecture. Explicit
`--android-abis` / `--macos-arch` overrides belong to `build` and `package`,
where cross-architecture artifacts are intentional.

Re-running `lingxia dev` for the same platform **takes over**: it stops the
project's existing same-platform session automatically and starts fresh.
Different platforms don't conflict — `-p android` and `-p ios` run side by
side.

`lingxia dev` owns the session lifecycle — start, `status`, `stop`. For
automation, start it detached with `--background` (it returns once the session
and its runtime websocket are ready); a foreground run blocks the terminal and
takes the session down when it exits. Either way the session publishes metadata
and logs for `lxdev`. `lingxia dev status` reports `starting`, `ready`, or `stale`
and exposes the same state plus `runtime_connected` with `--json`.

`lingxia dev stop` has one terminal-state contract: it requests graceful
shutdown, waits for the owner to exit, and automatically terminates the owner
after a bounded timeout. There is no separate force mode. Session lifecycle
stays with `lingxia`; `lxdev` only connects to and drives a live session.

Desktop and Runner dev websockets stay loopback-only. A physical iOS device is
the exception: it connects to an authenticated LAN listener using the token in
`~/.lingxia/apple/dev-device-token`. On a remote development machine, run both
`lingxia dev` and `lxdev` there through SSH or the machine's existing
CI/device-lab agent.

When `lingxia dev` runs in an SSH session on Windows, the CLI bootstraps either
the native host app or the Runner through a temporary interactive-token task so
its window opens in the signed-in Windows desktop. The same Windows account must
already be signed in locally or through RDP; otherwise startup fails with an
actionable error. From the SSH client machine, use `--background`: the SSH
command returns only after the runtime is connected. Subsequent `lxdev` commands
should also run on that machine through SSH.

See `lingxia dev --help` for the flags.

> **Drive the live session with [`lxdev`](./lxdev.md)** — a separate binary that
> automates the running app (browser tabs, lxapp pages, screenshots, logs) and
> can rebuild + reload lxapps, without starting a new session. The split:
> `lingxia dev` owns process lifetime, `lxdev` drives.

### `lingxia devices`

List the connected/available devices for a platform (auto-detected, or scope
with `--platform`). Use it to find the device id you then pass to `install` /
`launch` / `dev` when more than one is connected.

See `lingxia devices --help`.

### `lingxia install`

Install a built artifact to a device. Auto-detects the artifact and platform, or
point it at a specific APK/HAP and device. Can reinstall cleanly
(uninstall-first, best effort) and suppress progress UI for automation.

See `lingxia install --help`.

### `lingxia uninstall`

Remove an installed app from a device. The bundle/package id is inferred from
`lingxia.yaml` when omitted, or pass it explicitly; target a specific device /
platform as needed.

See `lingxia uninstall --help`.

### `lingxia launch`

Launch an already-installed app on a device. The bundle id is inferred from
`lingxia.yaml` when omitted. `--restart` terminates a running instance first
(best effort) — currently supported on Android and iOS; HarmonyOS supports plain
launch only.

See `lingxia launch --help`.

### `lingxia icon`

Generate or update app icons from a single full-bleed source image. macOS art
is normalized automatically (Dock proportions, rounded corners, margin). For
Android/Harmony layered icons the foreground embeds the source's own background
by default — keep the background color matched to the source, or pass a
transparent foreground glyph to render the mark larger. Can also convert
standalone (write a `.ico`/`.png` master to a path instead of into a project).

See `lingxia icon --help` for the flags.

### `lingxia doctor`

Check development environment setup — prints pass/warn/fail checks for the common
toolchain plus the configured/target platforms. Scope it with `--platform` when
a specific platform build complains about missing tools.

```bash
lingxia doctor
lingxia doctor --platform harmony
```

### Distribution — `publish`, `auth`, `store`, `ds`, signing

Low-frequency: publish to the LingXia server, platform signing setup,
developer-account credentials, OS app-store submission, and developer-service
queries. All of it lives in [Distribution](./distribution.md).

---

## Environment Variables

Required during build/dev for the listed platforms. One-time SDK installation is
part of toolchain onboarding (out of scope here); the variables below must be
present in your shell every time you build.

| Variable | Used by | Description |
|----------|---------|-------------|
| `ANDROID_SDK_ROOT` | android | Android SDK root path |
| `ANDROID_NDK_ROOT` | android | Android NDK path (e.g. `$ANDROID_SDK_ROOT/ndk/<version>`) |
| `OHOS_NDK_HOME` | harmony | Harmony command-line tools SDK path |
| `JAVA_HOME` | android | Java JDK path |

If a platform build complains about missing tools, run
`lingxia doctor --platform <p>` to see exactly what's missing. Credential and
signing env overrides (e.g. `LINGXIA_APPLE_*`, `LINGXIA_AUTH_TOKEN`,
`LINGXIA_NATIVE_FEATURES`) are documented with their commands and in
[Distribution → App signing](./distribution.md#app-signing).

---

## Configuration Files

This reference focuses on what commands do. File schemas live in the dedicated guides:

| File | Purpose | Canonical guide |
|---|---|---|
| `lingxia.yaml` | Host app metadata, platform config, runtime-facing build inputs | [App Project](../app/project.md) |
| `lxapp.json` | LxApp runtime metadata such as `appId`, `version`, and `pages` | [LxApp Development Guide](../lxapp/guide.md) |
| `lxapp.config.ts` | LxApp build config such as aliases, view tooling, and `staticDirs` | [LxApp Development Guide](../lxapp/guide.md) |

Quick reminders:

- `lingxia.yaml` is the source of truth for host app build metadata.
- `homeAppVersion` is generated into runtime `app.json`; you do not set it manually.
- Storage/cache limits live under `storage`; set `storage.cacheMaxSizeMB` to `0` to disable usercache size enforcement.
