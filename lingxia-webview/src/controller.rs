use log::{error, warn};
use miniapp::PageController;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::{Mutex, OnceLock};
use std::thread;
use tokio::sync::{mpsc, oneshot};

use crate::WebView;

pub trait WebViewController {
    /// Load a URL in the WebView and wait for the operation to complete
    async fn load_url(&self, appid: String, path: String, url: String) -> bool;
}

static CONTROLLER: OnceLock<Controller> = OnceLock::new();

#[derive(Debug)]
pub enum ControllerCmd {
    LoadUrl {
        appid: String,
        path: String,
        url: String,
        responder: oneshot::Sender<bool>,
    },
    Shutdown,
}

pub struct Controller {
    pub(crate) webviews: Mutex<HashMap<(String, String), Arc<WebView>>>,
    sender: mpsc::Sender<ControllerCmd>,
}

impl Drop for Controller {
    fn drop(&mut self) {
        // Try to send shutdown command, ignore errors since we're dropping anyway
        let _ = self.sender.try_send(ControllerCmd::Shutdown);
    }
}

impl WebViewController for Controller {
    async fn load_url(&self, appid: String, path: String, url: String) -> bool {
        // Create a oneshot channel for the result
        let (tx, rx) = oneshot::channel();

        // Send request to UI thread
        if let Err(e) = self
            .sender
            .send(ControllerCmd::LoadUrl {
                appid,
                path,
                url,
                responder: tx,
            })
            .await
        {
            error!("Failed to send LoadUrl command: {}", e);
            return false;
        }

        // Wait for the result
        match rx.await {
            Ok(result) => result,
            Err(_) => {
                error!("UI thread dropped without sending result");
                false
            }
        }
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
            // Create a current thread runtime using Builder
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create tokio runtime");

            let _guard = runtime.enter();

            if !f() {
                return;
            }

            async fn process_commands(
                controller: &Controller,
                mut receiver: mpsc::Receiver<ControllerCmd>,
            ) {
                while let Some(cmd) = receiver.recv().await {
                    if !Controller::handle_request(controller, cmd) {
                        break;
                    }
                }
            }

            let controller = CONTROLLER.get().unwrap();

            runtime.block_on(process_commands(controller, receiver));
        });
    }

    /// Process a single request
    /// Returns true to continue processing, false to stop
    fn handle_request(controller: &Controller, request: ControllerCmd) -> bool {
        match request {
            ControllerCmd::LoadUrl {
                appid,
                path,
                url,
                responder,
            } => {
                let success = if let Ok(webviews) = controller.webviews.lock() {
                    if let Some(webview) = webviews.get(&(appid.clone(), path.clone())) {
                        webview.load_url(url)
                    } else {
                        warn!("WebView instance not found for {}/{}", appid, path);
                        false
                    }
                } else {
                    error!("Failed to lock webviews mutex");
                    false
                };

                let _ = responder.send(success);
                true // Continue processing requests
            }
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
            error!("Failed to lock webviews mutex");
            false
        }
    }

    /// Start the dedicated UI thread for business
    pub(crate) fn run<F>(f: F) -> bool
    where
        F: FnOnce() -> bool + Send + 'static,
    {
        let (sender, receiver) = mpsc::channel::<ControllerCmd>(10);
        let controller = Controller::new(sender);

        if CONTROLLER.set(controller).is_err() {
            error!("Failed to set global CONTROLLER");
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
