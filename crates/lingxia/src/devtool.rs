use std::path::PathBuf;
use std::sync::OnceLock;

use lingxia_platform::traits::app_runtime::AppRuntime;
use lxapp::automation as auto;

const LXAPP_PATH_ENV: &str = "LINGXIA_LXAPP_PATH";
const RUNNER_WEB_URL_ENV: &str = "LINGXIA_RUNNER_WEB_URL";

mod sync;

#[derive(Debug, serde::Deserialize)]
#[allow(non_snake_case)]
struct LxAppManifest {
    appId: String,
    version: String,
}

/// Identity of the LxApp currently exposed through host devtool helpers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LxAppDevIdentity {
    /// LxApp id from `lxapp.json`.
    pub appid: String,
    /// LxApp version from `lxapp.json`.
    pub version: String,
}

/// Explicit host devtool configuration for loading an unpacked LxApp bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LxAppDevConfig {
    /// Root directory that contains the runnable LxApp bundle.
    pub root: PathBuf,
    /// LxApp identity advertised to the host runtime.
    pub identity: LxAppDevIdentity,
}

static LXAPP_DEV_CONFIG: OnceLock<LxAppDevConfig> = OnceLock::new();

/// Installs an explicit devtool bundle override for the current process.
///
/// Returns `true` when the configuration is installed or when the same
/// configuration had already been installed. Returns `false` when a conflicting
/// config was already present.
pub fn install_lxapp_dev_config(config: LxAppDevConfig) -> bool {
    if let Some(existing) = LXAPP_DEV_CONFIG.get() {
        if existing == &config {
            return true;
        }
        log::warn!(
            "Lxapp dev config already set for appid={}, refusing conflicting appid={}",
            existing.identity.appid,
            config.identity.appid
        );
        return false;
    }

    match LXAPP_DEV_CONFIG.set(config.clone()) {
        Ok(()) => {
            log::info!(
                "Installed explicit lxapp dev config: appid={}, version={}, root={}",
                config.identity.appid,
                config.identity.version,
                config.root.display()
            );
            true
        }
        Err(_) => {
            log::warn!("Lxapp dev config already set");
            false
        }
    }
}

pub(crate) fn lxapp_dev_config() -> Option<&'static LxAppDevConfig> {
    LXAPP_DEV_CONFIG.get()
}

// Simulated-device control lives in `lxapp::device` so both the devtool
// handlers and the `lx.automation()` host tier share one registry. Re-exported
// here to keep the `lingxia::dev::device_*` / `register_device_controller`
// surface (and the runner's call site) unchanged.
pub use lxapp::device::{
    DeviceController, DeviceEntry, DeviceState, device_get, device_list, device_set,
    register_device_controller,
};

fn resolve_runnable_lxapp_path(path: &std::path::Path) -> PathBuf {
    let dist_path = path.join("dist");
    if dist_path.join("lxapp.json").exists() {
        return dist_path;
    }
    path.to_path_buf()
}

fn read_lxapp_manifest(path: &std::path::Path) -> Result<LxAppManifest, String> {
    let manifest_path = path.join("lxapp.json");
    let content = std::fs::read_to_string(&manifest_path)
        .map_err(|e| format!("failed to read {}: {}", manifest_path.display(), e))?;
    let manifest: LxAppManifest = serde_json::from_str(&content)
        .map_err(|e| format!("invalid {}: {}", manifest_path.display(), e))?;
    let appid = manifest.appId.trim();
    if appid.is_empty() {
        return Err(format!(
            r#""appId" must not be empty in {}"#,
            manifest_path.display()
        ));
    }
    let version = manifest.version.trim();
    if version.is_empty() {
        return Err(format!(
            r#""version" must not be empty in {}"#,
            manifest_path.display()
        ));
    }
    Ok(LxAppManifest {
        appId: appid.to_string(),
        version: version.to_string(),
    })
}

/// Installs devtool config from the `LINGXIA_LXAPP_PATH` environment variable.
///
/// The path may point either at a built `dist/` directory or at a project root
/// that contains one.
pub fn install_lxapp_dev_config_from_env() -> bool {
    let Ok(raw_path) = std::env::var(LXAPP_PATH_ENV) else {
        return false;
    };

    let path = raw_path.trim();
    if path.is_empty() {
        log::warn!("{LXAPP_PATH_ENV} is set but empty; ignoring");
        return false;
    }

    let root = resolve_runnable_lxapp_path(&PathBuf::from(path));
    if !root.exists() {
        log::warn!("{LXAPP_PATH_ENV} path does not exist: {}", root.display());
        return false;
    }
    if !root.join("logic.js").exists() {
        log::warn!(
            "{LXAPP_PATH_ENV} logic.js not found in {} (continuing; build output may be incomplete)",
            root.display()
        );
    }

    let manifest = match read_lxapp_manifest(&root) {
        Ok(manifest) => manifest,
        Err(err) => {
            log::warn!(
                "Failed to initialize lxapp dev config from {}={}: {}",
                LXAPP_PATH_ENV,
                path,
                err
            );
            return false;
        }
    };

    install_lxapp_dev_config(LxAppDevConfig {
        root,
        identity: LxAppDevIdentity {
            appid: manifest.appId,
            version: manifest.version,
        },
    })
}

