use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, mpsc};

use crate::error::MiniAppError;
use crate::log::LogLevel;
use serde::{Deserialize, Serialize};

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
    #[serde(rename = "homeMiniAppID")]
    pub home_mini_app_id: String, // ID of the default/home mini application to load

    #[serde(rename = "homeMiniAppVersion")]
    pub home_mini_app_version: String, // Version of the home mini application
}

impl AppConfig {
    /// Read, parse and validate app.json from the assets directory.
    pub(crate) fn load<T: AppController + ?Sized>(controller: &T) -> Result<Self, MiniAppError> {
        // Read app.json as a string
        let mut reader = controller.read_asset("app.json")?;
        let mut content = String::new();
        reader
            .read_to_string(&mut content)
            .map_err(|e| MiniAppError::IoError(format!("Failed to read app.json: {}", e)))?;

        // Parse the JSON into AppConfig
        let config = serde_json::from_str(&content).map_err(|e| {
            MiniAppError::InvalidJsonFile(format!("Failed to parse app.json: {}", e))
        })?;

        // Validate the config immediately
        Self::validate_config(&config)?;

        Ok(config)
    }

    /// Validate the AppConfig to ensure all mandatory fields are present and valid
    fn validate_config(config: &Self) -> Result<(), MiniAppError> {
        // Check all mandatory fields are not empty
        if config.product_name.is_empty() {
            return Err(MiniAppError::InvalidParameter(
                "productName is mandatory and cannot be empty".to_string(),
            ));
        }

        if config.version.is_empty() {
            return Err(MiniAppError::InvalidParameter(
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
            return Err(MiniAppError::InvalidParameter(
                "version must be in format x.y.z with numeric values".to_string(),
            ));
        }

        if config.identifier.is_empty() {
            return Err(MiniAppError::InvalidParameter(
                "identifier is mandatory and cannot be empty".to_string(),
            ));
        }

        // Check homeMiniAppID
        if config.home_mini_app_id.is_empty() {
            return Err(MiniAppError::InvalidParameter(
                "homeMiniAppID is mandatory and cannot be empty".to_string(),
            ));
        }

        // Check homeMiniAppVersion
        if config.home_mini_app_version.is_empty() {
            return Err(MiniAppError::InvalidParameter(
                "homeMiniAppVersion is mandatory and cannot be empty".to_string(),
            ));
        }

        Ok(())
    }
}

/// Interface for controlling app lifecycle and navigation
pub trait AppController: Send + Sync + 'static {
    /// Read asset file from platform-specific location as a streaming reader
    ///
    /// # Arguments
    /// * `path` - Path to the asset file to read
    ///
    /// # Returns
    /// * `Result<Box<dyn Read + '_>, MiniAppError>` - A reader for streaming the asset content, or an error
    fn read_asset<'a>(&'a self, path: &str) -> Result<Box<dyn Read + 'a>, MiniAppError>;

    /// Iterate over files in an asset directory.
    ///
    /// Returns an iterator of AssetFileEntry, each containing the file path and a reader implementing `Read`.
    ///
    /// # Arguments
    /// * `asset_dir` - Directory path in assets to iterate
    ///
    /// # Returns
    /// * `Box<dyn Iterator<Item = Result<AssetFileEntry, MiniAppError>>>` - Iterator over files in the directory
    ///   (If directory cannot be opened, the iterator's first element will be an error)
    fn asset_dir_iter<'a>(
        &'a self,
        asset_dir: &str,
    ) -> Box<dyn Iterator<Item = Result<AssetFileEntry<'a>, MiniAppError>> + 'a>;

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

    /// Log message to platform-specific logging system
    ///
    /// # Arguments
    /// * `appid` - Identifier of the mini application
    /// * `level` - Log severity level
    /// * `message` - Log message content
    fn log(&self, appid: &str, level: LogLevel, message: &str);

    /// Send a command to the controller and wait for the response
    ///
    /// This method creates a channel for the response, sends the command, and waits for the result
    ///
    /// # Arguments
    /// * `cmd` - Command to send to the controller
    ///
    /// # Returns
    /// * `Result<(), MiniAppError>` - Success or error response
    fn send_cmd(&self, cmd: ControllerCmd) -> Result<(), MiniAppError>;
}

