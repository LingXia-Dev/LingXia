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
- `register_provider` - Register provider
- `LxLogicExtension` - Trait for JS extensions
- `Provider` - Combined provider trait
- `UpdateProvider` - Trait for update checking
- `FingerprintProvider` - Trait for device fingerprint
- `UpdateCheckResult`, `UpdatePackageInfo` - Update result types

