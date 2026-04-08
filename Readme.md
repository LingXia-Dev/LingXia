<p align="center">
  <img src="LingXia.png" alt="LingXia" width="128" />
</p>

<h1 align="center">LingXia</h1>

<p align="center">
  /lɪŋ ʃiə/ — Cross-platform app runtime for React & Vue
</p>

<p align="center">
  <a href="docs/getting-started.md">Getting Started</a> &middot;
  <a href="docs/lxapp-guide.md">LxApp Guide</a> &middot;
  <a href="docs/bridge-guide.md">Bridge Guide</a> &middot;
  <a href="docs/cli.md">CLI Reference</a>
</p>

---

LingXia runs React and Vue pages inside native apps on **iOS, macOS, Android, and HarmonyOS**. It splits every page into a **View** (WebView, rendering only) and a **Logic** (native JS runtime, business only), connected by a Rust bridge. The two layers live in separate runtimes — they can't accidentally couple.

## Why LingXia

Hybrid frameworks pack UI and business logic into a single WebView. LingXia doesn't.

```
 View  (WebView)              Logic  (JS Runtime)
 React / Vue                  state + business logic
 rendering only      bridge   native sandboxed
       ◄──────────────────►
          Rust · JSON Patch · streaming
```

**View** renders UI. No network calls, no state mutations, no business logic.
**Logic** owns state and calls platform APIs. No DOM access, no rendering.
**Bridge** connects them with three primitives: state sync (`setData`), streaming (`yield`), and bidirectional channels (`ch.send`).

This separation means heavy computation, long-running tasks, and platform API calls in Logic never block rendering in View.

## Features

| | |
|---|---|
| **View / Logic isolation** | UI and business logic run in separate runtimes. Clean separation enforced at the architecture level, not by convention. |
| **Flexible business layer** | Write Logic in JavaScript (React/Vue lxapps) or in Rust (like the built-in Shell). Choose per module. |
| **LxApp hot delivery** | Each lxapp is a self-contained archive. Update and ship new features without rebuilding or resubmitting the host app. |
| **App self-update** | Android and macOS apps update in-app. iOS and HarmonyOS update through their App Stores. |
| **Four platforms, one codebase** | Same lxapp runs on iOS, macOS, Android, and HarmonyOS. Platform differences handled by the Rust runtime and native SDKs. |

## Quick Start

```bash
# Install CLI
curl -fsSL https://raw.githubusercontent.com/LingXia-Dev/LingXia/main/install.sh | sh

# Create and run a native host app
lingxia new my-app -t native-app -p android --package-id com.example.myapp -y
cd my-app && lingxia build && lingxia dev
```

**Prerequisites:** Node.js 18+, Rust toolchain, platform SDK ([details](docs/getting-started.md)).

To develop lxapp pages without native packaging:

```bash
lingxia new my-lxapp -t lxapp -y
cd my-lxapp && lingxia dev
```

## Platform Support

| Platform | Runtime | Self-update | SDK |
|----------|---------|-------------|-----|
| Android | JavaScriptCore | In-app | Kotlin AAR |
| iOS | JavaScriptCore | App Store | Swift Package |
| macOS | JavaScriptCore | In-app | Swift Package |
| HarmonyOS | NAPI | App Store | ArkTS HAR |

## Project Structure

```
crates/                  Rust core — runtime, bridge, platform abstraction
  lingxia/               Main framework crate
  lingxia-lxapp/         LxApp loader, lifecycle, plugin system
  lingxia-shell/         Built-in Shell (Rust-native lxapp)
  lingxia-platform/      Platform trait implementations
tools/lingxia-cli/       CLI — new, build, dev, publish, doctor
lingxia-sdk/             Native SDKs (Android Kotlin, Apple Swift, Harmony ArkTS)
packages/                npm packages (bridge runtime, React/Vue bindings)
examples/                Example host app + lingxia-showcase lxapp
docs/                    Documentation
```

## Documentation

| Guide | Description |
|-------|-------------|
| [Getting Started](docs/getting-started.md) | Install the CLI, scaffold a project, and run the first build |
| [LxApp Guide](docs/lxapp-guide.md) | Start writing pages with `Page({})`, `useLxPage`, events, and native components |
| [Bridge Guide](docs/bridge-guide.md) | Deep dive into `setData`, stream, and channel semantics |
| [App Project](docs/app-project.md) | Host app structure, `lingxia.config.json` |
| [CLI Reference](docs/cli.md) | All commands and flags |
| [Native Development](docs/native-development.md) | Extend LingXia from Rust |

## License

MIT
