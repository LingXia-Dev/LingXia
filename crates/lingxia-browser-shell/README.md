# lingxia-browser-shell

Browser shell product module and host registration crate for LingXia.

## What it wires together

- Browser capability from `lingxia-browser`
- Download/settings host APIs
- Proxy settings, Auto Switch rules, and local proxy runtime
- Address-bar resolution and navigation helpers
- The routes behind the host's `browser.webui` system pages (`app.lingxia.browser`), registered only when the host bundles them
- Panel-related helpers for shell-hosted lxapps

## Key APIs

- `register_runtime()`, `register_bundled_assets()`, and `warmup()`
- `open(...)`, `open_for_app(...)`, `close(...)`, `download(...)`
- `resolve_input(...)`, `classify_navigation(...)`
- `open_panel_lxapp(...)`, `panel_item_for_id(...)`, `panels_config_json()`

## Notes

This crate is the product-facing assembly layer. Lower-level reusable runtime
pieces live in `lingxia-browser`, `lingxia-transfer`, and `lingxia-settings`.

Proxy configuration is owned by `lingxia-browser-shell` and persisted
separately under the app state directory as `proxy-settings.json`. Shared
lightweight settings such as download directory preferences remain in
`lingxia-settings`.
