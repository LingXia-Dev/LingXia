# CLI Command Reference

Complete reference for the `lingxia` command-line interface. This skill assumes the CLI is already installed and you are working inside a LingXia project. For first-time install and onboarding, see `docs/quick-start.md`.

---

## Global Options

| Option | Description |
|--------|-------------|
| `--version`, `-V` | Print version |
| `--help`, `-h` | Print help |

---

## Commands

### `lingxia new`

Create a new LingXia project.

```bash
lingxia new [name] [options]
```

**Arguments:**

| Argument | Description | Required |
|----------|-------------|----------|
| `name` | Project name | No (prompted if omitted) |

**Options:**

| Option | Description | Default |
|--------|-------------|---------|
| `-t, --project-type <type>` | Project type: `native-app` or `lxapp` | prompted |
| `-p, --platform <platforms>` | Target platforms (comma-separated): android, ios, harmony, all | prompted |
| `--package-id <id>` | Package identifier (e.g., com.example.app) | prompted |
| `--icon <path>` | Path to app icon (PNG, recommended 1024x1024) | none |
| `-y, --yes` | Skip confirmation prompts | false |

**Examples:**

```bash
# Interactive mode
lingxia new

# With project name
lingxia new my-app

# Non-interactive with all options
lingxia new my-app -t native-app -p android,ios --package-id com.example.myapp -y

# Create LxApp only
lingxia new my-lxapp -t lxapp -y
```

---

### `lingxia build`

Build the project.

```bash
lingxia build [options]
```

**Options:**

| Option | Description | Default |
|--------|-------------|---------|
| `--release` | Release build (optimized) | false (debug) |
| `--env <env>` | Build environment: `developer` (or `dev`), `preview`, `release` | `developer` |
| `--abis <abis>` | Android ABIs (comma-separated): `arm64-v8a`, `armeabi-v7a` | auto (`arm64-v8a`) |
| `--macos-arch <arch>` | macOS build architecture: `arm64`, `x86_64` | host arch |
| `--platform <platforms>` | Platforms to build (comma-separated) | all detected |
| `--all-platforms` | Build every configured platform (mutually exclusive with `--platform`) | false |
| `--skip-native` | Skip native Rust library compilation | false |

