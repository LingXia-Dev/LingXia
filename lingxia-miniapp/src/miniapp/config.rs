use crate::miniapp::tabbar::TabBar;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// App config from app.json
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[allow(non_snake_case)]
pub(crate) struct MiniAppConfig {
    /// List of page paths (relative to app root)
    pub pages: Vec<String>,

    /// Tab bar configuration
    pub tabBar: Option<TabBar>,
}

impl MiniAppConfig {
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

    /// Get the tabBar configuration as JSON string with absolute paths
    ///
    /// # Arguments
    /// * `base_path` - Base path for resolving relative paths
    ///
    /// # Returns
    /// TabBar JSON string with icon paths converted to absolute paths
    pub fn get_tabbar_json_with_base_path(&self, base_path: &std::path::Path) -> Option<String> {
        // Only return tabbar JSON if it's valid (has enough items)
        self.tabBar
            .as_ref()
            .filter(|tab_bar| tab_bar.is_valid())
            .and_then(|tab_bar| {
                // Convert paths to absolute
                let tab_bar_with_abs_paths = tab_bar.with_absolute_paths(base_path);
                serde_json::to_string(&tab_bar_with_abs_paths).ok()
            })
    }
}

// Page configuration for a specific page
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[allow(non_snake_case)]
pub struct PageConfig {
    /// Navigation bar background color
    #[serde(default)]
    pub navigationBarBackgroundColor: String,

    /// Navigation bar text color, can be "black" or "white"
    #[serde(default)]
    pub navigationBarTextStyle: String,

    /// Navigation bar title
    #[serde(default)]
    pub navigationBarTitleText: String,

    /// Whether the navigation bar is transparent/custom
    #[serde(default)]
    pub navigationStyle: String,

    /// Whether to hide the navigation bar
    #[serde(default)]
    pub hidden: bool,
}

impl PageConfig {
    /// Create PageConfig from serde_json::Value
    pub fn from_value(value: Value) -> Result<Self, serde_json::Error> {
        serde_json::from_value(value)
    }
}
