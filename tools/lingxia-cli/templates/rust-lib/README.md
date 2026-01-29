# {{project_name}}

Native library for your LingXia app.

This crate builds a platform-native library (`.so` for Android/Harmony, `.a` for iOS) that re-exports LingXia SDK symbols.

## Build

This crate is automatically built by `lingxia-cli` when you run:
```bash
lingxia dev
```

The compiled library will be bundled into your app automatically.
