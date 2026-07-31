//! LingXia Platform
//!
//! This crate provides the platform-specific implementation for LingXia.

use std::io::Read;

/// Asset file entry with reader for streaming content
pub struct AssetFileEntry<'a> {
    pub path: String,
    pub reader: Box<dyn Read + 'a>,
}

/// Device information
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub brand: String,
    pub model: String,
    pub market_name: String,
    pub os_name: String,
    pub os_version: String,
}

/// Screen information reported in logical pixels (dp/pt) and scale factor
#[derive(Debug, Clone, serde::Serialize)]
pub struct ScreenInfo {
    pub width: f64,
    pub height: f64,
    pub scale: f64,
}

mod banner_background;
pub mod control_session;
pub(crate) mod rt;
pub mod traits;

/// Independent realtime capture contract. Not an `AppRuntime` supertrait.
#[cfg(feature = "capture-contract")]
pub mod capture;

#[cfg(target_os = "android")]
mod android;

#[cfg(any(target_os = "ios", target_os = "macos"))]
mod apple;

#[cfg(target_env = "ohos")]
pub mod harmony;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(not(any(
    target_os = "android",
    target_os = "ios",
    target_os = "macos",
    target_os = "windows",
    target_env = "ohos"
)))]
mod unsupported;

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub mod desktop;

/// Canonical platform-family label — the single source of truth for "which
/// OS is this," shared by the WebView bridge config injection
/// (`lingxia-lxapp`), `lx.host.getBaseInfo().os`, and `lx.getDeviceInfo().osName`
/// (`lingxia-logic`) so the three can never drift apart. Matches the values
/// the View-side bridge already exposes via `usePlatform().os`.
pub fn os_label() -> &'static str {
    #[cfg(any(target_os = "ios", target_os = "macos"))]
    {
        if cfg!(target_os = "macos") {
            "macOS"
        } else {
            "iOS"
        }
    }
    #[cfg(target_os = "android")]
    {
        "Android"
    }
    #[cfg(target_os = "windows")]
    {
        "Windows"
    }
    #[cfg(all(target_os = "linux", target_env = "ohos"))]
    {
        "Harmony"
    }
    #[cfg(not(any(
        target_os = "ios",
        target_os = "macos",
        target_os = "android",
        target_os = "windows",
        all(target_os = "linux", target_env = "ohos"),
    )))]
    {
        "unknown"
    }
}

/// Local notifications are implemented on every LingXia host. Presence of
/// `lx.host.notification` also requires the declared yaml capability.
#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    target_os = "ios",
    target_os = "android",
    all(target_os = "linux", target_env = "ohos"),
))]
pub fn notification_supported() -> bool {
    true
}

/// Desktop banner is a product-drawn overlay, not an OS notification.
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn banner_supported() -> bool {
    true
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn banner_supported() -> bool {
    false
}

/// The product-owned chrome this platform can actually paint a count on.
///
/// `lx.host.setBadge` skips unsupported surfaces and returns `false` when
/// none can be painted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BadgeSurfaces {
    /// Dock (macOS), taskbar (Windows), home-screen icon (iOS, HarmonyOS).
    pub app_icon: bool,
    /// Menu-bar (macOS) / notification-area (Windows) status item.
    pub tray: bool,
    /// The platform draws the badge itself and only understands a count, so a
    /// non-numeric value is a parameter error rather than a silent clear.
    pub numeric_only: bool,
}

