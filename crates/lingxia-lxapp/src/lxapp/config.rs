use super::security::{
    LxAppSecurityPrivilege, normalize_security_privilege_id, normalize_trusted_domain,
};
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
pub(crate) struct LxAppNetworkSecurityConfig {
    /// Remote hosts allowed for network access.
    ///
    /// Empty means deny all. Use `"*"` to explicitly allow all domains.
    /// LingXia Server policy can tighten this in the future.
    #[serde(default)]
    pub trustedDomains: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct LxAppSecurityConfig {
    #[serde(default)]
    pub network: LxAppNetworkSecurityConfig,

    /// High-risk capability classes requested by this lxapp.
    ///
    /// This is intentionally coarse-grained; ordinary host capabilities such
    /// as camera/media/location remain host/platform mediated.
    #[serde(default)]
    pub privileges: Vec<String>,
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

    /// Security policy declared by the lxapp package.
    #[serde(default)]
    pub(crate) security: LxAppSecurityConfig,
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
            validate_security_shape(object)?;
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

    pub(crate) fn trusted_domains(&self) -> &[String] {
        &self.security.network.trustedDomains
    }

    pub(crate) fn has_security_privilege(&self, privilege: &LxAppSecurityPrivilege) -> bool {
        self.security
            .privileges
            .iter()
            .any(|candidate| candidate == privilege.as_str())
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

        validate_security_config(&mut self.security)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{LxAppConfig, LxAppSecurityPrivilege};

    #[test]
    fn initial_route_is_empty_when_pages_are_empty() {
        let config = LxAppConfig::default();
        assert_eq!(config.get_initial_route(), "");
    }

    #[test]
    fn parses_security_network_and_privileges() {
        let config = LxAppConfig::from_value(serde_json::json!({
            "appId": "demo",
            "appName": "Demo",
            "version": "1.0.0",
            "security": {
                "network": {
                    "trustedDomains": [" API.Example.COM ", "localhost"]
                },
                "privileges": ["automation", "devtools"]
            },
            "pages": [{"name":"home","path":"pages/home/index"}]
        }))
        .unwrap();

        assert_eq!(
            config.trusted_domains(),
            &["api.example.com".to_string(), "localhost".to_string()]
        );
        let automation = LxAppSecurityPrivilege::new("automation").unwrap();
        let devtools = LxAppSecurityPrivilege::new("devtools").unwrap();
        let camera = LxAppSecurityPrivilege::new("camera").unwrap();
        assert!(config.has_security_privilege(&automation));
        assert!(config.has_security_privilege(&devtools));
        assert!(!config.has_security_privilege(&camera));
    }

    #[test]
    fn rejects_missing_security_config() {
        let err = LxAppConfig::from_value(serde_json::json!({
            "appId": "demo",
            "appName": "Demo",
            "version": "1.0.0",
            "pages": [{"name":"home","path":"pages/home/index"}]
        }))
        .unwrap_err();

        assert!(err.to_string().contains("\"security\" must be declared"));
    }

    #[test]
    fn rejects_invalid_security_domain() {
        let err = LxAppConfig::from_value(serde_json::json!({
            "appId": "demo",
            "appName": "Demo",
            "version": "1.0.0",
            "security": {
                "network": {
                    "trustedDomains": ["https://api.example.com"]
                },
                "privileges": []
            },
            "pages": [{"name":"home","path":"pages/home/index"}]
        }))
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("\"security.network.trustedDomains\"")
        );
    }

    #[test]
    fn parses_trusted_domain_wildcard() {
        let config = LxAppConfig::from_value(serde_json::json!({
            "appId": "demo",
            "appName": "Demo",
            "version": "1.0.0",
            "security": {
                "network": {
                    "trustedDomains": ["*"]
                },
                "privileges": []
            },
            "pages": [{"name":"home","path":"pages/home/index"}]
        }))
        .unwrap();

        assert_eq!(config.trusted_domains(), &["*".to_string()]);
    }

    #[test]
    fn rejects_wildcard_mixed_with_trusted_domains() {
        let err = LxAppConfig::from_value(serde_json::json!({
            "appId": "demo",
            "appName": "Demo",
            "version": "1.0.0",
            "security": {
                "network": {
                    "trustedDomains": ["api.example.com", "*"]
                },
                "privileges": []
            },
            "pages": [{"name":"home","path":"pages/home/index"}]
        }))
        .unwrap_err();

        assert!(err.to_string().contains("wildcard"));
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

fn validate_security_shape(
    object: &serde_json::Map<String, Value>,
) -> Result<(), serde_json::Error> {
    let security = object
        .get("security")
        .ok_or_else(|| serde_json::Error::custom(r#""security" must be declared in lxapp.json"#))?;
    let security = security
        .as_object()
        .ok_or_else(|| serde_json::Error::custom(r#""security" must be an object"#))?;
    let network = security
        .get("network")
        .ok_or_else(|| serde_json::Error::custom(r#""security.network" must be declared"#))?;
    let network = network
        .as_object()
        .ok_or_else(|| serde_json::Error::custom(r#""security.network" must be an object"#))?;
    if !network.contains_key("trustedDomains") {
        return Err(serde_json::Error::custom(
            r#""security.network.trustedDomains" must be declared"#,
        ));
    }
    if !security.contains_key("privileges") {
        return Err(serde_json::Error::custom(
            r#""security.privileges" must be declared"#,
        ));
    }
    Ok(())
}

fn validate_security_config(config: &mut LxAppSecurityConfig) -> Result<(), serde_json::Error> {
    let mut domains = BTreeSet::new();
    let mut normalized_domains = Vec::new();
    for domain in &config.network.trustedDomains {
        let normalized = normalize_trusted_domain(domain).ok_or_else(|| {
            serde_json::Error::custom(format!(
                r#""security.network.trustedDomains" entries must be host names without scheme/path: {:?}"#,
                domain
            ))
        })?;
        if domains.insert(normalized.clone()) {
            normalized_domains.push(normalized);
        }
    }
    if normalized_domains.len() > 1 && normalized_domains.iter().any(|domain| domain == "*") {
        return Err(serde_json::Error::custom(
            r#""security.network.trustedDomains" wildcard "*" cannot be combined with other hosts"#,
        ));
    }
    config.network.trustedDomains = normalized_domains;

    let mut privileges = BTreeSet::new();
    let mut normalized_privileges = Vec::new();
    for privilege in &config.privileges {
        let normalized = normalize_security_privilege_id(privilege).ok_or_else(|| {
            serde_json::Error::custom(format!(
                r#""security.privileges" entries must be lowercase identifiers: {:?}"#,
                privilege
            ))
        })?;
        if privileges.insert(normalized.clone()) {
            normalized_privileges.push(normalized);
        }
    }
    config.privileges = normalized_privileges;

    Ok(())
}
