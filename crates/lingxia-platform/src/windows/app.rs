use std::fs::{self, File};
use std::future::Future;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

use lingxia_webview::WebTag;
use lingxia_webview::runtime as webview_runtime;

use super::{file, not_supported, surface};
use crate::error::PlatformError;
use crate::traits::app_runtime::{AnimationType, AppRuntime, LxAppOpenMode, OpenUrlRequest};
use crate::traits::location::{Location, LocationRequestConfig};
use crate::traits::pull_to_refresh::PullToRefresh;
use crate::traits::share::{ShareRequest, ShareResult, ShareService};
use crate::traits::stream_decoder::{VideoStreamDecoderHandle, VideoStreamDecoderManager};
use crate::traits::video_player::{VideoPlayerHandle, VideoPlayerManager};
use crate::AssetFileEntry;

const DEFAULT_APP_IDENTIFIER: &str = "app.lingxia.windows";

type WindowsAppExitHandler = Arc<dyn Fn() + Send + Sync>;
static WINDOWS_APP_EXIT_HANDLER: Mutex<Option<WindowsAppExitHandler>> = Mutex::new(None);

pub fn set_windows_app_exit_handler(handler: WindowsAppExitHandler) {
    if let Ok(mut slot) = WINDOWS_APP_EXIT_HANDLER.lock() {
        *slot = Some(handler);
    }
}

pub(crate) fn request_windows_app_exit() {
    let handler = WINDOWS_APP_EXIT_HANDLER
        .lock()
        .ok()
        .and_then(|slot| slot.clone());
    if let Some(handler) = handler {
        handler();
    } else {
        std::process::exit(0);
    }
}

/// Process-wide interceptor for [`AppRuntime::open_url`] requests. Returns
/// `true` when the request was handled (e.g. routed into an in-app browser
/// tab); `false` falls back to the OS shell handler.
type WindowsOpenUrlHandler = Arc<dyn Fn(&OpenUrlRequest) -> bool + Send + Sync>;
static WINDOWS_OPEN_URL_HANDLER: Mutex<Option<WindowsOpenUrlHandler>> = Mutex::new(None);

/// Registers the open-url interceptor. Product shells (the `lingxia`
/// facade) use this to keep in-app targets (`SelfTarget`,
/// `NewBrowserTab`) inside the app instead of launching the system
/// default browser; the previous handler (if any) is replaced.
pub fn set_windows_open_url_handler(handler: WindowsOpenUrlHandler) {
    if let Ok(mut slot) = WINDOWS_OPEN_URL_HANDLER.lock() {
        *slot = Some(handler);
    }
}

fn invoke_windows_open_url_handler(req: &OpenUrlRequest) -> bool {
    let handler = WINDOWS_OPEN_URL_HANDLER
        .lock()
        .ok()
        .and_then(|slot| slot.clone());
    handler.map(|handler| handler(req)).unwrap_or(false)
}

#[derive(Debug, Clone)]
pub struct Platform {
    data_dir: PathBuf,
    cache_dir: PathBuf,
    asset_dir: PathBuf,
    locale: String,
    app_identifier: String,
    product_name: String,
}

impl Default for Platform {
    fn default() -> Self {
        let base = default_state_root();
        Self {
            data_dir: base.join("data"),
            cache_dir: base.join("cache"),
            asset_dir: default_asset_dir(),
            locale: default_locale(),
            app_identifier: DEFAULT_APP_IDENTIFIER.to_string(),
            product_name: "LingXia".to_string(),
        }
    }
}

impl Platform {
    pub fn new(
        data_dir: impl Into<PathBuf>,
        cache_dir: impl Into<PathBuf>,
        locale: impl Into<String>,
    ) -> Result<Self, PlatformError> {
        Ok(Self {
            data_dir: data_dir.into(),
            cache_dir: cache_dir.into(),
            asset_dir: default_asset_dir(),
            locale: locale.into(),
            app_identifier: DEFAULT_APP_IDENTIFIER.to_string(),
            product_name: "LingXia".to_string(),
        })
    }

