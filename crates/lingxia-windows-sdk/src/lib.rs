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
//! [`init_runtime`] and [`run_message_loop`] must be called on the same thread
//! for the default host: the message loop installs an exit handler that posts
//! `WM_QUIT` to the thread it runs on, and SDK-created webview windows are
//! serviced by that thread's message queue. [`quick_start`] performs both steps
//! in order on the calling thread.

#[cfg(all(target_os = "windows", feature = "runtime"))]
use std::path::Path;
#[cfg(feature = "runtime")]
use std::path::PathBuf;
#[cfg(all(target_os = "windows", feature = "host-api"))]
use std::sync::OnceLock;

#[cfg(all(target_os = "windows", feature = "host-api"))]
static HOST_MESSAGE_LOOP_THREAD: OnceLock<u32> = OnceLock::new();

#[cfg(all(target_os = "windows", feature = "host-api"))]
fn request_host_message_loop_exit() {
    use windows::Win32::UI::WindowsAndMessaging::{PostQuitMessage, PostThreadMessageW, WM_QUIT};

    if let Some(thread_id) = HOST_MESSAGE_LOOP_THREAD.get().copied() {
        unsafe {
            let _ = PostThreadMessageW(thread_id, WM_QUIT, Default::default(), Default::default());
        }
    } else {
        unsafe {
            PostQuitMessage(0);
        }
    }
}

#[cfg(all(target_os = "windows", feature = "runtime"))]
mod app_icon;
#[cfg(all(target_os = "windows", feature = "components"))]
mod app_menu;
#[cfg(all(target_os = "windows", feature = "components"))]
mod design_icons;
#[cfg(all(target_os = "windows", feature = "device-frame"))]
mod device_frame;
#[cfg(all(
    target_os = "windows",
    any(feature = "device-frame", feature = "shell-chrome")
))]
mod layered_text;
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
    WindowsBrowserEmulationProfile, WindowsDeviceFrame, WindowsDeviceFrameBadge,
    WindowsDeviceFrameCutout, WindowsDeviceFrameInfoSheet, WindowsDeviceFrameSheetAction,
    WindowsDeviceFrameStatusBar, WindowsDeviceFrameToolbar, open_current_page_devtools,
    set_app_window_device_frame, set_initial_app_window_device_frame,
    set_windows_browser_emulation_profile, show_device_frame_info_sheet,
};
#[cfg(feature = "runtime")]
pub use lingxia::RuntimeInfo;
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
    pub(crate) asset_dir: Option<PathBuf>,
    pub(crate) content: WindowsContent,
}

/// Primary content mounted by the SDK-managed Windows host.
#[derive(Debug, Clone, Default)]
#[cfg(feature = "runtime")]
pub enum WindowsContent {
    /// Open the lxapp declared by the generated host configuration.
    #[default]
    LxApp,
    /// Mount the managed browser at the URL.
    Browser(String),
}

#[cfg(feature = "runtime")]
impl WindowsApp {
    /// Creates an app description for the current process.
    ///
    /// App identity and state directories are resolved by the platform layer
    /// from `assets/app.json` beside the executable, which is derived from
    /// `lingxia.toml`.
    pub fn from_env() -> Self {
        Self {
            window_size: None,
            asset_dir: None,
            content: WindowsContent::LxApp,
        }
    }

    /// Sets the initial outer size, in pixels, of the app's webview windows,
    /// in particular the main window opened for the configured lxapp.
    ///
    /// When unset, windows open at the runtime default (1024x768). Users can
    /// still resize the window afterwards; non-positive dimensions are
    /// ignored by the runtime.
    pub fn with_window_size(mut self, width: i32, height: i32) -> Self {
        self.window_size = Some((width, height));
        self
    }

