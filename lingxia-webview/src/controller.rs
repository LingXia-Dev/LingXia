use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::{Mutex, OnceLock, mpsc};
use std::thread::{self, ThreadId};

use miniapp::{
    AppRuntime, AssetFileEntry, DeviceInfo, MiniAppError, WebViewController, error, info,
};

use crate::webview::ControllerCmd;
use crate::{App, webview::WebView};

mod app;
mod webview;

static CONTROLLER: OnceLock<Arc<Controller>> = OnceLock::new();

pub(crate) struct Controller {
    app: App,
    webviews: Mutex<HashMap<(String, String), WebView>>,
    sender: mpsc::Sender<ControllerCmd>,
    ui_thread_id: ThreadId,
}

impl Drop for Controller {
    fn drop(&mut self) {
        // Clean up all WebViews before dropping
        if let Ok(mut webviews) = self.webviews.lock() {
            webviews.clear();
        }

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

    fn device_info(&self) -> DeviceInfo {
        self.app.device_info()
    }

    fn create_webview(
        &self,
        appid: String,
        path: String,
    ) -> Result<Arc<dyn WebViewController>, MiniAppError> {
        // Check if we're on the UI thread
        let current_thread_id = std::thread::current().id();
        if current_thread_id == self.ui_thread_id {
            // We're on UI thread, create WebView directly
            let webview = WebView::create_and_register(
                appid.clone(),
                path.clone(),
                self.ui_thread_id,
                self.sender.clone(),
                |appid, path, webview| self.put_webview(appid, path, webview),
            )?;
            Ok(Arc::new(webview))
        } else {
            // We're not on UI thread, use message passing
            let (responder, receiver) = mpsc::channel();
            let cmd = ControllerCmd::CreateWebViewInstance {
                appid,
                path,
                responder,
            };

            self.sender.send(cmd).map_err(|e| {
                MiniAppError::WebView(format!("Failed to send create command: {}", e))
            })?;

            let webview = receiver.recv().map_err(|_| {
                MiniAppError::WebView("Failed to receive WebView creation result".to_string())
            })??;

            Ok(Arc::new(webview))
        }
    }

    fn open_miniapp(&self, appid: String, path: String) -> Result<(), MiniAppError> {
        let (responder, receiver) = mpsc::channel();
        let cmd = ControllerCmd::MiniAppOperation(crate::webview::MiniAppCmd::OpenMiniApp {
            appid,
            path,
            responder,
        });

        self.sender.send(cmd).map_err(|e| {
            MiniAppError::WebView(format!("Failed to send open miniapp command: {}", e))
        })?;

        receiver.recv().map_err(|_| {
            MiniAppError::WebView("Failed to receive open miniapp result".to_string())
        })?
    }

    fn close_miniapp(&self, appid: String) -> Result<(), MiniAppError> {
        let (responder, receiver) = mpsc::channel();
        let cmd = ControllerCmd::MiniAppOperation(crate::webview::MiniAppCmd::CloseMiniApp {
            appid,
            responder,
        });

        self.sender.send(cmd).map_err(|e| {
            MiniAppError::WebView(format!("Failed to send close miniapp command: {}", e))
        })?;

        receiver.recv().map_err(|_| {
            MiniAppError::WebView("Failed to receive close miniapp result".to_string())
        })?
    }

    fn switch_page(&self, appid: String, path: String) -> Result<(), MiniAppError> {
        let (responder, receiver) = mpsc::channel();
        let cmd = ControllerCmd::MiniAppOperation(crate::webview::MiniAppCmd::SwitchPage {
            appid,
            path,
            responder,
        });

        self.sender.send(cmd).map_err(|e| {
            MiniAppError::WebView(format!("Failed to send switch page command: {}", e))
        })?;

        receiver.recv().map_err(|_| {
            MiniAppError::WebView("Failed to receive switch page result".to_string())
        })?
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
                info!("Shutdown command received, stopping command loop");
                return false; // Stop processing commands
            }
            ControllerCmd::WebViewOperation(cmd) => {
                if let Err(err) = webview::handle_webview_cmd(cmd) {
                    // Log error but continue processing
                    error!("Error processing WebView command: {}", err);
                }
            }
            ControllerCmd::MiniAppOperation(cmd) => {
                if let Err(err) = app::handle_miniapp_cmd(&controller.app, cmd) {
                    // Log error but continue processing
                    error!("Error processing MiniApp command: {}", err);
                }
            }
            ControllerCmd::CreateWebViewInstance {
                appid,
                path,
                responder,
            } => {
                let result = WebView::create_and_register(
                    appid.clone(),
                    path.clone(),
                    controller.ui_thread_id,
                    controller.sender.clone(),
                    |appid, path, webview| controller.put_webview(appid, path, webview),
                );

                let _ = responder.send(result);
            }
        }

        // Continue processing commands
        true
    }

    /// Get a WebView instance directly from the HashMap
    /// This is meant to be used only internally by the FFI layer
    pub(crate) fn get_webview(&self, appid: &str, path: &str) -> Option<WebView> {
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
    pub(crate) fn put_webview(&self, appid: String, path: String, webview: WebView) -> bool {
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
