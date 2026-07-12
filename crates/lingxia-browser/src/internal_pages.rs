//! Browser internal pages: route registry, `lingxia://` scheme routing, asset
//! request rewriting, and the shared startup-page bridge.

use crate::BUILTIN_BROWSER_APPID;
use crate::policy::{LINGXIA_SCHEME, extract_url_scheme, lingxia_url_host};
use crate::tabs::{
    INTERNAL_TAB_PATH_PREFIX, ensure_browser_lxapp, lock_state, normalize_runtime_tab_id,
};
use crate::webview::browser_find_webview;
use http::{Request, Response, StatusCode, Uri};
use lingxia_webview::{WebResourceResponse, WebView, WebViewController};
use lxapp::{LxApp, LxAppError, PageInstance};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

const BROWSER_LINGXIA_ASSET_HOSTS: &[&str] = &[
    "lxapp",
    "plugin",
    "usercache",
    "userdata",
    "assets",
    "proxy",
];

static BROWSER_STARTUP_PAGE_INIT_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
static BROWSER_DOCUMENT_SCRIPTS: OnceLock<Mutex<Vec<Arc<str>>>> = OnceLock::new();
static BROWSER_INTERNAL_PAGES: OnceLock<Mutex<HashMap<String, BrowserInternalPageRegistration>>> =
    OnceLock::new();

#[derive(Clone, Debug)]
pub(crate) struct BrowserInternalPageRegistration {
    entry_asset: String,
}

#[derive(Clone, Debug)]
pub(crate) enum InternalPageTarget {
    StartupPage { page_path: String },
    Registered(BrowserInternalPageRegistration),
}

/// Register a browser-owned script that runs after every browser tab document
/// load — internal pages and external sites alike (e.g. the context menu).
///
/// Injection happens in the tab delegate, not through an lxapp
/// `PageInstance`, so external documents get the scripts without driving any
/// page lifecycle.
pub(crate) fn register_browser_document_script(js: impl Into<String>) {
    let scripts = BROWSER_DOCUMENT_SCRIPTS.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut guard) = scripts.lock() {
        guard.push(Arc::from(js.into()));
    }
}

/// Snapshot of the registered browser document scripts, in registration order.
pub(crate) fn browser_document_scripts_snapshot() -> Vec<Arc<str>> {
    BROWSER_DOCUMENT_SCRIPTS
        .get()
        .and_then(|m| m.lock().ok())
        .map(|guard| guard.clone())
        .unwrap_or_default()
}

/// Register a browser-internal route and the packaged HTML entry that implements it.
///
/// Example: `register_browser_internal_page("settings", "pages/settings/index.html")`.
/// Runtime routing then resolves `lingxia://settings` through this registry instead of
/// assuming a file layout from the host name.
pub(crate) fn register_browser_internal_page(
    route: impl Into<String>,
    entry_asset: impl Into<String>,
) -> Result<(), LxAppError> {
    let route = normalize_internal_page_route_key(&route.into())?;
    let entry_asset = normalize_internal_page_entry_asset(&entry_asset.into())?;
    let pages = BROWSER_INTERNAL_PAGES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = pages.lock().unwrap_or_else(|e| e.into_inner());
    guard.insert(route, BrowserInternalPageRegistration { entry_asset });
    Ok(())
}

