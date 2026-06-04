//! LingXia host framework entry crate.
//!
//! Use this crate from native host apps and native Rust libraries. It provides:
//!
//! - platform bootstrap and FFI entry points for Android, Apple platforms, and
//!   HarmonyOS;
//! - the [`native`] macro for page-facing Rust APIs;
//! - host addon registration through [`HostAddon`] and [`register_host_addon`];
//! - native service facades such as [`app`], [`mod@file`], [`media`], [`task`],
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
        LxAppDevConfig, LxAppDevIdentity, LxAppDevPageInfo, install_lxapp_dev_config,
        install_lxapp_dev_config_from_env, list_app_windows, lxapp_dev_page_back,
        lxapp_dev_page_click, lxapp_dev_page_current, lxapp_dev_page_eval, lxapp_dev_page_fill,
        lxapp_dev_page_info, lxapp_dev_page_input_supported, lxapp_dev_page_list,
        lxapp_dev_page_press, lxapp_dev_page_query, lxapp_dev_page_screenshot, lxapp_dev_page_type,
        take_app_screenshot,
    };
}
#[cfg(feature = "devtool")]
mod devtool;
mod error;
/// File dialogs and host file-manager integrations.
pub mod file;
mod host_addon;
/// JS AppService extension registration helpers.
#[cfg(feature = "standard")]
pub mod js;
mod logging;
/// Media, camera, scanner, and media-preview helpers.
pub mod media;
/// Network status and change subscriptions (typed wrapper around the
/// platform `Network` trait).
pub mod network;
/// Provider traits and registration helpers.
pub mod provider;
/// Shared async task helpers backed by LingXia's global executor.
pub mod task;
/// Terminal backend status and integration helpers.
#[cfg(feature = "terminal-runtime")]
pub mod terminal {
    pub use lingxia_terminal::{
        BackendStatus, TerminalBackend, ghostty_available, ghostty_status, ghostty_status_json,
        terminal_close, terminal_create, terminal_exited, terminal_read, terminal_resize,
        terminal_snapshot, terminal_write,
    };
}
/// Host app update helpers and update event types.
pub mod update;

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
#[cfg(any(target_os = "ios", target_os = "macos"))]
#[path = "ffi/apple.rs"]
pub mod apple;

/// HarmonyOS platform bridge exports for the native host runtime.
#[cfg(target_env = "ohos")]
#[path = "ffi/harmony.rs"]
pub mod harmony;

pub(crate) mod browser;
pub(crate) mod push;
pub(crate) use bootstrap::init_with_platform;