impl<T: AppController + ?Sized> AppController for Arc<T> {
    fn read_asset<'a>(&'a self, path: &str) -> Result<Box<dyn Read + 'a>, MiniAppError> {
        (**self).read_asset(path)
    }

    fn asset_dir_iter<'a>(
        &'a self,
        asset_dir: &str,
    ) -> Box<dyn Iterator<Item = Result<AssetFileEntry<'a>, MiniAppError>> + 'a> {
        (**self).asset_dir_iter(asset_dir)
    }

    fn app_data_dir(&self) -> PathBuf {
        (**self).app_data_dir()
    }

    fn app_cache_dir(&self) -> PathBuf {
        (**self).app_cache_dir()
    }

    fn log(&self, appid: &str, level: LogLevel, message: &str) {
        (**self).log(appid, level, message)
    }

    fn send_cmd(&self, cmd: crate::ControllerCmd) -> Result<(), MiniAppError> {
        (**self).send_cmd(cmd)
    }
}

/// Send a command to switch to a different page within the same mini app
///
/// # Arguments
/// * `controller` - The controller to send the command to
/// * `appid` - The ID of the mini app to switch pages in
/// * `path` - The path of the page to switch to
///
/// # Returns
/// * `Ok(())` - If the command was sent successfully
/// * `Err(MiniAppError)` - If the command failed to send or execute
pub(crate) fn switch_page<T: AppController>(
    controller: &T,
    appid: &str,
    path: &str,
) -> Result<(), MiniAppError> {
    let (responder, receiver) = mpsc::channel();

    let cmd = MiniAppCmd::SwitchPage {
        appid: appid.to_string(),
        path: path.to_string(),
        responder,
    };

    controller.send_cmd(ControllerCmd::MiniApp(cmd))?;

    // Wait for the response
    receiver.recv().map_err(|_| {
        MiniAppError::WebView("UI thread dropped without sending result".to_string())
    })?
}

/// Send a command to open a mini app
///
/// # Arguments
/// * `controller` - The controller to send the command to
/// * `appid` - The ID of the mini app to open
/// * `path` - The initial path to navigate to within the mini app
///
/// # Returns
/// * `Ok(())` - If the command was sent successfully
/// * `Err(MiniAppError)` - If the command failed to send or execute
pub(crate) fn open_miniapp<T: AppController>(
    controller: &T,
    appid: &str,
    path: &str,
) -> Result<(), MiniAppError> {
    let (responder, receiver) = mpsc::channel();

    let cmd = MiniAppCmd::OpenMiniApp {
        appid: appid.to_string(),
        path: path.to_string(),
        responder,
    };

    controller.send_cmd(ControllerCmd::MiniApp(cmd))?;

    // Wait for the response
    receiver.recv().map_err(|_| {
        MiniAppError::WebView("UI thread dropped without sending result".to_string())
    })?
}

#[derive(Debug)]
pub enum ControllerCmd {
    WebView(WebViewCmd),
    MiniApp(MiniAppCmd),
    Shutdown,
}

#[derive(Debug)]
pub enum WebViewCmd {
    LoadUrl {
        appid: String,
        path: String,
        url: String,
        responder: mpsc::Sender<Result<(), MiniAppError>>,
    },
    EvaluateJavascript {
        appid: String,
        path: String,
        script: String,
        responder: mpsc::Sender<Result<(), MiniAppError>>,
    },
    PostMessage {
        appid: String,
        path: String,
        message: String,
        responder: mpsc::Sender<Result<(), MiniAppError>>,
    },
    SetDevtools {
        appid: String,
        enabled: bool,
        responder: mpsc::Sender<Result<(), MiniAppError>>,
    },
    ClearBrowsingData {
        appid: String,
        path: String,
        responder: mpsc::Sender<Result<(), MiniAppError>>,
    },
    SetUserAgent {
        appid: String,
        ua: String,
        responder: mpsc::Sender<Result<(), MiniAppError>>,
    },
}

#[derive(Debug)]
pub enum MiniAppCmd {
    SwitchPage {
        appid: String,
        path: String,
        responder: mpsc::Sender<Result<(), MiniAppError>>,
    },
    OpenMiniApp {
        appid: String,
        path: String,
        responder: mpsc::Sender<Result<(), MiniAppError>>,
    },
}
