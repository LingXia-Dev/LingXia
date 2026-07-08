//! LingXia host framework entry crate.
//!
//! Use this crate from native host apps and native Rust libraries. It provides:
//!
//! - platform bootstrap and FFI entry points for Android, Apple platforms
//!   (iOS and macOS), HarmonyOS, and Windows;
//! - the [`native`] macro for page-facing Rust APIs;
//! - host addon registration through [`HostAddon`] and [`register_host_addon`];
//! - native service APIs such as [`app`], [`device`], [`wifi`], [`media`], [`task`],
//!   and [`update`];
//! - optional JS AppService extension APIs under [`js`] when the `standard`
//!   feature is enabled;
//! - optional devtool helpers under `dev` when the `devtool` feature is
//!   enabled.
//!
//! Most applications should depend on `lingxia` rather than lower-level crates
//! such as `lingxia-lxapp`. Lower-level crates remain available for runtime
//! internals and advanced integrations.

extern crate self as lingxia;
pub use host_addon::{HostAddon, register_host_addon};
pub use lingxia_native_macros::native;

pub use lxapp::host;
pub use lxapp::host::{ChannelContext, ChannelMessage, StreamContext};
pub use lxapp::{LxApp, LxAppSecurityPrivilege};

/// Host app metadata, state-path helpers, and lifecycle helpers.
pub mod app;
pub use app::{home_app_id, lingxia_id, product_version};
mod applink;
mod bootstrap;
mod capabilities;
/// LxApp devtool helpers for host-side inspection and automation.
#[cfg(feature = "devtool")]
pub mod dev {
    pub use crate::devtool::{
        DeviceController, DeviceEntry, DeviceState, LxAppDevConfig, LxAppDevIdentity,
        LxAppDevPageInfo, device_get, device_list, device_set, install_lxapp_dev_config,
        install_lxapp_dev_config_from_env, list_app_windows, lxapp_dev_nav_back,
        lxapp_dev_nav_redirect, lxapp_dev_nav_relaunch, lxapp_dev_nav_switch_tab, lxapp_dev_nav_to,
        lxapp_dev_page_back, lxapp_dev_page_click, lxapp_dev_page_current, lxapp_dev_page_eval,
        lxapp_dev_page_fill, lxapp_dev_page_info, lxapp_dev_page_input_supported,
        lxapp_dev_page_list, lxapp_dev_page_press, lxapp_dev_page_query, lxapp_dev_page_screenshot,
        lxapp_dev_page_scroll, lxapp_dev_page_scroll_to, lxapp_dev_page_type, perform_app_keyboard,
        perform_app_mouse, register_device_controller, take_app_screenshot,
    };
    pub use lingxia_platform::traits::keyboard::{
        AppKeyboardAction, AppKeyboardModifier, AppKeyboardRequest, AppKeyboardResult,
    };
    pub use lingxia_platform::traits::mouse::{
        AppMouseAction, AppMouseButton, AppMouseRequest, AppMouseResult,
    };
}
/// Device identity, screen geometry, vibration, and system-setting APIs.
pub mod device;
#[cfg(feature = "devtool")]
mod devtool;
mod error;
/// File dialogs and host file-manager integrations.
pub mod file;
mod host_addon;
/// JS AppService extension registration helpers.
#[cfg(feature = "standard")]
pub mod js;
/// Geolocation APIs.
pub mod location;
mod logging;
/// Media, camera, scanner, and media-preview helpers.
pub mod media;
/// Network status and change subscriptions.
pub mod network;
/// Provider traits and registration helpers.
pub mod provider;
mod runtime;
/// Shared async task helpers backed by LingXia's global executor.
pub mod task;
/// Terminal backend status and integration helpers.
#[cfg(feature = "terminal-runtime")]
pub mod terminal {
    pub use lingxia_terminal::{
        BackendStatus, TerminalBackend, TerminalCell, TerminalSnapshot, ghostty_available,
        ghostty_status, ghostty_status_json, terminal_close, terminal_create, terminal_exited,
        terminal_read, terminal_resize, terminal_snapshot, terminal_snapshot_data, terminal_write,
    };
}
/// Host app update helpers and update event types.
pub mod update;
/// Process-local URL callback channels for native handoff flows.
pub mod url_callback;
/// Wi-Fi control, scanning, and state subscriptions.
pub mod wifi;

pub use error::{Error, Result};

/// Logging types and logger registration helpers.
pub mod log {
    pub use crate::logging::{DownstreamLoggerError, register_downstream_logger};
    pub use ::log::{debug, error, info, trace, warn};
    pub use lingxia_log::{
        AttachedLogStream, LogLevel, LogMessage, LogStreamError, LogTag, attach_log_stream,
        attach_log_stream_default,
    };
}

/// Android platform bridge exports for the native host runtime.
#[cfg(target_os = "android")]
#[path = "ffi/android.rs"]
pub mod android;

/// Apple platform bridge exports for iOS and macOS hosts.
// The swift_bridge `AppUiEventType` host-UI events intentionally share the
// `*Click` postfix (PanelIconClick / UpdateRestartClick / UpdateInstallClick).
#[cfg(any(target_os = "ios", target_os = "macos"))]
#[path = "ffi/apple.rs"]
#[allow(clippy::enum_variant_names)]
pub mod apple;

/// HarmonyOS platform bridge exports for the native host runtime.
#[cfg(target_env = "ohos")]
#[path = "ffi/harmony.rs"]
pub mod harmony;

/// Windows platform bootstrap for pure Rust host apps.
#[cfg(target_os = "windows")]
pub mod windows;

pub(crate) mod browser;
pub(crate) mod push;
pub(crate) use bootstrap::init_with_platform;

/// WebView debugging (inspectable) policy: on only for an active `lingxia dev`
/// session, so release/production builds are never inspectable. `is_dev_session`
/// covers both the `LINGXIA_DEV_WS_URL` env var and `app.json`'s `dev_ws_url`.
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "macos",
    target_env = "ohos"
))]
pub(crate) fn should_enable_webview_debugging() -> bool {
    lxapp::is_dev_session()
}
