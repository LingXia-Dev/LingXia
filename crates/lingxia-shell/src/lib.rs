extern crate self as lingxia;

mod address_bar;
mod downloads;
mod facade;
mod panel;
mod platform_error;
mod settings;

pub use address_bar::{resolve_input, resolve_input_json};
pub use facade::{
    APP_ID, classify_navigation, classify_navigation_json, close, download, open, open_for_app,
    should_hide_url, tab_path, update_tab,
};
pub use lingxia_browser::{
    BrowserAddressAction, BrowserAddressInputContext, BrowserAddressInputRequest,
    BrowserAddressInputResponse, BrowserAddressInputTrigger, BrowserAddressValueKind,
    BrowserNavigationPolicyDecision, BrowserNavigationPolicyRequest,
    BrowserNavigationPolicyResponse, BrowserTabInfo,
};
#[doc(hidden)]
pub use lingxia_macro::host;
#[doc(hidden)]
pub use lxapp::LxApp;
#[doc(hidden)]
pub use lxapp::host;
pub use panel::{open_panel_lxapp, panel_item_for_id, panels_config_json};
#[doc(hidden)]
pub use paste;
use serde::Deserialize;
use std::collections::BTreeMap;
#[doc(hidden)]
pub use tokio;

#[macro_export]
macro_rules! register_hosts {
    ($($handler:ident),+ $(,)?) => {{
        $crate::paste::paste! {
            $(
                $crate::host::register_host_entry([<$handler _host>]());
            )+
        }
    }};
}

/// Browser context menu script installed into each browser tab after page load.
const BROWSER_CONTEXT_MENU_JS: &str = include_str!("../webui/public/browser-context-menu.js");

const BROWSER_WEBUI_MANIFEST: &str = include_str!("../webui/lxapp.json");

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

fn embedded_internal_pages() -> BTreeMap<String, String> {
    serde_json::from_str::<BrowserWebUiManifest>(BROWSER_WEBUI_MANIFEST)
        .expect("failed to parse embedded browser webui manifest")
        .pages
        .named_pages()
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

#[doc(hidden)]
pub fn register() {
    lingxia_browser::install_runtime();
    for (route, entry_asset) in embedded_internal_pages() {
        lingxia_browser::register_internal_page(route, entry_asset)
            .expect("failed to register browser internal page");
    }
    downloads::register();
    settings::register();
    lingxia_browser::install_tab_page_finished_script(BROWSER_CONTEXT_MENU_JS);
}

#[doc(hidden)]
pub fn warmup() {
    lingxia_browser::warmup();
}

#[cfg(test)]
mod tests {
    use super::embedded_internal_pages;

    #[test]
    fn embedded_internal_pages_manifest_is_present() {
        let pages = embedded_internal_pages();
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
}
