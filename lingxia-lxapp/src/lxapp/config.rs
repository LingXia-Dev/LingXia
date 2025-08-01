pub use navbar::{NavigationBarConfig, NavigationStyle};
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub use tabbar::{TabBarConfig, TabBarPosition, TabItem, TabItemGroup};

mod navbar;
mod tabbar;

/// LxApp basic information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LxAppInfo {
    /// Initial route (first page in the pages array)
    pub initial_route: String,
    /// LxApp name
    pub app_name: String,
    /// Debug mode enabled
    pub debug: bool,
}

/// App config from app.json
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[allow(non_snake_case)]
pub struct LxAppConfig {
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
    pub(crate) tabBar: Option<TabBarConfig>,

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
            initial_route: self.get_initial_route(),
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

    /// Get NavigationBar configuration for a specific page
    #[allow(dead_code)]
    pub fn get_nav_bar_config(
        &self,
        lxapp: &crate::lxapp::LxApp,
        path: &str,
    ) -> NavigationBarConfig {
        // Convert path to JSON file path
        let json_path = path_to_json_path(path);

        // Try to read page-specific configuration
        match lxapp.read_json(&json_path) {
            Ok(json_value) => NavigationBarConfig::from_value(json_value).unwrap_or_default(),
            Err(_) => {
                // Fallback to default configuration
                NavigationBarConfig::default()
            }
        }
    }

    /// Get TabBar configuration with absolute paths
    #[allow(dead_code)]
    pub fn get_tab_bar_config(&self, lxapp: &crate::lxapp::LxApp) -> Option<TabBarConfig> {
        self.tabBar
            .as_ref()
            .filter(|tab_bar| tab_bar.is_valid())
            .map(|tab_bar| tab_bar.with_absolute_paths(&lxapp.lxapp_dir))
    }
}

/// Convert page path to JSON configuration path
#[allow(dead_code)]
fn path_to_json_path(path: &str) -> String {
    // Handle different possible path formats:
    // 1. "pages/home/index.html" -> "pages/home/index.json"
    // 2. "pages/home/index" -> "pages/home/index.json"
    // 3. "pages/home" -> "pages/home.json"
    if path.contains('.') {
        // Has extension: replace it with .json
        let pos = path.rfind('.').unwrap();
        format!("{}.json", &path[0..pos])
    } else {
        // No extension: append .json
        format!("{}.json", path)
    }
}