    pub fn with_assets(
        data_dir: impl Into<PathBuf>,
        cache_dir: impl Into<PathBuf>,
        asset_dir: impl Into<PathBuf>,
        locale: impl Into<String>,
        app_identifier: impl Into<String>,
        product_name: impl Into<String>,
    ) -> Result<Self, PlatformError> {
        Ok(Self {
            data_dir: data_dir.into(),
            cache_dir: cache_dir.into(),
            asset_dir: asset_dir.into(),
            locale: locale.into(),
            app_identifier: app_identifier.into(),
            product_name: product_name.into(),
        })
    }

    pub fn asset_dir(&self) -> &Path {
        &self.asset_dir
    }

    fn resolve_asset_path(&self, path: &str) -> Result<PathBuf, PlatformError> {
        let normalized = normalize_relative_path(path)?;
        Ok(self.asset_dir.join(normalized))
    }

    fn collect_files_recursively<'a>(
        &'a self,
        asset_dir: &str,
    ) -> Vec<Result<AssetFileEntry<'a>, PlatformError>> {
        let root = match self.resolve_asset_path(asset_dir) {
            Ok(path) => path,
            Err(err) => return vec![Err(err)],
        };
        let base = self.asset_dir.clone();
        let mut out = Vec::new();
        collect_asset_files(&base, &root, &mut out);
        out
    }
}

impl AppRuntime for Platform {
    fn read_asset<'a>(&'a self, path: &str) -> Result<Box<dyn Read + 'a>, PlatformError> {
        let path = self.resolve_asset_path(path)?;
        let file = File::open(&path).map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                PlatformError::AssetNotFound(path.display().to_string())
            } else {
                PlatformError::Platform(format!("failed to open asset {}: {err}", path.display()))
            }
        })?;
        Ok(Box::new(file))
    }

    fn asset_dir_iter<'a>(
        &'a self,
        asset_dir: &str,
    ) -> Box<dyn Iterator<Item = Result<AssetFileEntry<'a>, PlatformError>> + 'a> {
        Box::new(self.collect_files_recursively(asset_dir).into_iter())
    }

    fn app_data_dir(&self) -> PathBuf {
        self.data_dir.clone()
    }

    fn app_cache_dir(&self) -> PathBuf {
        self.cache_dir.clone()
    }

    fn get_app_identifier(&self) -> Result<String, PlatformError> {
        Ok(self.app_identifier.clone())
    }

    fn get_system_locale(&self) -> &str {
        &self.locale
    }

    fn show_lxapp(
        &self,
        appid: String,
        path: String,
        session_id: u64,
        open_mode: LxAppOpenMode,
        panel_id: String,
    ) -> Result<(), PlatformError> {
        let webtag = WebTag::new(&appid, &path, Some(session_id));
        surface::show_webtag_window(webtag, self.product_name.clone(), true, open_mode, panel_id);
        Ok(())
    }

    fn hide_lxapp(&self, appid: String, session_id: u64) -> Result<(), PlatformError> {
        surface::hide_lxapp_window(&appid, session_id);
        Ok(())
    }

    fn exit(&self) -> Result<(), PlatformError> {
        request_windows_app_exit();
        Ok(())
    }

    fn navigate(
        &self,
        appid: String,
        path: String,
        _animation_type: AnimationType,
    ) -> Result<(), PlatformError> {
        let session_id = webview_runtime::list_webviews()
            .into_iter()
            .find(|tag| tag.extract_appid() == appid)
            .and_then(|tag| tag.session_id());
        let webtag = WebTag::new(&appid, &path, session_id);
        surface::show_webtag_window(
            webtag,
            self.product_name.clone(),
            false,
            LxAppOpenMode::Normal,
            String::new(),
        );
        Ok(())
    }

    fn open_url(&self, req: OpenUrlRequest) -> Result<(), PlatformError> {
        // In-app targets (browser tabs) are owned by the registered product
        // shell handler; only unhandled requests reach the OS shell.
        if invoke_windows_open_url_handler(&req) {
            return Ok(());
        }
        // Sync trait method: launch without waiting so the executor never blocks.
        file::open_with_shell_detached(&req.url)
    }

    fn get_capsule_rect(&self) -> impl Future<Output = Result<String, PlatformError>> + Send {
        async { Ok(r#"{"width":0,"height":0,"top":0,"right":0,"bottom":0,"left":0}"#.to_string()) }
    }
}

impl crate::traits::device::DeviceHardware for Platform {}
impl crate::traits::network::Network for Platform {}
impl crate::traits::secure_store::SecureStore for Platform {}
impl crate::traits::ui::SurfacePresenter for Platform {}
impl crate::traits::update::UpdateService for Platform {}
impl crate::traits::wifi::Wifi for Platform {}

impl Location for Platform {
    fn is_location_enabled(&self) -> Result<bool, PlatformError> {
        not_supported("is_location_enabled")
    }

    fn request_location(
        &self,
        _config: LocationRequestConfig,
    ) -> impl Future<Output = Result<String, PlatformError>> + Send {
        async { not_supported("request_location") }
    }
}

impl PullToRefresh for Platform {
    fn start_pull_down_refresh(&self, _app_id: &str, _path: &str) -> Result<(), PlatformError> {
        not_supported("start_pull_down_refresh")
    }

    fn stop_pull_down_refresh(&self, _app_id: &str, _path: &str) -> Result<(), PlatformError> {
        not_supported("stop_pull_down_refresh")
    }
}

impl ShareService for Platform {
    fn share(
        &self,
        _request: ShareRequest,
    ) -> impl Future<Output = Result<ShareResult, PlatformError>> + Send {
        async { not_supported("share") }
    }
}

impl VideoPlayerManager for Platform {
    fn bind_player(
        &self,
        _component_id: &str,
    ) -> Result<Box<dyn VideoPlayerHandle>, PlatformError> {
        not_supported("bind_player")
    }
}

impl VideoStreamDecoderManager for Platform {
    fn create_stream_decoder(
        &self,
        _component_id: &str,
    ) -> Result<Box<dyn VideoStreamDecoderHandle>, PlatformError> {
        not_supported("create_stream_decoder")
    }
}

fn normalize_relative_path(path: &str) -> Result<PathBuf, PlatformError> {
    let mut out = PathBuf::new();
    for component in Path::new(path.trim_start_matches(['/', '\\'])).components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            Component::RootDir | Component::Prefix(_) | Component::ParentDir => {
                return Err(PlatformError::InvalidParameter(format!(
                    "asset path must be relative and stay inside assets: {path}"
                )));
            }
        }
    }
    Ok(out)
}

