extern crate self as lingxia;

mod address_bar;
mod downloads;
mod facade;
mod panel;
mod platform_error;
#[cfg(not(target_os = "android"))]
mod proxy;
#[cfg(not(target_os = "android"))]
mod proxy_settings;
mod settings;

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
pub use lingxia_macro::{host, register_hosts};
use lingxia_platform::traits::app_runtime::AppRuntime;
#[doc(hidden)]
pub use lxapp::LxApp;
#[doc(hidden)]
pub use lxapp::host;
#[doc(hidden)]
pub use lxapp::host::register_host_entry;
pub use panel::{open_panel_lxapp, panel_item_for_id, panels_config_json};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::io::Read;
#[doc(hidden)]
pub use tokio;

const BROWSER_WEBUI_MANIFEST_ASSET_PATH: &str = "app.lingxia.browser/lxapp.json";
const BROWSER_CONTEXT_MENU_ASSET_PATH: &str = "app.lingxia.browser/public/browser-context-menu.js";

#[derive(Debug, Deserialize)]
struct BrowserWebUiManifest {
    #[serde(default)]
    pages: BrowserWebUiPages,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum BrowserWebUiPages {
    Ordered(Vec<String>),
    Named(BTreeMap<String, String>),
}

impl Default for BrowserWebUiPages {
    fn default() -> Self {
        Self::Ordered(Vec::new())
    }
}

impl BrowserWebUiPages {
    fn named_pages(self) -> BTreeMap<String, String> {
        match self {
            Self::Ordered(pages) => {
                let _ = pages.len();
                BTreeMap::new()
            }
            Self::Named(pages) => pages,
        }
    }
}

fn parse_internal_pages(manifest_json: &str) -> Result<BTreeMap<String, String>, LxAppError> {
    serde_json::from_str::<BrowserWebUiManifest>(manifest_json)
        .map(|manifest| manifest.pages.named_pages())
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
    #[cfg(not(target_os = "android"))]
    proxy::register();
    settings::register();
}

#[doc(hidden)]
pub fn register_bundled_assets() {
    match bundled_internal_pages() {
        Ok(internal_pages) => {
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
    #[cfg(not(target_os = "android"))]
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
                "pages": {
                    "newtab": "pages/newtab/index.html",
                    "downloads": "pages/downloads/index.html",
                    "settings": "pages/settings/index.html"
                }
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
    fn ordered_pages_manifest_does_not_register_internal_routes() {
        let pages = parse_internal_pages(r#"{ "pages": ["pages/newtab/index.html"] }"#)
            .expect("manifest should parse");
        assert!(pages.is_empty());
    }
}
