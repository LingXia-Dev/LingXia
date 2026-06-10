//! Shell product module and host registrations for LingXia.
//!
//! This crate owns product-level shell behavior on top of the generic
//! runtime crates: address-bar resolution, downloads, settings, panels,
//! and — on Windows — the custom window chrome (GDI painting and
//! hit-testing) registered into `lingxia-webview`'s
//! `WindowsChromeRenderer` seam. `lingxia-webview` itself stays strictly
//! generic webview hosting.

extern crate self as lingxia;

mod address_bar;
mod downloads;
mod facade;
mod panel;
mod platform_error;
#[cfg(target_os = "macos")]
mod proxy;
#[cfg(target_os = "macos")]
mod proxy_settings;
mod settings;
#[cfg(target_os = "windows")]
mod windows;

pub use address_bar::{resolve_input, resolve_input_json};
pub use facade::{
    APP_ID, classify_navigation, classify_navigation_json, close, download, open, open_for_app,
    should_hide_url, tab_path, update_tab,
};
use lingxia_browser::LxAppError;
pub use lingxia_browser::{
    BrowserAddressAction, BrowserAddressInputContext, BrowserAddressInputRequest,
    BrowserAddressInputResponse, BrowserAddressInputTrigger, BrowserAddressValueKind,
    BrowserNavigationPolicyDecision, BrowserNavigationPolicyRequest,
    BrowserNavigationPolicyResponse, BrowserTabInfo,
};
#[doc(hidden)]
pub use lingxia_native_macros::native;
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
    #[serde(default)]
    pages: Vec<BrowserWebUiPage>,
}

#[derive(Debug, Deserialize)]
struct BrowserWebUiPage {
    name: String,
    path: String,
}

fn parse_internal_pages(manifest_json: &str) -> Result<BTreeMap<String, String>, LxAppError> {
    serde_json::from_str::<BrowserWebUiManifest>(manifest_json)
        .map(|manifest| {
            manifest
                .pages
                .into_iter()
                .map(|page| (page.name, page.path))
                .collect()
        })
        .map_err(|err| {
            LxAppError::InvalidJsonFile(format!("{}: {}", BROWSER_WEBUI_MANIFEST_ASSET_PATH, err))
        })
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

fn bundled_context_menu_script() -> Result<String, LxAppError> {
    read_browser_asset_text(BROWSER_CONTEXT_MENU_ASSET_PATH)
}

#[doc(hidden)]
pub fn register_runtime() {
    lingxia_browser::install_runtime();
    downloads::register();
    #[cfg(target_os = "macos")]
    proxy::register();
    #[cfg(target_os = "windows")]
    windows::install();
    settings::register();
}

#[doc(hidden)]
pub fn register_bundled_assets() {
    match bundled_internal_pages() {
        Ok(internal_pages) => {
            // Upgrade the browser host from Synthetic to a real asset bundle so the
            // lingxia:// scheme can serve newtab/settings/downloads pages.
            lxapp::register_builtin_asset_bundle(lingxia_browser::BUILTIN_BROWSER_APPID);
            for (route, entry_asset) in internal_pages {
                if let Err(err) = lingxia_browser::register_internal_page(route, entry_asset) {
                    lxapp::warn!(
                        "[InternalBrowser] failed to register bundled browser page: {}",
                        err
                    );
                }
            }
        }
        Err(err) => {
            lxapp::info!(
                "[InternalBrowser] bundled browser manifest unavailable; skipping bundled browser pages: {}",
                err
            );
            return;
        }
    }

    match bundled_context_menu_script() {
        Ok(script) => lingxia_browser::register_startup_page_script(script),
        Err(err) => {
            lxapp::info!(
                "[InternalBrowser] bundled browser context menu unavailable; skipping startup script: {}",
                err
            );
        }
    }
}

#[doc(hidden)]
pub fn warmup() {
    #[cfg(target_os = "macos")]
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
                "pages": [
                    { "name": "newtab", "path": "pages/newtab/index.html" },
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
        assert!(parse_internal_pages(r#"{ "pages": ["pages/newtab/index.html"] }"#).is_err());
    }
}
