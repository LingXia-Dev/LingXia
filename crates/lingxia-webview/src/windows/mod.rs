//! Windows (WebView2) platform implementation.
//!
//! This module is strictly generic WebView2 hosting: controller lifecycle,
//! the message loop, command dispatch, scheme/event plumbing, and the
//! minimal HWND surface required by WebView2.
//!
//! LingXia-specific host-window grouping, custom chrome, and native panel
//! support are compiled only with the `windows-host` feature. That feature
//! exists for the LingXia Windows runtime; standalone `lingxia-webview`
//! users get a plain WebView2 surface by default.
//!
//! The implementation is split into focused submodules; this module
//! declares them, hosts the shared import prelude (submodules pull it
//! in with `use super::*;`), and re-exports the public API surface.

#![cfg_attr(not(feature = "windows-host"), allow(dead_code, unused_imports))]

use crate::traits::{DownloadRequest, LoadDataRequest, NavigationPolicy, NewWindowPolicy};
use crate::webview::{
    EffectiveWebViewCreateOptions, SecurityProfile, WebTag, WebViewCreateSender,
    WebViewCreateStage, find_webview, find_webview_delegate, register_webview,
};
use crate::{
    LogLevel, WebResourceBody, WebResourceResponse, WebViewController, WebViewError,
    WebViewScriptError,
};
use http::{Request, StatusCode};
use std::cell::{Cell, RefCell};
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
        Foundation::{
            COLORREF, E_POINTER, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM,
        },
        Graphics::Dwm::{
            DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmExtendFrameIntoClientArea,
            DwmSetWindowAttribute,
        },
        Graphics::Gdi::{
            BeginPaint, BitBlt, ClientToScreen, CreateCompatibleBitmap, CreateCompatibleDC,
            DeleteDC, DeleteObject, EndPaint, GetMonitorInfoW, HDC, HGDIOBJ, InvalidateRect,
            MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow, PAINTSTRUCT, SRCCOPY,
            ScreenToClient, SelectObject,
        },
        System::{
            Com::{
                COINIT_APARTMENTTHREADED, IStream, STREAM_SEEK_SET,
                StructuredStorage::CreateStreamOnHGlobal,
            },
            LibraryLoader, Threading,
        },
        UI::{
            Controls::{MARGINS, WM_MOUSELEAVE},
            Input::KeyboardAndMouse::{
                GetKeyState, ReleaseCapture, SetCapture, SetFocus, TME_LEAVE, TME_NONCLIENT,
                TRACKMOUSEEVENT, TrackMouseEvent, VK_CONTROL, VK_MENU, VK_SHIFT,
            },
            Shell::SHCreateMemStream,
            WindowsAndMessaging::{
                self, CREATESTRUCTW, MINMAXINFO, MSG, WINDOW_EX_STYLE, WM_APP, WM_NCCREATE,
                WNDCLASSW, WS_OVERLAPPEDWINDOW,
            },
        },
    },
    core::{BOOL, Interface, PCWSTR, PWSTR, Result as WinResult, w},
};

mod controller;
mod environment;
mod events;
mod scheme;
#[cfg(not(feature = "windows-host"))]
mod surface;

#[cfg(feature = "windows-host")]
mod host;

pub(crate) use controller::WebViewInner;

#[cfg(not(feature = "windows-host"))]
pub use surface::{
    WindowsWebViewContentWindow, WindowsWebViewHandler, WindowsWebViewWindowSnapshot,
    find_webview_content_window, find_webview_handler, post_to_window_thread,
    set_webview_devtools_enabled, set_webview_user_data_dir,
};

#[cfg(feature = "windows-host")]
pub use host::{
    HostWindowCreatedHandler, WindowsCardDecorator, set_windows_card_decorator,
    WindowsChromeAttachedLayout, WindowsChromeAttachedState,
    WindowsChromeCommand, WindowsChromeHit, WindowsChromePanel, WindowsChromePanelLayout,
    WindowsChromePanelLayoutInput, WindowsChromeRenderer, WindowsChromeState, WindowsFrameButton,
    WindowsHostPanelContent, WindowsHostPanelInputHandler, WindowsHostPanelKeyEvent,
    WindowsHostPanelTab, WindowsPanelPosition, WindowsWebViewContentWindow, WindowsWebViewHandler,
    WindowsWebViewHostWindow, WindowsWebViewWindowSnapshot, WindowsWindowLayout,
    add_webview_host_window_created_handler, clear_host_panel_input_handler,
    find_webview_content_window, find_webview_handler, find_webview_host_window, hide_host_panel,
    invalidate_host_panel, is_panel_visible, post_to_window_thread,
    request_webview_host_window_layout, restore_presented_group_main, set_default_window_size,
    set_host_panel_input_handler, set_host_panel_maximized, set_host_panel_tabs,
    set_webview_chrome_event_handler, set_webview_close_handler, set_webview_devtools_enabled,
    set_webview_user_data_dir, set_webview_window_layout, set_windows_chrome_renderer,
    show_interactive_host_panel, update_host_panel_body,
};

// Private glob re-imports so submodules can reach their siblings (and this
// prelude) through a single `use super::*;`.
use controller::*;
use environment::*;
use events::*;
#[cfg(feature = "windows-host")]
use host::*;
use scheme::*;
#[cfg(not(feature = "windows-host"))]
use surface::*;

type StdResult<T, E = WebViewError> = std::result::Result<T, E>;
