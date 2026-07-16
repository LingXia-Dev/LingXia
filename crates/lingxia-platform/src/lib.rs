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

pub(crate) mod rt;
pub mod traits;

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
/// (`lingxia-lxapp`), `lx.app.getBaseInfo().os`, and `lx.getDeviceInfo().osName`
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

/// Whether launch-at-startup can actually work on this host, probed at
/// runtime. macOS builds target 12 but SMAppService needs 13+, so the
/// `lx.app.autostart` member must not be registered from a compile-time
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
    has_telephony_feature, init_cached_class, initialize_jni, read_external_storage_text,
    write_external_storage_text,
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
    WindowsVideoCommandDispatcher, apply_staged_windows_update, install_windows_aside_panel_bridge,
    register_windows_media_preview_host, register_windows_video_command_dispatcher,
    set_windows_activator_items_handler, set_windows_app_exit_handler,
    set_windows_layout_plan_handler, set_windows_managed_aside_event_handler,
    set_windows_managed_surface_toggle_handler, set_windows_managed_surface_visible_handler,
    set_windows_open_url_handler, set_windows_page_visibility_handler,
    set_windows_pull_to_refresh_handler, set_windows_shell_native_handlers,
    set_windows_shell_pins_handler, set_windows_surface_closed_handler,
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
