//! LingXia host framework entry crate.
//!
//! Use this crate from native host apps and native Rust libraries. It provides:
//!
//! - platform bootstrap and FFI entry points for Android, Apple platforms, and
//!   HarmonyOS;
//! - the [`native`] macro for page-facing Rust APIs;
//! - host addon registration through [`HostAddon`] and [`register_host_addon`];
//! - native service facades such as [`app`], [`mod@file`], [`media`],
//!   [`downloads`], [`settings`], [`push`], [`task`], and [`update`];
//! - optional JS AppService extension APIs under [`js`] when the `standard`
//!   feature is enabled;
//! - optional devtool helpers under [`dev`] when the `devtool` feature is
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
pub use lxapp::set_num_workers;
pub use lxapp::{
    CloseReason, CreatePageInstanceRequest, CreatedPageInstance, LxApp, PageDefinition,
    PageInstance, PageInstanceEvent, PageInstanceId, PageOwner, PageQueryInput, PageTarget,
    PageWarmDisposePolicy, PresentationKind, ResolvedPage, SceneId, ViewCallOptions,
    create_page_instance, dispose_page_instance, dispose_page_instance_by_id, notify_page_instance,
    notify_page_instance_by_id, touch_page_instance_by_id,
};

pub mod app;
pub use app::{config as app_config, lingxia_id, product_version};
mod applink;
mod bootstrap;
mod capabilities;
#[cfg(feature = "devtool")]
pub mod dev {
    pub use crate::devtool::{
        LxAppDevConfig, LxAppDevIdentity, LxAppDevPageInfo, install_lxapp_dev_config,
        install_lxapp_dev_config_from_env, lxapp_dev_page_back, lxapp_dev_page_click,
        lxapp_dev_page_current, lxapp_dev_page_eval, lxapp_dev_page_fill, lxapp_dev_page_info,
        lxapp_dev_page_input_supported, lxapp_dev_page_list, lxapp_dev_page_press,
        lxapp_dev_page_query, lxapp_dev_page_type,
    };
}
#[cfg(feature = "devtool")]
mod devtool;
pub mod downloads;
mod error;
pub mod file;
mod host_addon;
#[cfg(feature = "standard")]
pub mod js;
mod logging;
pub mod media;
pub mod provider;
pub mod settings;
pub mod task;
pub mod update;

pub use error::{Error, Result};

pub mod log {
    pub use crate::logging::{DownstreamLoggerError, register_downstream_logger};
    pub use lingxia_log::{
        AttachedLogStream, CollectedLogArchive, CollectedLogArchiveInfo,
        DEFAULT_LOG_HISTORY_CAPACITY, DEFAULT_LOG_LIVE_CAPACITY, LogBuffer, LogBufferConfig,
        LogLevel, LogManager, LogMessage, LogStreamError, LogTag, attach_log_stream,
        attach_log_stream_default, register_log_provider, tracing_layer, upload_collected_logs,
    };
}

#[cfg(target_os = "android")]
pub mod android;

#[cfg(any(target_os = "ios", target_os = "macos"))]
pub mod apple;

#[cfg(target_env = "ohos")]
pub mod harmony;

pub(crate) mod browser;
pub mod push;
pub(crate) use bootstrap::init_with_platform;
