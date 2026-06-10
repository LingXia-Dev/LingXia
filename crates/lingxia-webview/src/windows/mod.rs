//! Windows (WebView2) platform implementation.
//!
//! This module is strictly generic WebView2 hosting: window/controller
//! lifecycle, the message loop, command dispatch, scheme/event plumbing,
//! and window-group arrangement mechanics. It draws no product UI itself;
//! a product shell can register a [`WindowsChromeRenderer`] (see
//! `renderer.rs`) to paint custom window chrome and map chrome hit tests.
//! Without a registered renderer, windows keep a plain standard OS frame.
//!
//! The implementation is split into focused submodules; this module
//! declares them, hosts the shared import prelude (submodules pull it
//! in with `use super::*;`), and re-exports the public API surface.

use crate::traits::{DownloadRequest, LoadDataRequest, NavigationPolicy, NewWindowPolicy};
use crate::webview::{
    EffectiveWebViewCreateOptions, WebTag, WebViewCreateSender, WebViewCreateStage, find_webview,
    find_webview_delegate, register_webview,
};
use crate::{
    LogLevel, WebResourceBody, WebResourceResponse, WebViewController, WebViewError,
    WebViewScriptError,
};
use http::{Request, StatusCode};
use std::collections::HashMap;
use std::ffi::c_void;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use webview2_com::{Microsoft::Web::WebView2::Win32::*, *};
use windows::{
    Win32::{
        Foundation::{E_POINTER, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
        Graphics::Gdi::{
            BeginPaint, ClientToScreen, CreateBitmap, CreateRoundRectRgn, DeleteObject, EndPaint,
            GetMonitorInfoW, HDC, HGDIOBJ, InvalidateRect, MONITOR_DEFAULTTONEAREST, MONITORINFO,
            MonitorFromWindow, PAINTSTRUCT, ScreenToClient, SetWindowRgn,
        },
        System::{
            Com::{
                COINIT_APARTMENTTHREADED, IStream, STREAM_SEEK_SET,
                StructuredStorage::CreateStreamOnHGlobal,
            },
            LibraryLoader, Threading,
        },
        UI::{
            Input::KeyboardAndMouse::{
                GetKeyState, ReleaseCapture, SetCapture, SetFocus, VK_CONTROL, VK_MENU, VK_SHIFT,
            },
            Shell::SHCreateMemStream,
            WindowsAndMessaging::{
                self, CREATESTRUCTW, GCLP_HICON, GCLP_HICONSM, HICON, ICON_BIG, ICON_SMALL,
                ICONINFO, MINMAXINFO, MSG, WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WM_NCCREATE,
                WM_SETICON, WNDCLASSW, WS_OVERLAPPEDWINDOW,
            },
        },
    },
    core::{BOOL, Interface, PCWSTR, PWSTR, Result as WinResult, w},
};

mod api;
mod controller;
mod environment;
mod events;
mod groups;
mod icons;
mod renderer;
mod scheme;
mod window;

pub(crate) use controller::WebViewInner;

pub use api::{
    WindowsChromeEvent, WindowsNavigationBarLayout, WindowsPanelActivatorLayout,
    WindowsPanelInputHandler, WindowsPanelKeyEvent, WindowsPanelPosition, WindowsTabBarItemLayout,
    WindowsTabBarLayout, WindowsTabBarPosition, WindowsWebViewWindowSnapshot, WindowsWindowLayout,
    clear_native_panel_input_handler, hide_native_panel, hide_panel, hide_webview_window,
    is_panel_visible, set_native_panel_input_handler, set_webview_chrome_event_handler,
    set_webview_close_handler, set_webview_user_data_dir, set_webview_window_layout,
    show_native_panel, show_native_terminal_panel, show_webview_panel, show_webview_window,
    show_webview_window_inactive, update_native_panel_body, webview_window_snapshot,
};
pub use icons::{cached_png_icon_handle, set_app_icon_from_path};
pub use renderer::{
    WindowsChromeAttachedState, WindowsChromeHit, WindowsChromePanel, WindowsChromeRenderer,
    WindowsChromeState, WindowsFrameButton, WindowsNativePanelContent, WindowsNativePanelKind,
    set_windows_chrome_renderer,
};

// Private glob re-imports so submodules can reach their siblings (and this
// prelude) through a single `use super::*;`.
use api::*;
use controller::*;
use environment::*;
use events::*;
use groups::*;
use icons::*;
use renderer::*;
use scheme::*;
use window::*;

type StdResult<T, E = WebViewError> = std::result::Result<T, E>;
