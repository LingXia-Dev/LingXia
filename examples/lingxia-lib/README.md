# lingxia-lib

Example native extension crate for LingXia based App. This builds to a platform library (`.so` for Android/Harmony, `.a` for iOS) and exposes your custom JS extensions to the host app.

## What it does
- Re-exports LingXia platform FFI symbols so the host can link a single library.
- Registers app-specific extensions (see `src/hello.rs`).
- Optionally initializes the cloud provider when the `cloud` feature is enabled.

## How the host calls it
- Android: Kotlin `MainActivity.registerNativeExtensions()` calls the exported JNI symbol `Java_com_lingxia_example_lxapp_MainActivity_registerNativeExtensions`.
- Harmony/iOS/macOS: call `lingxia_register_extensions()` (exported by this crate) before `LxApp.initialize`.

Use this as a template for your own app-specific extension crate. Keep user extensions isolated here so the core SDK stays unchanged.
