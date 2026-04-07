//! LingXia framework.

extern crate self as lingxia;
pub use host_addon::{HostAddon, install_host_addon};
pub use lingxia_app_context::{
    AppConfig, app_config, app_state_dir, lingxia_id, product_name, product_version,
};
pub use lingxia_macro::{host, register_hosts};
pub use lingxia_platform as platform;

pub use lingxia_media::{
    FrameSink, StreamError, StreamProvider, StreamSession, register_stream_provider,
};

pub use lingxia_observability::{
    DEFAULT_DEVTOOLS_RECENT_LIMIT, DEFAULT_LOG_HISTORY_CAPACITY, DEFAULT_LOG_LIVE_CAPACITY,
    LogBuffer, LogBufferConfig, LogProvider,
};
pub use lingxia_provider::{
    BoxFuture, FingerprintProvider, ProviderError, ProviderErrorCode, PushNotificationProvider,
};
pub use lingxia_update::{
    LxAppUpdateQuery, ReleaseType, RuntimeCompatibilityError, SemanticVersion, UpdatePackageInfo,
    UpdateProvider, UpdateTarget, Version, VersionError,
};
pub use lxapp::host;
pub use lxapp::host::{ChannelContext, ChannelMessage, StreamContext};
#[doc(hidden)]
pub use lxapp::host::{HostRegistrationEntry, register_host_entry};
pub use lxapp::lx::{LxLogicExtension, register_logic_extension};
pub use lxapp::set_num_workers;
pub use lxapp::{
    LxApp, NoOpProvider, Provider, ProviderErrorExt, register_log_provider, register_provider,
};

mod bootstrap;
mod host_addon;
mod logging;

pub mod log {
    pub use crate::logging::{DownstreamLoggerError, register_downstream_logger};
    pub use lingxia_observability::{
        DEFAULT_DEVTOOLS_RECENT_LIMIT, DEFAULT_LOG_HISTORY_CAPACITY, DEFAULT_LOG_LIVE_CAPACITY,
        LogBuffer, LogBufferConfig,
    };
    pub use lxapp::log::{
        AttachedLogStream, CollectedLogArchive, CollectedLogArchiveInfo, LogLevel, LogManager,
        LogMessage, LogStreamError, LogTag, attach_log_stream, attach_log_stream_default,
        tracing_layer, upload_collected_logs,
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
#[doc(hidden)]
pub use tokio;
