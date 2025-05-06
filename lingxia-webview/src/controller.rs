use std::collections::HashMap;
use std::sync::Arc;
use std::sync::{Mutex, OnceLock, mpsc};
use std::thread;

use miniapp::{AppController, ControllerCmd, MiniAppError, log::LogLevel};

use crate::WebView;
pub mod webview;
use webview::handle_webview_cmd;

static CONTROLLER: OnceLock<Controller> = OnceLock::new();

pub(crate) struct Controller {
    pub(crate) webviews: Mutex<HashMap<(String, String), Arc<WebView>>>,
    sender: mpsc::Sender<ControllerCmd>,
}

impl Drop for Controller {
    fn drop(&mut self) {
        // Try to send shutdown command, ignore errors since we're dropping anyway
        let _ = self.sender.send(ControllerCmd::Shutdown);
    }
}

impl AppController for Controller {
    fn read_asset(&self, path: &str) -> Result<Vec<u8>, MiniAppError> {
        todo!()
    }

    fn app_data_dir(&self) -> std::path::PathBuf {
        todo!()
    }

    fn app_cache_dir(&self) -> std::path::PathBuf {
        todo!()
    }

    fn log(&self, level: LogLevel, app_id: &str, message: &str) {
        todo!()
    }

    fn send_cmd(&self, cmd: ControllerCmd) -> Result<(), MiniAppError> {
        // Create a channel for receiving the response
        let (_tx, rx) = mpsc::channel();

        // Send the command with the response channel
        self.sender
            .send(cmd)
            .map_err(|e| MiniAppError::WebView(format!("Failed to send command: {}", e)))?;

        // Wait for the response
        rx.recv().map_err(|_| {
            MiniAppError::WebView("UI thread dropped without sending result".to_string())
        })
    }
}

impl Controller {
    fn new(sender: mpsc::Sender<ControllerCmd>) -> Self {
        Self {
            webviews: Mutex::new(HashMap::new()),
            sender,
        }
    }

    fn spawn_ui_thread<F>(f: F, receiver: mpsc::Receiver<ControllerCmd>)
    where
        F: FnOnce() -> bool + Send + 'static,
    {
        thread::spawn(move || {
            if !f() {
                return;
            }

            let controller = CONTROLLER.get().unwrap();

            // Process commands loop
            while let Ok(cmd) = receiver.recv() {
                if !Controller::handle_request(controller, cmd) {
                    break;
                }
            }
        });
    }

    /// Process a single request
    /// Returns true to continue processing, false to stop
    fn handle_request(controller: &Controller, request: ControllerCmd) -> bool {
        match request {
            ControllerCmd::WebView(cmd) => handle_webview_cmd(&controller.webviews, cmd),
            ControllerCmd::MiniApp(cmd) => todo!(),
            ControllerCmd::Shutdown => {
                false // Stop processing loop
            }
        }
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
    pub(crate) fn put_webview(&self, appid: String, path: String, webview: WebView) -> bool {
        if let Ok(mut webviews) = self.webviews.lock() {
            webviews.insert((appid, path), Arc::new(webview));
            true
        } else {
            false
        }
    }

    /// Start the dedicated UI thread for business
    pub(crate) fn run<F>(f: F) -> bool
    where
        F: FnOnce() -> bool + Send + 'static,
    {
        let (sender, receiver) = mpsc::channel::<ControllerCmd>();
        let controller = Controller::new(sender);

        if CONTROLLER.set(controller).is_err() {
            return false;
        }

        Controller::spawn_ui_thread(f, receiver);
        true
    }

    /// Get the singleton controller instance
    pub(crate) fn get() -> Option<&'static Controller> {
        CONTROLLER.get()
    }
}
