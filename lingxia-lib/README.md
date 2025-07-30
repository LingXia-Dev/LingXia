# LingXia Lib

Rust crate that generates the native library (liblingxia.so/.a) for all platforms.

## Purpose

This crate serves as the main entry point that:
- Integrates `lingxia-webview` and `lingxia-miniapp`
- Provides FFI interfaces for Android, iOS/macOS, and HarmonyOS
- Generates the final native library used by platform SDKs