> **`--env` vs `--release`** — independent: `--env` picks the environment slot, `--release` the compiler profile; `lingxia build --env release --release` is the shippable combination. Defaults, package-id suffixing, and per-env server config: [App Project → Environment versions](../app/project.md#environment-versions).

**Examples:**

```bash
# Development build (default: --env developer)
lingxia build

# Optimized release build for shipping
lingxia build --env release --release

# Preview build (side-by-side installable next to release)
lingxia build --env preview

# Build for specific platform
lingxia build --platform android

# Skip native compilation (use existing binaries)
lingxia build --skip-native
```

When a host project has `lingxia.yaml`, `lingxia build` also prepares configured host assets. LxApp builds generate the Native client automatically when `lxapp.config.ts` contains `native`.

---

### `lingxia clean`

Remove generated artifacts for the current project context.

```bash
lingxia clean
```

Context rules:

- In a host app project with `lingxia.yaml`, cleans host outputs, generated host assets, platform build directories, configured bundle `dist/` directories, and native `target/`.
- In an lxapp or lxplugin project, cleans local `dist/` or `dist-plugin/`, `node_modules/`, and LingXia view build cache.
- In a standalone Apple Swift Package, such as runner development packages without `lingxia.yaml`, cleans `.build/` and `.lingxia/`.

---

### `lingxia package`

Package release artifacts for publishing or delivery.

```bash
lingxia package [options]
```

`lingxia package` always performs a release package build.

For native host projects, publishable Android artifacts are staged under
`dist/android/`; macOS update zips are written under `dist/macos/`.

**Options:**

| Option | Description | Default |
|--------|-------------|---------|
| `--env <env>` | Build environment: `developer` (or `dev`), `preview`, `release` | `release` |
| `--abis <abis>` | Android ABIs (comma-separated): `arm64-v8a`, `armeabi-v7a` | auto (`arm64-v8a`) |
| `--macos-arch <arch>` | macOS package architecture: `arm64`, `x86_64` | host arch |
| `--platform <platforms>` | Platforms to package (comma-separated) | all detected |
| `--all-platforms` | Package every configured platform (mutually exclusive with `--platform`) | false |
| `--skip-native` | Skip native Rust library compilation | false |
| `--framework <framework>` | Override lxapp view framework detection: `react`, `vue`, `html` | auto-detect |
| `--progress <mode>` | LxApp progress output mode: `task`, `plain` | default CLI output |

**Examples:**

```bash
# Package the current project for publishing
lingxia package

# Package only macOS output
lingxia package --platform macos
```

---

### `lingxia dev`

Development mode for both app and lxapp projects.

```bash
lingxia dev [options]
```

Behavior depends on the current project:

- In an app project, `lingxia dev` builds, installs, launches the host app, and starts a local dev websocket for `lxdev`.
- Android and Harmony dev sessions set reverse port forwarding so the device app can reach the local dev server.
- iOS dev sessions embed a LAN dev websocket URL into the dev build; the iOS device must be able to reach the host Mac on the local network.
- In a standalone lxapp project, `lingxia dev` builds the lxapp and launches LingXia Runner on macOS.

**Options:**

| Option | Description | Default |
|--------|-------------|---------|
| `-p, --platform <platform>` | Target platform: `android`, `ios`, `macos`, `harmony` | auto-detect for app projects |
| `-d, --device <id>` | Target device ID (required if multiple connected) | auto-detect |
| `--release` | Release build (optimized) | false (debug) |
| `--env <env>` | Build environment: `developer` (or `dev`), `preview`, `release` | `developer` |
| `--skip-native` | Skip native Rust library compilation | false |
| `--abis <abis>` | Android ABIs (comma-separated): `arm64-v8a`, `armeabi-v7a` | auto (`arm64-v8a`) |
| `--macos-arch <arch>` | macOS build architecture: `arm64`, `x86_64` (must match host for local app dev) | host arch |
| `--framework <framework>` | Override lxapp view framework detection: `react`, `vue`, `html` | auto-detect |
| `--progress <mode>` | LxApp progress output mode: `task`, `plain` | default CLI output |
| `--reinstall` | Reinstall app by uninstalling existing one first (best effort) | false |
| `--parallel` | Allow another dev session for the same platform in this project | false |

**Examples:**

```bash
# App project: build, install, and launch
lingxia dev

# App project: target a specific device
lingxia dev -d deviceid

# Standalone lxapp project: build and launch Runner
lingxia dev

# Use release build
lingxia dev --release
```

> **Note:** For standalone lxapp projects, `--device`, `--abis`, and non-macOS `--platform` are not supported. Runner currently launches locally on macOS.

> **Driving a live session:** once `lingxia dev` is running, use the separate **`lxdev`** binary to automate the running app — open URLs, click/type/eval in browser tabs and lxapp pages, screenshot windows, and tail logs without restarting. See [`lxdev` — Drive a running dev session](./lxdev.md).

---

### `lingxia install`

Install the built app to a device.

```bash
lingxia install [options]
```

**Options:**

| Option | Description | Default |
|--------|-------------|---------|
| `-a, --artifact <path>` | Path to artifact file (APK/HAP) | auto-detected |
| `-d, --device <id>` | Target device ID | auto-detect |
| `--reinstall` | Reinstall app by uninstalling existing one first (best effort) | false |
| `--quiet` | Suppress progress UI output (useful for automation) | false |

**Examples:**

```bash
# Install to default device
lingxia install

# Install specific artifact
lingxia install -a ./build/app-debug.apk

# Install to specific device
lingxia install -d emulator-5554

# Reinstall cleanly before install
lingxia install --reinstall
```

---

### `lingxia launch`

Launch the installed app on a device.

```bash
lingxia launch [bundle_id] [options]
```

**Options:**

| Option | Description | Default |
|--------|-------------|---------|
| `-d, --device <id>` | Target device ID | auto-detect |
| `-p, --platform <platform>` | Target platform | auto-detect |
| `--restart` | Restart app by terminating an existing instance before launch (best effort) | false |

**Examples:**

```bash
# Launch inferred app on default device
lingxia launch

# Launch specific app on a specific device
lingxia launch com.example.app -d emulator-5554 -p android

# Restart the app before launching
lingxia launch --restart
```

> **Note:** `--restart` is currently supported for Android and iOS. HarmonyOS currently supports plain `launch` only.

---

### `lingxia icon`

Generate or update app icons from a source image.

```bash
lingxia icon <icon_path> [options]
```

**Arguments:**

| Argument | Description | Required |
|----------|-------------|----------|
| `icon_path` | Path to source icon (PNG, recommended 1024x1024) | Yes |

**Options:**

| Option | Description | Default |
|--------|-------------|---------|
| `-p, --platform <platform>` | Target platform | all from config |
| `-b, --background-color <color>` | Background color for adaptive icons (hex) | #FFFFFF |
| `--foreground <path>` | Transparent artwork for Android/Harmony layered foregrounds | main icon |
| `--legacy` | Also generate legacy Android icons for minSdk < 26 | off |

The full-bleed source works for every platform: macOS art is normalized
automatically (content scaled to Dock proportions, rounded corners, transparent
margin). Without `--foreground`, the Android/Harmony foreground embeds the full
source — including its background — so keep `-b` matched to the source's
background color.

**Examples:**

```bash
# Generate icons for all platforms
lingxia icon logo.png

# With custom background color
lingxia icon logo.png -b "#1E88E5"

# For specific platform only
lingxia icon logo.png -p android

# Transparent glyph as the adaptive foreground (mark renders larger)
lingxia icon logo.png -p android -b "#FAFAF7" --foreground glyph.png
```

---

### `lingxia publish`

Publish a package to the API server.

```bash
lingxia publish --token <token> [options]
```

**Options:**

| Option | Description | Default |
|--------|-------------|---------|
| `--token <token>` | Bearer token (`LINGXIA_AUTH_TOKEN` env var also accepted) | required |
| `--lingxia-server <url>` | LingXia server URL (falls back to `app.lingxiaServer` when available) | from config |
| `--package-path <path>` | Path to the package file (`app` only) | auto |
| `--platform <platform>` | App platform to publish: `android`, `macos` | required for multi-platform app projects |
| `--env <env>` (alias `--channel`) | Release channel: `release`, `preview`, `developer` (or `dev`) — required for lxapp | none |
| `--framework <framework>` | Override lxapp view framework detection: `react`, `vue`, `html` | auto-detect |
| `--progress <mode>` | LxApp progress output mode: `task`, `plain` | default CLI output |

**Auto-detection:**

| Project file | Target | ID source | Version source |
|---|---|---|---|
| `lxapp.json` | `lxapp` | `appId` | `version` |
| `lxplugin.json` | `lxplugin` | `lxPluginId` | `version` |
| `lingxia.yaml` | `app` | `app.lingxiaId` | `app.productVersion` |

**Examples:**

```bash
# Set token once via env var
export LINGXIA_AUTH_TOKEN=lx_dev_your_token

# Publish lxapp (auto-detected from lxapp.json; packages current project automatically)
lingxia publish --release-type developer

# Publish preview build
lingxia publish --release-type preview

# Publish lxplugin (auto-detected from lxplugin.json)
lingxia publish --lingxia-server http://localhost:8080

# Publish Android host app package
lingxia publish --platform android --token <token>
```

> **Note:** `lxapp` and `lxplugin` publish always package the current project first. Only `app` publish supports `--package-path`.

---

### `lingxia doctor`

Check development environment setup.

```bash
lingxia doctor
```

**No options.**

Prints pass/warn/fail checks for common + target platforms.

Use `--platform` to scope checks:

```bash
lingxia doctor --platform android
lingxia doctor --platform harmony
```

---

### `lingxia ds`

Interact with developer services (Apple, Harmony, etc.).

```bash
lingxia ds <platform> <command>
```

**Platforms:**

| Platform | Description |
|----------|-------------|
| `apple`  | Apple Developer Services |

---

### `lingxia ds apple`

Interact with Apple Developer Services.

```bash
lingxia ds apple <command>
```

**Commands:**

| Command | Description |
|---------|-------------|
| `teams` | List development teams |
| `certificates` | List certificates |
| `identifiers` | List bundle identifiers (App IDs) |
| `devices` | List registered devices |
| `profiles` | List provisioning profiles |

**Examples:**

```bash
# List development teams
lingxia ds apple teams

# List certificates
lingxia ds apple certificates

# List bundle identifiers
lingxia ds apple identifiers

# List registered devices
lingxia ds apple devices

# List provisioning profiles
lingxia ds apple profiles
```

> **Note:** Requires authentication via `lingxia auth login` with password mode.

---

## Environment Variables

Required during build/dev for the listed platforms. One-time SDK installation is covered in `docs/quick-start.md`; the variables below must be present in your shell every time you build.

| Variable | Used by | Description |
|----------|---------|-------------|
| `ANDROID_SDK_ROOT` | android | Android SDK root path |
| `ANDROID_NDK_ROOT` | android | Android NDK path (e.g. `$ANDROID_SDK_ROOT/ndk/28.2.13676358`) |
| `OHOS_NDK_HOME` | harmony | Harmony command-line tools SDK path |
| `JAVA_HOME` | android | Java JDK path |

If a platform build complains about missing tools, run `lingxia doctor --platform <p>` to see exactly what's missing.

---

## Configuration Files

This reference focuses on commands and flags. File schemas live in the dedicated guides:

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
