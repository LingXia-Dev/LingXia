# lingxia-settings

Shared settings/preferences store for LingXia products.

## What it provides

- Load/save of JSON settings under the app state directory
- In-process settings cache
- Shared helpers for download directory preferences

## Key APIs

- `load(...)`, `save(...)`
- `settings_path(...)`
- `get_download_dir(...)`, `set_download_dir(...)`

## Notes

The schema is intentionally small today. This crate exists so download, shell,
and other runtime crates share one persistence location and format.
