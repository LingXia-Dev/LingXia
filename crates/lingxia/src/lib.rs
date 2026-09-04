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
#[doc(hidden)]
pub use lingxia_native_macros::framework_native;
pub use lingxia_native_macros::native;
#[cfg(feature = "devtool")]
pub use lxapp::host::NativeDevtoolsAuthority;
pub use lxapp::host::NativeHostRuntimeAuthority;

pub use lxapp::host;
pub use lxapp::host::{ChannelContext, ChannelMessage, StreamContext};
// Required by expansions of `#[lingxia::native]`; host applications should
// receive it through macro-generated parameters rather than orchestrate it.
pub use lingxia_app_context::SettingsDestination;
#[doc(hidden)]
pub use lxapp::{AppSessionClass, LxApp};
pub use lxapp::{
    FloatDismiss, LxAppSecurityPrivilege, PageQueryInput, PageSurface, PageSurfaceRequest,
    PageSurfaceTarget, SurfaceInteraction, SurfaceKind, SurfacePosition, SurfaceRole,
    UrlCallbackSurface, UrlCallbackWaitError, WindowChrome,
};

/// Result of successfully initializing a LingXia host runtime.
///
/// A browser-only host is valid without a configured lxapp; callers can inspect
/// [`RuntimeInfo::lxapp_id`] when their launch policy requires one.
#[derive(Clone)]
pub struct RuntimeInfo {
    lxapp_id: Option<String>,
    terminal_authority: std::sync::Arc<lxapp::terminal_automation::TerminalAutomationAuthority>,
}

impl RuntimeInfo {
    pub(crate) fn new(
        lxapp_id: Option<String>,
        terminal_authority: lxapp::terminal_automation::TerminalAutomationAuthority,
    ) -> Self {
        Self {
            lxapp_id,
            terminal_authority: std::sync::Arc::new(terminal_authority),
        }
    }

    /// The configured launch lxapp id, when this host has one.
    pub fn lxapp_id(&self) -> Option<&str> {
        self.lxapp_id.as_deref()
    }

    /// Consumes the snapshot and returns its configured launch lxapp id.
    pub fn into_lxapp_id(self) -> Option<String> {
        self.lxapp_id
    }

    /// Native terminal authority for this exact successfully bootstrapped host.
    /// The capability is available only through this runtime handle; there is
    /// no process-global getter.
    #[doc(hidden)]
    pub fn terminal_automation_authority(
        &self,
    ) -> lxapp::terminal_automation::TerminalAutomationAuthority {
        self.terminal_authority.as_ref().clone()
    }

    /// Resolve the sealed host Settings destination through this initialized
    /// platform runtime handle. There is no process-global safe entrypoint.
    pub fn resolve_settings_destination(
        &self,
    ) -> std::result::Result<SettingsDestinationResolution, SettingsDestinationResolveError> {
        settings_destination::resolve_settings_destination()
    }

    #[cfg(feature = "browser-runtime")]
    #[doc(hidden)]
    pub fn open_trusted_browser_page(
        &self,
        url: &str,
        tab_id: Option<&str>,
    ) -> std::result::Result<String, lxapp::LxAppError> {
        lingxia_browser::open_trusted(crate::browser::native_control_authority()?, url, tab_id)
    }

    #[cfg(feature = "browser-runtime")]
    #[doc(hidden)]
    pub fn open_trusted_browser_page_for_app(
        &self,
        app_id: &str,
        session_id: u64,
        url: &str,
        tab_id: Option<&str>,
    ) -> std::result::Result<String, lxapp::LxAppError> {
        lingxia_browser::open_trusted_for_app(
            crate::browser::native_control_authority()?,
            app_id,
            session_id,
            url,
            tab_id,
        )
    }
}

impl std::fmt::Debug for RuntimeInfo {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeInfo")
            .field("lxapp_id", &self.lxapp_id)
            .finish_non_exhaustive()
    }
}

impl PartialEq for RuntimeInfo {
    fn eq(&self, other: &Self) -> bool {
        self.lxapp_id == other.lxapp_id
            && std::sync::Arc::ptr_eq(&self.terminal_authority, &other.terminal_authority)
    }
}

impl Eq for RuntimeInfo {}

/// Explicitly installed realtime capture providers. Only compiled when the
/// host declared capture; the contract is never an `AppRuntime` supertrait.
#[cfg(feature = "realtime-capture")]
pub mod capture;

/// Isolated host-owned automation programs for trusted Agent-style products.
#[cfg(feature = "automation-runtime")]
pub mod automation_runtime {
    pub use lingxia_automation::runtime::*;
}

