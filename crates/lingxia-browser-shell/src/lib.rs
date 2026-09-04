//! Shell product module and host registrations for LingXia.
//!
//! This crate owns product-level shell behavior on top of the generic
//! runtime crates: address-bar resolution, downloads, settings, panels,
//! and bundled browser assets. Platform SDK crates own their native shell UI.

extern crate self as lingxia;

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
mod url_match;

pub use address_bar::{resolve_input, resolve_input_json};
pub use bookmarks::{
    BookmarkEntry, BookmarksSnapshot, command_json as bookmarks_command_json,
    favicon_path as bookmark_favicon_path, is_bookmarked,
    normalize_url_for_match as normalize_bookmark_url, pin_url as pin_bookmark_url,
    pin_url_with_favicon as pin_bookmark_url_with_favicon, remove_by_url as remove_bookmark_by_url,
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
use std::collections::BTreeMap;
use std::io::Read;

const BROWSER_WEBUI_MANIFEST_ASSET_PATH: &str = "app.lingxia.browser/lxapp.json";
const BROWSER_CONTEXT_MENU_ASSET_PATH: &str = "app.lingxia.browser/public/browser-context-menu.js";

#[derive(Debug, Deserialize)]
struct BrowserWebUiManifest {
    #[serde(rename = "appId")]
    app_id: String,
    #[serde(default)]
    version: Option<String>,
    #[serde(rename = "controlProtocolVersion")]
    control_protocol_version: u32,
    #[serde(default)]
    pages: Vec<BrowserWebUiPage>,
}

#[derive(Debug, Deserialize)]
struct BrowserWebUiPage {
    name: String,
    path: String,
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
    if manifest.control_protocol_version != lingxia_browser::CONTROL_PROTOCOL_VERSION {
        return Err(LxAppError::InvalidJsonFile(format!(
            "{}: controlProtocolVersion must be {}, got {}",
            BROWSER_WEBUI_MANIFEST_ASSET_PATH,
            lingxia_browser::CONTROL_PROTOCOL_VERSION,
            manifest.control_protocol_version
        )));
    }
    Ok(manifest)
}

fn parse_internal_pages(manifest_json: &str) -> Result<BTreeMap<String, String>, LxAppError> {
    Ok(parse_browser_webui_manifest(manifest_json)?
        .pages
        .into_iter()
        .map(|page| (page.name, page.path))
        .collect())
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

fn bundled_internal_pages() -> Result<BTreeMap<String, String>, LxAppError> {
    let manifest = read_browser_asset_text(BROWSER_WEBUI_MANIFEST_ASSET_PATH)?;
    parse_internal_pages(&manifest)
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
    match bundled_internal_pages() {
        Ok(internal_pages) => {
            // Upgrade the browser host from Synthetic to a real asset bundle so the
            // lingxia:// scheme can serve newtab/settings/downloads pages.
            lxapp::register_builtin_asset_bundle(lingxia_browser::BUILTIN_BROWSER_APPID);
            for (route, entry_asset) in internal_pages {
                if let Err(err) =
                    lingxia_browser::register_internal_page(native_authority, route, entry_asset)
                {
                    lxapp::warn!(
                        "[InternalBrowser] failed to register bundled browser page: {}",
                        err
                    );
                }
            }
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
    if let Err(err) = lingxia_browser::seal_control_registration(native_authority) {
        panic!("failed to seal browser control registration: {err}");
    }
}

#[doc(hidden)]
pub fn warmup() {
    #[cfg(all(any(target_os = "macos", target_os = "windows"), feature = "proxy"))]
    proxy::warmup();
    lingxia_browser::warmup();
}

#[cfg(test)]
mod tests {
    use super::parse_internal_pages;

    #[test]
    fn parses_named_internal_pages_manifest() {
        let pages = parse_internal_pages(
            r#"{
                "appId": "app.lingxia.browser",
                "controlProtocolVersion": 3,
                "pages": [
                    { "name": "newtab", "path": "pages/newtab/index.html" },
                    { "name": "history", "path": "pages/history/index.html" },
                    { "name": "downloads", "path": "pages/downloads/index.html" },
                    { "name": "settings", "path": "pages/settings/index.html" }
                ]
            }"#,
        )
        .expect("manifest should parse");
        assert_eq!(
            pages.get("newtab").map(String::as_str),
            Some("pages/newtab/index.html")
        );
        assert_eq!(
            pages.get("history").map(String::as_str),
            Some("pages/history/index.html")
        );
        assert_eq!(
            pages.get("downloads").map(String::as_str),
            Some("pages/downloads/index.html")
        );
        assert_eq!(
            pages.get("settings").map(String::as_str),
            Some("pages/settings/index.html")
        );
    }

    #[test]
    fn rejects_legacy_ordered_pages_manifest() {
        assert!(
            parse_internal_pages(
                r#"{
                    "appId": "app.lingxia.browser",
                    "controlProtocolVersion": 3,
                    "pages": ["pages/newtab/index.html"]
                }"#
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_missing_old_and_future_control_protocol_versions() {
        for version in [None, Some(2), Some(4)] {
            let version = version
                .map(|version| format!(r#", "controlProtocolVersion": {version}"#))
                .unwrap_or_default();
            let manifest = format!(r#"{{"appId":"app.lingxia.browser"{version},"pages":[]}}"#);
            let error = parse_internal_pages(&manifest).unwrap_err().to_string();
            assert!(error.contains("controlProtocolVersion"), "{error}");
        }
    }

    #[test]
    fn rejects_non_browser_app_id_even_at_protocol_v3() {
        let error = parse_internal_pages(
            r#"{"appId":"example.fake","controlProtocolVersion":3,"pages":[]}"#,
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("appId"), "{error}");
        assert!(error.contains("app.lingxia.browser"), "{error}");
    }
}
