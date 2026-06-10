//! LingXia host framework entry crate.
//!
//! Use this crate from native host apps and native Rust libraries. It provides:
//!
//! - platform bootstrap and FFI entry points for Android, Apple platforms, and
//!   HarmonyOS;
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
        LxAppDevConfig, LxAppDevIdentity, LxAppDevPageInfo, install_lxapp_dev_config,
        install_lxapp_dev_config_from_env, list_app_windows, lxapp_dev_nav_back,
        lxapp_dev_nav_redirect, lxapp_dev_nav_relaunch, lxapp_dev_nav_switch_tab, lxapp_dev_nav_to,
        lxapp_dev_page_back, lxapp_dev_page_click, lxapp_dev_page_current, lxapp_dev_page_eval,
        lxapp_dev_page_fill, lxapp_dev_page_info, lxapp_dev_page_input_supported,
        lxapp_dev_page_list, lxapp_dev_page_press, lxapp_dev_page_query, lxapp_dev_page_screenshot,
        lxapp_dev_page_type, perform_app_mouse, take_app_screenshot,
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
        BackendStatus, TerminalBackend, ghostty_available, ghostty_status, ghostty_status_json,
        terminal_close, terminal_create, terminal_exited, terminal_read, terminal_resize,
        terminal_snapshot, terminal_write,
    };
}
/// Host app update helpers and update event types.
pub mod update;
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
#[cfg(any(target_os = "ios", target_os = "macos"))]
#[path = "ffi/apple.rs"]
pub mod apple;

/// HarmonyOS platform bridge exports for the native host runtime.
#[cfg(target_env = "ohos")]
#[path = "ffi/harmony.rs"]
pub mod harmony;

/// Windows platform bootstrap for pure Rust host apps.
#[cfg(target_os = "windows")]
pub mod windows {
    mod shell;

    #[cfg(feature = "terminal-runtime")]
    use serde::Deserialize;
    #[cfg(feature = "terminal-runtime")]
    use std::collections::HashMap;
    use std::path::Path;
    #[cfg(feature = "terminal-runtime")]
    use std::sync::Arc;
    #[cfg(feature = "terminal-runtime")]
    use std::sync::atomic::{AtomicBool, Ordering};
    #[cfg(feature = "terminal-runtime")]
    use std::sync::{Mutex, OnceLock};
    #[cfg(feature = "terminal-runtime")]
    use std::thread;
    #[cfg(feature = "terminal-runtime")]
    use std::time::Duration;

    pub use lingxia_platform::Platform;
    use lingxia_platform::traits::app_runtime::AppRuntime;

    #[cfg(feature = "terminal-runtime")]
    struct WindowsTerminalPanelSession {
        session_id: u64,
        stop: Arc<AtomicBool>,
    }

    #[cfg(feature = "terminal-runtime")]
    #[derive(Deserialize)]
    struct WindowsTerminalSnapshot {
        lines: Vec<String>,
        exited: bool,
        title: Option<String>,
        process_title: Option<String>,
    }

    #[cfg(feature = "terminal-runtime")]
    static WINDOWS_TERMINAL_PANELS: OnceLock<Mutex<HashMap<String, WindowsTerminalPanelSession>>> =
        OnceLock::new();

    pub fn init(platform: Platform) -> Option<String> {
        crate::logging::init();
        lingxia_webview::platform::windows::set_webview_user_data_dir(
            platform.app_cache_dir().join("webview2"),
        );
        shell::install();
        crate::init_with_platform(platform)
    }

    pub fn open_home_app(appid: &str) -> Result<(), String> {
        lxapp::open_lxapp(appid, lxapp::LxAppStartupOptions::new(""))
            .map(|_| ())
            .map_err(|err| err.to_string())
    }

    pub fn set_app_icon_from_path(path: &Path) -> Result<(), String> {
        lingxia_webview::platform::windows::set_app_icon_from_path(path)
            .map_err(|err| err.to_string())
    }

    fn open_windows_terminal_panel(
        panel_id: &str,
        title: &str,
        position: lingxia_webview::platform::windows::WindowsPanelPosition,
    ) -> Result<(), String> {
        #[cfg(feature = "terminal-runtime")]
        {
            if crate::terminal::ghostty_available() {
                return open_windows_terminal_session_panel(panel_id, title, position);
            }
            lingxia_webview::platform::windows::show_native_terminal_panel(
                panel_id,
                title,
                terminal_panel_status_text(),
                position,
            )
            .map_err(|err| err.to_string())
        }
        #[cfg(not(feature = "terminal-runtime"))]
        {
            lingxia_webview::platform::windows::show_native_terminal_panel(
                panel_id,
                title,
                terminal_panel_status_text(),
                position,
            )
            .map_err(|err| err.to_string())
        }
    }

