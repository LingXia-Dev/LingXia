use thiserror::Error;

/// WebView-specific error types
#[derive(Error, Debug)]
pub enum WebViewError {
    #[error("WebView error: {0}")]
    WebView(String),
}

/// Log levels for WebView logging
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Verbose,
    Debug,
    Info,
    Warn,
    Error,
}

mod traits;
mod webview;

#[cfg(target_os = "android")]
mod android;

#[cfg(any(target_os = "ios", target_os = "macos"))]
mod apple;

#[cfg(all(target_os = "linux", target_env = "ohos"))]
mod harmony;

// Public exports
// WebViewError and LogLevel are defined above
pub use traits::{WebViewController, WebViewDelegate};
pub use webview::{
    WebTag, WebView, create_webview, destroy_webview, find_webview, init_webview_manager,
};

#[cfg(target_os = "android")]
pub use android::{get_env, initialize_jni};

#[cfg(all(target_os = "linux", target_env = "ohos"))]
pub use harmony::{schemehandler::register_custom_schemes, tsfn};
