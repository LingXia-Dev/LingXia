use std::collections::HashSet;

use crate::error::LxAppError;
use crate::lxapp::version::Version;
use serde::{Deserialize, Serialize};

/// Configuration loaded from app.json
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    #[serde(rename = "productName")]
    pub product_name: String,
    #[serde(rename = "productVersion")]
    pub product_version: String,

    #[serde(rename = "lingxiaId", default)]
    pub lingxia_id: Option<String>,

    // API server address (optional)
    #[serde(rename = "apiServer", default)]
    pub api_server: Option<String>,

    // Home/default mini application settings (mandatory)
    #[serde(rename = "homeLxAppID")]
    pub home_lxapp_appid: String,

    #[serde(rename = "homeLxAppVersion")]
    pub home_lxapp_version: String,

    #[serde(rename = "cacheMaxAgeDays", default = "default_cache_max_age_days")]
    pub cache_max_age_days: u64,

    #[serde(rename = "cacheMaxSizeMB", default = "default_cache_max_size_mb")]
    pub cache_max_size_mb: u64,

    #[serde(
        rename = "splashTimeout",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub splash_timeout_ms: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub panels: Option<PanelsConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PanelItem {
    pub id: String,
    pub label: String,
    pub icon: String,
    #[serde(default = "default_panel_position")]
    pub position: PanelPosition,
    pub content: PanelContent,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PanelContent {
    #[serde(rename = "appId")]
    pub app_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

pub(crate) fn default_cache_max_age_days() -> u64 {
    7
}

pub(crate) fn default_cache_max_size_mb() -> u64 {
    1024
}

fn default_panel_position() -> PanelPosition {
    PanelPosition::Right
}

impl AppConfig {
    pub(crate) fn parse_and_validate(content: &str) -> Result<Self, LxAppError> {
        let config: Self = serde_json::from_str(content)
            .map_err(|e| LxAppError::InvalidJsonFile(format!("Failed to parse app.json: {}", e)))?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), LxAppError> {
        if self.product_name.is_empty() {
            return Err(LxAppError::InvalidParameter(
                "productName is mandatory and cannot be empty".to_string(),
            ));
        }

        if self.product_version.is_empty() {
            return Err(LxAppError::InvalidParameter(
                "productVersion is mandatory and cannot be empty".to_string(),
            ));
        }

        Version::parse(&self.product_version).map_err(|_| {
            LxAppError::InvalidParameter(
                "productVersion must be a semantic version (major.minor.patch)".to_string(),
            )
        })?;

        if self.home_lxapp_appid.is_empty() {
            return Err(LxAppError::InvalidParameter(
                "homeLxAppID is mandatory and cannot be empty".to_string(),
            ));
        }

        if self.home_lxapp_version.is_empty() {
            return Err(LxAppError::InvalidParameter(
                "homeLxAppVersion is mandatory and cannot be empty".to_string(),
            ));
        }

        Version::parse(&self.home_lxapp_version).map_err(|_| {
            LxAppError::InvalidParameter(
                "homeLxAppVersion must be a semantic version (major.minor.patch)".to_string(),
            )
        })?;

        validate_panels(self.panels.as_ref())
    }
}

fn validate_panels(panels: Option<&PanelsConfig>) -> Result<(), LxAppError> {
    let Some(panels) = panels else {
        return Ok(());
    };

    let mut ids = HashSet::new();
    let mut positions = HashSet::new();
    let mut app_ids = HashSet::new();

    for item in &panels.items {
        if item.id.is_empty() {
            return Err(LxAppError::InvalidParameter(
                "panels.items[].id cannot be empty".to_string(),
            ));
        }
        if item.label.is_empty() {
            return Err(LxAppError::InvalidParameter(format!(
                "panel '{}' label cannot be empty",
                item.id
            )));
        }
        if item.content.app_id.is_empty() {
            return Err(LxAppError::InvalidParameter(format!(
                "panel '{}' content.appId cannot be empty",
                item.id
            )));
        }
        if !ids.insert(item.id.clone()) {
            return Err(LxAppError::InvalidParameter(format!(
                "duplicate panel id '{}'",
                item.id
            )));
        }
        if !positions.insert(item.position) {
            return Err(LxAppError::InvalidParameter(format!(
                "only one panel is supported at position '{}'",
                panel_position_name(item.position)
            )));
        }
        if !app_ids.insert(item.content.app_id.clone()) {
            return Err(LxAppError::InvalidParameter(format!(
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
