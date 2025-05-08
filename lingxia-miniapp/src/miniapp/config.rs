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
    pub fn get_initial_route(&self) -> Option<String> {
        self.pages.first().cloned()
    }

    /// Check if a path is a tab page
    pub fn is_tab_page(&self, path: &str) -> bool {
        self.tabBar
            .as_ref()
            .is_some_and(|tab_bar| tab_bar.list.iter().any(|item| item.pagePath == path))
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
    pub fn has_tab_bar(&self) -> bool {
        // Use TabBar's is_valid method to check requirements
        self.tabBar
            .as_ref()
            .is_some_and(|tab_bar| tab_bar.is_valid())
    }

    /// Get the tabBar configuration as JSON string
    /// This is optimized for passing to Java without re-parsing
    pub fn get_tabbar_json(&self) -> Option<String> {
        // Only return tabbar JSON if it's valid (has enough items)
        self.tabBar
            .as_ref()
            .filter(|tab_bar| tab_bar.is_valid())
            .and_then(|tab_bar| serde_json::to_string(tab_bar).ok())
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
