# lingxia-webview

Cross-platform WebView bindings

## Platform Support

### Android
- JNI bindings for Android WebView
- Optional Servo backend: `lingxia dev --native-feature servo`.
  Servo dev builds default to optimization level 1 without debug symbols because
  the engine itself is compiled into the app. Set `CARGO_PROFILE_DEV_OPT_LEVEL`
  or `CARGO_PROFILE_DEV_DEBUG` explicitly to override those defaults.

### iOS/macOS
- Objective-C WebKit bindings
- OSLog integration for native logging
- GCD dispatch for main thread operations

### HarmonyOS
- OpenHarmony ArkWeb integration
- NAPI bindings

### Windows
- WebView2 backend
- Hidden native host window managed by the Rust runtime

