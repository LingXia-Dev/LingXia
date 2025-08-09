use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::Sender;

use crate::error::LxAppError;
use rong::IntoJSObj;
use serde::{Deserialize, Serialize};

/// Device information
#[derive(Debug, Clone, IntoJSObj, Serialize)]
pub struct DeviceInfo {
    pub brand: String,
    pub model: String,
    pub system: String, // Operating system version
}

/// Asset file entry for iterator-based asset access
pub struct AssetFileEntry<'a> {
    pub path: String,
    pub reader: Box<dyn Read + 'a>,
}

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

    // Maximum number of lx applications allowed to run concurrently
    #[serde(
        rename = "maxAllowedLxApps",
        default = "AppConfig::default_max_allowed_lxapps"
    )]
    pub max_allowed_lxapps: usize,
}

impl AppConfig {
    /// Default value for max_allowed_lxapps
    fn default_max_allowed_lxapps() -> usize {
        3
    }

    /// Read, parse and validate app.json from the assets directory.
    pub(crate) fn load<T: AppRuntime + ?Sized>(controller: &T) -> Result<Self, LxAppError> {
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

        // Validate maxAllowedLxApps range
        if config.max_allowed_lxapps < 1 || config.max_allowed_lxapps > 5 {
            return Err(LxAppError::InvalidParameter(
                "maxAllowedLxApps must be between 1 and 5".to_string(),
            ));
        }

        Ok(())
    }
}

/// Base platform runtime capabilities
///
/// This trait defines the core capabilities required for the mini app platform,
/// including resource access, directory management
pub trait AppRuntime: Send + Sync + 'static {
    /// Read asset file from platform-specific location as a streaming reader
    ///
    /// # Arguments
    /// * `path` - Path to the asset file to read
    ///
    /// # Returns
    /// * `Result<Box<dyn Read + '_>, LxAppError>` - A reader for streaming the asset content, or an error
    fn read_asset<'a>(&'a self, path: &str) -> Result<Box<dyn Read + 'a>, LxAppError>;

    /// Iterate over files in an asset directory.
    ///
    /// Returns an iterator of AssetFileEntry, each containing the file path and a reader implementing `Read`.
    ///
    /// # Arguments
    /// * `asset_dir` - Directory path in assets to iterate
    ///
    /// # Returns
    /// * `Box<dyn Iterator<Item = Result<AssetFileEntry, LxAppError>>>` - Iterator over files in the directory
    ///   (If directory cannot be opened, the iterator's first element will be an error)
    fn asset_dir_iter<'a>(
        &'a self,
        asset_dir: &str,
    ) -> Box<dyn Iterator<Item = Result<AssetFileEntry<'a>, LxAppError>> + 'a>;

    /// Get data directory path
    ///
    /// # Returns
    /// * `PathBuf` - Path to the application's data directory
    fn app_data_dir(&self) -> PathBuf;

    /// Get cache directory path
    ///
    /// # Returns
    /// * `PathBuf` - Path to the application's cache directory
    fn app_cache_dir(&self) -> PathBuf;

    /// Get device information
    ///
    /// # Returns
    /// * `DeviceInfo` - Device information including brand, model, and screen dimensions
    fn device_info(&self) -> DeviceInfo;

    /// Create a WebView instance asynchronously
    ///
    /// # Arguments
    /// * `appid` - Application identifier
    /// * `path` - Page path within the application
    /// * `sender` - Channel sender to notify when WebView creation completes
    fn create_webview(
        &self,
        appid: String,
        path: String,
        sender: Sender<Result<Arc<dyn crate::page::WebViewController>, LxAppError>>,
    );

    /// Open a mini app
    ///
    /// # Arguments
    /// * `appid` - The ID of the mini app to open
    /// * `path` - The initial path to navigate to within the mini app
    ///
    /// # Returns
    /// * `Result<(), LxAppError>` - Success or error
    fn open_lxapp(&self, appid: String, path: String) -> Result<(), LxAppError>;

    /// Close a mini app
    ///
    /// # Arguments
    /// * `appid` - The ID of the mini app to close
    ///
    /// # Returns
    /// * `Result<(), LxAppError>` - Success or error
    fn close_lxapp(&self, appid: String) -> Result<(), LxAppError>;

    /// Switch to a different page within the same mini app
    ///
    /// # Arguments
    /// * `appid` - The ID of the mini app to switch pages in
    /// * `path` - The path of the page to switch to
    ///
    /// # Returns
    /// * `Result<(), LxAppError>` - Success or error
    fn switch_page(&self, appid: String, path: String) -> Result<(), LxAppError>;
}

impl<T: AppRuntime + ?Sized> AppRuntime for Arc<T> {
    fn read_asset<'a>(&'a self, path: &str) -> Result<Box<dyn Read + 'a>, LxAppError> {
        (**self).read_asset(path)
    }

    fn asset_dir_iter<'a>(
        &'a self,
        asset_dir: &str,
    ) -> Box<dyn Iterator<Item = Result<AssetFileEntry<'a>, LxAppError>> + 'a> {
        (**self).asset_dir_iter(asset_dir)
    }

    fn app_data_dir(&self) -> PathBuf {
        (**self).app_data_dir()
    }

    fn app_cache_dir(&self) -> PathBuf {
        (**self).app_cache_dir()
    }

    fn device_info(&self) -> DeviceInfo {
        (**self).device_info()
    }

    fn create_webview(
        &self,
        appid: String,
        path: String,
        sender: Sender<Result<Arc<dyn crate::page::WebViewController>, LxAppError>>,
    ) {
        (**self).create_webview(appid, path, sender)
    }

    fn open_lxapp(&self, appid: String, path: String) -> Result<(), LxAppError> {
        (**self).open_lxapp(appid, path)
    }

    fn close_lxapp(&self, appid: String) -> Result<(), LxAppError> {
        (**self).close_lxapp(appid)
    }

    fn switch_page(&self, appid: String, path: String) -> Result<(), LxAppError> {
        (**self).switch_page(appid, path)
    }
}
