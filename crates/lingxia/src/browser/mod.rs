//! Internal browser bridge facade.
//!
//! This module is intentionally crate-private. Public native APIs should stay in
//! focused facades such as `app`, `file`, `media`, and `update`.
#![cfg_attr(target_os = "windows", allow(unused_imports))]

mod runtime;
mod shell;

#[cfg(target_os = "android")]
pub(crate) use runtime::navigate;
#[cfg(all(target_env = "ohos", not(any(target_os = "ios", target_os = "macos"))))]
pub(crate) use runtime::navigate;
#[cfg(target_os = "windows")]
pub(crate) use runtime::open_standalone_for_app;
pub(crate) use runtime::{APP_ID, close, mark_active, open_for_app, tab_path};
#[cfg(target_os = "windows")]
pub(crate) use runtime::{
    BrowserTabSummary, activate, go_back, go_forward, navigate, reload, runtime_enabled,
    set_tabs_changed_handler, tab_summary, tabs,
};
#[cfg(any(target_os = "ios", target_os = "macos"))]
pub(crate) use runtime::{clear_active, discard, open_standalone_for_app, reactivate};
#[cfg(any(target_os = "ios", target_os = "macos"))]
pub(crate) use runtime::{download, navigate, update_tab};
pub(crate) use runtime::{open_aside_for_app, tab_is_aside};
#[cfg(any(target_os = "ios", target_os = "macos", target_env = "ohos"))]
pub(crate) use shell::should_hide_url;
#[cfg(any(target_os = "ios", target_os = "macos"))]
pub(crate) use shell::{
    bookmark_favicon_path, bookmark_pin, bookmark_remove_by_url, bookmark_state, bookmark_status,
    bookmark_toggle, bookmarks_command_json, bookmarks_snapshot_json, normalize_bookmark_url,
    open_panel_lxapp, panel_item_for_id, panels_config_json, resolve_input_json, store_favicon,
};

pub(crate) fn register_bundled_app() {
    runtime::register_bundled_app();
}

#[cfg_attr(not(any(target_os = "android", target_env = "ohos")), allow(dead_code))]
pub(crate) fn classify_navigation_json(request_json: &str) -> Option<String> {
    runtime::classify_navigation_json(request_json)
}

pub(crate) fn register_builtin_runtime() {
    #[cfg(feature = "browser-shell")]
    shell::register_runtime();
    #[cfg(all(feature = "browser-runtime", not(feature = "browser-shell")))]
    runtime::install_runtime_once();
}

pub(crate) fn register_builtin_assets() {
    #[cfg(feature = "browser-shell")]
    shell::register_bundled_assets();
    #[cfg(all(feature = "browser-runtime", not(feature = "browser-shell")))]
    runtime::register_bundled_app_once();
}

pub(crate) fn warmup() {
    #[cfg(feature = "browser-shell")]
    shell::warmup();
    #[cfg(all(feature = "browser-runtime", not(feature = "browser-shell")))]
    runtime::warmup();
}