fn normalize_internal_page_route_key(raw: &str) -> Result<String, LxAppError> {
    let trimmed = raw.trim().to_ascii_lowercase();
    if trimmed.is_empty() {
        return Err(LxAppError::InvalidParameter(
            "browser internal route must not be empty".to_string(),
        ));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    {
        return Err(LxAppError::InvalidParameter(format!(
            "invalid browser internal route '{}'",
            raw.trim()
        )));
    }
    Ok(trimmed)
}

fn normalize_internal_page_entry_asset(raw: &str) -> Result<String, LxAppError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(LxAppError::InvalidParameter(
            "browser internal page entry asset must not be empty".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

fn browser_internal_page_for_host(host: &str) -> Option<BrowserInternalPageRegistration> {
    let route = normalize_internal_page_route_key(host).ok()?;
    BROWSER_INTERNAL_PAGES
        .get()?
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&route)
        .cloned()
}

fn internal_page_target_for_host(startup_path: &str, host: &str) -> Option<InternalPageTarget> {
    match host {
        "" => Some(InternalPageTarget::StartupPage {
            page_path: startup_path.to_string(),
        }),
        _ => browser_internal_page_for_host(host)
            .map(InternalPageTarget::Registered)
            .or_else(|| {
                (host == "newtab").then(|| InternalPageTarget::StartupPage {
                    page_path: startup_path.to_string(),
                })
            }),
    }
}

fn internal_page_target_entry_path(target: &InternalPageTarget) -> &str {
    match target {
        InternalPageTarget::StartupPage { page_path } => page_path.as_str(),
        InternalPageTarget::Registered(registration) => registration.entry_asset.as_str(),
    }
}

fn internal_page_target_for_url(startup_path: &str, url: &str) -> Option<InternalPageTarget> {
    if extract_url_scheme(url).as_deref() != Some(LINGXIA_SCHEME) {
        return None;
    }
    let host = lingxia_url_host(url);
    internal_page_target_for_host(startup_path, &host)
}

fn is_browser_lingxia_asset_host(host: &str) -> bool {
    BROWSER_LINGXIA_ASSET_HOSTS.contains(&host)
}

// ---------------------------------------------------------------------------
// Startup page bridge: headless page setup + tab page binding
// ---------------------------------------------------------------------------

/// Ensure the browser lxapp has a headless startup PageInstance + a live PageSvc.
///
/// Idempotent: if the page already exists in the browser lxapp's page map, returns it directly.
/// Otherwise creates a headless PageInstance (nonce, no WebView), registers it, starts the AppSvc,
/// and asynchronously awaits the PageSvc ack before signalling the page as "ready".
pub(crate) fn ensure_browser_startup_page(
    browser: &Arc<LxApp>,
) -> Result<PageInstance, LxAppError> {
    let startup_path = browser.initial_route();

    // Return existing page if already registered (idempotent).
    if let Some(page) = browser.get_page(&startup_path) {
        return Ok(page);
    }

    // Serialize one-time startup page initialization to avoid duplicate CreatePage races.
    let _startup_guard = BROWSER_STARTUP_PAGE_INIT_MUTEX
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    // Another task may have finished initialization while we were waiting on the lock.
    if let Some(page) = browser.get_page(&startup_path) {
        return Ok(page);
    }

    // Ensure the JS app service worker is running for this browser lxapp.
    if let Err(e) = browser.ensure_app_service_running() {
        lxapp::warn!("[InternalBrowser] Failed to start app service: {}", e);
    }

    browser.ensure_headless_page_service(&startup_path)
}

fn detach_internal_tab_pages_except(tab_path: &str, keep_appid: &str) {
    if let Some(browser) = lxapp::try_get(BUILTIN_BROWSER_APPID)
        && browser.appid != keep_appid
        && let Some(page) = browser.get_page(tab_path)
    {
        page.detach_webview();
    }
}

fn bind_internal_tab_page(tab_path: &str, session_id: u64) -> Result<PageInstance, LxAppError> {
    let owner = ensure_browser_lxapp()?;
    ensure_browser_startup_page(&owner)?;
    let page = owner.ensure_headless_page_service(tab_path)?;
    detach_internal_tab_pages_except(tab_path, &owner.appid);
    if let Ok(webview) = browser_find_webview(tab_path, session_id) {
        page.attach_webview(webview);
    }
    Ok(page)
}

pub(crate) fn browser_resolve_delegate_context(
    tab_path: &str,
    session_id: u64,
) -> Result<(Arc<LxApp>, PageInstance), LxAppError> {
    let browser = ensure_browser_lxapp()?;
    let page = bind_internal_tab_page(tab_path, session_id)?;
    Ok((browser, page))
}

pub(crate) fn browser_resolve_delegate_page(
    tab_path: &str,
    session_id: u64,
) -> Result<PageInstance, LxAppError> {
    browser_resolve_delegate_context(tab_path, session_id).map(|(_, page)| page)
}

fn rewrite_internal_page_asset_request(
    owner: &LxApp,
    target: &InternalPageTarget,
    req: Request<Vec<u8>>,
) -> Result<Request<Vec<u8>>, LxAppError> {
    let (mut parts, body) = req.into_parts();
    let req_uri = parts.uri.clone();
    let entry_asset = internal_page_target_entry_path(target);
    let base_dir = entry_asset
        .rsplit_once('/')
        .map(|(dir, _)| dir)
        .unwrap_or("");
    let asset_rel = req_uri.path().trim_start_matches('/');
    let asset_path = if asset_rel.eq_ignore_ascii_case("favicon.ico") {
        "public/favicon.ico".to_string()
    } else if asset_rel.is_empty() {
        entry_asset.to_string()
    } else if base_dir.is_empty() {
        asset_rel.to_string()
    } else {
        format!("{base_dir}/{asset_rel}")
    };

    let mut rewritten = format!("lx://lxapp/{}/{}", owner.appid, asset_path);
    if let Some(query) = req_uri.query() {
        rewritten.push('?');
        rewritten.push_str(query);
    }
    let uri = Uri::try_from(rewritten).map_err(|err| {
        LxAppError::InvalidParameter(format!("invalid internal asset uri: {err}"))
    })?;
    parts.uri = uri;
    Ok(Request::from_parts(parts, body))
}

// ---------------------------------------------------------------------------
// lingxia:// scheme routing for browser tab WebViews
// ---------------------------------------------------------------------------

/// Per-tab context for the browser `lingxia://` scheme handler.
pub(crate) struct LingxiaSchemeContext {
    pub(crate) browser: Arc<LxApp>,
    pub(crate) startup_path: String,
    pub(crate) tab_id: String,
    pub(crate) tab_path: String,
    pub(crate) session_id: u64,
}

/// Serve a `lingxia://` request issued by a browser tab WebView.
///
/// - Asset hosts (`lxapp`, `assets`, ...) delegate to the shared `lx://` handler.
/// - The document root of a registered internal route serves the page HTML
///   (with the bridge nonce); sub-resources are rewritten relative to the
///   internal page bundle.
pub(crate) async fn handle_browser_lingxia_scheme(
    ctx: &LingxiaSchemeContext,
    req: Request<Vec<u8>>,
) -> Option<WebResourceResponse> {
    // Map `lingxia://` hosts to browser internal pages.
    let host = req.uri().host().unwrap_or("").to_ascii_lowercase();
    if is_browser_lingxia_asset_host(&host) {
        let page = match bind_internal_tab_page(&ctx.tab_path, ctx.session_id) {
            Ok(page) => page,
            Err(err) => {
                lxapp::warn!(
                    "[InternalBrowser] Failed to bind asset page for tab {} host {}: {}",
                    ctx.tab_id,
                    host,
                    err
                );
                return None;
            }
        };
        return ctx.browser.handle_lingxia_request(&page, req);
    }
    let Some(target) = internal_page_target_for_host(&ctx.startup_path, &host) else {
        lxapp::warn!(
            "[InternalBrowser] Unregistered browser internal route host={}",
            host
        );
        return None;
    };
    let page = match bind_internal_tab_page(&ctx.tab_path, ctx.session_id) {
        Ok(page) => page,
        Err(err) => {
            lxapp::warn!(
                "[InternalBrowser] Failed to bind internal page for tab {} host {}: {}",
                ctx.tab_id,
                host,
                err
            );
            return None;
        }
    };
    // Serve page HTML (with bridge nonce) for the document root.
    let req_path = req.uri().path();
    if req_path == "/" || req_path.is_empty() {
        let nonce = page.bridge_nonce();
        let html = ctx
            .browser
            .generate_page_html(internal_page_target_entry_path(&target), nonce.as_deref());
        let response = Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/html; charset=utf-8")
            .header("Access-Control-Allow-Origin", "null")
            .body(())
            .unwrap_or_else(|_| {
                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(())
                    .expect("Failed to build fallback lingxia response")
            });
        let (parts, _) = response.into_parts();
        return Some((parts, html).into());
    }
    // Route sub-resources relative to the browser internal page bundle.
    match rewrite_internal_page_asset_request(&ctx.browser, &target, req) {
        Ok(rewritten) => ctx.browser.handle_lingxia_request(&page, rewritten),
        Err(err) => {
            lxapp::warn!(
                "[InternalBrowser] Failed to rewrite internal asset request for host {}: {}",
                host,
                err
            );
            None
        }
    }
}

/// Attach the given tab WebView to its headless page and load a lingxia:// URL into it.
/// Waits for the PageSvc to be ready first.
///
/// `page_url`: the `lx://` URL to load. `None` loads the default startup/newtab page;
/// `Some(url)` loads a specific internal browser page (e.g. `lx://lxapp/.../downloads`).
pub(crate) async fn browser_attach_tab_page(
    webview: Arc<WebView>,
    page_path: &str,
    session_id: u64,
    tab_id: &str,
    page_url: Option<&str>,
) -> Result<(), LxAppError> {
    let browser = ensure_browser_lxapp()?;
    // Validate that an explicit URL maps to a registered internal route.
    if let Some(url) = page_url {
        internal_page_target_for_url(&browser.initial_route(), url).ok_or_else(|| {
            LxAppError::ResourceNotFound(format!(
                "browser internal route not registered for url: {}",
                url
            ))
        })?;
    }
    let page = bind_internal_tab_page(page_path, session_id)?;

    // Wait until PageSvc signals ready (ack from JS worker).
    if let Err(e) = page.wait_webview_ready().await {
        lxapp::warn!(
            "[InternalBrowser] Tab PageSvc not ready for tab {}: {}",
            tab_id,
            e
        );
    }

    // Attach this tab's WebView so bridge responses are delivered here.
    page.attach_webview(webview.clone());

    // Load the requested URL (or `lingxia://newtab` for the default startup page).
    let url_to_load = page_url
        .map(|u| u.to_string())
        .unwrap_or_else(|| format!("{}://newtab", LINGXIA_SCHEME));
    webview
        .load_url(&url_to_load)
        .map_err(|e| LxAppError::WebView(e.to_string()))
}

// ---------------------------------------------------------------------------
// Page-path resolution for tab paths
// ---------------------------------------------------------------------------

fn browser_internal_page_path_for_url(browser: &LxApp, url: &str) -> Option<String> {
    let target = internal_page_target_for_url(&browser.initial_route(), url)?;
    Some(
        browser
            .find_page_path(internal_page_target_entry_path(&target))
            .unwrap_or_else(|| internal_page_target_entry_path(&target).to_string()),
    )
}

pub(crate) fn browser_logic_page_path_for_tab_path(
    browser: &LxApp,
    tab_path: &str,
) -> Option<String> {
    let tab_id = tab_path.strip_prefix(INTERNAL_TAB_PATH_PREFIX)?;
    let normalized = normalize_runtime_tab_id(tab_id)?;
    let target_url = {
        let state = lock_state();
        let tab = state.tabs.get(&normalized)?;
        tab.current_url
            .as_ref()
            .or(tab.pending_url.as_ref())
            .cloned()?
    };
    browser_internal_page_path_for_url(browser, &target_url)
}

pub(crate) fn warmup_builtin_browser_runtime() -> Result<(), LxAppError> {
    let browser = ensure_browser_lxapp()?;
    let _ = ensure_browser_startup_page(&browser)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;

    static TEST_BROWSER_INTERNAL_PAGES: Once = Once::new();

    fn register_test_browser_internal_pages() {
        TEST_BROWSER_INTERNAL_PAGES.call_once(|| {
            register_browser_internal_page("downloads", "pages/downloads/index.html").unwrap();
            register_browser_internal_page("settings", "pages/settings/index.html").unwrap();
        });
    }

    #[test]
    fn registered_internal_page_route_resolves_to_entry_asset() {
        register_test_browser_internal_pages();
        let target = internal_page_target_for_url("pages/newtab/index.html", "lingxia://settings")
            .expect("settings route should resolve");
        assert_eq!(
            internal_page_target_entry_path(&target),
            "pages/settings/index.html"
        );
    }

    #[test]
    fn registered_internal_page_route_resolves_with_fragment_query_or_slash() {
        register_test_browser_internal_pages();
        for url in [
            "lingxia://settings#clear-browsing-data",
            "lingxia://settings#clear-site-data?tabId=tab-1",
            "lingxia://settings?section=privacy",
            "lingxia://settings/",
        ] {
            let target = internal_page_target_for_url("pages/newtab/index.html", url)
                .unwrap_or_else(|| panic!("settings route should resolve for {url}"));
            assert_eq!(
                internal_page_target_entry_path(&target),
                "pages/settings/index.html"
            );
        }
    }

    #[test]
    fn unknown_internal_page_route_does_not_resolve() {
        register_test_browser_internal_pages();
        assert!(
            internal_page_target_for_url("pages/newtab/index.html", "lingxia://unknown").is_none()
        );
    }

    #[test]
    fn lingxia_asset_hosts_delegate_to_lx_handler() {
        assert!(is_browser_lingxia_asset_host("lxapp"));
        assert!(is_browser_lingxia_asset_host("assets"));
        assert!(is_browser_lingxia_asset_host("plugin"));
        assert!(!is_browser_lingxia_asset_host("settings"));
        assert!(!is_browser_lingxia_asset_host("downloads"));
    }
}