    /// Overrides the generated assets directory.
    ///
    /// Normal host apps should not call this: `lingxia build` places `assets/`
    /// beside the executable so Explorer double-click works. The Windows dev
    /// runner uses this because one installed runner executable can host many
    /// transient lxapp asset roots.
    pub fn with_asset_dir(mut self, asset_dir: impl Into<PathBuf>) -> Self {
        self.asset_dir = Some(asset_dir.into());
        self
    }

    /// Selects the primary content mounted by the host container.
    pub fn with_content(mut self, content: WindowsContent) -> Self {
        self.content = content;
        self
    }

    /// Mounts the managed browser instead of the configured lxapp. The browser
    /// owns editable URL chrome, history, and tab state.
    pub fn with_browser(mut self, url: impl Into<String>) -> Self {
        self.content = WindowsContent::Browser(url.into());
        self
    }

    #[cfg(target_os = "windows")]
    fn platform(&self) -> Result<lingxia::windows::Platform> {
        match &self.asset_dir {
            Some(asset_dir) => Ok(lingxia::windows::Platform::from_asset_dir(asset_dir)?),
            None => Ok(lingxia::windows::Platform::from_env()?),
        }
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
    /// The shared LingXia runtime failed to initialize.
    #[error(transparent)]
    Runtime(#[from] lingxia::Error),
    /// A caller that requires an lxapp was started without one.
    #[error("LingXia host does not define an lxapp")]
    MissingLxApp,
    /// The configured lxapp could not be opened.
    #[error("failed to open lxapp: {0}")]
    OpenLxApp(String),
    /// The managed browser could not be opened.
    #[error("failed to open browser: {0}")]
    OpenBrowser(String),
    /// The mounted primary content could not be controlled.
    #[error("failed to control primary content: {0}")]
    ControlContent(String),
    /// The window icon could not be loaded from the resolved path.
    #[error("failed to set Windows app icon from {path:?}: {message}")]
    AppIcon { path: PathBuf, message: String },
}

/// Convenience alias for results produced by this crate.
#[cfg(feature = "runtime")]
pub type Result<T> = std::result::Result<T, WindowsHostError>;

/// Handle returned after the SDK-managed Windows host has started.
#[derive(Debug, Clone)]
#[cfg(feature = "runtime")]
pub struct WindowsHost {
    runtime: RuntimeInfo,
    #[cfg_attr(not(feature = "device-frame"), allow(dead_code))]
    content: Option<MountedContent>,
}

#[derive(Debug, Clone)]
#[cfg(feature = "runtime")]
#[cfg_attr(not(feature = "device-frame"), allow(dead_code))]
enum MountedContent {
    LxApp(String),
    Browser,
}

#[cfg(feature = "runtime")]
impl WindowsHost {
    /// Runtime initialization details, including the optional configured lxapp.
    pub fn runtime(&self) -> &RuntimeInfo {
        &self.runtime
    }

    /// Consumes the host handle and returns its runtime initialization details.
    pub fn into_runtime(self) -> RuntimeInfo {
        self.runtime
    }

    /// Applies runner presentation to the mounted lxapp or browser.
    #[cfg(all(
        target_os = "windows",
        feature = "device-frame",
        feature = "shell-chrome"
    ))]
    pub fn set_primary_device_frame(
        &self,
        frame: WindowsDeviceFrame,
        tabbar_position: WindowsShellTabBarPosition,
    ) -> Result<()> {
        match self.content.as_ref() {
            Some(MountedContent::LxApp(appid)) => {
                device_frame::set_app_window_device_frame_and_tabbar_position(
                    appid,
                    frame,
                    tabbar_position,
                )
            }
            #[cfg(feature = "browser-runtime")]
            Some(MountedContent::Browser) => {
                device_frame::set_browser_device_frame_and_tabbar_position(frame, tabbar_position)
            }
            #[cfg(not(feature = "browser-runtime"))]
            Some(MountedContent::Browser) => {
                Err("managed browser is not enabled in this host".to_string())
            }
            None => Err("host has no mounted primary content".to_string()),
        }
        .map_err(WindowsHostError::ControlContent)
    }

    /// Opens DevTools for the mounted lxapp page or browser tab.
    #[cfg(all(target_os = "windows", feature = "device-frame"))]
    pub fn open_primary_devtools(&self) -> Result<()> {
        match self.content.as_ref() {
            Some(MountedContent::LxApp(appid)) => device_frame::open_current_page_devtools(appid),
            #[cfg(feature = "browser-runtime")]
            Some(MountedContent::Browser) => device_frame::open_browser_devtools(),
            #[cfg(not(feature = "browser-runtime"))]
            Some(MountedContent::Browser) => {
                Err("managed browser is not enabled in this host".to_string())
            }
            None => Err("host has no mounted primary content".to_string()),
        }
        .map_err(WindowsHostError::ControlContent)
    }
}

