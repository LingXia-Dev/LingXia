//! Internal browser bridge facade.
//!
//! This module is intentionally crate-private. Public native APIs should stay in
//! focused facades such as `app`, `file`, `media`, and `update`.
#![cfg_attr(target_os = "windows", allow(unused_imports))]

mod runtime;
mod shell;

pub(crate) use runtime::{APP_ID, close, open_for_app, tab_path};
#[cfg(target_os = "windows")]
pub(crate) use runtime::{
    BrowserTabSummary, activate, go_back, go_forward, navigate, reload, runtime_enabled,
    set_tabs_changed_handler, tab_summary, tabs,
};
#[cfg(any(target_os = "ios", target_os = "macos"))]
pub(crate) use runtime::{download, update_tab};
#[cfg(any(target_os = "ios", target_os = "macos"))]
pub(crate) use shell::{
    open_panel_lxapp, panel_item_for_id, panels_config_json, resolve_input_json, should_hide_url,
};

pub(crate) fn register_bundled_app() {
    runtime::register_bundled_app();
}

#[cfg_attr(not(any(target_os = "android", target_env = "ohos")), allow(dead_code))]
pub(crate) fn classify_navigation_json(request_json: &str) -> Option<String> {
    runtime::classify_navigation_json(request_json)
}

pub(crate) fn register_builtin_runtime() {
    #[cfg(feature = "shell-runtime")]
    shell::register_runtime();
    #[cfg(all(feature = "browser-runtime", not(feature = "shell-runtime")))]
    runtime::install_runtime_once();
}

pub(crate) fn register_builtin_assets() {
    #[cfg(feature = "shell-runtime")]
    shell::register_bundled_assets();
    #[cfg(all(feature = "browser-runtime", not(feature = "shell-runtime")))]
    runtime::register_bundled_app_once();
}

pub(crate) fn warmup() {
    #[cfg(feature = "shell-runtime")]
    shell::warmup();
    #[cfg(all(feature = "browser-runtime", not(feature = "shell-runtime")))]
    runtime::warmup();
}
