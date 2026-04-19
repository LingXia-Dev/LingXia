# lingxia

Platform entry crate for the LingXia framework. Provides cross-platform FFI bindings, host addon lifecycle, and JS extension registration APIs.


## Platform Modules

| Module | Target |
|--------|--------|
| `android` | Android (JNI) |
| `apple` | iOS / macOS (Swift Bridge) |
| `harmony` | HarmonyOS (NAPI) |

## Exports

- `HostAddon` - Trait for host bootstrap, services, and JS API registration
- `register_host_addon` - Register a host addon before runtime initialization
- `register_logic_extension` - Register custom JS logic extensions
- `native` - Attribute macro for custom page-facing native APIs
- `register_provider` - Register provider
- `register_log_provider` - Register optional log upload/sink provider
- `LxLogicExtension` - Trait for JS extensions
- `Provider` - Combined provider trait
- `LogProvider` - Trait for realtime log sink and collected log upload
- `UpdateProvider` - Trait for update checking
- `FingerprintProvider` - Trait for device fingerprint
- `UpdatePackageInfo`, `UpdateTarget`, `LxAppUpdateQuery` - Update contract types
