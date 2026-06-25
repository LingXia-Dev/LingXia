//! Windows host SDK for LingXia.
//!
//! Two usage modes, selected by Cargo features:
//!
//! - **quick-start** (`standard` / `browser-shell`, the default): batteries
//!   included. [`quick_start`] boots the runtime via [`init_runtime`] and pumps
//!   the Win32 message loop until the app exits — a pure Rust Windows executable
//!   needs nothing else.
//! - **advanced** (`components`): the SDK provides only the embeddable view
//!   components + native capabilities. The host brings its own window and
//!   message loop, registers its own `WindowsHostBackend`, and drives the
//!   components itself. This tier does not pull the [`lingxia`] runtime facade.
//!
//! [`init_runtime`] and [`run_message_loop`] must be called on the same thread:
//! the message loop installs an exit handler that posts `WM_QUIT` to the thread
//! it runs on, and the webview windows created during [`init_runtime`] are
//! serviced by that thread's message queue. [`quick_start`] performs both steps
//! in order on the calling thread.

#[cfg(feature = "runtime")]
use std::path::{Path, PathBuf};

#[cfg(all(target_os = "windows", feature = "components"))]
mod app_icon;
#[cfg(all(target_os = "windows", feature = "components"))]
mod app_menu;
#[cfg(all(target_os = "windows", feature = "components"))]
mod design_icons;
#[cfg(all(target_os = "windows", feature = "device-frame"))]
mod device_frame;
#[cfg(all(target_os = "windows", feature = "components"))]
mod media_preview;
#[cfg(all(target_os = "windows", feature = "components"))]
mod native_components;
#[cfg(all(target_os = "windows", feature = "components"))]
mod pull_to_refresh;
#[cfg(all(target_os = "windows", feature = "shell-chrome"))]
mod shell;
#[cfg(all(target_os = "windows", feature = "browser-shell"))]
mod tray_icon;
#[cfg(all(target_os = "windows", feature = "components"))]
mod video_controls;
#[cfg(all(target_os = "windows", feature = "components"))]
mod video_player;
#[cfg(all(target_os = "windows", feature = "host-api"))]
pub mod window_host;

#[cfg(all(target_os = "windows", feature = "components"))]
pub use app_menu::{
    WindowsAppMenu, WindowsAppMenuCommandHandler, WindowsAppMenuEntry, WindowsAppMenuItem,
    set_windows_app_menu, set_windows_app_menu_command_handler,
};
#[cfg(all(target_os = "windows", feature = "components"))]
pub use design_icons::{
    WindowsDesignIcon, draw_windows_design_icon, draw_windows_design_icon_with_color,
    set_windows_design_icon_dir,
};
#[cfg(all(
    target_os = "windows",
    feature = "device-frame",
    feature = "shell-chrome"
))]
pub use device_frame::set_app_window_device_frame_and_tabbar_position;
#[cfg(all(target_os = "windows", feature = "device-frame"))]
pub use device_frame::{
    WindowsDeviceFrame, WindowsDeviceFrameBadge, WindowsDeviceFrameCutout,
    WindowsDeviceFrameInfoSheet, WindowsDeviceFrameSheetAction, WindowsDeviceFrameStatusBar,
    WindowsDeviceFrameToolbar, open_current_page_devtools, set_app_window_device_frame,
    set_initial_app_window_device_frame, show_device_frame_info_sheet,
};
#[cfg(all(target_os = "windows", feature = "shell-chrome"))]
pub use shell::{
    WindowsShellTabBarPosition, set_windows_default_shell_tabbar_position,
    set_windows_shell_tabbar_position, windows_shell_background_color,
};

/// Host process description used to initialize the LingXia runtime.
///
/// Construct it with [`WindowsApp::from_env`] before passing it to
/// [`init_runtime`]. App identity comes from generated assets, not from host
/// entry code.
#[derive(Debug, Clone)]
#[cfg(feature = "runtime")]
pub struct WindowsApp {
    pub(crate) window_size: Option<(i32, i32)>,
}

