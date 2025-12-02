# lingxia

Platform entry crate for the LingXia framework. Provides cross-platform FFI bindings and extension registration APIs.

## Platform Modules

| Module | Target |
|--------|--------|
| `android` | Android (JNI) |
| `apple` | iOS / macOS (Swift Bridge) |
| `harmony` | HarmonyOS (NAPI) |

## Exports

- `register_logic_extension` - Register custom JS logic extensions
- `register_cloud_provider` - Register cloud service provider
- `LxLogicExtension` - Trait for JS extensions
- `CloudUpdateProvider` - Trait for cloud update checking
- `UpdateCheckResult`, `UpdatePackageInfo` - Update result types

