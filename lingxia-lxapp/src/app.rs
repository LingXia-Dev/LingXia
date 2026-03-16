use std::io::Read;
use std::sync::Arc;
use std::sync::OnceLock;

use crate::error::LxAppError;
use crate::lxapp::version::Version;
use lingxia_platform::Platform;
use lingxia_platform::traits::app_runtime::AppRuntime;
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
    pub home_lxapp_appid: String, // ID of the default/home lx application to load

    #[serde(rename = "homeLxAppVersion")]
    pub home_lxapp_version: String, // Version of the home lx application

    #[serde(rename = "cacheMaxAgeDays", default = "default_cache_max_age_days")]
    pub cache_max_age_days: u64,

    #[serde(rename = "cacheMaxSizeMB", default = "default_cache_max_size_mb")]
    pub cache_max_size_mb: u64,
}

fn default_cache_max_age_days() -> u64 {
    7
}

fn default_cache_max_size_mb() -> u64 {
    1024
}

static APP_CONFIG: OnceLock<AppConfig> = OnceLock::new();

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

pub fn cache_max_age() -> std::time::Duration {
    std::time::Duration::from_secs(cache_max_age_days() * 86400)
}

pub fn cache_max_size_bytes() -> u64 {
    const MIB: u64 = 1024 * 1024;
    APP_CONFIG
        .get()
        .map(|c| c.cache_max_size_mb.saturating_mul(MIB))
        .unwrap_or_else(|| default_cache_max_size_mb().saturating_mul(MIB))
}

impl AppConfig {
    /// Read, parse and validate app.json from the assets directory.
    pub(crate) fn load(controller: Arc<Platform>) -> Result<Self, LxAppError> {
        // Read app.json as a string
        let mut reader = controller.read_asset("app.json")?;
        let mut content = String::new();
        reader
            .read_to_string(&mut content)
            .map_err(|e| LxAppError::IoError(format!("Failed to read app.json: {}", e)))?;

        // Parse the JSON into AppConfig
        let config = serde_json::from_str(&content)
            .map_err(|e| LxAppError::InvalidJsonFile(format!("Failed to parse app.json: {}", e)))?;

        // Validate the config immediately
        Self::validate_config(&config)?;

        let _ = APP_CONFIG.set(config.clone());
        Ok(config)
    }

    /// Validate the AppConfig to ensure all mandatory fields are present and valid
    fn validate_config(config: &Self) -> Result<(), LxAppError> {
        // Check all mandatory fields are not empty
        if config.product_name.is_empty() {
            return Err(LxAppError::InvalidParameter(
                "productName is mandatory and cannot be empty".to_string(),
            ));
        }

        if config.product_version.is_empty() {
            return Err(LxAppError::InvalidParameter(
                "productVersion is mandatory and cannot be empty".to_string(),
            ));
        }

        Version::parse(&config.product_version).map_err(|_| {
            LxAppError::InvalidParameter(
                "productVersion must be a semantic version (major.minor.patch)".to_string(),
            )
        })?;

        // Check homeLxAppID
        if config.home_lxapp_appid.is_empty() {
            return Err(LxAppError::InvalidParameter(
                "homeLxAppID is mandatory and cannot be empty".to_string(),
            ));
        }

        // Check homeLxAppVersion
        if config.home_lxapp_version.is_empty() {
            return Err(LxAppError::InvalidParameter(
                "homeLxAppVersion is mandatory and cannot be empty".to_string(),
            ));
        }

        Version::parse(&config.home_lxapp_version).map_err(|_| {
            LxAppError::InvalidParameter(
                "homeLxAppVersion must be a semantic version (major.minor.patch)".to_string(),
            )
        })?;

        Ok(())
    }
}
