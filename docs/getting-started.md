# Getting Started

This guide is a quick path to get a demo running with CLI.

If you want command details, see [CLI Command Reference](./cli.md).
If you want project/file layout details, see [LxApp Project Structure](./lxapp-structure.md).

---

## 1. Prerequisites

- **Node.js** 18 or later
- **Rust** (for host app with native runtime)
- Platform toolchains for your target:
- Android: Android SDK/NDK
- iOS/macOS: Xcode (macOS host only)
- Harmony: command-line tools SDK

Verify your environment:

```bash
lingxia doctor
```

---

## 2. Install CLI

Install the LingXia CLI globally:

```bash
npm install -g @lingxia/cli
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
- `lingxia.config.json` (host project config)
- native platform project folders (`android/`, `ios/`, `macos/`, `harmony/` based on selection)
- home lxapp folder (default `homelxapp/`)

---

## 4. Build and Run Demo

Build once:

```bash
lingxia build
```

Run on device/emulator (when available):

```bash
lingxia run
```

Release build:

```bash
lingxia build --release
```

---

## 5. Optional: LxApp-Only Demo

Create a standalone lxapp project:

```bash
lingxia new my-lxapp -t lxapp -y
cd my-lxapp
lingxia build
```

This mode is useful for page/logic development without native host packaging workflow.

---

## 6. Next

- [CLI Command Reference](./cli.md)
- [LxApp Project Structure](./lxapp-structure.md)
- [lingxia.config.json Reference](./lingxia-config.md)
