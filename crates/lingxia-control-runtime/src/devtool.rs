//! The devtool commands' implementations: lxapp pages and navigation,
//! restart, and host window capture and input. The `lingxia` facade keeps
//! only what a host calls (dev bundles, the simulated device); these serve
//! this crate's handlers alone.

use lingxia::dev::{device_get, device_set, lxapp_dev_page_input_supported, sync_dev_bundle};
use lxapp::automation as auto;

/// Snapshot of a page exposed by the active devtool target.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct LxAppDevPageInfo {
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
pub(crate) enum LxAppDevPageWaitState {
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
pub(crate) struct LxAppDevPageWaitResult {
    pub state: LxAppDevPageWaitState,
    pub selector: Option<String>,
    pub page: LxAppDevPageInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element: Option<serde_json::Value>,
}

/// Returns information about the current page for the selected app.
pub(crate) fn lxapp_dev_page_current(appid: Option<&str>) -> Result<LxAppDevPageInfo, String> {
    let app = auto::resolve_lxapp(appid.unwrap_or("current"))?;
    let (page, name) = auto::resolve_page(&app, None)?;
    Ok(dev_page_info(&app, &page, name.as_deref()))
}

/// Lists known pages for the selected app and marks current/stack state.
pub(crate) fn lxapp_dev_page_list(appid: Option<&str>) -> Result<Vec<LxAppDevPageInfo>, String> {
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
pub(crate) fn lxapp_dev_page_info(
    appid: Option<&str>,
    page_name: Option<&str>,
) -> Result<LxAppDevPageInfo, String> {
    let app = auto::resolve_lxapp(appid.unwrap_or("current"))?;
    let (page, name) = auto::resolve_page(&app, page_name)?;
    Ok(dev_page_info(&app, &page, name.as_deref()))
}

/// Waits for an lxapp page to finish its runtime lifecycle, or for a DOM
/// element in that stable page instance to reach the requested state.
pub(crate) async fn lxapp_dev_page_wait(
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
        return Err("a selector cannot be combined with --state ready".to_string());
    }
    if state != LxAppDevPageWaitState::Ready && selector.is_none() {
        return Err(format!("--state {} needs a selector", state.as_str()));
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

/// Cold restarts an lxapp and waits for its live page to finish loading.
///
/// Dev reload must preserve the host-owned surface. A full product restart
/// closes that surface and cannot reconstruct whether a dynamic lxapp was a
/// main or aside, leaving it detached from the layout that opened it.
pub(crate) async fn lxapp_dev_restart(
    appid: &str,
    timeout: std::time::Duration,
) -> Result<LxAppDevPageInfo, String> {
    let appid = resolve_dev_appid(appid)?;
    // Pull this app id before deciding it is live. A closed guest keeps the
    // files for its next open; a live one is retargeted before cold restart.
    sync_dev_bundle(&appid)?;
    let home_id = lingxia_app_context::app_config()
        .map(|config| config.home_app_id.trim().to_string())
        .unwrap_or_default();
    // A closed instance stays in the manager; it is not open either.
    let open = lxapp::try_get(&appid).filter(|app| app.runtime_info().status == "opened");
    let Some(app) = open else {
        // A guest that is not open stays closed; `not_open` tells the dev
        // session its build loads on the next open. Home still reopens: its
        // surface is the product, and a failed load has nothing else to show.
        if home_id != appid {
            return Err(format!("(not_open): lxapp is not open: {appid}"));
        }
        // A development bundle whose last load failed — a reopen that found
        // the build output mid-rebuild — is not live. The rebuild that asks
        // for this restart is what makes it loadable again, so open it.
        if lxapp::is_dev_bundle_appid(&appid) {
            return open_unloaded_dev_lxapp(&appid, timeout).await;
        }
        return Err(format!("lxapp is not active: {appid}"));
    };
    let previous_session = app.runtime_info().session_id;
    let previous_page = app.current_page().map_err(|error| error.to_string())?;
    let previous_page_id = previous_page.instance_id_string();
    let previous_load = *previous_page.subscribe_loaded().borrow();
    let simulated_device = device_get().ok();

    app.cold_restart_and_wait()
        .await
        .map_err(|err| err.to_string())?;
    let deadline = tokio::time::Instant::now()
        .checked_add(timeout)
        .ok_or_else(|| "restart timeout is too large".to_string())?;

    loop {
        if let Some(app) = lxapp::try_get(&appid) {
            let runtime = app.runtime_info();
            if runtime.session_id == previous_session
                && runtime.status == "opened"
                && let Ok((page, name)) = resolve_dev_page(&app, None)
            {
                let state = page.automation_state();
                if let Some(error) = state.webview_error {
                    return Err(format!("reloaded page WebView failed: {error}"));
                }
                let load = *page.subscribe_loaded().borrow();
                let reload_finished =
                    page.instance_id_string() != previous_page_id || load != previous_load;
                // A page hidden at restart (covered by a tab, window
                // inactive) is reloaded once its new onLoad lands; its
                // onReady belongs to the next show.
                if reload_finished && (state.ready || page.loaded_while_hidden()) {
                    if let Some(device) = &simulated_device {
                        device_set(
                            Some(&device.id),
                            Some(device.landscape),
                            Some(device.appearance),
                        )
                        .await
                        .map_err(|error| {
                            format!("failed to restore simulated device after restart: {error}")
                        })?;
                    }
                    return Ok(dev_page_info(&app, &page, name.as_deref()));
                }
            }
        }
        let now = tokio::time::Instant::now();
        if now >= deadline {
            return Err(format!(
                "timed out after {}ms waiting for lxapp {appid} to reload",
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

/// Open a development lxapp that is not live on its initial page and wait
/// for that page to be ready.
async fn open_unloaded_dev_lxapp(
    appid: &str,
    timeout: std::time::Duration,
) -> Result<LxAppDevPageInfo, String> {
    sync_dev_bundle(appid)?;
    log::info!("{appid} is not loaded; opening it after the rebuild");
    let app = lxapp::open_lxapp(appid, lxapp::LxAppStartupOptions::new(""))
        .map_err(|err| format!("failed to open {appid}: {err}"))?;
    let deadline = tokio::time::Instant::now()
        .checked_add(timeout)
        .ok_or_else(|| "restart timeout is too large".to_string())?;
    loop {
        if let Ok((page, name)) = resolve_dev_page(&app, None) {
            let state = page.automation_state();
            if let Some(error) = state.webview_error {
                return Err(format!("reopened page WebView failed: {error}"));
            }
            if state.ready {
                return Ok(dev_page_info(&app, &page, name.as_deref()));
            }
        }
        let now = tokio::time::Instant::now();
        if now >= deadline {
            return Err(format!(
                "timed out after {}ms waiting for lxapp {appid} to open",
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
pub(crate) async fn lxapp_dev_page_eval(
    appid: Option<&str>,
    page_name: Option<&str>,
    js: &str,
) -> Result<serde_json::Value, String> {
    let app = auto::resolve_lxapp(appid.unwrap_or("current"))?;
    auto::page_eval(&app, page_name, js).await
}

/// Captures a host window and returns the concrete window selected alongside
/// the PNG, so screenshot coordinates can be fed back into app input safely.
pub(crate) async fn take_app_screenshot_with_info(
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

/// Bring a host app window in front of other apps' windows, activating the
/// app and making the window key. The app raises its own window, so this
/// needs no Accessibility permission. macOS only for now.
pub(crate) async fn focus_app_window(
    window_id: Option<&str>,
) -> Result<lingxia_platform::traits::screenshot::WindowInfo, String> {
    use lingxia_platform::traits::screenshot::AppScreenshot;
    let platform =
        lxapp::get_platform().ok_or_else(|| "platform is not initialized".to_string())?;
    platform
        .bring_app_window_to_front(window_id, true)
        .await
        .map_err(|err| err.to_string())
}

/// Enumerate the host app's top-level windows. Mobile platforms return a
/// single entry; desktop platforms return one entry per open window.
pub(crate) async fn list_app_windows()
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
pub(crate) async fn perform_app_mouse(
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
pub(crate) async fn perform_app_keyboard(
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

/// Captures a page screenshot and reports metadata for the exact page instance
/// that supplied the bytes.
pub(crate) async fn lxapp_dev_page_screenshot_with_info(
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
pub(crate) async fn lxapp_dev_page_query(
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
pub(crate) async fn lxapp_dev_page_click(
    appid: Option<&str>,
    page_name: Option<&str>,
    selector: &str,
    index: Option<usize>,
) -> Result<(), String> {
    let app = auto::resolve_lxapp(appid.unwrap_or("current"))?;
    auto::page_click(&app, page_name, selector, index).await
}

/// Types text into the matching editable DOM node without clearing existing content.
pub(crate) async fn lxapp_dev_page_type(
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
pub(crate) async fn lxapp_dev_page_fill(
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
pub(crate) async fn lxapp_dev_page_press(
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
pub(crate) async fn lxapp_dev_page_scroll(
    appid: Option<&str>,
    page_name: Option<&str>,
    dx: f64,
    dy: f64,
) -> Result<(), String> {
    let app = auto::resolve_lxapp(appid.unwrap_or("current"))?;
    auto::page_scroll(&app, page_name, dx, dy).await
}

/// Scrolls the first matching DOM node into view.
pub(crate) async fn lxapp_dev_page_scroll_to(
    appid: Option<&str>,
    page_name: Option<&str>,
    selector: &str,
) -> Result<(), String> {
    let app = auto::resolve_lxapp(appid.unwrap_or("current"))?;
    auto::page_scroll_to(&app, page_name, selector).await
}

/// Navigate to a configured page by page name.
pub(crate) async fn lxapp_dev_nav_to(
    appid: Option<&str>,
    page_name: &str,
    query: Option<serde_json::Value>,
) -> Result<LxAppDevPageInfo, String> {
    lxapp_dev_nav_with_kind(appid, page_name, query, lxapp::NavigationType::Forward).await
}

/// Replace the current page with a configured page by page name.
pub(crate) async fn lxapp_dev_nav_redirect(
    appid: Option<&str>,
    page_name: &str,
    query: Option<serde_json::Value>,
) -> Result<LxAppDevPageInfo, String> {
    lxapp_dev_nav_with_kind(appid, page_name, query, lxapp::NavigationType::Replace).await
}

/// Switch to a configured tab page by page name.
pub(crate) async fn lxapp_dev_nav_switch_tab(
    appid: Option<&str>,
    page_name: &str,
    query: Option<serde_json::Value>,
) -> Result<LxAppDevPageInfo, String> {
    lxapp_dev_nav_with_kind(appid, page_name, query, lxapp::NavigationType::SwitchTab).await
}

/// Relaunch the lxapp at a configured page by page name.
pub(crate) async fn lxapp_dev_nav_relaunch(
    appid: Option<&str>,
    page_name: &str,
    query: Option<serde_json::Value>,
) -> Result<LxAppDevPageInfo, String> {
    lxapp_dev_nav_with_kind(appid, page_name, query, lxapp::NavigationType::Launch).await
}

/// Navigate back in the current page stack and return the destination page.
pub(crate) async fn lxapp_dev_nav_back(
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
    auto::wait_page_runtime_ready(app, page, auto::PAGE_READY_TIMEOUT).await?;
    Ok(dev_page_info(app, page, name))
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

#[cfg(test)]
mod tests {
    use super::*;

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
