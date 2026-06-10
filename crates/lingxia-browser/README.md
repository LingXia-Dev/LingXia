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

## Automation

Per-tab automation helpers for driving live browser tabs (used by devtools):

- Element queries: `query(...)`, `query_with_max_text(...)`
- Input: `click(...)`, `fill(...)`, `type_text(...)`, `press(...)`,
  `scroll(...)`, `scroll_to(...)`
- Waiting: `wait(...)` with conditions (loaded, selector visible/hidden/editable,
  JS predicate, URL match), plus `wait_for_url(...)`, `wait_for_url_contains(...)`,
  `wait_for_navigation(...)`
- Cookies: `list_cookies(...)`, `list_all_cookies(...)`, `set_cookie(...)`,
  `delete_cookie(...)`, `clear_cookies(...)`
- Inspection: `evaluate_javascript(...)`, `take_screenshot(...)`, `current_url(...)`
- Navigation: `reload(...)`, `go_back(...)`, `go_forward(...)`, `activate(...)`

## Notes

This crate is an internal runtime layer, not a standalone end-user browser.
The higher-level `lingxia-shell` crate wires it into host registrations and the
bundled browser Web UI.
