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
mod apple;

#[cfg(all(target_os = "linux", target_env = "ohos"))]
mod harmony;

// Public exports
// WebViewError and LogLevel are defined above
pub use traits::{
    DownloadRequest, FileChooserFile, FileChooserRequest, FileChooserResponse, LoadDataRequest,
    LoadError, LoadErrorKind, NavigationPolicy, NewWindowPolicy, SchemeOutcome, SystemPipeReader,
    WebResourceBody, WebResourceResponse, WebViewController, WebViewDelegate,
};
pub use webview::{
    BrowserWebViewBuilder, ProxyActivation, ProxyApplyReport, ProxyApplyStatus, ProxyConfig,
    StrictWebViewBuilder, WebTag, WebView, WebViewBuilder, WebViewCreateStage, WebViewEvent,
    WebViewEventSubscription, WebViewSession,
};

/// Runtime-scoped APIs (instance lookup/destruction, proxy state).
pub mod runtime {
    use std::sync::Arc;

    use crate::webview;
    use crate::{ProxyApplyReport, ProxyConfig, WebTag, WebView, WebViewError};

    pub fn find_webview(webtag: &WebTag) -> Option<Arc<WebView>> {
        webview::find_webview(webtag)
    }

    pub fn destroy_webview(webtag: &WebTag) {
        webview::destroy_webview(webtag);
    }

    pub fn set_proxy(config: Option<ProxyConfig>) -> Result<ProxyApplyReport, WebViewError> {
        webview::set_proxy(config)
    }

    pub fn current_proxy() -> Option<ProxyConfig> {
        webview::current_proxy()
    }
}

/// Platform-specific APIs used by SDK/FFI integration layers.
pub mod platform {
    #[cfg(target_os = "android")]
    pub mod android {
        pub use crate::android::{initialize_jni, with_env};
    }

    #[cfg(target_os = "macos")]
    pub mod apple {
        pub use crate::apple::toggle_webview_devtools_by_swift_ptr;
    }

    #[cfg(all(target_os = "linux", target_env = "ohos"))]
    pub mod harmony {
        pub use crate::harmony::{
            check_navigation_policy, on_file_chooser_requested,
            schemehandler::register_custom_schemes, tsfn, webview_controller_created,
            webview_controller_destroyed,
        };

        #[doc(hidden)]
        pub fn on_load_error(webtag: &str, url: &str, error_code: i32, description: &str) {
            crate::harmony::on_load_error(webtag, url, error_code, description);
        }

        #[doc(hidden)]
        pub fn on_download_start(
            webtag_str: &str,
            url: &str,
            user_agent: &str,
            content_disposition: &str,
            mime_type: &str,
            content_length: i64,
        ) -> bool {
            crate::harmony::on_download_start(
                webtag_str,
                url,
                user_agent,
                content_disposition,
                mime_type,
                content_length,
            )
        }
    }
}
