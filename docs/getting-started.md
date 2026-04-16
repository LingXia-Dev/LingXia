# Getting Started

This guide is a quick path to get a demo running with CLI.

If you want command details, see [CLI Command Reference](./cli.md).
If you want host app project details, see [App Project](./app-project.md).
If you want to write lxapp pages, see [LxApp Development Guide](./lxapp-guide.md).
If you want to extend LingXia from native Rust, see [Native Development Guide](./native-development.md).

---

## 1. Prerequisites

- Core tools:
  - **Node.js** 18 or later
  - **Rust** toolchain for host apps with native runtime
- Platform toolchains for your target:
  - Android: Android SDK/NDK
  - iOS/macOS: Xcode on a macOS host
  - Harmony: Harmony command-line tools SDK

Verify your environment:

```bash
lingxia doctor
```

---

## 2. Install CLI

Recommended: install the prebuilt CLI binary from GitHub Release:

```bash
curl -fsSL https://raw.githubusercontent.com/LingXia-Dev/LingXia/main/install.sh | sh
```

Verify installation:

```bash
lingxia --version
```

---

## 3. Create Demo Project

Create a host app demo (contains embedded home lxapp):

```bash
lingxia new my-app -t native-app -p android --package-id com.example.myapp -y
```

Go into the project:

```bash
cd my-app
```

`my-app` contains:
- `lingxia.yaml` (host project config)
- native platform project folders (`android/`, `ios/`, `macos/`, `harmony/` based on selection)
- home lxapp folder (default `lingxia-showcase/`)

---

## 4. Build and Run Host App

Build once:

```bash
lingxia build
```

Run on device/emulator (when available):

```bash
lingxia dev
```

Release build:

```bash
lingxia build --release
```

---

## 5. Build an LxApp Only

Create a standalone lxapp project:

```bash
lingxia new my-lxapp -t lxapp -y
cd my-lxapp
lingxia dev
```

This mode is useful when you want to focus on page and logic authoring without native host packaging.

---

## 6. Next

- If you are building a host app shell: [App Project](./app-project.md)
- If you are writing page UI and page logic: [LxApp Development Guide](./lxapp-guide.md)
- If you want the bridge details: [Bridge Guide](./bridge-guide.md)
- If you need full command coverage: [CLI Command Reference](./cli.md)
- If you are extending LingXia from Rust: [Native Development Guide](./native-development.md)
