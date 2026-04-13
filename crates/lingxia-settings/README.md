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

The schema is intentionally small. This crate is for shared lightweight
preferences such as download directory settings. Product-specific settings such
as browser proxy configuration or Auto Switch rules should live in their owning
product/runtime crate instead of this shared settings crate.
