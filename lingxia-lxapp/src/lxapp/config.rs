use crate::lxapp::tabbar::TabBar;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// LxApp basic information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LxAppInfo {
    /// LxApp name
    pub app_name: String,
    /// LxApp version
    pub version: String,
    /// LxApp release type (release|preview|developer)
    pub release_type: String,
}

/// Plugin definition embedded in `lxapp.json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct LxPlugin {
    /// Plugin unique identifier - must match the plugin's lxPluginId.
    #[serde(default, rename = "lxPluginId")]
    pub lx_plugin_id: String,
    /// Plugin version.
    #[serde(default)]
    pub version: String,
    /// Plugin logic entry JS filename inside the plugin package directory.
    ///
    /// If empty, defaults to `logic.js`.
    #[serde(default)]
    pub main: String,
    /// Page alias mapping: { "alias": "internal/path" }
    /// e.g., { "home": "pages/home/index" }
    #[serde(default)]
    pub pages: BTreeMap<String, String>,
}

/// App config from lxapp.json
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[allow(non_snake_case)]
pub(crate) struct LxAppConfig {
    /// LingXia App ID
    #[serde(default)]
    pub appId: String,

    /// LingXia App name
    #[serde(default)]
    pub appName: String,

    /// LingXia App version
    #[serde(default)]
    pub version: String,

    /// List of page paths (relative to app root)
    pub(crate) pages: Vec<String>,

    /// Tab bar configuration
    pub(crate) tabBar: Option<TabBar>,

    /// Plugin definitions.
    #[serde(default)]
    pub(crate) plugins: BTreeMap<String, LxPlugin>,
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
    pub fn get_lxapp_info(&self, release_type: &str) -> LxAppInfo {
        LxAppInfo {
            app_name: self.appName.clone(),
            version: self.version.clone(),
            release_type: release_type.to_string(),
        }
    }
}
