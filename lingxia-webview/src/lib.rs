use thiserror::Error;

/// WebView-specific error types
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum WebViewError {
    #[error("WebView error: {0}")]
    WebView(String),

    #[error("Invalid WebView create options: {0}")]
    InvalidCreateOptions(String),
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
pub mod apple;

#[cfg(all(target_os = "linux", target_env = "ohos"))]
mod harmony;

// Public exports
// WebViewError and LogLevel are defined above
pub use traits::{
    LoadDataRequest, NavigationPolicy, NewWindowPolicy, SchemeOutcome, SystemPipeReader,
    WebResourceBody, WebResourceResponse, WebViewController, WebViewDelegate,
};
pub use webview::{
    WebTag, WebView, WebViewCreateOptions, WebViewCreateStage, WebViewEvent,
    WebViewEventSubscription, WebViewSession, create_webview, create_webview_strict,
    destroy_webview, find_webview, get_webview_delegate, init_webview_manager,
    set_webview_delegate,
};

#[cfg(target_os = "android")]
pub use android::{initialize_jni, with_env};

#[cfg(all(target_os = "linux", target_env = "ohos"))]
pub use harmony::{
    check_navigation_policy, schemehandler::register_custom_schemes, tsfn,
    webview_controller_created, webview_controller_destroyed,
};
