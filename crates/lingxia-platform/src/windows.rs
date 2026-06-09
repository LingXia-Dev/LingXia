#![allow(clippy::manual_async_fn)]

use std::fs::{self, File};
use std::future::Future;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use lingxia_webview::runtime as webview_runtime;
use lingxia_webview::{FileChooserFile, FileChooserResponse, WebTag};

use crate::error::PlatformError;
use crate::traits::app_runtime::{AnimationType, AppRuntime, LxAppOpenMode, OpenUrlRequest};
use crate::traits::device::{Device, DeviceHardware};
use crate::traits::file::{
    ChooseDirectoryRequest, ChooseFileRequest, FileDialogResult, FileService, OpenFileRequest,
    RevealInFileManagerRequest,
};
use crate::traits::location::{Location, LocationRequestConfig};
use crate::traits::media_interaction::{
    ChooseMediaRequest, MediaInteraction, PreviewMediaRequest, SaveMediaRequest, ScanCodeRequest,
};
use crate::traits::media_runtime::{
    CompressImageRequest, CompressVideoRequest, CompressedVideo, ExtractVideoThumbnailRequest,
    ImageInfo, MediaRuntime, VideoInfo, VideoThumbnail,
};
use crate::traits::pull_to_refresh::PullToRefresh;
use crate::traits::screenshot::{AppScreenshot, WindowInfo};
use crate::traits::share::{ShareRequest, ShareResult, ShareService};
use crate::traits::stream_decoder::{VideoStreamDecoderHandle, VideoStreamDecoderManager};
use crate::traits::ui::{ModalOptions, ToastOptions, UIUpdate, UserFeedback};
use crate::traits::video_player::{VideoPlayerHandle, VideoPlayerManager};
use crate::{AssetFileEntry, DeviceInfo, ScreenInfo};
use async_trait::async_trait;

