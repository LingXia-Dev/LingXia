# LingXia SDK Runtime Assets

This directory contains static assets that are bundled with the native SDKs for each platform.

## Files

- **`404.html`** - Error page template displayed when a miniapp page cannot be loaded. Contains a `{{FAILED_PATH}}` placeholder that is replaced at runtime.
- **`webview-bridge.js`** - JavaScript bridge that enables communication between the WebView (View Layer) and the native runtime (Logic Layer). Implements the bridge protocol with `call`, `reply`, `event`, and `callback` message types.



## Platform Integration

During build, these files are copied into each platform's SDK assets:

- **Android**: `lingxia-sdk/android/lingxia/src/main/assets/`
- **iOS/macOS**: App bundle resources
- **HarmonyOS**: `lingxia-sdk/harmony/lingxia/src/main/resources/rawfile/`

See platform-specific build scripts (`examples/android/build.sh`, `examples/ios/build_deploy_ios.sh`, etc.) for details.