/// Android is deliberately absent: there is no cross-vendor launcher badge.
/// What a launcher shows comes from active notifications, so a standalone
/// count is not something the platform can honour — and claiming it, then
/// no-opping, is what this reports instead of.
pub fn badge_surfaces() -> BadgeSurfaces {
    #[cfg(target_os = "macos")]
    {
        BadgeSurfaces {
            app_icon: true,
            tray: true,
            numeric_only: false,
        }
    }
    #[cfg(target_os = "ios")]
    {
        // The home-screen badge is drawn by the notification system: it takes
        // a count, and only with notification permission.
        BadgeSurfaces {
            app_icon: true,
            tray: false,
            numeric_only: true,
        }
    }
    #[cfg(target_os = "windows")]
    {
        // Taskbar overlay only. The notify-area icon lives in the host SDK and
        // would need its own compositing path; claiming it here before that
        // exists is exactly the lie this type is for.
        BadgeSurfaces {
            app_icon: true,
            tray: false,
            numeric_only: true,
        }
    }
    #[cfg(target_env = "ohos")]
    {
        BadgeSurfaces {
            app_icon: true,
            tray: false,
            numeric_only: true,
        }
    }
    #[cfg(not(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "windows",
        target_env = "ohos"
    )))]
    {
        BadgeSurfaces::default()
    }
}

/// Whether launch-at-startup can actually work on this host, probed at
/// runtime. macOS builds target 12 but SMAppService needs 13+, so the
/// `lx.host.autostart` member must not be registered from a compile-time
/// gate alone — presence is the JS support contract.
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn autostart_supported() -> bool {
    #[cfg(target_os = "macos")]
    {
        apple::autostart_probe_supported()
    }
    #[cfg(target_os = "windows")]
    {
        true
    }
}

#[cfg(target_os = "android")]
pub use android::{
    CachedClass, Platform, get_android_id, get_api_level, get_system_property,
    has_telephony_feature, init_cached_class, initialize_jni, present_browser_tab,
    read_external_storage_text, write_external_storage_text,
};

pub use control_session::{
    ControlSessionStopHandler, request_control_session_stop, set_control_session_stop_handler,
};

#[cfg(any(target_os = "ios", target_os = "macos"))]
pub use apple::Platform;
#[cfg(any(target_os = "ios", target_os = "macos"))]
pub use apple::apply_staged_macos_update;

#[cfg(target_env = "ohos")]
pub use harmony::Platform;

#[cfg(target_os = "windows")]
pub use windows::{
    Platform, WindowsMediaPreviewCancel, WindowsMediaPreviewOpen, WindowsUrlSurfaceWebTag,
    WindowsVideoCommandDispatcher, apply_staged_windows_update, ensure_toast_activator,
    install_windows_aside_panel_bridge, register_windows_media_preview_host,
    register_windows_video_command_dispatcher, remove_toast_registration,
    replay_windows_exclusive_update_ready, set_toast_activate_handler,
    set_windows_activate_browser_tab_handler, set_windows_app_exit_handler,
    set_windows_builtin_browser_downloads_handler, set_windows_capsule_rect_provider,
    set_windows_close_browser_tab_handler, set_windows_exclusive_update_ready_handler,
    set_windows_home_first_ready_handler, set_windows_host_appearance_dark,
    set_windows_host_color_mode_handler, set_windows_layout_plan_handler,
    set_windows_lxapp_hidden_handler, set_windows_lxapp_main_activation_handler,
    set_windows_managed_aside_event_handler, set_windows_managed_native_surface_open_handler,
    set_windows_managed_surface_close_handler, set_windows_managed_surface_visible_handler,
    set_windows_open_url_handler, set_windows_page_visibility_handler,
    set_windows_pull_to_refresh_handler, set_windows_shell_pins_handler,
    set_windows_sidebar_actions_handler, set_windows_surface_closed_handler,
    set_windows_surface_dispose_handler, set_windows_tray_click_intercept_handler,
    set_windows_tray_menu_handler, set_windows_ui_update_async_handler,
    set_windows_ui_update_handler, set_windows_url_surface_handler, sync_windows_ui,
};

#[cfg(not(any(
    target_os = "android",
    target_os = "ios",
    target_os = "macos",
    target_os = "windows",
    target_env = "ohos"
)))]
pub use unsupported::Platform;

pub mod error;
pub use error::*;

pub mod i18n;

#[cfg(all(test, not(windows)))]
#[path = "windows/update/installer.rs"]
mod windows_update_installer_tests;
