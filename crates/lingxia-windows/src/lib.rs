//! Windows host entry crate for LingXia.
//!
//! This crate sits above the [`lingxia`] facade and provides the thin host
//! glue a pure Rust Windows executable needs: [`WindowsApp`] describes the
//! host process, [`init`] boots the LingXia runtime and opens the home lxapp,
//! and [`run_message_loop`] pumps the Win32 message loop until the app exits.
//!
//! [`init`] and [`run_message_loop`] must be called on the same thread: the
//! message loop installs an exit handler that posts `WM_QUIT` to the thread it
//! runs on, and the webview windows created during [`init`] are serviced by
//! that thread's message queue. [`quick_start`] performs both steps in order
//! on the calling thread.

use std::path::{Path, PathBuf};

#[cfg(target_os = "windows")]
mod native_components;
#[cfg(target_os = "windows")]
mod video_controls;
#[cfg(target_os = "windows")]
mod video_player;

/// Host process description used to initialize the LingXia runtime.
///
/// Construct it with [`WindowsApp::new`] or [`WindowsApp::from_env`] and
/// customize it through the `with_*` builder methods before passing it to
/// [`init`].
#[derive(Debug, Clone)]
pub struct WindowsApp {
    pub(crate) data_dir: PathBuf,
    pub(crate) cache_dir: PathBuf,
    pub(crate) asset_dir: PathBuf,
    pub(crate) locale: String,
    pub(crate) app_identifier: String,
    pub(crate) product_name: String,
    pub(crate) icon_path: Option<PathBuf>,
    pub(crate) window_size: Option<(i32, i32)>,
}

impl WindowsApp {
    /// Creates an app description with the given state and asset directories.
    ///
    /// The locale defaults to the user's Windows display locale, the app
    /// identifier to `app.lingxia.windows`, and the product name to `LingXia`.
    pub fn new(
        data_dir: impl Into<PathBuf>,
        cache_dir: impl Into<PathBuf>,
        asset_dir: impl Into<PathBuf>,
    ) -> Self {
        Self {
            data_dir: data_dir.into(),
            cache_dir: cache_dir.into(),
            asset_dir: asset_dir.into(),
            locale: default_locale(),
            app_identifier: "app.lingxia.windows".to_string(),
            product_name: "LingXia".to_string(),
            icon_path: None,
            window_size: None,
        }
    }