fn build_host_app_config(
    runtime: &lingxia_platform::Platform,
    dev_config: &LxAppDevConfig,
) -> lingxia_app_context::AppConfig {
    build_default_host_app_config(
        runtime,
        dev_config.identity.appid.clone(),
        dev_config.identity.version.clone(),
    )
}

fn build_default_host_app_config(
    runtime: &lingxia_platform::Platform,
    home_app_id: String,
    home_app_version: String,
) -> lingxia_app_context::AppConfig {
    let product_name = runtime
        .get_app_identifier()
        .ok()
        .filter(|value: &String| !value.trim().is_empty())
        .unwrap_or_else(|| "LingXia Host".to_string());

    lingxia_app_context::AppConfig {
        product_name,
        product_version: env!("CARGO_PKG_VERSION").to_string(),
        lingxia_id: None,
        lingxia_server: None,
        env_version: lingxia_app_context::EnvVersion::Developer,
        home_app_id,
        home_app_version,
        cache_max_size_mb: 1024,
        storage: None,
        dev_ws_url: None,
        dev_bundle_base_url: None,
        app_links: None,
        capabilities: None,
        panels: None,
    }
}

fn build_web_runner_app_config(
    runtime: &lingxia_platform::Platform,
) -> lingxia_app_context::AppConfig {
    build_default_host_app_config(runtime, String::new(), String::new())
}

pub(crate) fn prepare_host_app_config(
    runtime: &std::sync::Arc<lingxia_platform::Platform>,
    load_bundled: impl FnOnce(
        &std::sync::Arc<lingxia_platform::Platform>,
    ) -> Option<lingxia_app_context::AppConfig>,
) -> Option<lingxia_app_context::AppConfig> {
    let _ = install_lxapp_dev_config_from_env();

    let Some(dev_config) = lxapp_dev_config() else {
        if std::env::var(RUNNER_WEB_URL_ENV)
            .ok()
            .is_some_and(|value| !value.trim().is_empty())
        {
            let mut config = match runtime.read_asset("app.json") {
                Ok(_) => load_bundled(runtime)?,
                Err(lingxia_platform::error::PlatformError::AssetNotFound(path))
                    if path == "app.json" =>
                {
                    build_web_runner_app_config(runtime.as_ref())
                }
                Err(error) => {
                    log::error!("Failed to read app.json: {}", error);
                    return None;
                }
            };
            config.home_app_id.clear();
            config.home_app_version.clear();
            return Some(config);
        }
        return load_bundled(runtime);
    };

    let mut app_config = match runtime.read_asset("app.json") {
        Ok(_) => load_bundled(runtime)?,
        Err(lingxia_platform::error::PlatformError::AssetNotFound(path)) if path == "app.json" => {
            log::info!(
                "Bootstrapping host in explicit lxapp dev mode using host defaults for {}",
                dev_config.identity.appid
            );
            build_host_app_config(runtime.as_ref(), dev_config)
        }
        Err(e) => {
            log::error!("Failed to read app.json: {}", e);
            return None;
        }
    };
    app_config.home_app_id = dev_config.identity.appid.clone();
    app_config.home_app_version = dev_config.identity.version.clone();
    Some(app_config)
}

pub(crate) fn prepare_bundle_sources(runtime: &std::sync::Arc<lingxia_platform::Platform>) {
    if let Err(err) = sync::sync_dev_home_bundle(runtime.clone()) {
        log::warn!("Failed to sync dev home lxapp bundle: {}", err);
    }
    register_bundle_source_override();
}

fn register_bundle_source_override() {
    let Some(dev_config) = lxapp_dev_config() else {
        return;
    };
    lxapp::register_dev_bundle_source(dev_config.identity.appid.clone(), dev_config.root.clone());
}

/// Snapshot of a page exposed by the active devtool target.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LxAppDevPageInfo {
    /// App id that owns the page.
    pub appid: String,
    /// Whether this route is declared in lxapp.json.
    pub declared: bool,
    /// Whether a live runtime instance currently exists.
    pub opened: bool,
    /// Stable id for this runtime page instance.
    pub instance_id: String,
    /// Declarative page name from the runtime manifest, when available.
    pub name: String,
    /// Runtime page path.
    pub path: String,
    /// Current page query data.
    pub query: serde_json::Value,
    /// Whether this page is the current foreground page.
    pub current: bool,
    /// Whether this page still exists in the navigation stack.
    pub in_stack: bool,
    /// Position in the navigation stack, oldest first.
    pub stack_index: Option<usize>,
    /// Runtime owner (`host`, `scene`, or an owning page instance).
    pub owner: serde_json::Value,
    /// Presentation kind for this page instance.
    pub presentation: Option<String>,
    /// Runtime page-instance lifecycle.
    pub lifecycle: Option<String>,
    /// Dynamic surfaces currently hosting this page instance.
    pub surface_ids: Vec<String>,
    /// Whether the lxapp page lifecycle has fired `OnReady`.
    pub ready: bool,
    /// Whether the page currently has an attached WebView.
    pub webview_attached: bool,
    pub webview_ready: bool,
    pub webview_error: Option<String>,
    pub bridge_ready: bool,
    pub render_state: String,
    /// Whether page element input actions are supported on this platform.
    pub input_supported: bool,
}

