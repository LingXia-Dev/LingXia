use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct WindowsApp {
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub asset_dir: PathBuf,
    pub locale: String,
    pub app_identifier: String,
    pub product_name: String,
}

impl WindowsApp {
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
        }
    }

    pub fn from_env() -> Self {
        let root = state_root();
        let asset_dir = std::env::var_os("LINGXIA_ASSET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(default_asset_dir);
        Self::new(root.join("data"), root.join("cache"), asset_dir)
            .with_locale(default_locale())
            .with_app_identifier(env_or("LINGXIA_APP_ID", "app.lingxia.windows"))
            .with_product_name(env_or("LINGXIA_PRODUCT_NAME", "LingXia"))
    }

    pub fn with_locale(mut self, locale: impl Into<String>) -> Self {
        self.locale = locale.into();
        self
    }

    pub fn with_app_identifier(mut self, app_identifier: impl Into<String>) -> Self {
        self.app_identifier = app_identifier.into();
        self
    }

    pub fn with_product_name(mut self, product_name: impl Into<String>) -> Self {
        self.product_name = product_name.into();
        self
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WindowsHostError {
    #[error(transparent)]
    Platform(#[from] lingxia_platform::PlatformError),
    #[error("LingXia runtime did not return a home app id")]
    MissingHomeApp,
}

pub type Result<T> = std::result::Result<T, WindowsHostError>;

#[cfg(target_os = "windows")]
pub fn init(app: WindowsApp) -> Result<String> {
    let platform = lingxia::windows::Platform::with_assets(
        app.data_dir,
        app.cache_dir,
        app.asset_dir,
        app.locale,
        app.app_identifier,
        app.product_name,
    )?;
    lingxia::windows::init(platform).ok_or(WindowsHostError::MissingHomeApp)
}

#[cfg(not(target_os = "windows"))]
pub fn init(_app: WindowsApp) -> Result<String> {
    Err(WindowsHostError::Platform(
        lingxia_platform::PlatformError::NotSupported(
            "lingxia-windows can only initialize on target_os = \"windows\"".to_string(),
        ),
    ))
}

#[cfg(target_os = "windows")]
pub fn run_message_loop() -> i32 {
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, MSG, TranslateMessage, WM_QUIT,
    };

    let mut msg = MSG::default();
    loop {
        let result = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        match result.0 {
            -1 => return 1,
            0 => return msg.wParam.0 as i32,
            _ => {
                if msg.message != WM_QUIT {
                    unsafe {
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn run_message_loop() -> i32 {
    0
}

pub fn quick_start() -> Result<String> {
    let home_app_id = init(WindowsApp::from_env())?;
    #[cfg(target_os = "windows")]
    {
        let _ = run_message_loop();
    }
    Ok(home_app_id)
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

fn default_locale() -> String {
    std::env::var("LANG")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "en-US".to_string())
}

fn env_or(name: &str, fallback: &str) -> String {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| fallback.to_string())
}