#[cfg(feature = "runtime")]
impl WindowsApp {
    /// Creates an app description from the process environment.
    ///
    /// App identity and state directories are resolved by the platform layer
    /// from generated `assets/app.json`, which is derived from `lingxia.toml`.
    pub fn from_env() -> Self {
        Self { window_size: None }
    }

    /// Sets the initial outer size, in pixels, of the app's webview windows,
    /// in particular the main window opened for the home lxapp.
    ///
    /// When unset, windows open at the runtime default (1024x768). Users can
    /// still resize the window afterwards; non-positive dimensions are
    /// ignored by the runtime.
    pub fn with_window_size(mut self, width: i32, height: i32) -> Self {
        self.window_size = Some((width, height));
        self
    }
}

/// Errors surfaced while bootstrapping the Windows host.
#[derive(Debug, thiserror::Error)]
#[cfg(feature = "runtime")]
pub enum WindowsHostError {
    /// The LingXia platform layer failed to initialize.
    #[cfg(all(target_os = "windows", feature = "runtime"))]
    #[error(transparent)]
    Platform(#[from] lingxia::windows::PlatformError),
    /// The host crate was built for a target other than Windows.
    #[cfg(not(target_os = "windows"))]
    #[error("{0}")]
    Platform(String),
    /// The runtime initialized but did not report a home app id.
    #[error("LingXia runtime did not return a home app id")]
    MissingHomeApp,
    /// The home lxapp could not be opened.
    #[error("failed to open home lxapp: {0}")]
    OpenHomeApp(String),
    /// The window icon could not be loaded from the resolved path.
    #[error("failed to set Windows app icon from {path:?}: {message}")]
    AppIcon { path: PathBuf, message: String },
}

/// Convenience alias for results produced by this crate.
#[cfg(feature = "runtime")]
pub type Result<T> = std::result::Result<T, WindowsHostError>;

/// Boots the LingXia runtime and opens the home lxapp. **Host-agnostic**: it
/// installs no Windows host backend and opens no window.
///
/// For the batteries-included host call [`start_default_host`] (or
/// [`quick_start`]). For a custom host, register your own
/// `lingxia_windows_contract::WindowsHostBackend` (optionally alongside
/// [`install_default_windows_host`]) before calling this, then drive your own
/// Win32 message loop.
///
/// Returns the home app id. Must run on the thread that will later pump
/// messages.
#[cfg(all(target_os = "windows", feature = "runtime"))]
pub fn init_runtime(app: WindowsApp) -> Result<String> {
    if let Some((width, height)) = app.window_size {
        lingxia::windows::set_default_window_size(width, height);
    }
    let platform = lingxia::windows::Platform::from_env()?;
    lingxia::windows::init(platform).ok_or(WindowsHostError::MissingHomeApp)
}

/// Installs the SDK's default Windows host: the built-in `WindowsHostBackend`,
/// the embedded native components (input/textarea/video overlays),
/// pull-to-refresh, the app menu, and — under `browser-shell` — the native
/// shell. Registrations must happen before the first page mounts a component.
///
/// Call this for the batteries-included host; skip it and register your own
/// `WindowsHostBackend` for a custom host. [`start_default_host`] /
/// [`quick_start`] call it for you.
#[cfg(all(target_os = "windows", feature = "runtime"))]
pub fn install_default_windows_host() {
    window_host::install_native_view_host();
    native_components::install();
    pull_to_refresh::install();
    #[cfg(feature = "shell-chrome")]
    shell::install();
    app_menu::install_host_window_menu_support();
}

/// Default-host post-boot wiring: design-icon directory, window icon, taskbar
/// policy, opening the home window, and — under `browser-shell` — the tray.
#[cfg(all(target_os = "windows", feature = "runtime"))]
fn present_default_home(home_app_id: &str) -> Result<()> {
    // `Platform::from_env` only re-reads generated config (the runtime was
    // already initialized in `init_runtime`), so it is safe to rebuild here to
    // recover the asset dir without threading it through the public boot API.
    let asset_dir = lingxia::windows::Platform::from_env()?
        .asset_dir()
        .to_path_buf();
    set_windows_design_icon_dir(asset_dir.join("icons").join("design"));
    #[cfg(feature = "shell-chrome")]
    shell::set_home_app_id(home_app_id);
    if let Some(icon_path) = resolve_app_icon_path(&asset_dir, home_app_id) {
        app_icon::set_app_icon_from_path(&icon_path).map_err(|message| {
            WindowsHostError::AppIcon {
                path: icon_path,
                message,
            }
        })?;
    }
    // Tray-exclusive apps live only in the system tray, so their windows
    // must be created without a taskbar button. Apply before any window opens.
    window_host::set_hide_from_taskbar(should_hide_taskbar(&asset_dir));
    if should_open_on_launch(&asset_dir) {
        open_home_app(home_app_id).map_err(WindowsHostError::OpenHomeApp)?;
    }
    #[cfg(feature = "browser-shell")]
    {
        // Wire the cross-platform tray JS APIs to the native system-tray icon:
        // `lx.tray.setMenu` builds the right-click menu (no default items) and
        // `lx.tray.onClick` claims the left-click. The runtime layer cannot see
        // this SDK, so it invokes these registered handlers.
        lingxia_platform::set_windows_tray_menu_handler(std::sync::Arc::new(tray_icon::set_menu));
        lingxia_platform::set_windows_tray_click_intercept_handler(std::sync::Arc::new(
            tray_icon::set_click_intercept,
        ));
        if let Err(message) = tray_icon::install_from_ui(&asset_dir) {
            log::warn!("failed to install Windows tray icon: {message}");
        }
    }
    Ok(())
}

/// Brings up the batteries-included default Windows host *without* pumping the
/// message loop: installs the default host, boots the runtime, and opens the
/// home window. Returns the home app id so the caller can do further setup
/// (menus, device frame, …) before calling [`run_message_loop`] itself.
/// [`quick_start`] wraps this.
#[cfg(all(target_os = "windows", feature = "runtime"))]
pub fn start_default_host(app: WindowsApp) -> Result<String> {
    install_default_windows_host();
    // Own a message queue before any page can request exit from a WebView UI thread.
    install_current_thread_exit_handler();
    let home_app_id = init_runtime(app)?;
    present_default_home(&home_app_id)?;
    Ok(home_app_id)
}

#[cfg(all(target_os = "windows", feature = "shell-chrome"))]
fn open_home_app(appid: &str) -> std::result::Result<(), String> {
    shell::open_home_app(appid)
}

#[cfg(all(
    target_os = "windows",
    feature = "runtime",
    not(feature = "shell-chrome")
))]
fn open_home_app(appid: &str) -> std::result::Result<(), String> {
    lingxia::windows::open_home_app(appid)
}

