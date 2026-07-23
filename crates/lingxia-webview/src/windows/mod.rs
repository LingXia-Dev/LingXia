//! Windows (WebView2) platform implementation.
//!
//! This module is strictly generic WebView2 hosting: controller lifecycle,
//! the message loop, command dispatch, scheme/event plumbing, and WebView2
//! controller binding to a parent HWND supplied by the Windows UI layer.
//!
//! The implementation is split into focused submodules; this module
//! declares them, keeps the shared import prelude for WebView2 plumbing,
//! and re-exports the public Windows WebView API.

use crate::traits::{
    DownloadRequest, LoadDataRequest, NavigationPolicy, NetworkCaptureSnapshot, NewWindowPolicy,
    WebViewCookie, WebViewCookieSameSite, WebViewCookieSetRequest,
};
use crate::webview::{
    EffectiveWebViewCreateOptions, SecurityProfile, WebTag, WebViewCreateSender,
    WebViewCreateStage, WebViewDataMode, find_webview, find_webview_delegate, register_webview,
};
use crate::{
    ClearSiteDataOptions, ClearSiteDataResult, UserAgentOverride, WebResourceBody,
    WebResourceResponse, WebViewController, WebViewError, WebViewScriptError,
};
use http::{Request, StatusCode};
use std::collections::HashMap;
use std::ffi::c_void;
use std::io::Read;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use webview2_com::{Microsoft::Web::WebView2::Win32::*, *};
use windows::{
    Win32::{
        Foundation::{E_POINTER, HWND, LPARAM, RECT, WPARAM},
        System::{
            Com::{IStream, STREAM_SEEK_SET, StructuredStorage::CreateStreamOnHGlobal},
            Threading,
        },
        UI::{
            Shell::SHCreateMemStream,
            WindowsAndMessaging::{self, MSG, WM_APP},
        },
    },
    core::{BOOL, Interface, PCWSTR, PWSTR, Result as WinResult},
};

mod browser_emulation;
mod composition;
mod console;
mod controller;
pub(crate) mod data_store;
mod environment;
mod events;
#[cfg(feature = "webview-input")]
mod input;
mod native_view;
mod network;
mod scheme;

pub(crate) use controller::WebViewInner;

pub use browser_emulation::{
    WindowsBrowserEmulationProfile, set_windows_browser_emulation_profile_for_new_webviews,
};
pub use composition::{set_webview_composition_hosting, webview_composition_hosting_enabled};
pub use environment::set_windows_context_menu_refresh_provider;
pub use native_view::{
    WindowsWebViewHandler, WindowsWebViewNativeView, WindowsWebViewNativeViewHost,
    find_webview_handler, set_webview_devtools_enabled, set_webview_native_view_host,
    set_webview_user_data_dir,
};

// Private glob re-imports so submodules can reach their siblings (and this
// prelude) through a single `use super::*;`.
use composition::*;
use environment::*;
use events::*;
use native_view::*;
use scheme::*;

type StdResult<T, E = WebViewError> = std::result::Result<T, E>;
