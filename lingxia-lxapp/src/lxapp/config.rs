use crate::lxapp::tabbar::TabBar;
use serde::de::Error as DeError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Component, Path};

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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum LxAppLogicEntry {
    Enabled(bool),
    Entry(String),
}

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

    /// Logic entry configuration.
    ///
    /// - omitted => defaults to `logic.js`
    /// - false => disable logic/appservice entirely, and ignore page.json config
    /// - true => use default `logic.js`
    /// - "path/to/entry.js" => use a custom entry inside the lxapp package
    #[serde(default)]
    pub logic: Option<LxAppLogicEntry>,

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
        if value
            .as_object()
            .is_some_and(|object| object.contains_key("appService"))
        {
            return Err(serde_json::Error::custom(
                r#""appService" is no longer supported; use "logic" instead"#,
            ));
        }

        let mut config: Self = serde_json::from_value(value)?;
        config.validate()?;
        Ok(config)
    }

    /// Get the initial route (first page in the pages array)
    pub fn get_initial_route(&self) -> String {
        self.pages
            .first()
            .cloned()
            .unwrap_or("PagesEmpty".to_string())
    }

    pub fn logic_entry(&self) -> Option<String> {
        match &self.logic {
            Some(LxAppLogicEntry::Enabled(false)) => None,
            Some(LxAppLogicEntry::Enabled(true)) => Some("logic.js".to_string()),
            Some(LxAppLogicEntry::Entry(entry)) => Some(entry.clone()),
            None => Some("logic.js".to_string()),
        }
    }

    /// Get LxApp basic information for FFI
    pub fn get_lxapp_info(&self, release_type: &str) -> LxAppInfo {
        LxAppInfo {
            app_name: self.appName.clone(),
            version: self.version.clone(),
            release_type: release_type.to_string(),
        }
    }

    fn validate(&mut self) -> Result<(), serde_json::Error> {
        if let Some(LxAppLogicEntry::Entry(entry)) = &mut self.logic {
            let trimmed = entry.trim();
            if trimmed.is_empty() {
                return Err(serde_json::Error::custom(
                    r#""logic" entry must not be empty"#,
                ));
            }
            if !is_safe_logic_entry(trimmed) {
                return Err(serde_json::Error::custom(format!(
                    r#""logic" entry must stay within the lxapp package: {:?}"#,
                    entry
                )));
            }
            *entry = trimmed.to_string();
        }

        Ok(())
    }
}

fn is_safe_logic_entry(entry: &str) -> bool {
    if entry.contains('\\') {
        return false;
    }

    Path::new(entry)
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
}
