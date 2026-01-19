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

    // API server address (optional)
    #[serde(rename = "apiServer", default)]
    pub api_server: Option<String>,

    // Application-level authentication fields
    #[serde(rename = "apiKey", default)]
    pub api_key: Option<String>, // Client identifier, sent in request headers

    #[serde(rename = "apiSecret", default)]
    pub api_secret: Option<String>, // Shared secret for request signing, never transmitted

    // Home/default mini application settings (mandatory)
    #[serde(rename = "homeLxAppID")]
    pub home_lxapp_appid: String, // ID of the default/home lx application to load

    #[serde(rename = "homeLxAppVersion")]
    pub home_lxapp_version: String, // Version of the home lx application
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
