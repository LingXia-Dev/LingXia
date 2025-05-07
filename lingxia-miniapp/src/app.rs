use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, mpsc};

use crate::error::MiniAppError;
use crate::log::LogLevel;

/// Asset file entry for iterator-based asset access
pub struct AssetFileEntry<'a> {
    pub path: String,
    pub reader: Box<dyn Read + 'a>,
}

/// Interface for controlling app lifecycle and navigation
pub trait AppController: Send + Sync + 'static {
    /// Read asset file from platform-specific location
    ///
    /// # Arguments
    /// * `path` - Path to the asset file to read
    ///
    /// # Returns
    /// * `Result<Vec<u8>, MiniAppError>` - The file content as bytes, or an error
    fn read_asset(&self, path: &str) -> Result<Vec<u8>, MiniAppError>;

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
    fn read_asset(&self, path: &str) -> Result<Vec<u8>, MiniAppError> {
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
