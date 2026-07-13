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

Re-running `lingxia dev` for the same platform **takes over**: it stops the
project's existing same-platform session automatically and starts fresh.
Different platforms don't conflict — `-p android` and `-p ios` run side by
side.

`lingxia dev` owns the session lifecycle — start, `status`, `stop`. For
automation, start it detached with `--background` (it returns once the session
is live); a foreground run blocks the terminal and takes the session down when
it exits. Either way the session publishes metadata + logs for `lxdev`.

`--lan` (desktop platforms and the Runner) exposes the dev websocket on all
interfaces behind a session token and prints a tokened attach URL for
[`lxdev attach`](./lxdev.md) on another machine. The URL is stable across
restarts (token persists in `~/.lingxia/dev-lan-token`; delete it to rotate).
Without `--lan` the websocket stays loopback-only. The OS firewall prompts
once per executable path for the inbound listener; over ssh no prompt can
appear, so pre-authorize once from an elevated shell:
`New-NetFirewallRule -DisplayName "LingXia Dev" -Direction Inbound -Program
"<lingxia.exe path>" -Action Allow -Profile Private`.

See `lingxia dev --help` for the flags.

**Runner cloud defaults (`~/.lingxia/runner/config.toml`):**

Standalone lxapp dev on macOS/Windows launches LingXia Runner. When the Runner
uses the cloud provider, this hand-edited file can override the backend and app
identity. Each value follows the same shape as `app.lingxiaServer` in
`lingxia.yaml`: a scalar applies to every env, an env-keyed map is explicit per
env (`developer` / `preview` / `release`) with no fallback for envs it omits.
The active env comes from `lingxia dev --env` (`developer` when omitted).

```toml
lingxiaId = "com.example.app"      # scalar: same for every env

[lingxiaServer]                    # map: explicit per env
developer = "http://127.0.0.1:8787"
release = "https://api.example.com"
```

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
