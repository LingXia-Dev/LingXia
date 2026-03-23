mod app;
mod appservice;
mod archive;
pub mod browser;
mod cache;
mod config;
mod delegate;
pub mod download_manager;
mod error;
mod executor;
pub(crate) mod host;
pub mod key_event;
pub mod lifecycle;
pub mod log;
pub mod lx;
mod lxapp;
mod page;
pub mod panel;
pub(crate) mod plugin;
pub mod provider;
pub mod push_notification;
mod route;
pub mod startup;
pub mod stream_source;
mod update;

/// SDK/runtime version of lingxia-lxapp.
/// This is used for update compatibility checks and can be reported to update services.
pub const SDK_RUNTIME_VERSION: &str = env!("CARGO_PKG_VERSION");

pub use app::{app_config, lingxia_id, product_name, product_version};
pub use appservice::PageSvc;
pub use appservice::event_bus::{
    publish_app_event, publish_page_event, register_app_handler, register_page_handler,
    unregister_app_handler, unregister_page_handler,
};
pub use appservice::native_component::on_native_component_event;
pub use browser::{
    BUILTIN_BROWSER_APPID, BrowserTabInfo, browser_download_dir, browser_tab_exists,
    browser_tab_info, browser_tab_info_json, browser_tab_infos, browser_tab_infos_json,
    browser_tab_path_for_id, browser_update_tab_info, browser_url_is_hidden, close_browser_tab,
    close_internal_browser_tab, generate_browser_startup_html, handle_browser_address_input,
    handle_browser_address_input_json, handle_browser_navigation_policy_json,
    open_internal_browser_tab, reset_browser_download_dir, resolve_owner_lxapp,
    set_browser_download_dir,
};
pub use cache::{LxAppCache, ResolveResult};
pub use delegate::{LxAppDelegate, LxAppUiEventType};
pub use error::LxAppError;
pub use lifecycle::{
    AppServiceEvent, AppServiceEventArgs, AppServiceEventReason, AppServiceEventSource,
    LxAppLifecycleEvent, PageLifecycleEvent, PageServiceEvent,
};
pub use lxapp::set_home_lxapp_dev_path;
pub use lxapp::set_num_workers;
pub use lxapp::{
    LxApp, PopupMode, ReleaseType, config::LxAppInfo, get_current_lxapp, get_locale, get_platform,
    init, is_pull_down_refresh_enabled, on_low_memory, page_config::OrientationConfig,
    page_config::PageOrientation, tabbar, try_get,
};
pub use page::{NavigationType, Page, ViewCallOptions};
pub use panel::{open_lxapp_for_panel, panel_item_for_id, panels_config_json};
pub use plugin::{build_plugin_page_path, parse_plugin_page_path, parse_plugin_url};
pub use provider::{
    BoxFuture, FingerprintProvider, LxAppUpdateQuery, NoOpProvider, Provider, ProviderError,
    ProviderErrorCode, PushNotificationProvider, UpdatePackageInfo, UpdateProvider, UpdateTarget,
    register_provider,
};
pub use startup::{LxAppStartupOptions, Scene, parse_env_release_type};
pub use stream_source::{
    FrameSink, StreamError, StreamProvider, StreamSession, register_stream_provider,
};
pub use update::{
    DownloadedUpdateInfo, OtaUpdateTarget, UpdateManager, ensure_force_update_for_installed,
    is_force_update_downloading,
};

// Re-export for internal crate usage
pub(crate) use provider::get_provider;
