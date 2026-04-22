//! LingXia framework.

extern crate self as lingxia;
pub use host_addon::{HostAddon, register_host_addon};
pub use lingxia_native_macros::native;

pub use lxapp::LxApp;
pub use lxapp::host;
pub use lxapp::host::{ChannelContext, ChannelMessage, StreamContext};
pub use lxapp::set_num_workers;

pub mod app;
mod bootstrap;
mod capabilities;
pub mod dev {
    pub use crate::lxapp_dev::{
        LxAppDevConfig, LxAppDevIdentity, LxAppDevPageInfo, install_lxapp_dev_config,
        install_lxapp_dev_config_from_env, lxapp_dev_page_back, lxapp_dev_page_click,
        lxapp_dev_page_current, lxapp_dev_page_eval, lxapp_dev_page_fill, lxapp_dev_page_info,
        lxapp_dev_page_input_supported, lxapp_dev_page_list, lxapp_dev_page_press,
        lxapp_dev_page_query, lxapp_dev_page_type,
    };
}
pub mod downloads;
mod error;
pub mod file;
mod host_addon;
#[cfg(feature = "js-lxapp")]
pub mod js;
mod logging;
mod lxapp_dev;
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
