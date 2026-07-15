//! Cross-platform WebView hosting layer for LingXia.
//!
//! This crate is strictly *generic* webview hosting: webview creation and
//! lifecycle, navigation/scheme/event plumbing, and minimal native surface
//! ownership required by each platform WebView runtime. It contains no
//! product UI.
//!
//! On Windows, host-window grouping, chrome, panels, and app layout live in
//! `lingxia-windows-sdk`; this crate only provides the WebView2 surface.

use thiserror::Error;

/// WebView-specific error types
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum WebViewError {
    #[error("WebView error: {0}")]
    WebView(String),

    #[error("Invalid WebView create options: {0}")]
    InvalidCreateOptions(String),

    /// The named operation is not available on this platform's WebView runtime.
    #[error("{0} is not supported on this platform")]
    Unsupported(String),
}

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum WebViewScriptError {
    #[error("JavaScript error: {0}")]
    Js(String),

    #[error("JavaScript evaluation timed out")]
    Timeout,

    #[error("JavaScript evaluation unsupported: {0}")]
    Unsupported(&'static str),

    #[error("WebView destroyed during JavaScript evaluation")]
    Destroyed,

    #[error("Navigation changed during JavaScript evaluation")]
    NavigationChanged,

    #[error("Platform JavaScript evaluation error: {0}")]
    Platform(String),
}

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum WebViewInputError {
    #[error(transparent)]
    Script(#[from] WebViewScriptError),

    #[error("Element not found: {0}")]
    ElementNotFound(String),

    #[error("Element not interactable: {0}")]
    ElementNotInteractable(String),

    #[error("Input unsupported: {0}")]
    Unsupported(&'static str),

    #[error("WebView destroyed during input handling")]
    Destroyed,

    #[error("Navigation changed during input handling")]
    NavigationChanged,

    #[error("Platform input error: {0}")]
    Platform(String),
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

mod error_page;
/// Typed delegate events: correlated navigation lifecycle, observable state
/// snapshots, and the canonical derived-state folds.
pub mod events;
mod input_helper;
mod traits;
/// Process-local URL callback channels for navigation handoff.
pub mod url_callback;
mod webview;

#[cfg(target_os = "android")]
mod android;

#[cfg(any(target_os = "ios", target_os = "macos"))]
mod apple;

#[cfg(all(target_os = "linux", target_env = "ohos"))]
mod harmony;

#[cfg(target_os = "windows")]
mod windows;

// Public exports
// WebViewError and LogLevel are defined above
pub use error_page::{LoadErrorPage, render_load_error_page};
pub use events::{
    NavigationCancellationReason, NavigationEvent, NavigationId, NavigationProgress,
    ObservedWebViewState, WebViewEventObserver, WebViewObservedEvent, WebViewStateChange,
};
pub use traits::{
    ClearSiteDataOptions, ClearSiteDataResult, ClickOptions, DownloadRequest, FileChooserFile,
    FileChooserRequest, FileChooserResponse, FillOptions, LoadDataRequest, LoadError,
    LoadErrorKind, NavigationPolicy, NetworkBody, NetworkCaptureSnapshot, NetworkEntry,
    NewWindowPolicy, PressOptions, SchemeOutcome, ScrollOptions, SystemPipeReader, TypeOptions,
    WebResourceBody, WebResourceResponse, WebViewController, WebViewCookie, WebViewCookieSameSite,
    WebViewCookieSetRequest, WebViewDelegate, WebViewInputController,
};
pub use webview::{
    BrowserWebViewBuilder, ProxyActivation, ProxyApplyReport, ProxyApplyStatus, ProxyConfig,
    StrictWebViewBuilder, WebTag, WebView, WebViewBuilder, WebViewCreateStage, WebViewDataMode,
    WebViewEvent, WebViewEventSubscription, WebViewSession,
};

/// Global website-data operations for privacy surfaces: usage counts,
/// clear cache, clear cookies & site data.
///
/// Every operation here is profile-wide: all browser tabs share one browser
/// profile (the platform's default data store), so clears affect every site,
/// not just the current tab. On Windows, [`cache_site_count`] returns `Ok(0)`
/// because WebView2 cannot enumerate HTTP-cache origins (clearing still
/// works). Unsupported platforms return [`WebViewError::Unsupported`].
pub mod data_store {
    /// Profile-wide cookies/site-data footprint.
    #[derive(Debug, Clone, Copy)]
    pub struct SiteDataUsage {
        /// Sites storing cookies or other site data.
        pub sites: usize,
        /// Total cookie count across all sites.
        pub cookies: usize,
    }

    #[cfg(any(target_os = "ios", target_os = "macos"))]
    pub use crate::apple::data_store::{
        cache_site_count, clear_all_site_data, clear_cache, site_data_usage,
    };

    #[cfg(target_os = "windows")]
    pub use crate::windows::data_store::{
        cache_site_count, clear_all_site_data, clear_cache, site_data_usage,
    };

    #[cfg(not(any(target_os = "ios", target_os = "macos", target_os = "windows")))]
    mod unsupported {
        use super::SiteDataUsage;
        use crate::WebViewError;

        fn err(action: &str) -> WebViewError {
            WebViewError::Unsupported(action.to_string())
        }

        pub async fn cache_site_count() -> Result<usize, WebViewError> {
            Err(err("cache usage query"))
        }

        pub async fn site_data_usage() -> Result<SiteDataUsage, WebViewError> {
            Err(err("site data usage query"))
        }

        pub async fn clear_cache(_since_unix_ms: Option<u64>) -> Result<(), WebViewError> {
            Err(err("clear cache"))
        }

        pub async fn clear_all_site_data(_since_unix_ms: Option<u64>) -> Result<(), WebViewError> {
            Err(err("clear cookies & site data"))
        }
    }
    #[cfg(not(any(target_os = "ios", target_os = "macos", target_os = "windows")))]
    pub use unsupported::*;
}

/// Runtime-scoped APIs (instance lookup/destruction, proxy state).
pub mod runtime {
    use std::sync::Arc;

    use crate::webview;
    use crate::{ProxyApplyReport, ProxyConfig, WebTag, WebView, WebViewError};

    pub fn find_webview(webtag: &WebTag) -> Option<Arc<WebView>> {
        webview::find_webview(webtag)
    }

    pub fn list_webviews() -> Vec<WebTag> {
        webview::list_webviews()
    }

    pub fn destroy_webview(webtag: &WebTag) {
        webview::destroy_webview(webtag);
    }

    pub fn configure_proxy_for_new_webviews(
        config: Option<ProxyConfig>,
    ) -> Result<(), WebViewError> {
        webview::configure_proxy_for_new_webviews(config)
    }

    pub fn apply_proxy_to_current_runtime(
        config: Option<ProxyConfig>,
    ) -> Result<ProxyApplyReport, WebViewError> {
        webview::apply_proxy_to_current_runtime(config)
    }

    pub fn configured_proxy_for_new_webviews() -> Option<ProxyConfig> {
        webview::configured_proxy_for_new_webviews()
    }
}

/// Platform-specific APIs used by SDK/FFI integration layers.
pub mod platform {
    #[cfg(target_os = "android")]
    pub mod android {
        pub use crate::android::{initialize_jni, with_env};
    }

    #[cfg(any(target_os = "ios", target_os = "macos"))]
    pub mod apple {
        pub use crate::apple::BRIDGE_DOWNSTREAM_CSP_SOURCE;
        pub use crate::apple::BRIDGE_DOWNSTREAM_URL;
        #[cfg(target_os = "macos")]
        pub use crate::apple::toggle_webview_devtools_by_swift_ptr;
    }

    #[cfg(all(target_os = "linux", target_env = "ohos"))]
    pub mod harmony {
        pub use crate::harmony::{
            check_navigation_policy, complete_pending_screenshot_request, notify_webview_state,
            on_file_chooser_requested, schemehandler::register_custom_schemes, tsfn,
            webview_controller_created, webview_controller_destroyed,
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

    #[cfg(target_os = "windows")]
    pub mod windows {
        pub use crate::windows::{
            WindowsWebViewHandler, WindowsWebViewNativeView, WindowsWebViewNativeViewHost,
            find_webview_handler, set_webview_devtools_enabled, set_webview_native_view_host,
            set_webview_user_data_dir, set_windows_context_menu_refresh_provider,
        };
    }
}
