use std::path::PathBuf;
use std::sync::mpsc;

use crate::error::MiniAppError;
use crate::log::LogLevel;

/// Interface for controlling app lifecycle and navigation
pub trait AppController: Send + Sync + 'static {
    /// Read asset file from platform-specific location
    fn read_asset(&self, path: &str) -> Result<Vec<u8>, MiniAppError>;

    /// Get data directory path
    fn app_data_dir(&self) -> PathBuf;

    /// Get cache directory path
    fn app_cache_dir(&self) -> PathBuf;

    /// Log message to platform-specific logging system
    fn log(&self, level: LogLevel, app_id: &str, message: &str);

    /// Switch to another page within the same mini app
    fn switch_page(&self, app_id: &str, path: &str) -> Result<(), MiniAppError>;

    /// Open a mini app in platform-specific way
    fn open_miniapp(&self, app_id: &str, path: &str) -> Result<(), MiniAppError>;

    /// Send a command to the controller and wait for the response
    /// This method creates a channel for the response, sends the command, and waits for the result
    fn send_cmd(&self, cmd: ControllerCmd) -> Result<(), MiniAppError>;
}

#[derive(Debug)]
pub enum ControllerCmd {
    WebView(WebViewCmd),
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
