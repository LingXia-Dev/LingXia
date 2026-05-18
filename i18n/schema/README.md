# `schema/` — JSON Schemas for i18n YAML files

| File | Used by | Allows variant overrides? |
|---|---|---|
| `ui.schema.json` | `ui/`, `error/`, `permission/runtime/` | yes (`default / android / apple / ios / harmony / rust`) |
| `permission.schema.json` | `permission/cli/` | n/a (flat `apple.info_plist.*` keys) |
| `native.schema.json` | `logic/`, `android/`, `apple/`, `harmony/` | no — plain string leaves only |

If you add a new scope under `i18n/` you must also wire it up in
`tools/lingxia-cli/src/gen/i18n.rs::Scope` and choose (or add) the schema
it validates against.