/// Page or DOM condition accepted by [`lxapp_dev_page_wait`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LxAppDevPageWaitState {
    Ready,
    Attached,
    Detached,
    Visible,
    Hidden,
    Enabled,
    Editable,
}

impl LxAppDevPageWaitState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Attached => "attached",
            Self::Detached => "detached",
            Self::Visible => "visible",
            Self::Hidden => "hidden",
            Self::Enabled => "enabled",
            Self::Editable => "editable",
        }
    }

    fn matches(self, element: &serde_json::Value) -> bool {
        let exists = element
            .get("exists")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        match self {
            Self::Ready => false,
            Self::Attached => exists,
            Self::Detached => !exists,
            Self::Visible => exists && json_bool(element, "visible"),
            Self::Hidden => exists && !json_bool(element, "visible"),
            Self::Enabled => exists && json_bool(element, "enabled"),
            Self::Editable => exists && json_bool(element, "editable"),
        }
    }
}

fn json_bool(value: &serde_json::Value, key: &str) -> bool {
    value
        .get(key)
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

/// Result of a successful page wait.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LxAppDevPageWaitResult {
    pub state: LxAppDevPageWaitState,
    pub selector: Option<String>,
    pub page: LxAppDevPageInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element: Option<serde_json::Value>,
}

/// Returns information about the current page for the selected app.
pub fn lxapp_dev_page_current(appid: Option<&str>) -> Result<LxAppDevPageInfo, String> {
    let app = auto::resolve_lxapp(appid.unwrap_or("current"))?;
    let (page, name) = auto::resolve_page(&app, None)?;
    Ok(dev_page_info(&app, &page, name.as_deref()))
}

/// Lists known pages for the selected app and marks current/stack state.
pub fn lxapp_dev_page_list(appid: Option<&str>) -> Result<Vec<LxAppDevPageInfo>, String> {
    let app = resolve_dev_lxapp(appid.unwrap_or("current"))?;
    let runtime = app.runtime_info();
    let surfaces = app.runtime_surface_info();
    let pages = app
        .page_instance_runtime_info()
        .into_iter()
        .map(|page| dev_page_runtime_info(&runtime.appid, &runtime.page_entries, &surfaces, page))
        .collect::<Vec<_>>();
    Ok(complete_dev_page_list(
        &runtime.appid,
        &runtime.page_entries,
        pages,
    ))
}

fn complete_dev_page_list(
    appid: &str,
    entries: &[lxapp::LxAppRuntimePageInfo],
    mut pages: Vec<LxAppDevPageInfo>,
) -> Vec<LxAppDevPageInfo> {
    for entry in entries {
        if pages
            .iter()
            .any(|page| dev_page_paths_match(&page.path, &entry.path))
        {
            continue;
        }
        pages.push(LxAppDevPageInfo {
            appid: appid.to_string(),
            declared: true,
            opened: false,
            instance_id: String::new(),
            name: entry.name.clone(),
            path: entry.path.clone(),
            query: serde_json::Value::Null,
            current: false,
            in_stack: false,
            stack_index: None,
            owner: serde_json::Value::Null,
            presentation: None,
            lifecycle: None,
            surface_ids: Vec::new(),
            ready: false,
            webview_attached: false,
            webview_ready: false,
            webview_error: None,
            bridge_ready: false,
            render_state: "unopened".to_string(),
            input_supported: lxapp_dev_page_input_supported(),
        });
    }
    pages
}

/// Returns information for a specific page, or the current page if omitted.
pub fn lxapp_dev_page_info(
    appid: Option<&str>,
    page_name: Option<&str>,
) -> Result<LxAppDevPageInfo, String> {
    let app = auto::resolve_lxapp(appid.unwrap_or("current"))?;
    let (page, name) = auto::resolve_page(&app, page_name)?;
    Ok(dev_page_info(&app, &page, name.as_deref()))
}

