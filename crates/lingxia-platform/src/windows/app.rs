use std::fs::{self, File};
use std::future::Future;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

use lingxia_webview::WebTag;
use lingxia_webview::runtime as webview_runtime;

use super::{file, not_supported, surface, ui_update};
use crate::AssetFileEntry;
use crate::error::PlatformError;
use crate::traits::app_runtime::{AnimationType, AppRuntime, LxAppOpenMode, OpenUrlRequest};
use crate::traits::share::{ShareRequest, ShareResult, ShareService};
use crate::traits::stream_decoder::{VideoStreamDecoderHandle, VideoStreamDecoderManager};

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

/// Per-user launch-at-startup entries; HKCU needs no elevation. The value name
/// is the app identifier so renamed product titles keep pointing at one entry.
const AUTOSTART_RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

fn autostart_command(exe: &Path) -> String {
    format!("\"{}\"", exe.display())
}

fn read_autostart_run_entry(name: &str) -> Option<String> {
    use windows::Win32::System::Registry::HKEY_CURRENT_USER;

    super::registry::read_string(HKEY_CURRENT_USER, AUTOSTART_RUN_KEY, name)
}

fn write_autostart_run_entry(name: &str, exe: &Path) -> Result<(), PlatformError> {
    use windows::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey,
        RegCreateKeyExW, RegSetValueExW,
    };
    use windows::core::{HSTRING, PCWSTR};

    let subkey = HSTRING::from(AUTOSTART_RUN_KEY);
    let value = HSTRING::from(name);
    let data: Vec<u8> = autostart_command(exe)
        .encode_utf16()
        .chain(std::iter::once(0))
        .flat_map(u16::to_le_bytes)
        .collect();

    unsafe {
        let mut key = HKEY::default();
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            &subkey,
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut key,
            None,
        )
        .ok()
        .map_err(|err| PlatformError::Platform(format!("cannot open Run key: {err}")))?;
        let status = RegSetValueExW(key, &value, None, REG_SZ, Some(&data));
        let _ = RegCloseKey(key);
        status
            .ok()
            .map_err(|err| PlatformError::Platform(format!("cannot write Run entry: {err}")))
    }
}

fn remove_autostart_run_entry(name: &str) -> Result<(), PlatformError> {
    use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
    use windows::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, RegCloseKey, RegDeleteValueW, RegOpenKeyExW,
    };
    use windows::core::HSTRING;

    let subkey = HSTRING::from(AUTOSTART_RUN_KEY);
    let value = HSTRING::from(name);
    unsafe {
        let mut key = HKEY::default();
        let open = RegOpenKeyExW(HKEY_CURRENT_USER, &subkey, None, KEY_SET_VALUE, &mut key);
        if open == ERROR_FILE_NOT_FOUND {
            return Ok(());
        }
        open.ok()
            .map_err(|err| PlatformError::Platform(format!("cannot open Run key: {err}")))?;
        let status = RegDeleteValueW(key, &value);
        let _ = RegCloseKey(key);
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(());
        }
        status
            .ok()
            .map_err(|err| PlatformError::Platform(format!("cannot delete Run entry: {err}")))
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

/// Process-wide handler that pushes the JS-registered tray menu spec
/// (`items_json`, the array `lx.tray.setMenu` produced) to the native
/// system-tray icon. Registered by the Windows host SDK; when absent (headless
/// / non-shell builds) `set_tray_menu` no-ops, matching the trait contract.
type WindowsTrayMenuHandler = Arc<dyn Fn(&str) + Send + Sync>;
static WINDOWS_TRAY_MENU_HANDLER: Mutex<Option<WindowsTrayMenuHandler>> = Mutex::new(None);

pub fn set_windows_tray_menu_handler(handler: WindowsTrayMenuHandler) {
    if let Ok(mut slot) = WINDOWS_TRAY_MENU_HANDLER.lock() {
        *slot = Some(handler);
    }
}

fn invoke_windows_tray_menu_handler(items_json: &str) {
    let handler = WINDOWS_TRAY_MENU_HANDLER
        .lock()
        .ok()
        .and_then(|slot| slot.clone());
    if let Some(handler) = handler {
        handler(items_json);
    }
}

/// Process-wide handler toggling whether a tray left-click is delivered to JS
/// (`lx.tray.onClick`) instead of running the tray's configured surface action.
type WindowsTrayClickInterceptHandler = Arc<dyn Fn(bool) + Send + Sync>;
static WINDOWS_TRAY_CLICK_INTERCEPT_HANDLER: Mutex<Option<WindowsTrayClickInterceptHandler>> =
    Mutex::new(None);