fn collect_asset_files<'a>(
    base: &Path,
    dir: &Path,
    out: &mut Vec<Result<AssetFileEntry<'a>, PlatformError>>,
) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => {
            out.push(Err(PlatformError::Platform(format!(
                "failed to read asset directory {}: {err}",
                dir.display()
            ))));
            return;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                out.push(Err(PlatformError::Platform(format!(
                    "failed to read asset directory entry: {err}"
                ))));
                continue;
            }
        };
        let path = entry.path();
        if path.is_dir() {
            collect_asset_files(base, &path, out);
            continue;
        }
        if !path.is_file() {
            continue;
        }
        let relative = path
            .strip_prefix(base)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        match File::open(&path) {
            Ok(file) => out.push(Ok(AssetFileEntry {
                path: relative,
                reader: Box::new(file),
            })),
            Err(err) => out.push(Err(PlatformError::Platform(format!(
                "failed to open asset {}: {err}",
                path.display()
            )))),
        }
    }
}

fn default_state_root() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("LingXia")
}

fn default_asset_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("LINGXIA_ASSET_DIR") {
        return PathBuf::from(path);
    }
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        .map(|dir| dir.join("assets"))
        .unwrap_or_else(|| PathBuf::from("assets"))
}

fn default_locale() -> String {
    use windows::Win32::Globalization::GetUserDefaultLocaleName;

    // LOCALE_NAME_MAX_LENGTH (85); the pinned windows-rs rev does not export it.
    let mut buffer = [0u16; 85];
    let len = unsafe { GetUserDefaultLocaleName(&mut buffer) };
    if len > 1 {
        // Returned length includes the terminating NUL; the name is already BCP-47.
        String::from_utf16_lossy(&buffer[..len as usize - 1])
    } else {
        "en-US".to_string()
    }
}