/// Waits for an lxapp page to finish its runtime lifecycle, or for a DOM
/// element in that stable page instance to reach the requested state.
pub async fn lxapp_dev_page_wait(
    appid: Option<&str>,
    page_name: Option<&str>,
    selector: Option<&str>,
    index: Option<usize>,
    state: LxAppDevPageWaitState,
    timeout: std::time::Duration,
) -> Result<LxAppDevPageWaitResult, String> {
    let app = resolve_dev_lxapp(appid.unwrap_or("current"))?;
    let (page, name) = resolve_dev_page(&app, page_name)?;
    let selector = selector.map(str::trim).filter(|value| !value.is_empty());

    if state == LxAppDevPageWaitState::Ready && selector.is_some() {
        return Err("--css cannot be combined with --state ready".to_string());
    }
    if state != LxAppDevPageWaitState::Ready && selector.is_none() {
        return Err(format!("--css is required for --state {}", state.as_str()));
    }

    let query_script = selector
        .map(|selector| auto::build_query_script(selector, index, false, Some(4096)))
        .transpose()?;
    let deadline = tokio::time::Instant::now()
        .checked_add(timeout)
        .ok_or_else(|| "page wait timeout is too large".to_string())?;
    let instance_id = page.instance_id_string();
    let mut last_element = None;
    let mut last_error = None;

    loop {
        if app.get_page_by_instance_id_str(&instance_id).is_none() {
            return Err(format!(
                "page instance {instance_id} was disposed before wait completed"
            ));
        }
        let automation = page.automation_state();
        if let Some(error) = automation.webview_error {
            return Err(format!(
                "page WebView failed before wait completed: {error}"
            ));
        }
        if state == LxAppDevPageWaitState::Ready && automation.ready {
            return Ok(LxAppDevPageWaitResult {
                state,
                selector: None,
                page: dev_page_info(&app, &page, name.as_deref()),
                element: None,
            });
        }

        if let (Some(script), Some(webview)) = (query_script.as_deref(), page.webview()) {
            match webview.evaluate_javascript(script).await {
                Ok(element) => {
                    last_error = None;
                    if state.matches(&element) {
                        return Ok(LxAppDevPageWaitResult {
                            state,
                            selector: selector.map(str::to_string),
                            page: dev_page_info(&app, &page, name.as_deref()),
                            element: Some(element),
                        });
                    }
                    last_element = Some(element);
                }
                Err(error) => {
                    let error = error.to_string();
                    if error.to_ascii_lowercase().contains("invalid selector") {
                        return Err(error);
                    }
                    last_error = Some(error);
                }
            }
        }

        let now = tokio::time::Instant::now();
        if now >= deadline {
            let target = selector
                .map(|selector| format!(" selector {selector:?}"))
                .unwrap_or_default();
            let detail = last_error
                .map(|error| format!("; last evaluation error: {error}"))
                .or_else(|| last_element.map(|element| format!("; last element state: {element}")))
                .unwrap_or_default();
            return Err(format!(
                "timed out after {}ms waiting for page {instance_id}{target} to become {}{detail}",
                timeout.as_millis(),
                state.as_str(),
            ));
        }
        tokio::time::sleep(std::cmp::min(
            std::time::Duration::from_millis(50),
            deadline.saturating_duration_since(now),
        ))
        .await;
    }
}

/// Restarts an lxapp and waits for its replacement runtime session and entry
/// page to finish the lxapp lifecycle.
pub async fn lxapp_dev_restart(
    appid: &str,
    timeout: std::time::Duration,
) -> Result<LxAppDevPageInfo, String> {
    let appid = resolve_dev_appid(appid)?;
    let app = resolve_dev_lxapp(&appid)?;
    let previous_session = app.runtime_info().session_id;

    // A host dev session runs from its synchronized bundle cache rather than
    // directly from the project dist directory. Pull the freshly generated
    // manifest and files before restarting so reload cannot serve stale code.
    let runtime = crate::runtime::platform().map_err(|err| err.to_string())?;
    sync::sync_dev_home_bundle(runtime)?;

    lxapp::restart_lxapp(&appid).map_err(|err| err.to_string())?;
    let deadline = tokio::time::Instant::now()
        .checked_add(timeout)
        .ok_or_else(|| "restart timeout is too large".to_string())?;

    loop {
        if let Some(app) = lxapp::try_get(&appid) {
            let runtime = app.runtime_info();
            if runtime.session_id != previous_session
                && runtime.status == "opened"
                && let Ok((page, name)) = resolve_dev_page(&app, None)
            {
                let state = page.automation_state();
                if let Some(error) = state.webview_error {
                    return Err(format!("restarted page WebView failed: {error}"));
                }
                if state.ready {
                    return Ok(dev_page_info(&app, &page, name.as_deref()));
                }
            }
        }
        let now = tokio::time::Instant::now();
        if now >= deadline {
            return Err(format!(
                "timed out after {}ms waiting for lxapp {appid} to restart",
                timeout.as_millis()
            ));
        }
        tokio::time::sleep(std::cmp::min(
            std::time::Duration::from_millis(50),
            deadline.saturating_duration_since(now),
        ))
        .await;
    }
}

/// Evaluates JavaScript in the target page WebView and returns the raw JSON result.
pub async fn lxapp_dev_page_eval(
    appid: Option<&str>,
    page_name: Option<&str>,
    js: &str,
) -> Result<serde_json::Value, String> {
    let app = auto::resolve_lxapp(appid.unwrap_or("current"))?;
    auto::page_eval(&app, page_name, js).await
}

/// Capture a PNG snapshot of the host app's entire window (not just a
/// WebView). Returns raw PNG bytes. The window includes host UI overlays,
/// native controls, and any composited WebViews — useful when verifying
/// app-level layout or surfacing host-drawn elements that WebView-only
/// screenshots cannot see.
///
/// `window_id` is the platform-specific id returned by [`list_app_windows`].
/// Passing `None` lets the platform pick (focused/main on desktop; the sole
/// window on mobile).
pub async fn take_app_screenshot(window_id: Option<&str>) -> Result<Vec<u8>, String> {
    take_app_screenshot_with_info(window_id)
        .await
        .map(|(_, bytes)| bytes)
}