pub fn set_windows_tray_click_intercept_handler(handler: WindowsTrayClickInterceptHandler) {
    if let Ok(mut slot) = WINDOWS_TRAY_CLICK_INTERCEPT_HANDLER.lock() {
        *slot = Some(handler);
    }
}

fn invoke_windows_tray_click_intercept_handler(intercept: bool) {
    let handler = WINDOWS_TRAY_CLICK_INTERCEPT_HANDLER
        .lock()
        .ok()
        .and_then(|slot| slot.clone());
    if let Some(handler) = handler {
        handler(intercept);
    }
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
        Self::from_env().unwrap_or_else(|_| {
            let base = default_state_root();
            Self {
                data_dir: base.join("data"),
                cache_dir: base.join("cache"),
                asset_dir: default_asset_dir(),
                locale: default_locale(),
                app_identifier: DEFAULT_APP_IDENTIFIER.to_string(),
                product_name: "LingXia".to_string(),
            }
        })
    }
}

impl Platform {
    pub fn from_env() -> Result<Self, PlatformError> {
        Self::from_asset_dir(default_asset_dir())
    }

    pub fn from_asset_dir(asset_dir: impl Into<PathBuf>) -> Result<Self, PlatformError> {
        let asset_dir = asset_dir.into();
        let config = GeneratedAppConfig::read_from_assets(&asset_dir);
        let product_name = config.product_name.unwrap_or_else(|| "LingXia".to_string());
        let app_identifier = config
            .windows_app_id
            .unwrap_or_else(|| DEFAULT_APP_IDENTIFIER.to_string());
        let root = state_root_for_product(&product_name);
        Ok(Self {
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            asset_dir,
            locale: default_locale(),
            app_identifier,
            product_name,
        })
    }

    pub fn asset_dir(&self) -> &Path {
        &self.asset_dir
    }

    pub(super) fn app_identifier(&self) -> &str {
        &self.app_identifier
    }

    /// Run-entry value name. `app_identifier` falls back to a constant shared
    /// by every LingXia app without an explicit `windows_app_id`, so two such
    /// apps would clobber each other's entry — disambiguate with the product
    /// name (the same per-app key `state_root_for_product` uses).
    fn autostart_value_name(&self) -> String {
        if self.app_identifier != DEFAULT_APP_IDENTIFIER {
            self.app_identifier.clone()
        } else {
            format!("{DEFAULT_APP_IDENTIFIER}.{}", self.product_name)
        }
    }

    pub(super) fn product_name(&self) -> &str {
        &self.product_name
    }

    /// Stamps this process's explicit AppUserModelID from the app identity so
    /// the taskbar groups windows per app, not per executable. Without it two
    /// apps served by the same exe (e.g. two dev runners for different
    /// projects) collapse into one taskbar button. Must run before the first
    /// window is created — Windows samples the id at window registration.
    pub fn install_taskbar_identity(&self) {
        use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
        use windows::core::PCWSTR;

        // Same disambiguation rule as `autostart_value_name`: an explicit
        // `windows_app_id` is authoritative; the shared default identifier is
        // qualified by product name so distinct products don't collide. AUMIDs
        // must contain no spaces and stay under 128 characters.
        let id = self.autostart_value_name();
        let id: String = id
            .chars()
            .map(|c| if c.is_whitespace() { '.' } else { c })
            .take(127)
            .collect();
        let wide: Vec<u16> = id.encode_utf16().chain(std::iter::once(0)).collect();
        if let Err(err) = unsafe { SetCurrentProcessExplicitAppUserModelID(PCWSTR(wide.as_ptr())) }
        {
            log::warn!("failed to set process AppUserModelID {id:?}: {err}");
        }
    }

