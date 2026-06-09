use std::path::PathBuf;
use std::sync::OnceLock;

use lingxia_platform::traits::app_runtime::AppRuntime;

const LXAPP_PATH_ENV: &str = "LINGXIA_LXAPP_PATH";

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
        home_app_id: dev_config.identity.appid.clone(),
        home_app_version: dev_config.identity.version.clone(),
        cache_max_size_mb: 1024,
        storage: None,
        dev_ws_url: None,
        app_links: None,
        capabilities: None,
        panels: None,
    }
}

pub(crate) fn load_host_app_config(
    runtime: &std::sync::Arc<lingxia_platform::Platform>,
    load_bundled: impl FnOnce(
        &std::sync::Arc<lingxia_platform::Platform>,
    ) -> Option<lingxia_app_context::AppConfig>,
) -> Option<lingxia_app_context::AppConfig> {
    let Some(dev_config) = lxapp_dev_config() else {
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

pub(crate) fn register_bundle_source_override() {
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
    /// Declarative page name from the runtime manifest, when available.
    pub name: String,
    /// Runtime page path.
    pub path: String,
    /// Whether this page is the current foreground page.
    pub current: bool,
    /// Whether this page still exists in the navigation stack.
    pub in_stack: bool,
    /// Whether the page currently has an attached WebView.
    pub ready: bool,
    /// Whether direct text-input actions are supported on this platform.
    pub input_supported: bool,
}

/// Returns information about the current page for the selected app.
pub fn lxapp_dev_page_current(appid: Option<&str>) -> Result<LxAppDevPageInfo, String> {
    let app = resolve_dev_lxapp(appid.unwrap_or("current"))?;
    let (page, name) = resolve_dev_page(&app, None)?;
    Ok(dev_page_info(&app, &page, name.as_deref()))
}

/// Lists known pages for the selected app and marks current/stack state.
pub fn lxapp_dev_page_list(appid: Option<&str>) -> Result<Vec<LxAppDevPageInfo>, String> {
    let app = resolve_dev_lxapp(appid.unwrap_or("current"))?;
    let info = app.runtime_info();
    Ok(info
        .page_entries
        .iter()
        .map(|entry| {
            let active = app.require_page(&entry.path).ok();
            LxAppDevPageInfo {
                appid: info.appid.clone(),
                name: entry.name.clone(),
                path: entry.path.clone(),
                current: info
                    .current_page
                    .as_deref()
                    .is_some_and(|current_page| dev_page_paths_match(current_page, &entry.path)),
                in_stack: info
                    .page_stack
                    .iter()
                    .any(|stack_page| dev_page_paths_match(stack_page, &entry.path)),
                ready: active.as_ref().is_some_and(|page| page.webview().is_some()),
                input_supported: lxapp_dev_page_input_supported(),
            }
        })
        .collect())
}

/// Returns information for a specific page, or the current page if omitted.
pub fn lxapp_dev_page_info(
    appid: Option<&str>,
    page_name: Option<&str>,
) -> Result<LxAppDevPageInfo, String> {
    let app = resolve_dev_lxapp(appid.unwrap_or("current"))?;
    let (page, name) = resolve_dev_page(&app, page_name)?;
    Ok(dev_page_info(&app, &page, name.as_deref()))
}

/// Evaluates JavaScript in the target page WebView and returns the raw JSON result.
pub async fn lxapp_dev_page_eval(
    appid: Option<&str>,
    page_name: Option<&str>,
    js: &str,
) -> Result<serde_json::Value, String> {
    let app = resolve_dev_lxapp(appid.unwrap_or("current"))?;
    let (page, _) = resolve_dev_page(&app, page_name)?;
    page.webview()
        .ok_or_else(|| "page WebView is not ready".to_string())?
        .evaluate_javascript(js)
        .await
        .map_err(|err| err.to_string())
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
    use lingxia_platform::traits::screenshot::AppScreenshot;
    let platform =
        lxapp::get_platform().ok_or_else(|| "platform is not initialized".to_string())?;
    platform
        .take_app_screenshot(window_id)
        .await
        .map_err(|err| err.to_string())
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

/// Capture a PNG screenshot of the target lxapp page's WebView.
/// Returns raw PNG bytes.
pub async fn lxapp_dev_page_screenshot(
    appid: Option<&str>,
    page_name: Option<&str>,
) -> Result<Vec<u8>, String> {
    let app = resolve_dev_lxapp(appid.unwrap_or("current"))?;
    let (page, _) = resolve_dev_page(&app, page_name)?;
    page.webview()
        .ok_or_else(|| "page WebView is not ready".to_string())?
        .take_screenshot()
        .await
        .map_err(|err| err.to_string())
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
    let app = resolve_dev_lxapp(appid.unwrap_or("current"))?;
    let (page, _) = resolve_dev_page(&app, page_name)?;
    let script = build_dev_page_query_script(selector, index, all, max_text)?;
    page.webview()
        .ok_or_else(|| "page WebView is not ready".to_string())?
        .evaluate_javascript(&script)
        .await
        .map_err(|err| err.to_string())
}

/// Clicks the matching DOM node in the target page.
pub async fn lxapp_dev_page_click(
    appid: Option<&str>,
    page_name: Option<&str>,
    selector: &str,
    index: Option<usize>,
) -> Result<(), String> {
    let app = resolve_dev_lxapp(appid.unwrap_or("current"))?;
    let (page, _) = resolve_dev_page(&app, page_name)?;
    let webview = page
        .webview()
        .ok_or_else(|| "page WebView is not ready".to_string())?;
    webview
        .click(selector, lingxia_webview::ClickOptions { index })
        .await
        .map_err(|err| err.to_string())
}

/// Types text into the matching editable DOM node without clearing existing content.
pub async fn lxapp_dev_page_type(
    appid: Option<&str>,
    page_name: Option<&str>,
    selector: &str,
    index: Option<usize>,
    text: &str,
) -> Result<(), String> {
    let app = resolve_dev_lxapp(appid.unwrap_or("current"))?;
    let (page, _) = resolve_dev_page(&app, page_name)?;
    let webview = page
        .webview()
        .ok_or_else(|| "page WebView is not ready".to_string())?;
    webview
        .type_text(
            selector,
            text,
            lingxia_webview::TypeOptions {
                index,
                replace: false,
            },
        )
        .await
        .map_err(|err| err.to_string())
}

/// Replaces the matching editable DOM node content with the provided text.
pub async fn lxapp_dev_page_fill(
    appid: Option<&str>,
    page_name: Option<&str>,
    selector: &str,
    index: Option<usize>,
    text: &str,
) -> Result<(), String> {
    let app = resolve_dev_lxapp(appid.unwrap_or("current"))?;
    let (page, _) = resolve_dev_page(&app, page_name)?;
    let webview = page
        .webview()
        .ok_or_else(|| "page WebView is not ready".to_string())?;
    webview
        .fill(selector, text, lingxia_webview::FillOptions { index })
        .await
        .map_err(|err| err.to_string())
}

/// Sends a key press to the target page WebView.
pub async fn lxapp_dev_page_press(
    appid: Option<&str>,
    page_name: Option<&str>,
    key: &str,
) -> Result<(), String> {
    let app = resolve_dev_lxapp(appid.unwrap_or("current"))?;
    let (page, _) = resolve_dev_page(&app, page_name)?;
    page.webview()
        .ok_or_else(|| "page WebView is not ready".to_string())?
        .press(key, lingxia_webview::PressOptions)
        .await
        .map_err(|err| err.to_string())
}

/// Navigates back in the current page stack by the requested delta.
pub fn lxapp_dev_page_back(appid: Option<&str>, delta: u32) -> Result<(), String> {
    let app = resolve_dev_lxapp(appid.unwrap_or("current"))?;
    app.current_page()
        .map_err(|err| err.to_string())?
        .navigate_back(delta)
        .map_err(|err| err.to_string())
}

/// Navigation mode used by [`lxapp_dev_nav_with_kind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LxAppDevNavKind {
    To,
    Redirect,
    SwitchTab,
    Relaunch,
}

impl LxAppDevNavKind {
    fn navigation_type(self) -> lxapp::NavigationType {
        match self {
            Self::To => lxapp::NavigationType::Forward,
            Self::Redirect => lxapp::NavigationType::Replace,
            Self::SwitchTab => lxapp::NavigationType::SwitchTab,
            Self::Relaunch => lxapp::NavigationType::Launch,
        }
    }
}

/// Navigate to a configured page by page name.
pub async fn lxapp_dev_nav_to(
    appid: Option<&str>,
    page_name: &str,
    query: Option<serde_json::Value>,
) -> Result<LxAppDevPageInfo, String> {
    lxapp_dev_nav_with_kind(appid, page_name, query, LxAppDevNavKind::To).await
}

/// Replace the current page with a configured page by page name.
pub async fn lxapp_dev_nav_redirect(
    appid: Option<&str>,
    page_name: &str,
    query: Option<serde_json::Value>,
) -> Result<LxAppDevPageInfo, String> {
    lxapp_dev_nav_with_kind(appid, page_name, query, LxAppDevNavKind::Redirect).await
}

/// Switch to a configured tab page by page name.
pub async fn lxapp_dev_nav_switch_tab(
    appid: Option<&str>,
    page_name: &str,
    query: Option<serde_json::Value>,
) -> Result<LxAppDevPageInfo, String> {
    lxapp_dev_nav_with_kind(appid, page_name, query, LxAppDevNavKind::SwitchTab).await
}

/// Relaunch the lxapp at a configured page by page name.
pub async fn lxapp_dev_nav_relaunch(
    appid: Option<&str>,
    page_name: &str,
    query: Option<serde_json::Value>,
) -> Result<LxAppDevPageInfo, String> {
    lxapp_dev_nav_with_kind(appid, page_name, query, LxAppDevNavKind::Relaunch).await
}

/// Navigate back in the current page stack and return the destination page.
pub async fn lxapp_dev_nav_back(
    appid: Option<&str>,
    delta: u32,
) -> Result<LxAppDevPageInfo, String> {
    let app = resolve_dev_lxapp(appid.unwrap_or("current"))?;
    app.current_page()
        .map_err(|err| err.to_string())?
        .navigate_back(delta)
        .map_err(|err| err.to_string())?;

    let (page, name) = resolve_dev_page(&app, None)?;
    page.wait_webview_ready()
        .await
        .map_err(|err| err.to_string())?;
    Ok(dev_page_info(&app, &page, name.as_deref()))
}

async fn lxapp_dev_nav_with_kind(
    appid: Option<&str>,
    page_name: &str,
    query: Option<serde_json::Value>,
    kind: LxAppDevNavKind,
) -> Result<LxAppDevPageInfo, String> {
    let app = resolve_dev_lxapp(appid.unwrap_or("current"))?;
    let target_url = resolve_dev_page_target(&app, page_name, query.as_ref())?;

    if kind == LxAppDevNavKind::Redirect && is_dev_tabbar_page_url(&app, &target_url) {
        return Err("redirectTo cannot navigate to a tabBar page".to_string());
    }

    app.ensure_page_exists(&target_url)
        .map_err(|err| err.to_string())?;

    let current_path = app
        .peek_current_page()
        .ok_or_else(|| "No current page found".to_string())?;
    let current_page = app
        .get_page(&current_path)
        .ok_or_else(|| "Current page not found".to_string())?;
    let target_page = app.get_or_create_page(&target_url);
    let target_page = current_page
        .navigate_to(target_page, kind.navigation_type())
        .map_err(|err| err.to_string())?;

    target_page
        .wait_webview_ready()
        .await
        .map_err(|err| err.to_string())?;

    let (page, name) = resolve_dev_page(&app, None)?;
    Ok(dev_page_info(&app, &page, name.as_deref()))
}

fn resolve_dev_page_target(
    app: &std::sync::Arc<lxapp::LxApp>,
    page_name: &str,
    query: Option<&serde_json::Value>,
) -> Result<String, String> {
    let page_name = page_name.trim();
    if page_name.is_empty() {
        return Err("page name is required".to_string());
    }
    let path = app
        .find_page_path_by_name(page_name)
        .ok_or_else(|| format!("unknown page name: {page_name}"))?;
    match query {
        Some(query) => lxapp::append_page_query(path, query),
        None => Ok(path),
    }
}

fn normalize_dev_tabbar_path(url: &str) -> String {
    let (path, _) = lxapp::startup::split_path_query(url);
    let mut trimmed = path.trim_start_matches('/').to_string();
    if let Some(dot_pos) = trimmed.rfind('.')
        && trimmed.rfind('/').is_none_or(|slash| dot_pos > slash)
    {
        trimmed.truncate(dot_pos);
    }
    trimmed
}

fn is_dev_tabbar_page_url(app: &lxapp::LxApp, url: &str) -> bool {
    let Some(tabbar) = app.get_tabbar() else {
        return false;
    };
    let target = normalize_dev_tabbar_path(url);
    tabbar
        .list
        .iter()
        .any(|item| normalize_dev_tabbar_path(&item.pagePath) == target)
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
    let Some(page_name) = page_name.map(str::trim).filter(|value| !value.is_empty()) else {
        let page = app.current_page().map_err(|err| err.to_string())?;
        let name = dev_page_name_for_path(app, &page.path());
        return Ok((page, name));
    };

    if page_name.eq_ignore_ascii_case("current") {
        let page = app.current_page().map_err(|err| err.to_string())?;
        let name = dev_page_name_for_path(app, &page.path());
        return Ok((page, name));
    }

    let path = app
        .find_page_path_by_name(page_name)
        .ok_or_else(|| format!("unknown page name: {page_name}"))?;
    let page = resolve_active_dev_page_by_path(app, &path)
        .ok_or_else(|| format!("page is not active: {page_name}"))?;
    Ok((page, Some(page_name.to_string())))
}

fn resolve_active_dev_page_by_path(
    app: &std::sync::Arc<lxapp::LxApp>,
    path: &str,
) -> Option<lxapp::PageInstance> {
    if let Ok(page) = app.require_page(path) {
        return Some(page);
    }

    let info = app.runtime_info();
    info.current_page
        .iter()
        .chain(info.page_stack.iter().rev())
        .find(|candidate| dev_page_paths_match(candidate, path))
        .and_then(|candidate| app.get_page(candidate))
}

fn dev_page_name_for_path(app: &std::sync::Arc<lxapp::LxApp>, path: &str) -> Option<String> {
    app.runtime_info()
        .page_entries
        .into_iter()
        .find(|entry| dev_page_paths_match(&entry.path, path))
        .map(|entry| entry.name)
        .filter(|name| !name.is_empty())
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
    let path = page.path();
    LxAppDevPageInfo {
        appid: info.appid,
        name: name.unwrap_or("").to_string(),
        path: path.clone(),
        current: info
            .current_page
            .as_deref()
            .is_some_and(|current_page| dev_page_paths_match(current_page, &path)),
        in_stack: info
            .page_stack
            .iter()
            .any(|stack_page| dev_page_paths_match(stack_page, &path)),
        ready: page.webview().is_some(),
        input_supported: lxapp_dev_page_input_supported(),
    }
}

/// Reports whether direct WebView input actions are supported on this platform build.
pub fn lxapp_dev_page_input_supported() -> bool {
    cfg!(all(feature = "webview-input", target_os = "macos"))
}

fn build_dev_page_query_script(
    selector: &str,
    index: Option<usize>,
    all: bool,
    max_text_chars: Option<usize>,
) -> Result<String, String> {
    let selector_json =
        serde_json::to_string(selector).map_err(|err| format!("invalid selector: {err}"))?;
    let index_json =
        serde_json::to_string(&index).map_err(|err| format!("invalid index: {err}"))?;
    let max_text_json = serde_json::to_string(&max_text_chars)
        .map_err(|err| format!("invalid query limit: {err}"))?;
    Ok(format!(
        r#"
(() => {{
  const selector = {selector_json};
  const requestedIndex = {index_json};
  const all = {};
  const maxText = {max_text_json};
  const truncate = (value) => {{
    const text = String(value ?? "");
    if (typeof maxText === "number" && maxText >= 0 && text.length > maxText) {{
      return {{ value: text.slice(0, maxText), truncated: true }};
    }}
    return {{ value: text, truncated: false }};
  }};
  if (typeof selector !== "string" || selector.trim() === "") {{
    throw new Error("selector must not be empty");
  }}
  let nodes;
  try {{
    nodes = Array.from(document.querySelectorAll(selector));
  }} catch (err) {{
    throw new Error("invalid selector: " + String(err && err.message ? err.message : err));
  }}
  const describe = (el, index, count) => {{
    const rect = el.getBoundingClientRect();
    const style = window.getComputedStyle(el);
    const disabled = !!el.disabled || el.getAttribute("aria-disabled") === "true";
    const tag = (el.tagName || "").toLowerCase();
    const inputType = tag === "input" ? String(el.type || "text").toLowerCase() : "";
    const blockedInputTypes = new Set([
      "button", "checkbox", "color", "file", "hidden", "image", "radio",
      "range", "reset", "submit"
    ]);
    const editable = !!el.isContentEditable ||
      (tag === "textarea" && !disabled && !el.readOnly) ||
      (tag === "input" && !disabled && !el.readOnly && !blockedInputTypes.has(inputType));
    const visible = rect.width > 0 &&
      rect.height > 0 &&
      rect.bottom > 0 &&
      rect.right > 0 &&
      rect.top < window.innerHeight &&
      rect.left < window.innerWidth &&
      style.visibility !== "hidden" &&
      style.display !== "none" &&
      Number(style.opacity || "1") !== 0;
    const hasValue = "value" in el;
    const text = truncate(el.innerText || el.textContent || "");
    const value = hasValue ? truncate(el.value ?? "") : null;
    return {{
      exists: true,
      index,
      count,
      tag,
      type: inputType || null,
      id: el.id || null,
      name: el.getAttribute("name"),
      role: el.getAttribute("role"),
      aria_label: el.getAttribute("aria-label"),
      placeholder: el.getAttribute("placeholder"),
      visible,
      enabled: !disabled,
      editable,
      text: text.value,
      text_truncated: text.truncated,
      value: value ? value.value : null,
      value_truncated: value ? value.truncated : false,
      rect: {{
        left: rect.left,
        top: rect.top,
        width: rect.width,
        height: rect.height,
        right: rect.right,
        bottom: rect.bottom,
        center_x: rect.left + (rect.width / 2),
        center_y: rect.top + (rect.height / 2),
        viewport_width: window.innerWidth,
        viewport_height: window.innerHeight
      }}
    }};
  }};
  const count = nodes.length;
  if (all) {{
    return {{ count, items: nodes.map((el, index) => describe(el, index, count)) }};
  }}
  const index = typeof requestedIndex === "number" ? requestedIndex : 0;
  const el = nodes[index];
  if (!el) {{
    return {{
      exists: false,
      index,
      count,
      visible: false,
      enabled: false,
      editable: false
    }};
  }}
  return describe(el, index, count);
}})()
"#,
        all
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
}