/// Captures a host window and returns the concrete window selected alongside
/// the PNG, so screenshot coordinates can be fed back into app input safely.
pub async fn take_app_screenshot_with_info(
    window_id: Option<&str>,
) -> Result<(lingxia_platform::traits::screenshot::WindowInfo, Vec<u8>), String> {
    use lingxia_platform::traits::screenshot::AppScreenshot;
    let platform =
        lxapp::get_platform().ok_or_else(|| "platform is not initialized".to_string())?;
    let window = platform
        .resolve_app_window(window_id)
        .await
        .map_err(|err| err.to_string())?;
    let bytes = platform
        .take_app_screenshot(Some(&window.id))
        .await
        .map_err(|err| err.to_string())?;
    Ok((window, bytes))
}

/// Enumerate the host app's top-level windows. Mobile platforms return a
/// single entry; desktop platforms return one entry per open window.
pub async fn list_app_windows()
-> Result<Vec<lingxia_platform::traits::screenshot::WindowInfo>, String> {
    use lingxia_platform::traits::screenshot::AppScreenshot;
    let platform =
        lxapp::get_platform().ok_or_else(|| "platform is not initialized".to_string())?;
    platform
        .list_app_windows()
        .await
        .map_err(|err| err.to_string())
}

/// Dispatch mouse input to the host app's window.
///
/// Coordinates use platform window-content units: client pixels on Windows
/// and points on macOS, with origin at the top-left corner.
pub async fn perform_app_mouse(
    request: lingxia_platform::traits::mouse::AppMouseRequest,
) -> Result<lingxia_platform::traits::mouse::AppMouseResult, String> {
    use lingxia_platform::traits::mouse::AppMouse;
    let platform =
        lxapp::get_platform().ok_or_else(|| "platform is not initialized".to_string())?;
    platform
        .perform_app_mouse(request)
        .await
        .map_err(|err| err.to_string())
}

/// Dispatch keyboard input to the host app's focused window control.
pub async fn perform_app_keyboard(
    request: lingxia_platform::traits::keyboard::AppKeyboardRequest,
) -> Result<lingxia_platform::traits::keyboard::AppKeyboardResult, String> {
    use lingxia_platform::traits::keyboard::AppKeyboard;
    let platform =
        lxapp::get_platform().ok_or_else(|| "platform is not initialized".to_string())?;
    platform
        .perform_app_keyboard(request)
        .await
        .map_err(|err| err.to_string())
}

/// Capture a PNG screenshot of the target lxapp page's WebView.
/// Returns raw PNG bytes.
pub async fn lxapp_dev_page_screenshot(
    appid: Option<&str>,
    page_name: Option<&str>,
) -> Result<Vec<u8>, String> {
    lxapp_dev_page_screenshot_with_info(appid, page_name)
        .await
        .map(|(_, bytes)| bytes)
}

/// Captures a page screenshot and reports metadata for the exact page instance
/// that supplied the bytes.
pub async fn lxapp_dev_page_screenshot_with_info(
    appid: Option<&str>,
    page_name: Option<&str>,
) -> Result<(LxAppDevPageInfo, Vec<u8>), String> {
    let app = resolve_dev_lxapp(appid.unwrap_or("current"))?;
    let (page, name) = resolve_dev_page(&app, page_name)?;
    let info = dev_page_info(&app, &page, name.as_deref());
    let bytes = page
        .webview()
        .ok_or_else(|| "page WebView is not ready".to_string())?
        .take_screenshot()
        .await
        .map_err(|err| err.to_string())?;
    Ok((info, bytes))
}

/// Queries DOM nodes in the target page and returns a JSON description payload.
pub async fn lxapp_dev_page_query(
    appid: Option<&str>,
    page_name: Option<&str>,
    selector: &str,
    index: Option<usize>,
    all: bool,
    max_text: Option<usize>,
) -> Result<serde_json::Value, String> {
    let app = auto::resolve_lxapp(appid.unwrap_or("current"))?;
    auto::page_query(&app, page_name, selector, index, all, max_text).await
}

/// Clicks the matching DOM node in the target page.
pub async fn lxapp_dev_page_click(
    appid: Option<&str>,
    page_name: Option<&str>,
    selector: &str,
    index: Option<usize>,
) -> Result<(), String> {
    let app = auto::resolve_lxapp(appid.unwrap_or("current"))?;
    auto::page_click(&app, page_name, selector, index).await
}

/// Types text into the matching editable DOM node without clearing existing content.
pub async fn lxapp_dev_page_type(
    appid: Option<&str>,
    page_name: Option<&str>,
    selector: &str,
    index: Option<usize>,
    text: &str,
) -> Result<(), String> {
    let app = auto::resolve_lxapp(appid.unwrap_or("current"))?;
    auto::page_type(&app, page_name, selector, index, text).await
}

/// Replaces the matching editable DOM node content with the provided text.
pub async fn lxapp_dev_page_fill(
    appid: Option<&str>,
    page_name: Option<&str>,
    selector: &str,
    index: Option<usize>,
    text: &str,
) -> Result<(), String> {
    let app = auto::resolve_lxapp(appid.unwrap_or("current"))?;
    auto::page_fill(&app, page_name, selector, index, text).await
}

