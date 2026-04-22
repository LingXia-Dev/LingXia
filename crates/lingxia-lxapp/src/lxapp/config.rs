use crate::lxapp::tabbar::TabBar;
use crate::lxapp::version::Version;
use serde::de::Error as DeError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
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
    /// Page alias mapping: { "alias": "pages/path" }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LxAppPageEntry {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[allow(non_snake_case)]
pub(crate) struct LxAppConfig {
    /// LingXia App ID
    #[serde(default)]
    pub appId: String,

    /// LingXia App name
    #[serde(default, alias = "name")]
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
    #[serde(default)]
    pub(crate) pages: Vec<LxAppPageEntry>,

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
            .map(|page| page.path.clone())
            .unwrap_or_default()
    }

    pub fn page_paths(&self) -> Vec<String> {
        self.pages.iter().map(|page| page.path.clone()).collect()
    }

    pub fn page_entries(&self) -> Vec<LxAppPageEntry> {
        self.pages.clone()
    }

    pub fn page_path_by_name(&self, name: &str) -> Option<String> {
        self.pages
            .iter()
            .find(|page| page.name == name)
            .map(|page| page.path.clone())
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
        if self.version.trim().is_empty() {
            return Err(serde_json::Error::custom(r#""version" must not be empty"#));
        }
        Version::parse(self.version.trim()).map_err(|_| {
            serde_json::Error::custom(r#""version" must be a semantic version (major.minor.patch)"#)
        })?;
        self.version = self.version.trim().to_string();

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

        if self.pages.is_empty() {
            return Err(serde_json::Error::custom(r#""pages" must not be empty"#));
        }

        let mut page_names = BTreeSet::new();
        for page in &self.pages {
            if !is_valid_page_name(&page.name) {
                return Err(serde_json::Error::custom(format!(
                    r#""pages" entry name must use letters, numbers, '_' or '-': {:?}"#,
                    page.name
                )));
            }
            if !page_names.insert(page.name.as_str()) {
                return Err(serde_json::Error::custom(format!(
                    r#""pages" entry name must be unique: {:?}"#,
                    page.name
                )));
            }
            if !is_safe_page_path(&page.path) {
                return Err(serde_json::Error::custom(format!(
                    r#""pages" entry path must stay within the lxapp package: {:?}"#,
                    page.path
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::LxAppConfig;

    #[test]
    fn initial_route_is_empty_when_pages_are_empty() {
        let config = LxAppConfig::default();
        assert_eq!(config.get_initial_route(), "");
    }
}

fn is_valid_page_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn is_safe_page_path(path: &str) -> bool {
    let path = path.trim();
    !path.is_empty()
        && !path.contains('\\')
        && !Path::new(path).is_absolute()
        && Path::new(path)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn is_safe_logic_entry(entry: &str) -> bool {
    if entry.contains('\\') {
        return false;
    }

    Path::new(entry)
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
}
