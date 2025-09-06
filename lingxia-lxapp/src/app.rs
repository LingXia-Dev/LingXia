use std::io::Read;
use std::sync::Arc;

use crate::error::LxAppError;
use lingxia_platform::{AppRuntime, Platform};
use serde::{Deserialize, Serialize};

/// Configuration loaded from app.json
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    #[serde(rename = "productName")]
    pub product_name: String,
    pub version: String,
    pub identifier: String, // Unique identifier for this application, used by the server to identify different clients

    // API server address (optional)
    #[serde(rename = "apiServer", default)]
    pub api_server: Option<String>,

    // Application-level authentication fields
    #[serde(rename = "apiKey", default)]
    pub api_key: Option<String>, // Key for simple API authentication

    // Home/default mini application settings (mandatory)
    #[serde(rename = "homeLxAppID")]
    pub home_lxapp_appid: String, // ID of the default/home lx application to load

    #[serde(rename = "homeLxAppVersion")]
    pub home_lxapp_version: String, // Version of the home lx application
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

        if config.version.is_empty() {
            return Err(LxAppError::InvalidParameter(
                "version is mandatory and cannot be empty".to_string(),
            ));
        }

        // Basic semver format check (major.minor.patch)
        if !config.version.chars().any(|c| c == '.')
            || !config
                .version
                .chars()
                .all(|c| c.is_ascii_digit() || c == '.')
        {
            return Err(LxAppError::InvalidParameter(
                "version must be in format x.y.z with numeric values".to_string(),
            ));
        }

        if config.identifier.is_empty() {
            return Err(LxAppError::InvalidParameter(
                "identifier is mandatory and cannot be empty".to_string(),
            ));
        }

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

        Ok(())
    }
}