/// Sends a key press to the target page WebView.
pub async fn lxapp_dev_page_press(
    appid: Option<&str>,
    page_name: Option<&str>,
    key: &str,
    selector: Option<&str>,
    index: Option<usize>,
) -> Result<(), String> {
    let app = auto::resolve_lxapp(appid.unwrap_or("current"))?;
    auto::page_press(&app, page_name, key, selector, index).await
}

/// Scrolls the page DOM by `(dx, dy)`, walking up to the nearest scrollable
/// container so internal scroll regions move, not just the document.
pub async fn lxapp_dev_page_scroll(
    appid: Option<&str>,
    page_name: Option<&str>,
    dx: f64,
    dy: f64,
) -> Result<(), String> {
    let app = auto::resolve_lxapp(appid.unwrap_or("current"))?;
    auto::page_scroll(&app, page_name, dx, dy).await
}

/// Scrolls the first matching DOM node into view.
pub async fn lxapp_dev_page_scroll_to(
    appid: Option<&str>,
    page_name: Option<&str>,
    selector: &str,
) -> Result<(), String> {
    let app = auto::resolve_lxapp(appid.unwrap_or("current"))?;
    auto::page_scroll_to(&app, page_name, selector).await
}

/// Navigates back in the current page stack by the requested delta.
pub fn lxapp_dev_page_back(appid: Option<&str>, delta: u32) -> Result<(), String> {
    let app = auto::resolve_lxapp(appid.unwrap_or("current"))?;
    app.current_page()
        .map_err(|err| err.to_string())?
        .navigate_back(delta)
        .map_err(|err| err.to_string())
}

/// Navigate to a configured page by page name.
pub async fn lxapp_dev_nav_to(
    appid: Option<&str>,
    page_name: &str,
    query: Option<serde_json::Value>,
) -> Result<LxAppDevPageInfo, String> {
    lxapp_dev_nav_with_kind(appid, page_name, query, lxapp::NavigationType::Forward).await
}

/// Replace the current page with a configured page by page name.
pub async fn lxapp_dev_nav_redirect(
    appid: Option<&str>,
    page_name: &str,
    query: Option<serde_json::Value>,
) -> Result<LxAppDevPageInfo, String> {
    lxapp_dev_nav_with_kind(appid, page_name, query, lxapp::NavigationType::Replace).await
}

/// Switch to a configured tab page by page name.
pub async fn lxapp_dev_nav_switch_tab(
    appid: Option<&str>,
    page_name: &str,
    query: Option<serde_json::Value>,
) -> Result<LxAppDevPageInfo, String> {
    lxapp_dev_nav_with_kind(appid, page_name, query, lxapp::NavigationType::SwitchTab).await
}

/// Relaunch the lxapp at a configured page by page name.
pub async fn lxapp_dev_nav_relaunch(
    appid: Option<&str>,
    page_name: &str,
    query: Option<serde_json::Value>,
) -> Result<LxAppDevPageInfo, String> {
    lxapp_dev_nav_with_kind(appid, page_name, query, lxapp::NavigationType::Launch).await
}

/// Navigate back in the current page stack and return the destination page.
pub async fn lxapp_dev_nav_back(
    appid: Option<&str>,
    delta: u32,
) -> Result<LxAppDevPageInfo, String> {
    let app = auto::resolve_lxapp(appid.unwrap_or("current"))?;
    let (page, name) = auto::navigate_back(&app, delta, false).await?;
    wait_dev_page_runtime_ready(&app, &page, name.as_deref()).await
}

async fn lxapp_dev_nav_with_kind(
    appid: Option<&str>,
    page_name: &str,
    query: Option<serde_json::Value>,
    kind: lxapp::NavigationType,
) -> Result<LxAppDevPageInfo, String> {
    let app = auto::resolve_lxapp(appid.unwrap_or("current"))?;
    let (page, name) = auto::navigate(&app, page_name, query.as_ref(), kind, false).await?;
    wait_dev_page_runtime_ready(&app, &page, name.as_deref()).await
}

async fn wait_dev_page_runtime_ready(
    app: &std::sync::Arc<lxapp::LxApp>,
    page: &lxapp::PageInstance,
    name: Option<&str>,
) -> Result<LxAppDevPageInfo, String> {
    let timeout = std::time::Duration::from_secs(15);
    let deadline = tokio::time::Instant::now() + timeout;
    let instance_id = page.instance_id_string();
    loop {
        let state = page.automation_state();
        if let Some(error) = state.webview_error {
            return Err(format!(
                "page WebView failed before runtime became ready: {error}"
            ));
        }
        if state.ready {
            return Ok(dev_page_info(app, page, name));
        }
        if app.get_page_by_instance_id_str(&instance_id).is_none() {
            return Err(format!(
                "page instance {instance_id} was disposed before runtime became ready"
            ));
        }
        let now = tokio::time::Instant::now();
        if now >= deadline {
            return Err(format!(
                "timed out after {}ms waiting for page {instance_id} to become ready",
                timeout.as_millis()
            ));
        }
        tokio::time::sleep(std::cmp::min(
            std::time::Duration::from_millis(50),
            deadline.saturating_duration_since(now),
        ))
        .await;
    }
}