    /// Creates an app description from the process environment.
    ///
    /// State directories live under `%LOCALAPPDATA%\<product name>`; the
    /// `LINGXIA_ASSET_DIR`, `LINGXIA_APP_ID`, and `LINGXIA_PRODUCT_NAME`
    /// environment variables override the asset directory, app identifier,
    /// and product name.
    pub fn from_env() -> Self {
        let root = state_root();
        let asset_dir = std::env::var_os("LINGXIA_ASSET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(default_asset_dir);
        Self::new(root.join("data"), root.join("cache"), asset_dir)
            .with_app_identifier(env_or("LINGXIA_APP_ID", "app.lingxia.windows"))
            .with_product_name(env_or("LINGXIA_PRODUCT_NAME", "LingXia"))
    }

    /// Overrides the BCP-47 locale reported to the runtime (e.g. `en-US`).
    pub fn with_locale(mut self, locale: impl Into<String>) -> Self {
        self.locale = locale.into();
        self
    }

    /// Overrides the reverse-DNS application identifier.
    pub fn with_app_identifier(mut self, app_identifier: impl Into<String>) -> Self {
        self.app_identifier = app_identifier.into();
        self
    }

    /// Overrides the user-visible product name.
    pub fn with_product_name(mut self, product_name: impl Into<String>) -> Self {
        self.product_name = product_name.into();
        self
    }

    /// Sets an explicit window/taskbar icon image path.
    ///
    /// When unset, [`init`] falls back to `AppIcon.png` from the home app's
    /// `public` directory or the asset directory root.
    pub fn with_icon_path(mut self, icon_path: impl Into<PathBuf>) -> Self {
        self.icon_path = Some(icon_path.into());
        self
    }

    /// Sets the initial outer size, in pixels, of the app's webview windows
    /// — in particular the main window opened for the home lxapp.
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
pub enum WindowsHostError {
    /// The LingXia platform layer failed to initialize.
    #[cfg(target_os = "windows")]
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
pub type Result<T> = std::result::Result<T, WindowsHostError>;

/// Initializes the LingXia runtime and opens the home lxapp.
///
/// Returns the home app id on success. Must run on the thread that will later
/// call [`run_message_loop`].
#[cfg(target_os = "windows")]
pub fn init(app: WindowsApp) -> Result<String> {
    // Embedded native components (input/textarea/video overlays) are part
    // of the host SDK — every Windows host gets them, like the managers in
    // the Android/iOS SDK layers. Must register before the first page can
    // mount a component.
    native_components::install();

    let asset_dir = app.asset_dir.clone();
    let icon_path = app.icon_path.clone();
    if let Some((width, height)) = app.window_size {
        lingxia::windows::set_default_window_size(width, height);
    }
    let platform = lingxia::windows::Platform::with_assets(
        app.data_dir,
        app.cache_dir,
        app.asset_dir,
        app.locale,
        app.app_identifier,
        app.product_name,
    )?;
    let home_app_id = lingxia::windows::init(platform).ok_or(WindowsHostError::MissingHomeApp)?;
    if let Some(icon_path) = resolve_app_icon_path(&asset_dir, &home_app_id, icon_path) {
        lingxia::windows::set_app_icon_from_path(&icon_path).map_err(|message| {
            WindowsHostError::AppIcon {
                path: icon_path,
                message,
            }
        })?;
    }
    lingxia::windows::open_home_app(&home_app_id).map_err(WindowsHostError::OpenHomeApp)?;
    Ok(home_app_id)
}

/// Initializes the LingXia runtime and opens the home lxapp.
///
/// This non-Windows stub always fails with [`WindowsHostError::Platform`].
#[cfg(not(target_os = "windows"))]
pub fn init(_app: WindowsApp) -> Result<String> {
    Err(WindowsHostError::Platform(
        "lingxia-windows can only initialize on target_os = \"windows\"".to_string(),
    ))
}

/// Runs the Win32 message loop until the application quits.
///
/// Installs the LingXia app exit handler for the calling thread and pumps
/// messages until `WM_QUIT`, returning the loop exit code. Must run on the
/// same thread that called [`init`].
#[cfg(target_os = "windows")]
pub fn run_message_loop() -> i32 {
    use std::sync::Arc;

    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, MSG, PostThreadMessageW, TranslateMessage, WM_QUIT,
    };

    let main_thread_id = unsafe { windows::Win32::System::Threading::GetCurrentThreadId() };
    lingxia::windows::set_windows_app_exit_handler(Arc::new(move || unsafe {
        let _ = PostThreadMessageW(
            main_thread_id,
            WM_QUIT,
            Default::default(),
            Default::default(),
        );
    }));

    let mut msg = MSG::default();
    loop {
        let result = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        match result.0 {
            -1 => return 1,
            0 => return msg.wParam.0 as i32,
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
#[cfg(not(target_os = "windows"))]
pub fn run_message_loop() -> i32 {
    0
}

/// Boots the host from the environment and blocks until the app exits.
///
/// Equivalent to [`init`] with [`WindowsApp::from_env`] followed by
/// [`run_message_loop`] on the calling thread. Returns the message-loop exit
/// code once the application quits.
pub fn quick_start() -> Result<i32> {
    init(WindowsApp::from_env())?;
    Ok(run_message_loop())
}

fn state_root() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(env_or("LINGXIA_PRODUCT_NAME", "LingXia"))
}

fn default_asset_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from))
        .map(|dir| dir.join("assets"))
        .unwrap_or_else(|| PathBuf::from("assets"))
}

fn resolve_app_icon_path(
    asset_dir: &Path,
    home_app_id: &str,
    explicit: Option<PathBuf>,
) -> Option<PathBuf> {
    if explicit.is_some() {
        return explicit;
    }

    [
        asset_dir
            .join(home_app_id)
            .join("public")
            .join("AppIcon.png"),
        asset_dir.join("AppIcon.png"),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

#[cfg(target_os = "windows")]
fn default_locale() -> String {
    use windows::Win32::Globalization::GetUserDefaultLocaleName;

    // LOCALE_NAME_MAX_LENGTH is 85; the returned length includes the
    // terminating null, so anything above 1 carries a real locale name.
    let mut buffer = [0_u16; 85];
    let len = unsafe { GetUserDefaultLocaleName(&mut buffer) };
    if len > 1 {
        String::from_utf16_lossy(&buffer[..len as usize - 1])
    } else {
        "en-US".to_string()
    }
}

#[cfg(not(target_os = "windows"))]
fn default_locale() -> String {
    "en-US".to_string()
}

fn env_or(name: &str, fallback: &str) -> String {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| fallback.to_string())
}