/// Initializes the LingXia runtime and opens the home lxapp.
///
/// This non-Windows stub always fails with [`WindowsHostError::Platform`].
#[cfg(all(not(target_os = "windows"), feature = "runtime"))]
pub fn init_runtime(_app: WindowsApp) -> Result<String> {
    Err(WindowsHostError::Platform(
        "lingxia-windows-sdk can only initialize on target_os = \"windows\"".to_string(),
    ))
}

/// Brings up the default Windows host.
///
/// This non-Windows stub always fails with [`WindowsHostError::Platform`].
#[cfg(all(not(target_os = "windows"), feature = "runtime"))]
pub fn start_default_host(_app: WindowsApp) -> Result<String> {
    Err(WindowsHostError::Platform(
        "lingxia-windows-sdk can only initialize on target_os = \"windows\"".to_string(),
    ))
}

/// Runs the Win32 message loop until the application quits.
///
/// Installs the LingXia app exit handler for the calling thread and pumps
/// messages until `WM_QUIT`, returning the loop exit code. Must run on the
/// same thread that called [`init_runtime`] / [`start_default_host`].
#[cfg(all(target_os = "windows", feature = "runtime"))]
pub fn run_message_loop() -> i32 {
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, MSG, TranslateMessage,
    };

    install_current_thread_exit_handler();

    let mut msg = MSG::default();
    loop {
        let result = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        match result.0 {
            -1 => {
                #[cfg(feature = "browser-shell")]
                tray_icon::uninstall();
                return 1;
            }
            0 => {
                #[cfg(feature = "browser-shell")]
                tray_icon::uninstall();
                return msg.wParam.0 as i32;
            }
            _ => unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            },
        }
    }
}

