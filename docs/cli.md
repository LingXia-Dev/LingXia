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
| `--prod` | Production build (minified, optimized) | false |
| `--dev` | Development build | false |
| `--plugin` | Build as LxPlugin instead of LxApp | false |
| `-p, --profile <profile>` | Build profile: `debug` or `release` | debug |
| `-f, --features <features>` | Rust features to enable (comma-separated) | none |
| `-t, --targets <targets>` | Target architectures (comma-separated) | auto |
| `--platform <platforms>` | Platforms to build (comma-separated) | all detected |
| `--skip-native` | Skip native Rust library compilation | false |

**Examples:**

```bash
# Development build (default)
lingxia build

# Production build
lingxia build --prod

# Release profile for native code
lingxia build -p release

# Build for specific platform
lingxia build --platform android

# Skip native compilation (use existing binaries)
lingxia build --skip-native
```

---

### `lingxia dev`

Development mode: build, install, and launch app on device.

```bash
lingxia dev [options]
```

**Options:**

| Option | Description | Default |
|--------|-------------|---------|
| `-d, --device <id>` | Target device ID (required if multiple connected) | auto-detect |
| `-p, --profile <profile>` | Build profile: `debug` or `release` | debug |
| `-f, --features <features>` | Rust features to enable (comma-separated) | none |
| `-t, --targets <targets>` | Target architectures (comma-separated) | auto |
| `--skip-native` | Skip native Rust library compilation | false |

**Examples:**

```bash
# Start dev mode (auto-detect device)
lingxia dev

# Target specific device
lingxia dev -d emulator-5554

# Use release profile
lingxia dev -p release
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

### `lingxia doctor`

Check development environment setup.

```bash
lingxia doctor
```

**No options.**

**Output:**

```
Checking Java... ✅ Found (version 17.0.1)
Checking Android SDK... ✅ Found (ANDROID_HOME set)
   - platform-tools: ✅
Checking Gradle... ✅ Found (version 8.5)
Checking Rust... ✅ Found (rustc 1.75.0)
Checking Android NDK... ✅ Found (ANDROID_NDK_HOME set)
```

---

## Environment Variables

| Variable | Description |
|----------|-------------|
| `ANDROID_HOME` | Android SDK path |
| `ANDROID_NDK_HOME` | Android NDK path |
| `JAVA_HOME` | Java JDK path |
| `LINGXIA_API_KEY` | API key for backend authentication |
| `LINGXIA_API_SECRET` | API secret for backend authentication |

### API Credentials

API credentials can be provided in two ways (priority from high to low):

1. **Environment variables** (recommended for CI/CD):
   ```bash
   export LINGXIA_API_KEY="your-key"
   export LINGXIA_API_SECRET="your-secret"
   ```

2. **Hidden secrets file** `.lingxia.secrets.json` (for local development):
   ```json
   {
     "apiKey": "your-key",
     "apiSecret": "your-secret"
   }
   ```
   This file is automatically added to `.gitignore`.

---

## Configuration Files

### `lingxia.config.json` (Host App)

```json
{
  "app": {
    "productName": "MyApp",
    "productVersion": "1.0.0",
    "apiServer": "https://api.example.com",
    "platforms": ["android"],
    "homeLxAppID": "homelxapp",
    "homeLxAppVersion": "1.0.0"
  },
  "android": {
    "packageId": "com.example.myapp",
    "minSdk": 29,
    "targetSdk": 35,
    "compileSdk": 35
  }
}
```

### `lxapp.json` (LxApp)

```json
{
  "lxAppId": "homelxapp",
  "lxAppName": "My LxApp",
  "version": "0.1.0",
  "pages": ["pages/home/index.tsx"]
}
```

### `lxapp.config.json` (LxApp Build Config)

```json
{
  "alias": {
    "@": "src",
    "@shared": "shared"
  },
  "sourceDirs": ["pages", "shared"]
}
```

---

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Error |
