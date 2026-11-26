//! Shell product module and host registrations for LingXia.
//!
//! This crate owns product-level shell behavior on top of the generic
//! runtime crates: address-bar resolution, downloads, settings, panels,
//! and bundled browser assets. Platform SDK crates own their native shell UI.

extern crate self as lingxia;

#[cfg(all(test, target_vendor = "apple"))]
#[path = "../../lingxia-platform/tests/support/apple_host_stubs.rs"]
mod apple_host_stubs;

mod address_bar;
mod bookmarks;
mod bookmarks_html;
mod downloads;
mod facade;
mod history;
mod panel;
mod platform_error;
mod privacy;
#[cfg(all(any(target_os = "macos", target_os = "windows"), feature = "proxy"))]
mod proxy;
#[cfg(all(any(target_os = "macos", target_os = "windows"), feature = "proxy"))]
mod proxy_settings;
mod settings;
mod shell;
mod url_match;

pub use address_bar::{resolve_input, resolve_input_json};
pub use bookmarks::{
    BookmarkEntry, BookmarksSnapshot, command_json as bookmarks_command_json,
    favicon_path as bookmark_favicon_path, is_bookmarked,
    normalize_url_for_match as normalize_bookmark_url, pin_url as pin_bookmark_url,
    pin_url_with_favicon as pin_bookmark_url_with_favicon,
    pinned_entry_for_url as pinned_bookmark_for_url, remove_by_url as remove_bookmark_by_url,
    set_change_listener as set_bookmarks_change_listener, snapshot as bookmarks_snapshot,
    snapshot_json as bookmarks_snapshot_json, store_favicon as store_bookmark_favicon,
    toggle_bookmark,
};
pub use facade::{
    APP_ID, classify_navigation, classify_navigation_json, close, download, open, open_for_app,
    should_hide_url, tab_path, update_tab,
};
pub use history::{record_visit as record_history_visit, update_title as update_history_title};
pub use lingxia_browser::{
    BrowserAddressAction, BrowserAddressInputContext, BrowserAddressInputRequest,
    BrowserAddressInputResponse, BrowserAddressInputTrigger, BrowserAddressValueKind,
    BrowserNavigationPolicyDecision, BrowserNavigationPolicyRequest,
    BrowserNavigationPolicyResponse, BrowserTabInfo,
};
#[doc(hidden)]
pub use lingxia_native_macros::framework_native;
use lingxia_platform::traits::app_runtime::AppRuntime;
#[doc(hidden)]
pub use lxapp::LxApp;
#[doc(hidden)]
pub use lxapp::host;
pub use panel::{open_panel_lxapp, panel_item_for_id, panels_config_json};
use serde::Deserialize;
use std::io::Read;

const BROWSER_WEBUI_MANIFEST_ASSET_PATH: &str = "app.lingxia.browser/lxapp.json";

#[derive(Debug, Deserialize)]
struct BrowserWebUiManifest {
    #[serde(rename = "appId")]
    app_id: String,
    #[serde(default)]
    version: Option<String>,
}

fn parse_browser_webui_manifest(
    manifest_json: &str,
) -> Result<BrowserWebUiManifest, lingxia_error::Error> {
    let manifest = serde_json::from_str::<BrowserWebUiManifest>(manifest_json).map_err(|err| {
        lingxia_error::Error::invalid_data(format!(
            "{}: {}",
            BROWSER_WEBUI_MANIFEST_ASSET_PATH, err
        ))
    })?;
    if manifest.app_id != lingxia_browser::BUILTIN_BROWSER_APPID {
        return Err(lingxia_error::Error::invalid_data(format!(
            "{}: appId must be {}, got {}",
            BROWSER_WEBUI_MANIFEST_ASSET_PATH,
            lingxia_browser::BUILTIN_BROWSER_APPID,
            manifest.app_id
        )));
    }
    Ok(manifest)
}

fn read_browser_asset_text(asset_path: &str) -> Result<String, lingxia_error::Error> {
    let runtime = lxapp::get_platform().ok_or_else(|| {
        lingxia_error::Error::internal("browser asset loading requires an initialized host runtime")
    })?;
    let mut reader = runtime.read_asset(asset_path).map_err(|err| {
        lingxia_error::Error::not_found(format!("browser asset {} ({})", asset_path, err))
    })?;
    let mut content = String::new();
    reader.read_to_string(&mut content).map_err(|err| {
        lingxia_error::Error::io(format!("failed to read {}: {}", asset_path, err))
    })?;
    Ok(content)
}

pub(crate) fn bundled_webui_version() -> Option<String> {
    let manifest = read_browser_asset_text(BROWSER_WEBUI_MANIFEST_ASSET_PATH).ok()?;
    parse_browser_webui_manifest(&manifest).ok()?.version
}

include!(concat!(env!("OUT_DIR"), "/lingxia_native_handlers.rs"));

#[doc(hidden)]
pub fn register_route_inventory() {
    static REGISTERED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    REGISTERED.get_or_init(__lingxia_native::install);
}

#[doc(hidden)]
pub fn register_runtime() {
    register_route_inventory();
    static REGISTERED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    REGISTERED.get_or_init(|| {
        lingxia_browser::install_runtime();
        lingxia_browser::set_navigation_finished_handler(std::sync::Arc::new(|url, title| {
            history::record_visit(url, title);
        }));
        lingxia_browser::set_title_changed_handler(std::sync::Arc::new(|url, title| {
            history::update_title(url, title);
        }));
    });
}

/// Registers `app.lingxia.browser` as a host-bundled lxapp (GPU View + Rust
/// Logic), so the browser can open it as a ControlSurface. A host without
/// `browser.webui` has no system pages to register.
#[doc(hidden)]
pub fn register_bundled_assets() {
    if !lingxia_app_context::browser_system_pages_enabled() {
        return;
    }
    if let Err(err) = read_browser_asset_text(BROWSER_WEBUI_MANIFEST_ASSET_PATH)
        .and_then(|manifest| parse_browser_webui_manifest(&manifest))
    {
        panic!(
            "browser-shell requires bundled browser webui manifest at {}: {}",
            BROWSER_WEBUI_MANIFEST_ASSET_PATH, err
        );
    }
    lxapp::register_builtin_asset_bundle(lingxia_browser::BUILTIN_BROWSER_APPID);
}

#[doc(hidden)]
pub fn warmup() {
    #[cfg(all(any(target_os = "macos", target_os = "windows"), feature = "proxy"))]
    proxy::warmup();
}

#[cfg(test)]
mod tests {
    use super::parse_browser_webui_manifest;

    #[test]
    fn rejects_non_browser_app_id() {
        let error = parse_browser_webui_manifest(r#"{"appId":"example.fake"}"#)
            .unwrap_err()
            .to_string();
        assert!(error.contains("app.lingxia.browser"), "{error}");
        let manifest =
            parse_browser_webui_manifest(r#"{"appId":"app.lingxia.browser","version":"2.0.0"}"#)
                .unwrap();
        assert_eq!(manifest.version.as_deref(), Some("2.0.0"));
    }
}
