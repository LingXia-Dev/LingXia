# CLI Command Reference

What the `lingxia` command-line interface can **do** — each command's purpose, the
capability worth knowing, and when to reach for it. Assumes the CLI is installed
and you are inside a LingXia project.

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
package id, or pass those up front to script it. Can seed an app icon and, for
lxapps, optionally scaffold typed cloud functions (a mock + live `server/`
worker wired to `lx.cloud`).

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

Flags: `lingxia build --help`. Platform signing setup: [App signing](./signing.md).

### `lingxia clean`

Remove generated artifacts for the current project context (host outputs and
platform build directories in a host app; `dist/` and build caches in an
lxapp / lxplugin). Reach for it when a `lingxia.yaml` change seems ignored
after a rebuild.

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

It **refuses a second same-platform session** in a project (stop the first with
`lingxia dev stop`) so that `lxdev` never silently connects to the wrong target.
Different platforms don't conflict — `-p android` and `-p ios` run side by side.

`lingxia dev` owns the session lifecycle — start, `status`, `stop`. For
automation, start it detached with `--background` (it returns once the session
is live); a foreground run blocks the terminal and takes the session down when
it exits. Either way the session publishes metadata + logs for `lxdev`.

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
> can rebuild/restart lxapps, without starting a new session. The split:
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

### `lingxia publish`

Publish a package to the **LingXia server** (not an OS app store — that's
`store`). Auto-detects what it's publishing from the project marker file
(`lxapp.json` → lxapp, `lxplugin.json` → lxplugin, `lingxia.yaml` → host app) and
reads the id/version from it. lxapp / lxplugin publishes package the current
project first and default to the `developer` env when `--env` is omitted; only
host-app publish accepts a
prebuilt package path. Authenticates with a bearer token: the `--token` flag,
or `[publish] token` in `~/.lingxia/cli/config.toml`.

See `lingxia publish --help` for the flags.

**Machine-wide publish defaults (`~/.lingxia/cli/config.toml`):**

Set per-user defaults so lxapp/lxplugin projects (which have no `lingxia.yaml`)
need not pass `--token` / `--lingxia-server` on every publish. The flags (and,
for the server, project `app.lingxiaServer`) take precedence. Publish keys sit
under `[publish]` (the file holds more areas than publishing), and each value
follows the same shape as `app.lingxiaServer` in `lingxia.yaml`: a scalar
applies to every env, an env-keyed map is explicit per env with no fallback for
envs it omits. The env comes from the package's `--env`/`--channel`
(`developer` when omitted for lxapp/lxplugin; host-app publish reads the
package's `app.json envVersion`). The file is CLI-managed — `lingxia publish
login` writes it, hand comments are lost.

```toml
[publish.token]                    # map: explicit per env — each env is a
developer = "lx_dev_token"         # distinct backend with its own credentials
release = "lx_prod_token"

[publish.lingxiaServer]
developer = "http://localhost:8080"
release = "https://prod.example.com"
```

A scalar form covers the single-backend case: `token = "lx_tok"` under
`[publish]` applies to every env.

### `lingxia doctor`

Check development environment setup — prints pass/warn/fail checks for the common
toolchain plus the configured/target platforms. Scope it with `--platform` when
a specific platform build complains about missing tools.

```bash
lingxia doctor
lingxia doctor --platform harmony
```

### `lingxia auth`

Store developer-account credentials so builds can sign and notarize without
interactive prompts. Credentials live under `~/.lingxia/` (mode `0600`), and
environment variables override the stored files (handy in CI). Two providers:

- **`lingxia auth apple`** — `login` stores App Store Connect API (or Apple ID)
  credentials used for **notarization**; `import-developer-id <p12>` stores a
  **Developer ID Application** certificate used for **code-signing**; plus
  `logout` / `status`. On a local Mac, importing the `.p12` is optional when the
  Developer ID identity is already in your login keychain. To create a `.p12`,
  see [App signing → Apple](./signing.md#get-a-developer-id-p12-with-xcode).
- **`lingxia auth harmony`** — `login` / `logout` / `status` for Harmony
  developer credentials.

See `lingxia auth apple --help` / `lingxia auth harmony --help` for the flags,
and [App signing](./signing.md) for how signing/notarization resolves at build
time.

### `lingxia store`

Submit a built installable to an **OS app store**. Talks to stores only — never
the LingXia server (that's `publish`) and never builds (run `build`/`package`
first; `submit` consumes the staged `dist/<platform>/` and fails clearly if it's
missing). Each platform has a `login` / `logout` / `submit` / `status` flow;
store identity lives in `lingxia.yaml` and credentials in
`~/.lingxia/store/credentials.toml`, with **env vars overriding the file** for
CI.

Run `lingxia store --help` for the current set of supported stores and per-action
flags (`--draft`, release notes, track, etc.).

### `lingxia ds`

Query **developer services** read-only. `lingxia ds apple` lists Apple Developer
resources (teams, certificates, bundle identifiers, registered devices,
provisioning profiles); `lingxia ds harmony` covers Harmony developer services.
Requires the matching `lingxia auth` credentials.

See `lingxia ds apple --help` / `lingxia ds harmony --help`.

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
[App signing](./signing.md).

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

---

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Error |
| 2 | Usage error (bad flags/arguments) |