/// Boots the LingXia runtime and returns its initialization snapshot.
/// **Host-agnostic**: it
/// installs no Windows host backend and presents no window.
///
/// For the batteries-included host call [`start_default_host`] (or
/// [`quick_start`]). For a custom host, register your own
/// `lingxia_windows_contract::WindowsHostBackend`, optionally call
/// [`install_windows_components`], then drive your own Win32 message loop.
///
/// Must run on the thread that will later pump messages.
#[cfg(all(target_os = "windows", feature = "runtime"))]
pub fn init_runtime(app: WindowsApp) -> Result<RuntimeInfo> {
    if let Some((width, height)) = app.window_size {
        lingxia::windows::set_default_window_size(width, height);
    }
    let platform = app.platform()?;
    // Before any window exists: stamp the process's per-app taskbar identity
    // (AppUserModelID) so two apps hosted by the same exe — e.g. two dev
    // runners for different projects — get separate taskbar buttons.
    platform.install_taskbar_identity();
    Ok(lingxia::windows::init(platform)?)
}

/// Installs the SDK-managed native component integrations:
/// input/textarea/video overlays, media preview, and pull-to-refresh support.
/// Call this before the first page mounts a component.
///
/// Custom hosts can call this after registering their own
/// `lingxia_windows_contract::WindowsHostBackend`. The default host calls it for
/// you.
#[cfg(all(target_os = "windows", feature = "components"))]
pub fn install_windows_components() {
    native_components::install();
    pull_to_refresh::install();
}

/// Installs the SDK's default Windows host: the built-in WebView parent-window
/// host, `WindowsHostBackend`, native components, app menu, and — under
/// `browser-shell` — the native shell. Registrations must happen before the
/// first page mounts a component.
///
/// Call this for the batteries-included host; skip it and register your own
/// `WindowsHostBackend` for a custom host. [`start_default_host`] /
/// [`quick_start`] call it for you.
#[cfg(all(target_os = "windows", feature = "runtime"))]
pub fn install_default_windows_host() {
    window_host::install_default_windows_backend();
    install_windows_components();
    #[cfg(feature = "shell-chrome")]
    shell::install();
    app_menu::install_host_window_menu_support();
}

/// Default-host post-boot wiring: design-icon directory, window icon, taskbar
/// policy, opening the home window, and — under `browser-shell` — the tray.
#[cfg(all(target_os = "windows", feature = "runtime"))]
fn present_default_host(lxapp_id: Option<&str>, asset_dir: &Path) -> Result<()> {
    #[cfg(feature = "shell-chrome")]
    if let Some(lxapp_id) = lxapp_id {
        shell::set_home_app_id(lxapp_id);
    }
    if let Some(icon_path) = lxapp_id.and_then(|app_id| resolve_app_icon_path(asset_dir, app_id)) {
        app_icon::set_app_icon_from_path(&icon_path).map_err(|message| {
            WindowsHostError::AppIcon {
                path: icon_path,
                message,
            }
        })?;
    }
    // Tray-exclusive apps live only in the system tray, so their windows
    // must be created without a taskbar button. Apply before any window opens.
    window_host::set_hide_from_taskbar(should_hide_taskbar(asset_dir));
    if should_open_on_launch(asset_dir)
        && let Some(lxapp_id) = lxapp_id
    {
        open_home_app(lxapp_id).map_err(WindowsHostError::OpenLxApp)?;
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
        if let Err(message) = tray_icon::install_from_ui(asset_dir) {
            log::warn!("failed to install Windows tray icon: {message}");
        }
    }
    Ok(())
}

