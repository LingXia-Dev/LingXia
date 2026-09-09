use crate::lxapp::page_chrome::AppearancePreference;
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
    /// PageInstance alias mapping: { "alias": "pages/path" }
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

    /// Lxapp-scoped palette preference. A saved user preference takes
    /// precedence over this manifest default.
    #[serde(default)]
    pub(crate) appearance: AppearancePreference,

    /// Plugin definitions.
    #[serde(default)]
    pub(crate) plugins: BTreeMap<String, LxPlugin>,
}

impl LxAppConfig {
    /// Create AppConfig from serde_json::Value
    pub fn from_value(value: Value) -> Result<Self, serde_json::Error> {
        if let Some(object) = value.as_object() {
            if object.contains_key("appService") {
                return Err(serde_json::Error::custom(
                    r#""appService" is no longer supported; use "logic" instead"#,
                ));
            }
            validate_removed_tabbar_fields(object)?;
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

        for (plugin_name, plugin) in &mut self.plugins {
            if !is_safe_plugin_component(plugin_name) {
                return Err(serde_json::Error::custom(format!(
                    r#""plugins" names must be safe path components: {:?}"#,
                    plugin_name
                )));
            }
            let version = plugin.version.trim();
            if !is_safe_plugin_component(version) {
                return Err(serde_json::Error::custom(format!(
                    r#""plugins.{}.version" must be a safe path component: {:?}"#,
                    plugin_name, plugin.version
                )));
            }
            plugin.version = version.to_string();
        }

        let pages: Vec<(&str, &str)> = self
            .pages
            .iter()
            .map(|page| (page.name.as_str(), page.path.as_str()))
            .collect();
        if let Some(tabbar) = &mut self.tabBar {
            tabbar.validate(&pages).map_err(serde_json::Error::custom)?;
        }

        Ok(())
    }
}

fn validate_removed_tabbar_fields(
    object: &serde_json::Map<String, Value>,
) -> Result<(), serde_json::Error> {
    let Some(tabbar) = object.get("tabBar").and_then(Value::as_object) else {
        return Ok(());
    };
    for (field, replacement) in [
        ("list", "tabBar.items"),
        ("color", "tabBar.style.foregroundColor"),
        ("selectedColor", "tabBar.style.selectedForegroundColor"),
        ("backgroundColor", "tabBar.style.backgroundColor"),
        ("borderStyle", "tabBar.style.dividerColor"),
        ("position", "remove it; the host owns tabbar placement"),
        ("dimension", "remove it; the host owns tabbar size"),
    ] {
        if tabbar.contains_key(field) {
            return Err(serde_json::Error::custom(format!(
                "tabBar.{field}: removed; use {replacement}"
            )));
        }
    }
    if let Some(items) = tabbar.get("items").and_then(Value::as_array) {
        for (index, item) in items.iter().enumerate() {
            if item.get("pagePath").is_some() {
                return Err(serde_json::Error::custom(format!(
                    "tabBar.items[{index}].pagePath: removed; use page (the configured page name from pages[].name)"
                )));
            }
        }
    }
    Ok(())
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

fn is_safe_plugin_component(value: &str) -> bool {
    !value.is_empty()
        && !value.contains('/')
        && !value.contains('\\')
        && !value.contains(':')
        && Path::new(value)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

#[cfg(test)]
mod tests {
    use super::LxAppConfig;

    #[test]
    fn initial_route_is_empty_when_pages_are_empty() {
        let config = LxAppConfig::default();
        assert_eq!(config.get_initial_route(), "");
    }

    #[test]
    fn accepts_omitted_security_config() {
        let config = LxAppConfig::from_value(serde_json::json!({
            "appId": "demo",
            "appName": "Demo",
            "version": "1.0.0",
            "pages": [{"name":"home","path":"pages/home/index"}]
        }))
        .unwrap();

        let value = serde_json::to_value(&config).unwrap();
        assert!(value.get("security").is_none());
    }

    #[test]
    fn a_leftover_security_key_is_ignored() {
        // Permissions live in the registry record. A package that still carries
        // the old key must keep loading — an unopenable installed app is worse
        // than a field that no longer does anything.
        let config = LxAppConfig::from_value(serde_json::json!({
            "appId": "demo",
            "appName": "Demo",
            "version": "1.0.0",
            "security": { "network": { "trustedDomains": ["api.example.com"] } },
            "pages": [{"name":"home","path":"pages/home/index"}]
        }))
        .unwrap();

        assert_eq!(config.get_initial_route(), "pages/home/index");
        let value = serde_json::to_value(&config).unwrap();
        assert!(value.get("security").is_none());
    }

    #[test]
    fn rejects_removed_tab_item_page_path() {
        let err = LxAppConfig::from_value(serde_json::json!({
            "appId": "demo",
            "appName": "Demo",
            "version": "1.0.0",
            "pages": [
                {"name":"home","path":"pages/home/index"},
                {"name":"profile","path":"pages/profile/index"}
            ],
            "tabBar": {
                "items": [
                    {"pagePath":"pages/home/index"},
                    {"pagePath":"pages/profile/index"}
                ]
            }
        }))
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("tabBar.items[0].pagePath: removed; use page"),
            "{err}"
        );
    }

    fn tabbar_config(pages: serde_json::Value, items: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "appId": "demo",
            "appName": "Demo",
            "version": "1.0.0",
            "pages": pages,
            "tabBar": { "items": items }
        })
    }

    #[test]
    fn tab_item_page_resolves_to_the_catalog_path() {
        let config = LxAppConfig::from_value(tabbar_config(
            serde_json::json!([
                {"name":"home","path":"pages/home/index.tsx"},
                {"name":"settings","path":"pages/settings/index"}
            ]),
            serde_json::json!([{ "page": "home" }, { "page": "settings" }]),
        ))
        .unwrap();
        let tabbar = config.tabBar.as_ref().unwrap();
        assert_eq!(tabbar.items[0].page, "home");
        assert_eq!(tabbar.items[0].page_path, "pages/home/index.tsx");
        assert_eq!(tabbar.items[1].page, "settings");
        assert_eq!(tabbar.items[1].page_path, "pages/settings/index");
    }

    #[test]
    fn tab_item_page_must_be_a_configured_name() {
        let err = LxAppConfig::from_value(tabbar_config(
            serde_json::json!([
                {"name":"home","path":"pages/home/index"},
                {"name":"profile","path":"pages/profile/index"}
            ]),
            serde_json::json!([{ "page": "pages/home/index" }, { "page": "profile" }]),
        ))
        .unwrap_err()
        .to_string();
        assert!(err.contains("not a registered page name"), "{err}");
        assert!(
            !err.contains("pages[].name, not path"),
            "path hint is CLI-only: {err}"
        );
    }

    #[test]
    fn rejects_plugin_name_or_version_that_escapes_storage() {
        for plugins in [
            serde_json::json!({ "../evil": { "version": "1.0.0" } }),
            serde_json::json!({ "evil": { "version": "C:\\Users\\victim" } }),
        ] {
            let err = LxAppConfig::from_value(serde_json::json!({
                "appId": "demo",
                "appName": "Demo",
                "version": "1.0.0",
                "pages": [{"name":"home","path":"pages/home/index"}],
                "plugins": plugins
            }))
            .unwrap_err();

            assert!(err.to_string().contains("safe path component"));
        }
    }
}
