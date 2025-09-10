use serde::{Deserialize, Serialize};
use serde_json::Value;

/// LxApp basic information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LxAppInfo {
    /// LxApp name
    pub app_name: String,
    /// Debug mode enabled
    pub debug: bool,
}

/// App config from app.json
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[allow(non_snake_case)]
pub(crate) struct LxAppConfig {
    /// LingXia App ID
    #[serde(default)]
    pub lxAppId: String,

    /// LingXia App name
    #[serde(default)]
    pub lxAppName: String,

    /// LingXia App version
    #[serde(default)]
    pub version: String,

    /// List of page paths (relative to app root)
    pub(crate) pages: Vec<String>,

    /// Tab bar configuration
    tabBar: Option<crate::lxapp::tabbar::TabBar>,

    /// Debug mode - when true, developer tools will be enabled for all pages
    #[serde(default)]
    pub debug: bool,
}

impl LxAppConfig {
    /// Create AppConfig from serde_json::Value
    pub fn from_value(value: Value) -> Result<Self, serde_json::Error> {
        serde_json::from_value(value)
    }

    /// Get the initial route (first page in the pages array)
    pub fn get_initial_route(&self) -> String {
        self.pages
            .first()
            .cloned()
            .unwrap_or("PagesEmpty".to_string())
    }

    /// Get LxApp basic information for FFI
    pub fn get_lxapp_info(&self) -> LxAppInfo {
        LxAppInfo {
            app_name: self.lxAppName.clone(),
            debug: self.debug,
        }
    }

    /// Check if debug mode is enabled
    pub fn is_debug_enabled(&self) -> bool {
        self.debug
    }

    /// Check if a path is a tab page
    pub fn is_tab_page(&self, path: &str) -> bool {
        self.tabBar
            .as_ref()
            .is_some_and(|tab_bar| tab_bar.list.iter().any(|item| item.pagePath == path))
    }

    /// Check if a path is the initial route (first page in the pages array)
    pub fn is_initial_route(&self, path: &str) -> bool {
        self.pages
            .first()
            .is_some_and(|initial_route| initial_route == path)
    }

    /// Get all tab page paths
    pub fn get_tab_pages(&self) -> Vec<String> {
        match &self.tabBar {
            Some(tab_bar) => tab_bar
                .list
                .iter()
                .map(|item| item.pagePath.clone())
                .collect(),
            None => Vec::new(),
        }
    }

    /// Check if the app has a tab bar
    #[allow(dead_code)]
    pub fn has_tab_bar(&self) -> bool {
        // Use TabBar's is_valid method to check requirements
        self.tabBar
            .as_ref()
            .is_some_and(|tab_bar| tab_bar.is_valid())
    }

    /// Get TabBar with absolute paths
    #[allow(dead_code)]
    pub fn get_tab_bar(&self, lxapp: &crate::lxapp::LxApp) -> Option<crate::lxapp::tabbar::TabBar> {
        self.tabBar
            .as_ref()
            .map(|tab_bar| tab_bar.with_absolute_paths(&lxapp.lxapp_dir))
    }
}

