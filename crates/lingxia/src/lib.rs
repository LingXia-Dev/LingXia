//! LingXia framework.

extern crate self as lingxia;
pub use lingxia_app_context::{AppConfig, app_config, lingxia_id, product_name, product_version};
pub use lingxia_macro::{host, register_hosts};

pub use lingxia_media::{
    FrameSink, StreamError, StreamProvider, StreamSession, register_stream_provider,
};

pub use lxapp::host;
pub use lxapp::host::{ChannelContext, ChannelMessage, StreamContext};
#[doc(hidden)]
pub use lxapp::host::{HostRegistrationEntry, register_host_entry};
pub use lxapp::lx::{LxLogicExtension, register_logic_extension};
pub use lxapp::set_num_workers;
pub use lxapp::{
    BoxFuture, FingerprintProvider, LxApp, NoOpProvider, Provider, ProviderError,
    PushNotificationProvider, UpdatePackageInfo, UpdateProvider, UpdateTarget, register_provider,
};

mod bootstrap;

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
