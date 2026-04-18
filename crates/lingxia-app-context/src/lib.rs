use semver::Version;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use thiserror::Error;

static APP_CONFIG: OnceLock<AppConfig> = OnceLock::new();
const APP_STATE_DIR: &str = "app_state";

#[derive(Debug, Error)]
pub enum AppContextError {
    #[error("invalid app.json: {0}")]
    InvalidJson(String),
    #[error("invalid app config: {0}")]
    InvalidConfig(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct AppConfig {
    #[serde(rename = "productName")]
    pub product_name: String,
    #[serde(rename = "productVersion")]
    pub product_version: String,

    #[serde(rename = "lingxiaId", default)]
    pub lingxia_id: Option<String>,

    #[serde(rename = "apiServer", default)]
    pub api_server: Option<String>,

    #[serde(rename = "homeLxAppID")]
    pub home_lxapp_appid: String,

    #[serde(rename = "homeLxAppVersion")]
    pub home_lxapp_version: String,

    #[serde(rename = "cacheMaxAgeDays", default = "default_cache_max_age_days")]
    pub cache_max_age_days: u64,

    #[serde(rename = "cacheMaxSizeMB", default = "default_cache_max_size_mb")]
    pub cache_max_size_mb: u64,

    #[serde(rename = "devWsUrl", default, skip_serializing_if = "Option::is_none")]
    pub dev_ws_url: Option<String>,

    #[serde(rename = "appLinks", default, skip_serializing_if = "Option::is_none")]
    pub app_links: Option<AppLinksConfig>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub panels: Option<PanelsConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct AppLinksConfig {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hosts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PanelsConfig {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<PanelItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PanelPosition {
    Left,
    Right,
    Bottom,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PanelItem {
    pub id: String,
    pub label: String,
    pub icon: String,
    #[serde(default = "default_panel_position")]
    pub position: PanelPosition,
    pub content: PanelContent,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PanelContent {
    #[serde(rename = "appId")]
    pub app_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

fn default_cache_max_age_days() -> u64 {
    7
}

fn default_cache_max_size_mb() -> u64 {
    1024
}

fn default_panel_position() -> PanelPosition {
    PanelPosition::Right
}

impl AppConfig {
    pub fn parse_and_validate(content: &str) -> Result<Self, AppContextError> {
        let config: Self = serde_json::from_str(content).map_err(|e| {
            AppContextError::InvalidJson(format!("Failed to parse app.json: {}", e))
        })?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), AppContextError> {
        if self.product_name.is_empty() {
            return Err(AppContextError::InvalidConfig(
                "productName is mandatory and cannot be empty".to_string(),
            ));
        }
        if self.product_version.is_empty() {
            return Err(AppContextError::InvalidConfig(
                "productVersion is mandatory and cannot be empty".to_string(),
            ));
        }
        Version::parse(&self.product_version).map_err(|_| {
            AppContextError::InvalidConfig(
                "productVersion must be a semantic version (major.minor.patch)".to_string(),
            )
        })?;
        if self.home_lxapp_appid.is_empty() {
            return Err(AppContextError::InvalidConfig(
                "homeLxAppID is mandatory and cannot be empty".to_string(),
            ));
        }
        if self.home_lxapp_version.is_empty() {
            return Err(AppContextError::InvalidConfig(
                "homeLxAppVersion is mandatory and cannot be empty".to_string(),
            ));
        }
        Version::parse(&self.home_lxapp_version).map_err(|_| {
            AppContextError::InvalidConfig(
                "homeLxAppVersion must be a semantic version (major.minor.patch)".to_string(),
            )
        })?;
        validate_panels(self.panels.as_ref())
    }
}

pub fn set_app_config(config: AppConfig) -> Result<(), AppContextError> {
    if let Some(existing) = APP_CONFIG.get() {
        if existing == &config {
            return Ok(());
        }
        return Err(AppContextError::InvalidConfig(
            "app config is already initialized with different values".to_string(),
        ));
    }

    APP_CONFIG
        .set(config)
        .map_err(|_| {
            AppContextError::InvalidConfig(
                "app config was initialized concurrently with different values".to_string(),
            )
        })
        .map(|_| ())
}

pub fn app_config() -> Option<&'static AppConfig> {
    APP_CONFIG.get()
}

pub fn product_name() -> Option<&'static str> {
    APP_CONFIG.get().map(|c| c.product_name.as_str())
}

pub fn product_version() -> Option<&'static str> {
    APP_CONFIG.get().map(|c| c.product_version.as_str())
}

pub fn lingxia_id() -> Option<&'static str> {
    APP_CONFIG
        .get()
        .and_then(|c| c.lingxia_id.as_deref())
        .filter(|s| !s.is_empty())
}

pub fn cache_max_age_days() -> u64 {
    APP_CONFIG
        .get()
        .map(|c| c.cache_max_age_days)
        .unwrap_or_else(default_cache_max_age_days)
}

pub fn cache_max_size_bytes() -> u64 {
    const MIB: u64 = 1024 * 1024;
    APP_CONFIG
        .get()
        .map(|c| c.cache_max_size_mb.saturating_mul(MIB))
        .unwrap_or_else(|| default_cache_max_size_mb().saturating_mul(MIB))
}

pub fn app_state_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join(APP_STATE_DIR)
}

pub fn app_state_file(app_data_dir: &Path, name: &str) -> PathBuf {
    app_state_dir(app_data_dir).join(name)
}

fn validate_panels(panels: Option<&PanelsConfig>) -> Result<(), AppContextError> {
    let Some(panels) = panels else {
        return Ok(());
    };

    let mut ids = HashSet::new();
    let mut positions = HashSet::new();
    let mut app_ids = HashSet::new();

    for item in &panels.items {
        if item.id.is_empty() {
            return Err(AppContextError::InvalidConfig(
                "panels.items[].id cannot be empty".to_string(),
            ));
        }
        if item.label.is_empty() {
            return Err(AppContextError::InvalidConfig(format!(
                "panel '{}' label cannot be empty",
                item.id
            )));
        }
        if item.content.app_id.is_empty() {
            return Err(AppContextError::InvalidConfig(format!(
                "panel '{}' content.appId cannot be empty",
                item.id
            )));
        }
        if !ids.insert(item.id.clone()) {
            return Err(AppContextError::InvalidConfig(format!(
                "duplicate panel id '{}'",
                item.id
            )));
        }
        if !positions.insert(item.position) {
            return Err(AppContextError::InvalidConfig(format!(
                "only one panel is supported at position '{}'",
                panel_position_name(item.position)
            )));
        }
        if !app_ids.insert(item.content.app_id.clone()) {
            return Err(AppContextError::InvalidConfig(format!(
                "panel appId '{}' must be unique",
                item.content.app_id
            )));
        }
    }

    Ok(())
}

fn panel_position_name(position: PanelPosition) -> &'static str {
    match position {
        PanelPosition::Left => "left",
        PanelPosition::Right => "right",
        PanelPosition::Bottom => "bottom",
    }
}

#[cfg(test)]
mod tests {
    use super::{AppConfig, AppContextError, set_app_config};

    fn test_config(product_name: &str) -> AppConfig {
        AppConfig {
            product_name: product_name.to_string(),
            product_version: "1.0.0".to_string(),
            lingxia_id: Some("lingxia".to_string()),
            api_server: None,
            home_lxapp_appid: "home".to_string(),
            home_lxapp_version: "1.0.0".to_string(),
            cache_max_age_days: 7,
            cache_max_size_mb: 1024,
            dev_ws_url: None,
            app_links: None,
            panels: None,
        }
    }

    #[test]
    fn set_app_config_rejects_mismatched_value_after_initialization() {
        let cfg = test_config("LingXia");
        assert!(set_app_config(cfg.clone()).is_ok());
        assert!(set_app_config(cfg).is_ok());
        let err = set_app_config(test_config("Other")).unwrap_err();
        assert!(matches!(err, AppContextError::InvalidConfig(_)));
    }
}
