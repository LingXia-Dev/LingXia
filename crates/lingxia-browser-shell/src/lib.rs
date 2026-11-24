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
use lingxia_browser::LxAppError;
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
const BROWSER_CONTEXT_MENU_ASSET_PATH: &str = "app.lingxia.browser/public/browser-context-menu.js";

#[derive(Debug, Deserialize)]
struct BrowserWebUiManifest {
    #[serde(rename = "appId")]
    app_id: String,
    #[serde(default)]
    version: Option<String>,
}

fn parse_browser_webui_manifest(manifest_json: &str) -> Result<BrowserWebUiManifest, LxAppError> {
    let manifest = serde_json::from_str::<BrowserWebUiManifest>(manifest_json).map_err(|err| {
        LxAppError::InvalidJsonFile(format!("{}: {}", BROWSER_WEBUI_MANIFEST_ASSET_PATH, err))
    })?;
    if manifest.app_id != lingxia_browser::BUILTIN_BROWSER_APPID {
        return Err(LxAppError::InvalidJsonFile(format!(
            "{}: appId must be {}, got {}",
            BROWSER_WEBUI_MANIFEST_ASSET_PATH,
            lingxia_browser::BUILTIN_BROWSER_APPID,
            manifest.app_id
        )));
    }
    Ok(manifest)
}

fn read_browser_asset_text(asset_path: &str) -> Result<String, LxAppError> {
    let runtime = lxapp::get_platform().ok_or_else(|| {
        LxAppError::Runtime(
            "browser asset loading requires an initialized host runtime".to_string(),
        )
    })?;
    let mut reader = runtime.read_asset(asset_path).map_err(|err| {
        LxAppError::ResourceNotFound(format!("browser asset {} ({})", asset_path, err))
    })?;
    let mut content = String::new();
    reader
        .read_to_string(&mut content)
        .map_err(|err| LxAppError::IoError(format!("failed to read {}: {}", asset_path, err)))?;
    Ok(content)
}

pub(crate) fn bundled_webui_version() -> Option<String> {
    let manifest = read_browser_asset_text(BROWSER_WEBUI_MANIFEST_ASSET_PATH).ok()?;
    parse_browser_webui_manifest(&manifest).ok()?.version
}

fn bundled_context_menu_script() -> Result<String, LxAppError> {
    read_browser_asset_text(BROWSER_CONTEXT_MENU_ASSET_PATH)
}

#[doc(hidden)]
pub fn register_route_inventory() {
    static REGISTERED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    REGISTERED.get_or_init(|| {
        downloads::register();
        bookmarks::register();
        shell::register();
        history::register();
        privacy::register();
        #[cfg(all(any(target_os = "macos", target_os = "windows"), feature = "proxy"))]
        proxy::register();
        settings::register_routes();
    });
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

#[doc(hidden)]
pub fn register_bundled_assets(native_authority: &lxapp::NativeControlPlaneAuthority) {
    match read_browser_asset_text(BROWSER_WEBUI_MANIFEST_ASSET_PATH)
        .and_then(|manifest| parse_browser_webui_manifest(&manifest))
    {
        Ok(_) => {
            // app.lingxia.browser is a host-bundled ControlSurface lxapp (GPU View + Rust Logic).
            lxapp::register_builtin_asset_bundle(lingxia_browser::BUILTIN_BROWSER_APPID);
        }
        Err(err) => {
            panic!(
                "browser-shell requires bundled browser webui manifest at {}: {}",
                BROWSER_WEBUI_MANIFEST_ASSET_PATH, err
            );
        }
    }

    match bundled_context_menu_script() {
        Ok(script) => {
            if let Err(err) = lingxia_browser::register_document_script(native_authority, script) {
                lxapp::warn!(
                    "[InternalBrowser] failed to register browser document script: {}",
                    err
                );
            }
        }
        Err(err) => {
            lxapp::info!(
                "[InternalBrowser] bundled browser context menu unavailable; skipping startup script: {}",
                err
            );
        }
    }
    if let Err(err) = lingxia_browser::seal_document_scripts(native_authority) {
        panic!("failed to seal browser document scripts: {err}");
    }
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