/// The product acting as its own command line.
///
/// One binary is the app when the OS launches it and the command line when
/// someone types it, which is what keeps the two from ever being different
/// versions of each other.
#[cfg(feature = "product-cli")]
pub mod product_cli;

/// Host app metadata, state-path helpers, and lifecycle helpers.
pub mod app;
mod terminal_automation;
pub use app::{home_app_id, lingxia_id, product_version};
mod applink;
/// Host assets packaged by the CLI (`assets:` in `lingxia.yaml`).
pub mod assets;
mod bootstrap;
mod capabilities;
mod display_language_host;
pub mod splash;
/// LxApp devtool helpers for host-side inspection and automation.
#[cfg(feature = "devtool")]
pub mod dev {
    pub use crate::devtool::{
        Appearance, DeviceController, DeviceEntry, DeviceState, LxAppDevConfig, LxAppDevIdentity,
        LxAppDevPageInfo, LxAppDevPageWaitResult, LxAppDevPageWaitState, device_get, device_list,
        device_set, install_lxapp_dev_config, install_lxapp_dev_config_from_env, list_app_windows,
        lxapp_dev_nav_back, lxapp_dev_nav_redirect, lxapp_dev_nav_relaunch,
        lxapp_dev_nav_switch_tab, lxapp_dev_nav_to, lxapp_dev_page_back, lxapp_dev_page_click,
        lxapp_dev_page_current, lxapp_dev_page_eval, lxapp_dev_page_fill, lxapp_dev_page_info,
        lxapp_dev_page_input_supported, lxapp_dev_page_list, lxapp_dev_page_press,
        lxapp_dev_page_query, lxapp_dev_page_screenshot, lxapp_dev_page_screenshot_with_info,
        lxapp_dev_page_scroll, lxapp_dev_page_scroll_to, lxapp_dev_page_type, lxapp_dev_page_wait,
        lxapp_dev_restart, perform_app_keyboard, perform_app_mouse, register_device_controller,
        take_app_screenshot, take_app_screenshot_with_info,
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
mod settings_destination;
mod settings_target;
pub(crate) mod shell;
pub use settings_destination::{
    NativeSettingsActionRegistrar, SettingsDestinationResolution, SettingsDestinationResolveError,
};
pub use settings_target::{
    SealedNativeActionRegistry, StaticSettingsTargetCatalog, StaticSettingsTargetError,
    ValidatedStaticSettingsTargets, static_settings_destination,
};
/// Shared async task helpers backed by LingXia's global executor.
pub mod task;
#[cfg(feature = "terminal-runtime")]
#[path = "terminal_config.rs"]
mod terminal_config_impl;

/// Runs a future on LingXia's runtime, from any phase of the process.
///
/// This is the host addon's one way to hand work to LingXia: safe even
/// before the runtime is initialized (e.g. inside `HostAddon::before_init`)
/// — early work is queued and starts right after initialization. For handles, joins, and blocking work inside
/// already-running runtime code, use [`task`].
pub fn spawn<F>(future: F)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    task::spawn_or_defer(future);
}

/// Terminal backend status and integration helpers.
#[cfg(feature = "terminal-runtime")]
pub mod terminal {
    pub use crate::terminal_config_impl::{
        app_data_dir, apply_theme, current_json as config_json, generation as config_generation,
        installed_fonts, load as load_config, load_for_app, refresh_appearance_for_app,
        set_installed_fonts, visual_generation,
    };
    pub use lingxia_terminal::{
        BackendStatus, FrameCell, RowDamage, TerminalBackend, TerminalCell, TerminalFrame,
        TerminalFrameView, TerminalSessionSpec, TerminalSnapshot, TerminalTheme, backend_available,
        backend_status, backend_status_json, terminal_close, terminal_create, terminal_create_at,
        terminal_create_with_spec, terminal_current_directory, terminal_exited,
        terminal_frame_view, terminal_image_snapshot, terminal_read, terminal_resize,
        terminal_resize_pixels, terminal_scroll, terminal_scroll_to_line, terminal_search,
        terminal_set_theme, terminal_set_theme_all, terminal_snapshot, terminal_snapshot_data,
        terminal_title_state_json, terminal_write,
    };
    pub use lingxia_terminal_config::{
        FontConfig, InstalledFont, ResolvedFont, TerminalConfig, ThemeConfig, ThemeDetails,
        ThemeMode, ThemeStore, resolve_font,
    };
}
/// Host app update helpers and update event types.
pub mod update;
/// Process-local URL callback channels for native handoff flows.
pub mod url_callback;
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "macos",
    target_env = "ohos"
))]
mod webview_error;
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
