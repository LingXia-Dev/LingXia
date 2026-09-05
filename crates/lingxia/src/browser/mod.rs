//! Internal browser bridge facade.
//!
//! This module is intentionally crate-private. Public native APIs should stay in
//! focused facades such as `app`, `file`, `media`, and `update`.
#![cfg_attr(target_os = "windows", allow(unused_imports))]

mod runtime;
mod shell;

#[cfg(feature = "browser-runtime")]
static NATIVE_CONTROL_AUTHORITY: std::sync::OnceLock<lxapp::NativeControlPlaneAuthority> =
    std::sync::OnceLock::new();

pub(crate) fn install_native_control_authority(
    authority: lxapp::NativeControlPlaneAuthority,
) -> Result<(), &'static str> {
    #[cfg(feature = "browser-runtime")]
    return NATIVE_CONTROL_AUTHORITY
        .set(authority)
        .map_err(|_| "native browser control authority was already installed");
    #[cfg(not(feature = "browser-runtime"))]
    {
        let _ = authority;
        Ok(())
    }
}

#[cfg(feature = "browser-runtime")]
pub(crate) fn native_control_authority()
-> Result<&'static lxapp::NativeControlPlaneAuthority, lxapp::LxAppError> {
    NATIVE_CONTROL_AUTHORITY.get().ok_or_else(|| {
        lxapp::LxAppError::UnsupportedOperation(
            "native browser control authority is not initialized".to_string(),
        )
    })
}

#[cfg(target_os = "android")]
pub(crate) use runtime::navigate;
#[cfg(all(target_env = "ohos", not(any(target_os = "ios", target_os = "macos"))))]
pub(crate) use runtime::navigate;
#[cfg(any(target_os = "ios", target_os = "macos"))]
pub(crate) use runtime::open;
pub(crate) use runtime::open_standalone_for_app;
pub(crate) use runtime::{
    APP_ID, close, mark_active, navigate_trusted_control_page, open_for_app, tab_path,
};
#[cfg(target_os = "windows")]
pub(crate) use runtime::{
    BrowserTabSummary, activate, go_back, go_forward, navigate, reload, runtime_enabled,
    set_tabs_changed_handler, tab_summary, tabs,
};
#[cfg(any(target_os = "ios", target_os = "macos"))]
pub(crate) use runtime::{clear_active, discard, reactivate};
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

#[cfg(feature = "browser-shell")]
fn settings_required_routes() -> Vec<&'static str> {
    let routes = vec![
        "app.getInfo",
        "downloads.chooseDirectory",
        "downloads.getSettings",
        "downloads.resetDirectory",
        "privacy.clearBrowsingData",
        "privacy.clearSiteData",
        "privacy.getSiteDataContext",
        "privacy.getUsage",
        "app.getDisplayLanguageState",
        "app.setDisplayLanguagePreference",
        "app.watchDisplayLanguageState",
    ];
    #[cfg(feature = "proxy")]
    let routes = {
        let mut routes = routes;
        routes.extend([
            "proxy.getSettings",
            "proxy.refreshGfwList",
            "proxy.updateSettings",
            "proxy.watch",
        ]);
        routes
    };
    routes
}

pub(crate) fn configure_static_settings_targets(catalog: &mut crate::StaticSettingsTargetCatalog) {
    #[cfg(feature = "browser-shell")]
    catalog.require_browser_page_routes("/settings", settings_required_routes());
    #[cfg(not(feature = "browser-shell"))]
    let _ = catalog;
}

pub(crate) fn register_builtin_route_inventory() {
    #[cfg(feature = "browser-shell")]
    shell::register_route_inventory();
}

pub(crate) fn register_builtin_runtime() {
    static REGISTERED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    REGISTERED.get_or_init(|| {
        #[cfg(feature = "browser-shell")]
        shell::register_runtime();
        #[cfg(all(feature = "browser-runtime", not(feature = "browser-shell")))]
        runtime::install_runtime_once();
    });
}

pub(crate) fn register_builtin_assets(authority: &lxapp::NativeControlPlaneAuthority) {
    #[cfg(feature = "browser-shell")]
    shell::register_bundled_assets(authority);
    #[cfg(all(feature = "browser-runtime", not(feature = "browser-shell")))]
    runtime::register_bundled_app_once();
    #[cfg(not(feature = "browser-shell"))]
    let _ = authority;
}

pub(crate) fn warmup() {
    #[cfg(feature = "browser-shell")]
    shell::warmup();
    #[cfg(all(feature = "browser-runtime", not(feature = "browser-shell")))]
    runtime::warmup();
}

#[cfg(all(test, feature = "browser-shell"))]
mod tests {
    #[test]
    fn settings_catalog_routes_match_production_inventory_and_policy() {
        crate::host_addon::run_install_host_apis();
        crate::display_language_host::register();
        super::register_builtin_route_inventory();
        lxapp::host::register_builtin_routes();

        for route in super::settings_required_routes() {
            let audience = lxapp::host::route_policy(route)
                .unwrap_or_else(|error| panic!("{route}: {error}"))
                .unwrap_or_else(|| panic!("missing production Settings route: {route}"))
                .audience();
            assert!(
                matches!(
                    audience,
                    lxapp::host::RouteAudience::BrowserControlOnly
                        | lxapp::host::RouteAudience::ControlOnly
                ),
                "incompatible production Settings route {route}: {audience:?}"
            );
        }
    }
}