    pub(super) fn data_dir(&self) -> &Path {
        &self.data_dir
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
        if !matches!(open_mode, LxAppOpenMode::Panel) {
            ui_update::sync_windows_ui(&appid);
        }
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

    fn autostart_is_enabled(&self) -> Result<bool, PlatformError> {
        let exe = std::env::current_exe().map_err(|err| {
            PlatformError::Platform(format!("cannot resolve app executable: {err}"))
        })?;
        // Enabled means the entry points at *this* executable: after a
        // reinstall to a new path the stale entry no longer launches the app,
        // so it must read as disabled (re-enabling then rewrites the path).
        Ok(read_autostart_run_entry(&self.autostart_value_name())
            .is_some_and(|cmd| cmd.eq_ignore_ascii_case(&autostart_command(&exe))))
    }

    fn autostart_set_enabled(&self, enabled: bool) -> Result<(), PlatformError> {
        let name = self.autostart_value_name();
        if enabled {
            let exe = std::env::current_exe().map_err(|err| {
                PlatformError::Platform(format!("cannot resolve app executable: {err}"))
            })?;
            write_autostart_run_entry(&name, &exe)
        } else {
            remove_autostart_run_entry(&name)
        }
    }

    // Tray chrome. The system-tray icon and its menu live in the host SDK
    // (lingxia-windows-sdk), which this layer cannot reference directly, so the
    // SDK registers handlers we forward to. No registered handler => no-op,
    // honoring the "tray APIs never throw off-support" contract.

    fn set_tray_menu(&self, items_json: &str) -> Result<(), PlatformError> {
        invoke_windows_tray_menu_handler(items_json);
        Ok(())
    }

    fn set_tray_click_intercept(&self, intercept: bool) -> Result<(), PlatformError> {
        invoke_windows_tray_click_intercept_handler(intercept);
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
        ui_update::sync_windows_ui(&appid);
        surface::navigate_webtag_window(webtag, self.product_name.clone());
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
        // No capsule button exists in the Windows shell; a zero rect tells
        // callers there is nothing to avoid overlapping.
        async { Ok(r#"{"width":0,"height":0,"top":0,"right":0,"bottom":0,"left":0}"#.to_string()) }
    }
}

// lx.surface: the surface's content webview is created by lxapp; the
// presenter shows it as a desktop window and reports closes back to the logic
// layer through the callback the `lingxia` facade registers (see
// surface::set_windows_surface_closed_handler).
impl crate::traits::ui::SurfacePresenter for Platform {
    fn present_layout(
        &self,
        window_id: &str,
        plan: &lingxia_surface::LayoutPresentationPlan,
    ) -> Result<(), PlatformError> {
        surface::present_layout(window_id, plan, &self.product_name)
    }

    fn present_surface(
        &self,
        request: crate::traits::ui::SurfaceRequest,
    ) -> Result<(), PlatformError> {
        surface::present_surface(request, &self.product_name)
    }

    fn close_surface(&self, app_id: &str, id: &str, reason: &str) -> Result<(), PlatformError> {
        surface::close_surface(app_id, id, reason)
    }

    fn show_surface(&self, app_id: &str, id: &str) -> Result<(), PlatformError> {
        surface::show_surface(app_id, id)
    }

    fn hide_surface(&self, app_id: &str, id: &str) -> Result<(), PlatformError> {
        surface::hide_surface(app_id, id)
    }

    fn set_managed_surface_visible(
        &self,
        id: &str,
        visible: bool,
        edge: Option<&str>,
    ) -> Result<(), PlatformError> {
        surface::set_managed_surface_visible(id, visible, edge)
    }

    fn toggle_managed_surface(&self, id: &str) -> Result<(), PlatformError> {
        surface::toggle_managed_surface(id)
    }
}
impl ShareService for Platform {
    // Stubbed: the Windows share sheet (DataTransferManager) must be obtained
    // through IDataTransferManagerInterop with an owning HWND, and the
    // platform layer deliberately has no access to shell window handles.
    fn share(
        &self,
        _request: ShareRequest,
    ) -> impl Future<Output = Result<ShareResult, PlatformError>> + Send {
        async { not_supported("share") }
    }
}

impl VideoStreamDecoderManager for Platform {
    // Stubbed: requires a Media Foundation decode pipeline; see bind_player.
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

/// An explicit state root, when `LINGXIA_STATE_ROOT` is set. This isolates a
/// process's data/cache from the default per-product location — the dev runner
/// sets it per lxapp so two runners (different projects) don't collide on the
/// single per-product metadata database (redb takes an exclusive file lock) or
/// the shared WebView2 profile, which is what blocks running two at once.
fn state_root_override() -> Option<PathBuf> {
    std::env::var_os("LINGXIA_STATE_ROOT")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

fn default_state_root() -> PathBuf {
    state_root_override().unwrap_or_else(|| {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("LingXia")
    })
}

fn state_root_for_product(product_name: &str) -> PathBuf {
    state_root_override().unwrap_or_else(|| {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join(product_name)
    })
}

fn default_asset_dir() -> PathBuf {
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

#[derive(Debug, Default)]
struct GeneratedAppConfig {
    product_name: Option<String>,
    windows_app_id: Option<String>,
}

impl GeneratedAppConfig {
    fn read_from_assets(asset_dir: &Path) -> Self {
        let Ok(content) = std::fs::read_to_string(asset_dir.join("app.json")) else {
            return Self::default();
        };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) else {
            return Self::default();
        };
        Self {
            product_name: json
                .get("productName")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            windows_app_id: json
                .get("windowsAppId")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
        }
    }
}
