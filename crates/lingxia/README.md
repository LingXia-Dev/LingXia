# lingxia

Platform entry crate for the LingXia framework. Provides cross-platform FFI bindings, host addon lifecycle, and explicit JS AppService APIs under `lingxia::js`.


## Platform Modules

| Module | Target |
|--------|--------|
| `android` | Android (JNI) |
| `apple` | iOS / macOS (Swift Bridge) |
| `harmony` | HarmonyOS (NAPI) |

## Exports

- `HostAddon` - Trait for host bootstrap, services, and JS API registration
- `register_host_addon` - Register a host addon before runtime initialization
- `native` - Attribute macro for custom page-facing native APIs
- `Result`, `Error` - Public native facade result and error types
- `app` - App metadata and directory helpers
- `task` - Runtime task helpers
- `downloads`, `settings`, `file`, `media`, `push` - Native service facades
- `provider` - Provider traits and registration APIs
- `log` - Logging facade
- `update` - Update provider contracts and version types
- `js::register_logic_extension` - Register custom JS logic extensions when `js-lxapp` is enabled
- `js::LxLogicExtension` - Trait for JS extensions when `js-lxapp` is enabled
