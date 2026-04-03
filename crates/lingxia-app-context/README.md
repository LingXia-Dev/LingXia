# lingxia-app-context

Shared app/product context primitives for LingXia workspace crates.

## What it owns

- Parsing and validating `app.json`
- Global process-level app config access
- Shared derived paths under the app state directory
- Panel configuration structs used by shell/product crates

## Key APIs

- `AppConfig::parse_and_validate(...)`
- `set_app_config(...) -> Result<(), AppContextError>`
- `app_config()`
- `product_name()`, `product_version()`, `lingxia_id()`
- `app_state_dir(...)`, `app_state_file(...)`

## Notes

This crate is intentionally small and dependency-light so higher-level crates
can share app metadata without depending on `lxapp` runtime code.