    fn close_windows_terminal_panel(panel_id: &str) -> Result<(), String> {
        #[cfg(feature = "terminal-runtime")]
        {
            lingxia_webview::platform::windows::clear_native_panel_input_handler(panel_id);
            if let Some(session) = WINDOWS_TERMINAL_PANELS
                .get()
                .and_then(|panels| panels.lock().ok())
                .and_then(|mut panels| panels.remove(panel_id))
            {
                session.stop.store(true, Ordering::Release);
                crate::terminal::terminal_close(session.session_id);
            }
        }
        lingxia_webview::platform::windows::hide_native_panel(panel_id)
            .map_err(|err| err.to_string())
    }

    fn terminal_panel_status_text() -> &'static str {
        #[cfg(feature = "terminal-runtime")]
        {
            if crate::terminal::ghostty_available() {
                "Starting terminal..."
            } else {
                "Terminal runtime is not available"
            }
        }
        #[cfg(not(feature = "terminal-runtime"))]
        {
            "Terminal runtime is disabled"
        }
    }

    #[cfg(feature = "terminal-runtime")]
    fn open_windows_terminal_session_panel(
        panel_id: &str,
        title: &str,
        position: lingxia_webview::platform::windows::WindowsPanelPosition,
    ) -> Result<(), String> {
        close_existing_windows_terminal_session(panel_id);
        let session_id = crate::terminal::terminal_create(100, 24);
        if session_id == 0 {
            return lingxia_webview::platform::windows::show_native_terminal_panel(
                panel_id,
                title,
                "Terminal failed to start",
                position,
            )
            .map_err(|err| err.to_string());
        }

        lingxia_webview::platform::windows::show_native_terminal_panel(
            panel_id,
            title,
            "Starting terminal...",
            position,
        )
        .map_err(|err| err.to_string())?;

        let write_session_id = session_id;
        lingxia_webview::platform::windows::set_native_panel_input_handler(
            panel_id,
            Arc::new(move |input| {
                let _ = crate::terminal::terminal_write(write_session_id, &input);
            }),
        );

        let stop = Arc::new(AtomicBool::new(false));
        let panel_key = panel_id.to_string();
        WINDOWS_TERMINAL_PANELS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .map_err(|_| "Windows terminal panel registry is unavailable".to_string())?
            .insert(
                panel_key.clone(),
                WindowsTerminalPanelSession {
                    session_id,
                    stop: Arc::clone(&stop),
                },
            );

        thread::spawn(move || {
            let mut last_body = String::new();
            loop {
                if stop.load(Ordering::Acquire) {
                    break;
                }
                let snapshot_json = crate::terminal::terminal_snapshot(session_id);
                let body = windows_terminal_snapshot_body(&snapshot_json);
                if body != last_body {
                    let _ = lingxia_webview::platform::windows::update_native_panel_body(
                        &panel_key, &body,
                    );
                    last_body = body;
                }
                if crate::terminal::terminal_exited(session_id) {
                    let _ = lingxia_webview::platform::windows::update_native_panel_body(
                        &panel_key,
                        &format!("{last_body}\n[process exited]"),
                    );
                    break;
                }
                thread::sleep(Duration::from_millis(80));
            }
        });

        Ok(())
    }

    #[cfg(feature = "terminal-runtime")]
    fn close_existing_windows_terminal_session(panel_id: &str) {
        lingxia_webview::platform::windows::clear_native_panel_input_handler(panel_id);
        if let Some(session) = WINDOWS_TERMINAL_PANELS
            .get()
            .and_then(|panels| panels.lock().ok())
            .and_then(|mut panels| panels.remove(panel_id))
        {
            session.stop.store(true, Ordering::Release);
            crate::terminal::terminal_close(session.session_id);
        }
    }

    #[cfg(feature = "terminal-runtime")]
    fn windows_terminal_snapshot_body(snapshot_json: &str) -> String {
        match serde_json::from_str::<WindowsTerminalSnapshot>(snapshot_json) {
            Ok(snapshot) => {
                let mut lines = snapshot.lines;
                while lines.last().is_some_and(|line| line.trim().is_empty()) {
                    lines.pop();
                }
                if lines.is_empty() {
                    let title = snapshot
                        .title
                        .or(snapshot.process_title)
                        .filter(|title| !title.trim().is_empty())
                        .unwrap_or_else(|| "terminal".to_string());
                    if snapshot.exited {
                        format!("{title}\n[process exited]")
                    } else {
                        title
                    }
                } else if snapshot.exited {
                    format!("{}\n[process exited]", lines.join("\n"))
                } else {
                    lines.join("\n")
                }
            }
            Err(_) => snapshot_json.to_string(),
        }
    }
}

pub(crate) mod browser;
pub(crate) mod push;
pub(crate) use bootstrap::init_with_platform;