/// Brings up the batteries-included default Windows host *without* pumping the
/// message loop: installs the default host, boots the runtime, and opens the
/// configured [`WindowsContent`]. Returns a [`WindowsHost`] handle for setup
/// (menus, device frame, …) before calling [`run_message_loop`] itself.
/// [`quick_start`] wraps this.
#[cfg(all(target_os = "windows", feature = "runtime"))]
pub fn start_default_host(app: WindowsApp) -> Result<WindowsHost> {
    let content = app.content.clone();
    install_default_windows_host();
    // Own a message queue before any page can request exit from a WebView UI thread.
    install_current_thread_exit_handler();
    let asset_dir = app.platform()?.asset_dir().to_path_buf();
    // `init_runtime` may create and paint the first shell window. Register
    // generated SVG-derived icons before that first paint so native chrome
    // never starts with invisible icon-only controls.
    set_windows_design_icon_dir(asset_dir.join("icons").join("design"));
    let runtime = init_runtime(app)?;
    let configured_lxapp = matches!(content, WindowsContent::LxApp)
        .then(|| runtime.lxapp_id())
        .flatten();
    present_default_host(configured_lxapp, &asset_dir)?;
    let content = match content {
        WindowsContent::LxApp => configured_lxapp
            .map(str::to_string)
            .map(MountedContent::LxApp),
        WindowsContent::Browser(url) => {
            open_browser(&url).map_err(WindowsHostError::OpenBrowser)?;
            Some(MountedContent::Browser)
        }
    };
    Ok(WindowsHost { runtime, content })
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

#[cfg(all(target_os = "windows", feature = "shell-chrome"))]
fn open_browser(url: &str) -> std::result::Result<(), String> {
    shell::open_self_browser(url)
}

#[cfg(all(
    target_os = "windows",
    feature = "runtime",
    not(feature = "shell-chrome")
))]
fn open_browser(_url: &str) -> std::result::Result<(), String> {
    Err("managed browser requires the browser-runtime feature".to_string())
}

/// Boots the LingXia runtime and returns its initialization snapshot.
///
/// This non-Windows stub always fails with [`WindowsHostError::Platform`].
#[cfg(all(not(target_os = "windows"), feature = "runtime"))]
pub fn init_runtime(_app: WindowsApp) -> Result<RuntimeInfo> {
    Err(WindowsHostError::Platform(
        "lingxia-windows-sdk can only initialize on target_os = \"windows\"".to_string(),
    ))
}

/// Brings up the default Windows host.
///
/// This non-Windows stub always fails with [`WindowsHostError::Platform`].
#[cfg(all(not(target_os = "windows"), feature = "runtime"))]
pub fn start_default_host(_app: WindowsApp) -> Result<WindowsHost> {
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

    use windows::Win32::UI::WindowsAndMessaging::{MSG, PM_NOREMOVE, PeekMessageW};

    // Ensure this thread owns a message queue before page code can request
    // exit from a WebView UI thread.
    let mut msg = MSG::default();
    unsafe {
        let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);
    }

    let main_thread_id = unsafe { windows::Win32::System::Threading::GetCurrentThreadId() };
    let _ = HOST_MESSAGE_LOOP_THREAD.set(main_thread_id);
    lingxia::windows::set_windows_app_exit_handler(Arc::new(request_host_message_loop_exit));
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

#[cfg(all(target_os = "windows", feature = "runtime"))]
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

#[cfg(all(target_os = "windows", feature = "runtime"))]
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
