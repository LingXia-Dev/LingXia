use crate::WebViewInner;
use miniapp::{MiniAppError, WebViewController};
use std::sync::{Arc, mpsc};
use std::thread::ThreadId;

/// Cross-platform WebView with shared inner state
#[derive(Clone, Debug)]
pub struct WebView {
    inner: Arc<WebViewInner>,
    ui_thread_id: ThreadId,
    sender: mpsc::Sender<ControllerCmd>,
}

impl WebView {
    /// Create a new WebView and register it in the controller
    pub(crate) fn create_and_register(
        appid: String,
        path: String,
        ui_thread_id: ThreadId,
        sender: mpsc::Sender<ControllerCmd>,
        put_webview_fn: impl FnOnce(String, String, WebView) -> bool,
    ) -> Result<WebView, MiniAppError> {
        // Create WebView by calling platform-specific creation
        let platform_webview = WebViewInner::create(&appid, &path)?;

        let webview = Self {
            inner: Arc::new(platform_webview),
            ui_thread_id,
            sender,
        };

        // Add to registry
        if put_webview_fn(appid, path, webview.clone()) {
            Ok(webview)
        } else {
            Err(MiniAppError::WebView(
                "Failed to add WebView to registry".to_string(),
            ))
        }
    }

    pub(crate) fn inner(&self) -> &WebViewInner {
        self.inner.as_ref()
    }

    fn is_ui_thread(&self) -> bool {
        std::thread::current().id() == self.ui_thread_id
    }

    fn send_cmd(&self, cmd: WebViewCmd) -> Result<(), MiniAppError> {
        self.sender
            .send(ControllerCmd::WebViewOperation(cmd))
            .map_err(|e| MiniAppError::WebView(format!("Failed to send command: {}", e)))
    }
}

impl WebViewController for WebView {
    fn load_url(&self, url: String) -> Result<(), MiniAppError> {
        if self.is_ui_thread() {
            self.inner.load_url(url)
        } else {
            let (responder, receiver) = mpsc::channel();
            let cmd = WebViewCmd::LoadUrl {
                webview: self.clone(),
                url,
                responder,
            };
            self.send_cmd(cmd)?;
            receiver.recv().map_err(|_| {
                MiniAppError::WebView(
                    "WebView command 'LoadUrl' failed: channel closed".to_string(),
                )
            })?
        }
    }

    fn evaluate_javascript(&self, js: String) -> Result<(), MiniAppError> {
        if self.is_ui_thread() {
            self.inner.evaluate_javascript(js)
        } else {
            let (responder, receiver) = mpsc::channel();
            let cmd = WebViewCmd::EvaluateJavascript {
                webview: self.clone(),
                script: js,
                responder,
            };
            self.send_cmd(cmd)?;
            receiver.recv().map_err(|_| {
                MiniAppError::WebView(
                    "WebView command 'EvaluateJavascript' failed: channel closed".to_string(),
                )
            })?
        }
    }

    fn post_message(&self, message: String) -> Result<(), MiniAppError> {
        // post_message always goes through command sending
        let (responder, receiver) = mpsc::channel();
        let cmd = WebViewCmd::PostMessage {
            webview: self.clone(),
            message,
            responder,
        };
        self.send_cmd(cmd)?;
        receiver.recv().map_err(|_| {
            MiniAppError::WebView(
                "WebView command 'PostMessage' failed: channel closed".to_string(),
            )
        })?
    }

    fn set_devtools(&self, enabled: bool) -> Result<(), MiniAppError> {
        if self.is_ui_thread() {
            self.inner.set_devtools(enabled)
        } else {
            let (responder, receiver) = mpsc::channel();
            let cmd = WebViewCmd::SetDevtools {
                webview: self.clone(),
                enabled,
                responder,
            };
            self.send_cmd(cmd)?;
            receiver.recv().map_err(|_| {
                MiniAppError::WebView(
                    "WebView command 'SetDevtools' failed: channel closed".to_string(),
                )
            })?
        }
    }