fn resolve_dev_lxapp(raw: &str) -> Result<std::sync::Arc<lxapp::LxApp>, String> {
    let appid = resolve_dev_appid(raw)?;
    lxapp::try_get(&appid).ok_or_else(|| format!("lxapp is not active: {appid}"))
}

fn resolve_dev_appid(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.eq_ignore_ascii_case("current") {
        let (appid, _, _) = lxapp::get_current_lxapp();
        if appid.is_empty() {
            Err("no current lxapp".to_string())
        } else {
            Ok(appid)
        }
    } else if trimmed.is_empty() {
        Err("appid is required".to_string())
    } else {
        Ok(trimmed.to_string())
    }
}

fn resolve_dev_page(
    app: &std::sync::Arc<lxapp::LxApp>,
    page_name: Option<&str>,
) -> Result<(lxapp::PageInstance, Option<String>), String> {
    auto::resolve_page(app, page_name)
}

fn dev_page_path_key(path: &str) -> String {
    let (path, _) = lxapp::startup::split_path_query(path);
    path.trim_start_matches('/').to_string()
}

fn dev_page_paths_match(left: &str, right: &str) -> bool {
    dev_page_path_key(left) == dev_page_path_key(right)
}

fn dev_page_info(
    app: &std::sync::Arc<lxapp::LxApp>,
    page: &lxapp::PageInstance,
    name: Option<&str>,
) -> LxAppDevPageInfo {
    let info = app.runtime_info();
    if let Some(runtime_page) = app
        .page_instance_runtime_info()
        .into_iter()
        .find(|runtime_page| runtime_page.instance_id == page.instance_id_string())
    {
        return dev_page_runtime_info(
            &info.appid,
            &info.page_entries,
            &app.runtime_surface_info(),
            runtime_page,
        );
    }

    let state = page.automation_state();
    let ready = state.ready;
    let webview_attached = state.webview_attached;
    let webview_ready = state.webview_ready;
    let webview_error = state.webview_error.clone();
    let bridge_ready = state.bridge_ready;
    let render_state = state.render_state.to_string();
    let path = page.path();
    let instance_id = page.instance_id_string();
    let stack_index = info
        .page_stack
        .iter()
        .enumerate()
        .find_map(|(index, path)| {
            app.get_page(path)
                .is_some_and(|candidate| candidate.instance_id_string() == instance_id)
                .then_some(index)
        });
    let current = app
        .current_page()
        .ok()
        .is_some_and(|candidate| candidate.instance_id_string() == instance_id);
    let lifecycle = if current {
        "visible"
    } else if state.lifecycle == "onHide" {
        "hidden"
    } else if webview_attached {
        "mounted"
    } else {
        "created"
    };
    let declared = info
        .page_entries
        .iter()
        .any(|entry| dev_page_paths_match(&entry.path, &path));
    LxAppDevPageInfo {
        appid: info.appid,
        declared,
        opened: true,
        instance_id,
        name: name.unwrap_or("").to_string(),
        path,
        query: state.query,
        current,
        in_stack: stack_index.is_some(),
        stack_index,
        owner: serde_json::json!({ "kind": "host" }),
        presentation: Some("window".to_string()),
        lifecycle: Some(lifecycle.to_string()),
        surface_ids: Vec::new(),
        ready,
        webview_attached,
        webview_ready,
        webview_error,
        bridge_ready,
        render_state,
        input_supported: lxapp_dev_page_input_supported(),
    }
}

fn dev_page_runtime_info(
    appid: &str,
    entries: &[lxapp::LxAppRuntimePageInfo],
    surfaces: &[lxapp::LxAppRuntimeSurfaceInfo],
    page: lxapp::PageInstanceRuntimeInfo,
) -> LxAppDevPageInfo {
    let lxapp::PageInstanceRuntimeInfo {
        instance_id,
        name,
        path,
        query,
        owner,
        presentation,
        lifecycle,
        stack_index,
        current,
        state,
    } = page;
    let declared_entry = entries
        .iter()
        .find(|entry| dev_page_paths_match(&entry.path, &path));
    let surface_ids = surfaces
        .iter()
        .filter(|surface| surface.content_page_instance_id.as_deref() == Some(instance_id.as_str()))
        .map(|surface| surface.id.clone())
        .collect();
    let owner = dev_page_owner(&owner);
    let presentation = match presentation {
        lxapp::PresentationKind::Window => "window",
        lxapp::PresentationKind::Panel => "panel",
        lxapp::PresentationKind::Overlay => "overlay",
    };
    LxAppDevPageInfo {
        appid: appid.to_string(),
        declared: declared_entry.is_some(),
        opened: true,
        instance_id,
        name: name
            .or_else(|| declared_entry.map(|entry| entry.name.clone()))
            .unwrap_or_default(),
        path,
        query,
        current,
        in_stack: stack_index.is_some(),
        stack_index,
        owner,
        presentation: Some(presentation.to_string()),
        lifecycle: Some(lifecycle),
        surface_ids,
        ready: state.ready,
        webview_attached: state.webview_attached,
        webview_ready: state.webview_ready,
        webview_error: state.webview_error,
        bridge_ready: state.bridge_ready,
        render_state: state.render_state.to_string(),
        input_supported: lxapp_dev_page_input_supported(),
    }
}