/// Runs the Win32 message loop until the application quits.
///
/// This non-Windows stub returns immediately with exit code `0`.
#[cfg(all(not(target_os = "windows"), feature = "runtime"))]
pub fn run_message_loop() -> i32 {
    0
}

#[cfg(all(target_os = "windows", feature = "runtime"))]
fn install_current_thread_exit_handler() {
    use std::sync::Arc;

    use windows::Win32::UI::WindowsAndMessaging::{
        MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW, WM_QUIT,
    };

    // Ensure this thread owns a message queue before page code can request
    // exit from a WebView UI thread.
    let mut msg = MSG::default();
    unsafe {
        let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);
    }

    let main_thread_id = unsafe { windows::Win32::System::Threading::GetCurrentThreadId() };
    lingxia::windows::set_windows_app_exit_handler(Arc::new(move || unsafe {
        let _ = PostThreadMessageW(
            main_thread_id,
            WM_QUIT,
            Default::default(),
            Default::default(),
        );
    }));
}

/// Boots the default host from the environment and blocks until the app exits.
///
/// Equivalent to [`start_default_host`] with [`WindowsApp::from_env`] followed
/// by [`run_message_loop`] on the calling thread. Returns the message-loop exit
/// code once the application quits.
#[cfg(feature = "runtime")]
pub fn quick_start() -> Result<i32> {
    start_default_host(WindowsApp::from_env())?;
    Ok(run_message_loop())
}

#[cfg(all(target_os = "windows", feature = "runtime"))]
fn resolve_app_icon_path(asset_dir: &Path, home_app_id: &str) -> Option<PathBuf> {
    // `lingxia dev` stages a badged copy of the launcher icon and points this
    // env var at it, so dev/preview builds show the env badge without the CLI
    // mutating the prepared assets dir. Takes priority over the asset lookup.
    if let Some(path) = std::env::var_os("LINGXIA_APP_ICON_PATH").map(PathBuf::from)
        && path.is_file()
    {
        return Some(path);
    }
    [
        // Host-owned icon: the CLI stages a badged copy here for dev/preview
        // builds. Preferred over the lxapp's served public asset so env badges
        // never leak into the app's own UI (the home page renders that asset).
        asset_dir.join("AppIcon.png"),
        asset_dir
            .join(home_app_id)
            .join("public")
            .join("AppIcon.png"),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

#[cfg(feature = "runtime")]
fn should_open_on_launch(asset_dir: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(asset_dir.join("ui.json")) else {
        return true;
    };
    let Ok(ui) = serde_json::from_str::<serde_json::Value>(&text) else {
        return true;
    };
    ui.get("launch")
        .and_then(|launch| launch.get("openOnLaunch"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true)
}

#[cfg(feature = "runtime")]
fn should_hide_taskbar(asset_dir: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(asset_dir.join("ui.json")) else {
        return false;
    };
    let Ok(ui) = serde_json::from_str::<serde_json::Value>(&text) else {
        return false;
    };
    ui.get("launch")
        .and_then(|launch| launch.get("hideDockIcon"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}