    fn clear_browsing_data(&self) -> Result<(), MiniAppError> {
        if self.is_ui_thread() {
            self.inner.clear_browsing_data()
        } else {
            let (responder, receiver) = mpsc::channel();
            let cmd = WebViewCmd::ClearBrowsingData {
                webview: self.clone(),
                responder,
            };
            self.send_cmd(cmd)?;
            receiver.recv().map_err(|_| {
                MiniAppError::WebView(
                    "WebView command 'ClearBrowsingData' failed: channel closed".to_string(),
                )
            })?
        }
    }

    fn set_user_agent(&self, ua: String) -> Result<(), MiniAppError> {
        if self.is_ui_thread() {
            self.inner.set_user_agent(ua)
        } else {
            let (responder, receiver) = mpsc::channel();
            let cmd = WebViewCmd::SetUserAgent {
                webview: self.clone(),
                ua,
                responder,
            };
            self.send_cmd(cmd)?;
            receiver.recv().map_err(|_| {
                MiniAppError::WebView(
                    "WebView command 'SetUserAgent' failed: channel closed".to_string(),
                )
            })?
        }
    }

    fn set_scroll_listener_enabled(
        &self,
        enabled: bool,
        throttle_ms: Option<u64>,
    ) -> Result<(), MiniAppError> {
        if self.is_ui_thread() {
            self.inner.set_scroll_listener_enabled(enabled, throttle_ms)
        } else {
            let (responder, receiver) = mpsc::channel();
            let cmd = WebViewCmd::SetScrollListenerEnabled {
                webview: self.clone(),
                enabled,
                throttle_ms,
                responder,
            };
            self.send_cmd(cmd)?;
            receiver.recv().map_err(|_| {
                MiniAppError::WebView(
                    "WebView command 'SetScrollListenerEnabled' failed: channel closed".to_string(),
                )
            })?
        }
    }
}

/// WebView commands for the controller
#[derive(Debug)]
pub(crate) enum WebViewCmd {
    LoadUrl {
        webview: WebView,
        url: String,
        responder: mpsc::Sender<Result<(), MiniAppError>>,
    },
    EvaluateJavascript {
        webview: WebView,
        script: String,
        responder: mpsc::Sender<Result<(), MiniAppError>>,
    },
    PostMessage {
        webview: WebView,
        message: String,
        responder: mpsc::Sender<Result<(), MiniAppError>>,
    },
    SetDevtools {
        webview: WebView,
        enabled: bool,
        responder: mpsc::Sender<Result<(), MiniAppError>>,
    },
    ClearBrowsingData {
        webview: WebView,
        responder: mpsc::Sender<Result<(), MiniAppError>>,
    },
    SetUserAgent {
        webview: WebView,
        ua: String,
        responder: mpsc::Sender<Result<(), MiniAppError>>,
    },
    SetScrollListenerEnabled {
        webview: WebView,
        enabled: bool,
        throttle_ms: Option<u64>,
        responder: mpsc::Sender<Result<(), MiniAppError>>,
    },
}

/// Extended controller commands that include WebView commands
#[derive(Debug)]
pub(crate) enum ControllerCmd {
    WebViewOperation(WebViewCmd),
    CreateWebViewInstance {
        appid: String,
        path: String,
        responder: mpsc::Sender<Result<WebView, MiniAppError>>,
    },
    MiniAppOperation(MiniAppCmd),
    Shutdown,
}

/// MiniApp commands for the controller
#[derive(Debug)]
pub(crate) enum MiniAppCmd {
    OpenMiniApp {
        appid: String,
        path: String,
        responder: mpsc::Sender<Result<(), MiniAppError>>,
    },
    CloseMiniApp {
        appid: String,
        responder: mpsc::Sender<Result<(), MiniAppError>>,
    },
    SwitchPage {
        appid: String,
        path: String,
        responder: mpsc::Sender<Result<(), MiniAppError>>,
    },
}
