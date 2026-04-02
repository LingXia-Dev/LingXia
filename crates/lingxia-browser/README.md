# lingxia-browser

Browser runtime capability crate used by LingXia shell/product code.

## What it provides

- Internal browser tab lifecycle and tab registry
- Browser-facing navigation/address-bar data types
- Open/close/update helpers for managed browser tabs
- Hooks that integrate browser tabs with downloads and embedded internal pages

## Key APIs

- `open(...)`, `open_for_app(...)`, `close(...)`
- `tab_path(...)`, `update_tab(...)`
- `start_download(...)`
- `install_runtime()`, `register_internal_page(...)`, `warmup()`

## Notes

This crate is an internal runtime layer, not a standalone end-user browser.
The higher-level `lingxia-shell` crate wires it into host registrations and the
bundled browser Web UI.