const DEFAULT_APP_IDENTIFIER: &str = "app.lingxia.windows";
type WindowsUiUpdateHandler = Arc<dyn Fn(String) + Send + Sync>;
static WINDOWS_UI_UPDATE_HANDLER: OnceLock<Mutex<Option<WindowsUiUpdateHandler>>> =
    OnceLock::new();

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

    pub fn file_chooser_handler(
        &self,
    ) -> impl Fn(
        lingxia_webview::FileChooserRequest,
    ) -> std::pin::Pin<Box<dyn Future<Output = FileChooserResponse> + Send>>
    + Clone
    + Send
    + Sync
    + 'static {
        let platform = self.clone();
        move |request| {
            let platform = platform.clone();
            Box::pin(async move { platform.handle_file_chooser(request).await })
        }
    }

    async fn handle_file_chooser(
        &self,
        request: lingxia_webview::FileChooserRequest,
    ) -> FileChooserResponse {
        if request.capture {
            return FileChooserResponse::Error(
                "capture file chooser is not supported on Windows yet".to_string(),
            );
        }

        let result = if request.allow_directories {
            self.choose_directory(ChooseDirectoryRequest {
                title: Some("Select folder".to_string()),
                default_path: None,
            })
            .await
        } else {
            self.choose_file(ChooseFileRequest {
                multiple: request.allow_multiple,
                filters: Vec::new(),
                title: Some("Select file".to_string()),
                default_path: None,
            })
            .await
        };

        match result {
            Ok(result) if result.canceled => FileChooserResponse::Cancel,
            Ok(result) => FileChooserResponse::Files(
                result
                    .paths
                    .into_iter()
                    .map(|path| FileChooserFile {
                        path: Some(path),
                        uri: None,
                    })
                    .collect(),
            ),
            Err(err) => FileChooserResponse::Error(err.to_string()),
        }
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

pub fn set_windows_ui_update_handler(handler: WindowsUiUpdateHandler) {
    let slot = WINDOWS_UI_UPDATE_HANDLER.get_or_init(|| Mutex::new(None));
    if let Ok(mut slot) = slot.lock() {
        *slot = Some(handler);
    }
}

fn invoke_windows_ui_update_handler(appid: String) {
    let handler = WINDOWS_UI_UPDATE_HANDLER
        .get()
        .and_then(|slot| slot.lock().ok())
        .and_then(|slot| slot.clone());
    if let Some(handler) = handler {
        handler(appid);
    }
}

impl Device for Platform {
    fn device_info(&self) -> DeviceInfo {
        DeviceInfo {
            brand: "Microsoft".to_string(),
            model: std::env::consts::ARCH.to_string(),
            market_name: "Windows PC".to_string(),
            os_name: "Windows".to_string(),
            os_version: String::new(),
        }
    }

    fn screen_info(&self) -> ScreenInfo {
        use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

        let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
        ScreenInfo {
            width: width.max(0) as f64,
            height: height.max(0) as f64,
            scale: 1.0,
        }
    }

    fn vibrate(&self, _long: bool) -> Result<(), PlatformError> {
        not_supported("vibrate")
    }

    fn make_phone_call(&self, phone_number: &str) -> Result<(), PlatformError> {
        open_with_shell(&format!("tel:{phone_number}"))
    }
}

impl DeviceHardware for Platform {}
impl crate::traits::network::Network for Platform {}
impl crate::traits::secure_store::SecureStore for Platform {}
impl crate::traits::ui::SurfacePresenter for Platform {}
impl crate::traits::update::UpdateService for Platform {}
impl crate::traits::wifi::Wifi for Platform {}

#[async_trait]
impl AppScreenshot for Platform {
    async fn list_app_windows(&self) -> Result<Vec<WindowInfo>, PlatformError> {
        list_app_windows()
    }
}

impl FileService for Platform {
    fn review_file(
        &self,
        request: OpenFileRequest,
    ) -> impl Future<Output = Result<(), PlatformError>> + Send {
        async move { open_with_shell(&request.path) }
    }

    fn open_external(
        &self,
        request: OpenFileRequest,
    ) -> impl Future<Output = Result<(), PlatformError>> + Send {
        async move { open_with_shell(&request.path) }
    }

    fn reveal_in_file_manager(
        &self,
        request: RevealInFileManagerRequest,
    ) -> impl Future<Output = Result<(), PlatformError>> + Send {
        async move {
            let status = Command::new("explorer")
                .arg(format!("/select,{}", request.path))
                .status()
                .map_err(|err| {
                    PlatformError::Platform(format!("failed to start explorer: {err}"))
                })?;
            if status.success() {
                Ok(())
            } else {
                Err(PlatformError::Platform(format!(
                    "explorer exited with status {status}"
                )))
            }
        }
    }

    fn choose_file(
        &self,
        request: ChooseFileRequest,
    ) -> impl Future<Output = Result<FileDialogResult, PlatformError>> + Send {
        crate::desktop::file_dialog::choose_file_desktop(request)
    }

    fn choose_directory(
        &self,
        request: ChooseDirectoryRequest,
    ) -> impl Future<Output = Result<FileDialogResult, PlatformError>> + Send {
        crate::desktop::file_dialog::choose_directory_desktop(request)
    }
}

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

impl MediaInteraction for Platform {
    fn preview_media(&self, _request: PreviewMediaRequest) -> Result<(), PlatformError> {
        not_supported("preview_media")
    }

    fn cancel_preview(&self, _callback_id: u64) -> Result<(), PlatformError> {
        not_supported("cancel_preview")
    }

    fn choose_media(
        &self,
        _request: ChooseMediaRequest,
    ) -> impl Future<Output = Result<String, PlatformError>> + Send {
        async { not_supported("choose_media") }
    }

    fn scan_code(
        &self,
        request: ScanCodeRequest,
    ) -> impl Future<Output = Result<String, PlatformError>> + Send {
        crate::desktop::scan::scan_code_desktop(request)
    }

    fn save_image_to_photos_album(
        &self,
        _request: SaveMediaRequest,
    ) -> impl Future<Output = Result<(), PlatformError>> + Send {
        async { not_supported("save_image_to_photos_album") }
    }

    fn save_video_to_photos_album(
        &self,
        _request: SaveMediaRequest,
    ) -> impl Future<Output = Result<(), PlatformError>> + Send {
        async { not_supported("save_video_to_photos_album") }
    }
}

impl MediaRuntime for Platform {
    fn copy_album_media_to_file(
        &self,
        uri: &str,
        dest_path: &Path,
        _kind: crate::traits::media_interaction::MediaKind,
    ) -> Result<(), PlatformError> {
        let source = normalize_file_uri(uri)?;
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                PlatformError::Platform(format!(
                    "failed to create destination directory {}: {err}",
                    parent.display()
                ))
            })?;
        }
        fs::copy(&source, dest_path).map_err(|err| {
            PlatformError::Platform(format!(
                "failed to copy media {} -> {}: {err}",
                source.display(),
                dest_path.display()
            ))
        })?;
        Ok(())
    }

    fn get_image_info(&self, uri: &str) -> Result<ImageInfo, PlatformError> {
        crate::desktop::image::get_image_info_desktop(uri)
    }

    fn compress_image(&self, request: &CompressImageRequest) -> Result<PathBuf, PlatformError> {
        crate::desktop::image::compress_image_desktop(request)
    }

    fn compress_video(
        &self,
        _request: &CompressVideoRequest,
    ) -> Result<CompressedVideo, PlatformError> {
        not_supported("compress_video")
    }

    fn get_video_info(&self, _uri: &str) -> Result<VideoInfo, PlatformError> {
        not_supported("get_video_info")
    }

    fn extract_video_thumbnail(
        &self,
        _request: &ExtractVideoThumbnailRequest,
    ) -> Result<VideoThumbnail, PlatformError> {
        not_supported("extract_video_thumbnail")
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

impl UIUpdate for Platform {
    fn update_navbar_ui(&self, appid: String) -> Result<(), PlatformError> {
        invoke_windows_ui_update_handler(appid);
        Ok(())
    }

    fn update_tabbar_ui(&self, appid: String) -> Result<(), PlatformError> {
        invoke_windows_ui_update_handler(appid);
        Ok(())
    }
}

impl UserFeedback for Platform {
    fn show_toast(&self, _options: ToastOptions) -> Result<(), PlatformError> {
        Ok(())
    }

    fn hide_toast(&self) -> Result<(), PlatformError> {
        Ok(())
    }

    fn show_modal(
        &self,
        _options: ModalOptions,
    ) -> impl Future<Output = Result<String, PlatformError>> + Send {
        async { not_supported("show_modal") }
    }

    fn show_action_sheet(
        &self,
        _options: Vec<String>,
        _cancel_text: String,
        _item_color: String,
    ) -> impl Future<Output = Result<String, PlatformError>> + Send {
        async { not_supported("show_action_sheet") }
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
        _open_mode: LxAppOpenMode,
        _panel_id: String,
    ) -> Result<(), PlatformError> {
        let webtag = WebTag::new(&appid, &path, Some(session_id));
        hide_sibling_webtag_windows(&webtag);
        show_webtag_window(webtag, self.product_name.clone());
        Ok(())
    }

    fn hide_lxapp(&self, appid: String, session_id: u64) -> Result<(), PlatformError> {
        for webtag in webview_runtime::list_webviews() {
            if webtag.extract_appid() == appid && webtag.session_id() == Some(session_id) {
                let _ = lingxia_webview::platform::windows::hide_webview_window(&webtag);
            }
        }
        Ok(())
    }

    fn exit(&self) -> Result<(), PlatformError> {
        std::process::exit(0);
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
        hide_sibling_webtag_windows(&webtag);
        show_webtag_window(webtag, self.product_name.clone());
        Ok(())
    }

    fn open_url(&self, req: OpenUrlRequest) -> Result<(), PlatformError> {
        open_with_shell(&req.url)
    }

    fn get_capsule_rect(&self) -> impl Future<Output = Result<String, PlatformError>> + Send {
        async { Ok(r#"{"width":0,"height":0,"top":0,"right":0,"bottom":0,"left":0}"#.to_string()) }
    }
}

fn show_webtag_window(webtag: WebTag, title: String) {
    if webview_runtime::find_webview(&webtag).is_some() {
        install_close_handler(&webtag);
        let _ = lingxia_webview::platform::windows::show_webview_window(&webtag, &title);
        return;
    }

    let _ = thread::Builder::new()
        .name(format!("lingxia-windows-show-{}", webtag.key()))
        .spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            while Instant::now() < deadline {
                if webview_runtime::find_webview(&webtag).is_some() {
                    install_close_handler(&webtag);
                    let _ =
                        lingxia_webview::platform::windows::show_webview_window(&webtag, &title);
                    return;
                }
                thread::sleep(Duration::from_millis(50));
            }
            log::error!("Timed out waiting for Windows WebView {}", webtag.key());
        });
}

fn hide_sibling_webtag_windows(target: &WebTag) {
    let appid = target.extract_appid();
    let session_id = target.session_id();
    for webtag in webview_runtime::list_webviews() {
        if webtag.key() != target.key()
            && webtag.extract_appid() == appid
            && webtag.session_id() == session_id
        {
            let _ = lingxia_webview::platform::windows::hide_webview_window(&webtag);
        }
    }
}

fn install_close_handler(webtag: &WebTag) {
    let webtag_for_close = webtag.clone();
    lingxia_webview::platform::windows::set_webview_close_handler(
        webtag,
        Arc::new(move || {
            let _ = lingxia_webview::platform::windows::hide_webview_window(&webtag_for_close);
        }),
    );
}

fn list_app_windows() -> Result<Vec<WindowInfo>, PlatformError> {
    use windows::Win32::Foundation::{HWND, LPARAM, RECT};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetForegroundWindow, GetWindowRect, GetWindowThreadProcessId, IsIconic,
        IsWindowVisible,
    };
    use windows::core::BOOL;

    struct EnumState {
        pid: u32,
        foreground: HWND,
        windows: Vec<WindowInfo>,
    }

    unsafe extern "system" fn enum_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let state = unsafe { &mut *(lparam.0 as *mut EnumState) };
        let mut owner_pid = 0u32;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut owner_pid));
        }
        if owner_pid != state.pid {
            return BOOL(1);
        }

        let mut rect = RECT::default();
        let has_rect = unsafe { GetWindowRect(hwnd, &mut rect).is_ok() };
        let visible = unsafe { IsWindowVisible(hwnd).as_bool() && !IsIconic(hwnd).as_bool() };
        let title = window_title(hwnd);
        let width = if has_rect {
            (rect.right - rect.left).max(0) as u32
        } else {
            0
        };
        let height = if has_rect {
            (rect.bottom - rect.top).max(0) as u32
        } else {
            0
        };

        state.windows.push(WindowInfo {
            id: (hwnd.0 as usize).to_string(),
            title,
            focused: hwnd == state.foreground,
            main: hwnd == state.foreground,
            visible,
            width,
            height,
        });
        BOOL(1)
    }

    let mut state = EnumState {
        pid: std::process::id(),
        foreground: unsafe { GetForegroundWindow() },
        windows: Vec::new(),
    };

    unsafe {
        EnumWindows(
            Some(enum_window),
            LPARAM((&mut state as *mut EnumState) as isize),
        )
    }
    .map_err(|err| PlatformError::Platform(format!("EnumWindows failed: {err}")))?;

    state.windows.sort_by(|a, b| {
        b.focused
            .cmp(&a.focused)
            .then_with(|| b.visible.cmp(&a.visible))
            .then_with(|| a.title.cmp(&b.title))
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(state.windows)
}

fn window_title(hwnd: windows::Win32::Foundation::HWND) -> String {
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowTextLengthW, GetWindowTextW};

    let len = unsafe { GetWindowTextLengthW(hwnd) };
    if len <= 0 {
        return String::new();
    }
    let mut buffer = vec![0u16; len as usize + 1];
    let copied = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    if copied <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buffer[..copied as usize])
}

fn open_with_shell(target: &str) -> Result<(), PlatformError> {
    let status = Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", target])
        .status()
        .map_err(|err| PlatformError::Platform(format!("failed to launch shell open: {err}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(PlatformError::Platform(format!(
            "shell open exited with status {status}"
        )))
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

fn normalize_file_uri(uri: &str) -> Result<PathBuf, PlatformError> {
    let trimmed = uri.trim();
    if trimmed.is_empty() {
        return Err(PlatformError::InvalidParameter(
            "file uri is empty".to_string(),
        ));
    }
    let path = trimmed
        .strip_prefix("file:///")
        .or_else(|| trimmed.strip_prefix("file://"))
        .unwrap_or(trimmed);
    Ok(PathBuf::from(path.replace('/', "\\")))
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
    std::env::var("LANG")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "en-US".to_string())
}

fn not_supported<T>(name: &str) -> Result<T, PlatformError> {
    Err(PlatformError::NotSupported(format!(
        "{name} is not supported on Windows yet"
    )))
}
