//! Cross-platform WebView hosting layer for LingXia.
//!
//! This crate is strictly *generic* webview hosting: webview creation and
//! lifecycle, navigation/scheme/event plumbing, and minimal native surface
//! ownership required by each platform WebView runtime. It contains no
//! product UI.
//!
//! On Windows, LingXia's legacy host-window grouping/chrome bridge is behind
//! the `windows-host` feature so standalone `lingxia-webview` users get a
//! plain WebView2 surface by default.

use thiserror::Error;

/// WebView-specific error types
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum WebViewError {
    #[error("WebView error: {0}")]
    WebView(String),

    #[error("Invalid WebView create options: {0}")]
    InvalidCreateOptions(String),
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

mod input_helper;
mod traits;
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
pub use traits::{
    ClickOptions, DownloadRequest, FileChooserFile, FileChooserRequest, FileChooserResponse,
    FillOptions, LoadDataRequest, LoadError, LoadErrorKind, NavigationPolicy, NewWindowPolicy,
    PressOptions, SchemeOutcome, ScrollOptions, SystemPipeReader, TypeOptions, WebResourceBody,
    WebResourceResponse, WebViewController, WebViewCookie, WebViewCookieSameSite,
    WebViewCookieSetRequest, WebViewDelegate, WebViewInputController,
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
            check_navigation_policy, complete_pending_screenshot_request,
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
            WindowsWebViewContentWindow, WindowsWebViewHandler, WindowsWebViewWindowSnapshot,
            find_webview_content_window, find_webview_handler, post_to_window_thread,
            set_webview_devtools_enabled, set_webview_user_data_dir,
        };

        #[cfg(feature = "windows-host")]
        #[doc(hidden)]
        pub mod lingxia_host {
            pub use crate::windows::{
                HostWindowCreatedHandler, WindowsCardDecorator, set_windows_card_decorator,
                WindowsChromeAttachedLayout, WindowsChromeAttachedState,
                WindowsChromeCommand, WindowsChromeHit, WindowsChromePanel,
                WindowsChromePanelLayout, WindowsChromePanelLayoutInput, WindowsChromeRenderer,
                WindowsChromeState, WindowsFrameButton, WindowsHostPanelContent,
                WindowsHostPanelInputHandler, WindowsHostPanelKeyEvent, WindowsHostPanelTab,
                WindowsPanelPosition, WindowsWebViewHostWindow, WindowsWindowLayout,
                add_webview_host_window_created_handler, clear_host_panel_input_handler,
                clear_webview_group_override, clear_webview_os_frame, set_webview_group_override,
                set_webview_os_frame,
                find_webview_host_window, hide_host_panel, invalidate_host_panel, is_panel_visible,
                request_webview_host_window_layout, restore_presented_group_main,
                set_default_window_size, set_host_panel_input_handler, set_host_panel_maximized,
                set_host_panel_tabs, set_webview_chrome_event_handler, set_webview_close_handler,
                set_webview_window_layout, set_windows_chrome_renderer,
                show_interactive_host_panel, update_host_panel_body,
            };
        }
    }
}
