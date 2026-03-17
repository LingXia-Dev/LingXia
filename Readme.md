# LingXia

/lɪŋ ʃiə/ ("Ling Shia")

A cross-platform miniprogram style runtime built for the modern era.

## Overview

LingXia is a comprehensive solution for running miniprograms across multiple platforms including iOS, Android, macOS, and HarmonyOS. It provides a unified runtime environment with native performance and platform-specific optimizations.

## Architecture

```
LingXia/
├── lingxia-lxapp/      # Core runtime engine with JS execution
├── lingxia-sdk/        # Platform-specific native SDKs
├── lingxia-webview/    # WebView integration layer
├── packages/lingxia-components/ # UI component library (npm: @lingxia/components)
├── lingxia-builder/    # Build tools and utilities
└── examples/           # Sample applications and demos
```


## Platform Support

- **iOS** - Native iOS applications with JavaScriptCore
- **macOS** - Native macOS applications with JavaScriptCore
- **Android** - Android applications with QuickJS
- **HarmonyOS** - OpenHarmony applications with QuickJS
