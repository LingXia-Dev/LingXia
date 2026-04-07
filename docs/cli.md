# CLI Command Reference

Complete reference for the `lingxia` command-line interface.

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
| `-f, --features <features>` | Rust features to enable (comma-separated) | none |
| `--abis <abis>` | Android ABIs (comma-separated): `arm64-v8a`, `armeabi-v7a` | auto (`arm64-v8a`) |
| `--macos-arch <arch>` | macOS build architecture: `arm64`, `x86_64` | host arch |
| `--platform <platforms>` | Platforms to build (comma-separated) | all detected |
| `--package` | Package release build output into a publishable archive (`--release` required). Used for LxApp/LxPlugin dist archives and standalone macOS update zips. | false |
| `--skip-native` | Skip native Rust library compilation | false |

**Examples:**

```bash
# Development build (default)
lingxia build

# Release build
lingxia build --release

# Build for specific platform
lingxia build --platform android

# Skip native compilation (use existing binaries)
lingxia build --skip-native
```

---

### `lingxia run`

Development mode: build, install, and launch app on device.

```bash
lingxia run [options]
```

**Options:**

| Option | Description | Default |
|--------|-------------|---------|
| `-d, --device <id>` | Target device ID (required if multiple connected) | auto-detect |
| `--release` | Release build (optimized) | false (debug) |
| `-f, --features <features>` | Rust features to enable (comma-separated) | none |
| `--abis <abis>` | Android ABIs (comma-separated): `arm64-v8a`, `armeabi-v7a` | auto (`arm64-v8a`) |
| `--macos-arch <arch>` | macOS build architecture: `arm64`, `x86_64` (must match host for `run`) | host arch |
| `--skip-native` | Skip native Rust library compilation | false |

**Examples:**

```bash
# Start dev mode (auto-detect device)
lingxia run

# Target specific device
lingxia run -d deviceid

# Use release build
lingxia run --release
```

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

**Examples:**

```bash
# Install to default device
lingxia install

# Install specific artifact
lingxia install -a ./build/app-debug.apk

# Install to specific device
lingxia install -d emulator-5554
```

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

**Examples:**

```bash
# Generate icons for all platforms
lingxia icon logo.png

# With custom background color
lingxia icon logo.png -b "#1E88E5"

# For specific platform only
lingxia icon logo.png -p android
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
| `--api-server <url>` | API server URL (overrides `app.apiServer` in config) | from config |
| `--target <type>` | `lxapp`, `lxplugin`, or `app` (auto-detected from project files) | auto |
| `--package <path>` | Path to the package file (auto-detected if not specified) | auto |
| `--release-type <type>` | Release channel: `release`, `preview`, `developer` (lxapp only) | `developer` |

**Auto-detection:**

| Project file | Target | ID source | Version source |
|---|---|---|---|
| `lxapp.json` | `lxapp` | `appId` | `version` |
| `lxplugin.json` | `lxplugin` | `lxPluginId` | `version` |
| `lingxia.config.json` | `app` | `app.lingxiaId` | `app.productVersion` |

**Examples:**

```bash
# Set token once via env var
export LINGXIA_AUTH_TOKEN=lx_dev_your_token

# Publish lxapp (auto-detected from lxapp.json)
lingxia publish

# Publish preview build
lingxia publish --release-type preview

# Publish lxplugin explicitly
lingxia publish --target lxplugin --api-server http://localhost:8080
```

> **Note:** Run `lingxia build --release --package` first to produce the package archive.

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

For Android and Harmony workflows, LingXia CLI only requires command-line tools.


| Variable | Description |
|----------|-------------|
| `ANDROID_SDK_ROOT` | Android SDK root path (required) |
| `ANDROID_NDK_ROOT` | Android NDK path (required, e.g. `$ANDROID_SDK_ROOT/ndk/28.2.13676358`) |
| `OHOS_NDK_HOME` | Harmony command-line tools SDK path (required) |
| `JAVA_HOME` | Java JDK path |

### Android Command-Line Tools Setup

Download: https://developer.android.com/studio#command-tools

```bash
export ANDROID_SDK_ROOT=$HOME/android-sdk
export ANDROID_NDK_ROOT=$ANDROID_SDK_ROOT/ndk/28.2.13676358

$ANDROID_SDK_ROOT/cmdline-tools/latest/bin/sdkmanager --install \
  "build-tools;34.0.0" \
  "platform-tools" \
  "platforms;android-33" \
  "ndk;28.2.13676358"
```

If you hit a permission error, rerun the same command with `sudo`.

### Harmony Command-Line Tools Setup

Download: https://developer.huawei.com/consumer/en/download/command-line-tools-for-hmos

```bash
export OHOS_NDK_HOME=$HOME/OpenHarmony/command-line-tools/sdk/default/openharmony
```

---

## Configuration Files

This reference focuses on commands and flags. File schemas live in the dedicated guides:

| File | Purpose | Canonical guide |
|---|---|---|
| `lingxia.config.json` | Host app metadata, platform config, runtime-facing build inputs | [App Project](./app-project.md) |
| `lxapp.json` | LxApp runtime metadata such as `appId`, `version`, and `pages` | [LxApp Development Guide](./lxapp-guide.md) |
| `lxapp.config.ts` | LxApp build config such as aliases and source directories | [LxApp Development Guide](./lxapp-guide.md) |

Quick reminders:

- `lingxia.config.json` is the source of truth for host app build metadata.
- `homeLxAppVersion` is generated into runtime `app.json`; you do not set it manually.
- `app.cacheMaxAgeDays` and `app.cacheMaxSizeMB` are optional; set either to `0` to disable that cleanup policy.
- When `splash` is configured, CLI requires a PNG source image and writes `splashTimeout` into runtime `app.json`.

---

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Error |
