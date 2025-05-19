use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::{Mutex, OnceLock, mpsc};
use std::thread::{self, ThreadId};

use miniapp::{
    AppController, AppRuntime, AssetFileEntry, ControllerCmd, MiniAppError, log::LogLevel,
};

use crate::{App, WebView};

mod app;
mod webview;

static CONTROLLER: OnceLock<Arc<Controller>> = OnceLock::new();

pub(crate) struct Controller {
    app: App,
    webviews: Mutex<HashMap<(String, String), Arc<WebView>>>,
    sender: mpsc::Sender<ControllerCmd>,
    ui_thread_id: ThreadId,
}

impl Drop for Controller {
    fn drop(&mut self) {
        // Try to send shutdown command, ignore errors since we're dropping anyway
        let _ = self.sender.send(ControllerCmd::Shutdown);
    }
}

impl AppRuntime for Controller {
    fn read_asset<'a>(&'a self, path: &str) -> Result<Box<dyn Read + 'a>, MiniAppError> {
        self.app.read_asset(path)
    }

    fn asset_dir_iter<'a>(
        &'a self,
        asset_dir: &str,
    ) -> Box<dyn Iterator<Item = Result<AssetFileEntry<'a>, MiniAppError>> + 'a> {
        // Convert from our AssetFileEntry to miniapp's AssetFileEntry
        let iter = self.app.asset_dir_iter(asset_dir);
        Box::new(iter.map(|result| {
            result.map(|entry| AssetFileEntry {
                path: entry.path,
                reader: entry.reader,
            })
        }))
    }

    fn app_data_dir(&self) -> PathBuf {
        self.app.app_data_dir()
    }

    fn app_cache_dir(&self) -> PathBuf {
        self.app.app_cache_dir()
    }

    fn log(&self, level: LogLevel, message: &str) {
        self.app.log(level, message)
    }
}

impl AppController for Controller {
    fn send_cmd(&self, cmd: ControllerCmd) -> Result<(), MiniAppError> {
        // Check if we're on the UI thread
        let current_thread_id = thread::current().id();
        let is_ui_thread = self.ui_thread_id == current_thread_id;

        // If we're on the UI thread, process directly
        if is_ui_thread {
            // log::info!("On UI thread, directly handling command");

            Self::handle_request(self, cmd);
            return Ok(());
        } else {
            self.sender
                .send(cmd)
                .map_err(|e| MiniAppError::WebView(format!("Failed to send command: {}", e)))?;
            Ok(())
        }
    }
}

impl Controller {
    fn new(sender: mpsc::Sender<ControllerCmd>, app: App, id: ThreadId) -> Self {
        Self {
            webviews: Mutex::new(HashMap::new()),
            sender,
            app,
            ui_thread_id: id,
        }
    }

    fn spawn_command_thread<F>(
        f: F,
        controller: Arc<Controller>,
        receiver: mpsc::Receiver<ControllerCmd>,
    ) where
        F: FnOnce(Arc<Controller>) -> bool + Send + 'static,
    {
        thread::spawn(move || {
            if !f(controller) {
                return;
            }

            let controller = CONTROLLER.get().unwrap();

            // Process commands loop
            while let Ok(cmd) = receiver.recv() {
                // Process command and check if we should continue
                if !Controller::handle_request(controller, cmd) {
                    break;
                }
            }
        });
    }

    /// Process a single request
    /// Returns true if we should continue processing commands, false if we should stop
    fn handle_request(controller: &Controller, request: ControllerCmd) -> bool {
        match request {
            ControllerCmd::Shutdown => {
                controller.log(
                    LogLevel::Info,
                    "Shutdown command received, stopping command loop",
                );
                return false; // Stop processing commands
            }
            ControllerCmd::WebView(cmd) => {
                if let Err(err) = webview::handle_webview_cmd(&controller.webviews, cmd) {
                    // Log error but continue processing
                    controller.log(
                        LogLevel::Error,
                        &format!("Error processing WebView command: {}", err),
                    );
                }
            }
            ControllerCmd::MiniApp(cmd) => {
                if let Err(err) = app::handle_miniapp_cmd(&controller.app, cmd) {
                    // Log error but continue processing
                    controller.log(
                        LogLevel::Error,
                        &format!("Error processing MiniApp command: {}", err),
                    );
                }
            }
        }

        // Continue processing commands
        true
    }

    /// Get a WebView instance directly from the HashMap
    /// This is meant to be used only internally by the FFI layer
    pub(crate) fn get_webview(&self, appid: &str, path: &str) -> Option<Arc<WebView>> {
        if let Ok(webviews) = self.webviews.lock() {
            webviews
                .get(&(appid.to_string(), path.to_string()))
                .cloned()
        } else {
            None
        }
    }

    /// Put a WebView instance into the HashMap
    /// This is meant to be used only internally by the FFI layer
    pub(crate) fn put_webview(&self, appid: String, path: String, webview: Arc<WebView>) -> bool {
        if let Ok(mut webviews) = self.webviews.lock() {
            webviews.insert((appid, path), webview);
            true
        } else {
            false
        }
    }

    /// Start the dedicated command thread for UI business
    pub(crate) fn run<F>(f: F, app: App) -> bool
    where
        F: FnOnce(Arc<Controller>) -> bool + Send + 'static,
    {
        let (sender, receiver) = mpsc::channel::<ControllerCmd>();
        let id = thread::current().id();
        let controller = Arc::new(Controller::new(sender, app, id));

        if CONTROLLER.set(controller.clone()).is_err() {
            return false;
        }

        Controller::spawn_command_thread(f, controller, receiver);
        true
    }

    /// Get the singleton controller instance
    pub(crate) fn get() -> Option<&'static Controller> {
        CONTROLLER.get().map(|v| &**v)
    }
}