fn dev_page_owner(owner: &lxapp::PageOwner) -> serde_json::Value {
    match owner {
        lxapp::PageOwner::Host => serde_json::json!({ "kind": "host" }),
        lxapp::PageOwner::Scene(scene) => {
            serde_json::json!({ "kind": "scene", "scene_id": scene.0 })
        }
        lxapp::PageOwner::Page(page) => serde_json::json!({
            "kind": "page",
            "page_instance_id": page.as_str(),
        }),
    }
}

/// Reports whether page element input actions are supported on this platform build.
pub fn lxapp_dev_page_input_supported() -> bool {
    cfg!(any(
        target_os = "android",
        target_os = "ios",
        all(target_os = "linux", target_env = "ohos"),
        all(
            feature = "webview-input",
            any(target_os = "macos", target_os = "windows")
        )
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn read_lxapp_manifest_valid() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("lxapp.json"),
            r#"{"appId":"demo","version":"1.0.0"}"#,
        )
        .unwrap();
        let manifest = read_lxapp_manifest(tmp.path()).unwrap();
        assert_eq!(manifest.appId, "demo");
        assert_eq!(manifest.version, "1.0.0");
    }

    #[test]
    fn read_lxapp_manifest_rejects_empty_appid() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("lxapp.json"),
            r#"{"appId":"","version":"1.0.0"}"#,
        )
        .unwrap();
        let err = read_lxapp_manifest(tmp.path()).unwrap_err();
        assert!(err.contains("appId"), "unexpected error: {err}");
    }

    #[test]
    fn read_lxapp_manifest_rejects_empty_version() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("lxapp.json"),
            r#"{"appId":"demo","version":""}"#,
        )
        .unwrap();
        let err = read_lxapp_manifest(tmp.path()).unwrap_err();
        assert!(err.contains("version"), "unexpected error: {err}");
    }

    #[test]
    fn read_lxapp_manifest_rejects_malformed_json() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("lxapp.json"), "not json").unwrap();
        assert!(read_lxapp_manifest(tmp.path()).is_err());
    }

    #[test]
    fn read_lxapp_manifest_rejects_missing_file() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(read_lxapp_manifest(tmp.path()).is_err());
    }

    #[test]
    fn resolve_runnable_lxapp_path_prefers_dist() {
        let tmp = tempfile::tempdir().unwrap();
        let dist = tmp.path().join("dist");
        fs::create_dir_all(&dist).unwrap();
        fs::write(dist.join("lxapp.json"), "{}").unwrap();
        assert_eq!(resolve_runnable_lxapp_path(tmp.path()), dist);
    }

    #[test]
    fn resolve_runnable_lxapp_path_falls_back_when_dist_missing_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        let dist = tmp.path().join("dist");
        fs::create_dir_all(&dist).unwrap();
        assert_eq!(
            resolve_runnable_lxapp_path(tmp.path()),
            tmp.path().to_path_buf()
        );
    }

    #[test]
    fn page_wait_states_match_query_payloads() {
        let visible = serde_json::json!({
            "exists": true,
            "visible": true,
            "enabled": true,
            "editable": false,
        });
        assert!(LxAppDevPageWaitState::Attached.matches(&visible));
        assert!(LxAppDevPageWaitState::Visible.matches(&visible));
        assert!(LxAppDevPageWaitState::Enabled.matches(&visible));
        assert!(!LxAppDevPageWaitState::Editable.matches(&visible));
        assert!(LxAppDevPageWaitState::Detached.matches(&serde_json::json!({
            "exists": false
        })));
    }

    #[test]
    fn page_paths_ignore_query_fragment_and_leading_slash() {
        assert!(dev_page_paths_match(
            "/pages/home?tab=one#content",
            "pages/home"
        ));
        assert!(!dev_page_paths_match("pages/home", "pages/profile"));
    }

    #[test]
    fn page_inventory_keeps_every_live_instance_and_manifest_route() {
        let entries = vec![
            lxapp::LxAppRuntimePageInfo {
                name: "home".to_string(),
                path: "pages/home".to_string(),
            },
            lxapp::LxAppRuntimePageInfo {
                name: "settings".to_string(),
                path: "pages/settings".to_string(),
            },
        ];
        let mut template = complete_dev_page_list("demo", &entries[..1], Vec::new()).remove(0);
        template.opened = true;
        template.ready = true;
        template.instance_id = "instance-1".to_string();
        let mut second = template.clone();
        second.instance_id = "instance-2".to_string();

        let pages = complete_dev_page_list("demo", &entries, vec![template, second]);

        assert_eq!(pages.len(), 3);
        assert_eq!(pages.iter().filter(|page| page.opened).count(), 2);
        assert_eq!(
            pages
                .iter()
                .filter(|page| page.path == "pages/home")
                .count(),
            2
        );
        let settings = pages
            .iter()
            .find(|page| page.path == "pages/settings")
            .unwrap();
        assert!(settings.declared);
        assert!(!settings.opened);
    }
}
