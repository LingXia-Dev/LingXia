use crate::{LxApp, LxAppError};
use lingxia_webview::{
    WebTag, WebView, WebViewController, WebViewCreateOptions, WebViewError,
    create_webview_with_options, destroy_webview as destroy_managed_webview,
    find_webview as find_managed_webview,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use tokio::sync::oneshot;
use uuid::Uuid;

pub const BUILTIN_BROWSER_APPID: &str = "app.lingxia.browser";
const INTERNAL_TAB_PATH_PREFIX: &str = "/tabs/";

#[derive(Clone)]
struct BrowserTabState {
    source_appid: String,
    session_id: u64,
    /// Monotonic token to identify the current create lifecycle of this tab.
    /// Used to ignore stale async callbacks when tab gets recreated quickly.
    create_token: u64,
    /// URL queued for loading while WebView creation is in-flight.
    pending_url: Option<String>,
}

impl BrowserTabState {
    fn verify_owner(&self, lxapp: &LxApp, tab_id: &str) -> Result<(), LxAppError> {
        if self.source_appid != lxapp.appid || self.session_id != lxapp.session_id() {
            return Err(LxAppError::UnsupportedOperation(format!(
                "internal browser tab {} is owned by {}:{}, not {}:{}",
                tab_id,
                self.source_appid,
                self.session_id,
                lxapp.appid,
                lxapp.session_id()
            )));
        }
        Ok(())
    }
}

struct BrowserState {
    tabs: HashMap<String, BrowserTabState>,
}

static BROWSER_STATE: OnceLock<Mutex<BrowserState>> = OnceLock::new();
static BROWSER_CREATE_TOKEN: AtomicU64 = AtomicU64::new(1);

fn lock_state() -> MutexGuard<'static, BrowserState> {
    BROWSER_STATE
        .get_or_init(|| {
            Mutex::new(BrowserState {
                tabs: HashMap::new(),
            })
        })
        .lock()
        .unwrap_or_else(|e| {
            crate::warn!("[InternalBrowser] recovered poisoned browser state mutex");
            e.into_inner()
        })
}

fn sanitize_tab_id(input: &str) -> String {
    input
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

fn generate_tab_id() -> String {
    Uuid::new_v4().to_string()
}

fn next_browser_create_token() -> u64 {
    BROWSER_CREATE_TOKEN.fetch_add(1, Ordering::Relaxed)
}

fn latest_owner_tab_id(lxapp: &LxApp) -> Option<String> {
    lock_state()
        .tabs
        .iter()
        .filter(|(_, tab)| tab.source_appid == lxapp.appid && tab.session_id == lxapp.session_id())
        .max_by_key(|(_, tab)| tab.create_token)
        .map(|(tab_id, _)| tab_id.clone())
}

// ---------------------------------------------------------------------------
// WebView helpers — thin wrappers around lingxia-webview cross-platform API
// ---------------------------------------------------------------------------

fn browser_webtag(path: &str, session_id: u64) -> WebTag {
    WebTag::new(BUILTIN_BROWSER_APPID, path, Some(session_id))
}

fn browser_create_webview(
    path: &str,
    session_id: u64,
    tab_id: &str,
    create_token: u64,
) -> Result<(), LxAppError> {
    let webtag = browser_webtag(path, session_id);
    let (ready_tx, ready_rx) = oneshot::channel();
    create_webview_with_options(&webtag, WebViewCreateOptions::browser_relaxed(), ready_tx);
    let path_owned = path.to_string();
    let tab_id_owned = tab_id.to_string();

    if let Err(e) = rong::bg::spawn(async move {
        browser_on_webview_ready(path_owned, session_id, tab_id_owned, create_token, ready_rx)
            .await;
    }) {
        return Err(LxAppError::Runtime(format!(
            "failed to spawn browser webview ready task: {}",
            e
        )));
    }
    Ok(())
}

async fn browser_on_webview_ready(
    path: String,
    session_id: u64,
    tab_id: String,
    create_token: u64,
    ready_rx: oneshot::Receiver<Result<Arc<WebView>, WebViewError>>,
) {
    let webview = match ready_rx.await {
        Ok(Ok(webview)) => webview,
        Ok(Err(e)) => {
            crate::warn!(
                "[InternalBrowser] Failed to create webview for tab {}: {}",
                tab_id,
                e
            );
            browser_remove_tab_if_token_matches(&tab_id, session_id, create_token);
            return;
        }
        Err(e) => {
            crate::warn!(
                "[InternalBrowser] WebView ready signal failed for tab {}: {}",
                tab_id,
                e
            );
            browser_remove_tab_if_token_matches(&tab_id, session_id, create_token);
            return;
        }
    };

    let tab_state = browser_tab_create_state(&tab_id, session_id, create_token);
    match tab_state {
        TabCreateState::Missing => {
            // Tab was closed while creation was in-flight.
            browser_destroy_webview(&path, session_id);
            return;
        }
        TabCreateState::Stale => {
            // A newer create lifecycle already took ownership of this tab id.
            // Destroy the orphaned webview from this old create cycle.
            browser_destroy_webview(&path, session_id);
            return;
        }
        TabCreateState::Active { pending_url } => {
            if let Some(url) = pending_url {
                if let Err(e) = webview.load_url(url) {
                    crate::warn!(
                        "[InternalBrowser] Failed to load URL for tab {}: {}",
                        tab_id,
                        e
                    );
                }
            } else {
                let result = generate_browser_startup_html().and_then(|(html, base_url)| {
                    let html_str = String::from_utf8_lossy(&html);
                    webview
                        .load_data(html_str.into_owned(), base_url, None)
                        .map_err(|e| LxAppError::WebView(e.to_string()))
                });
                if let Err(e) = result {
                    crate::warn!(
                        "[InternalBrowser] Failed to load startup page for tab {}: {}",
                        tab_id,
                        e
                    );
                }
            }
            browser_clear_pending_if_token_matches(&tab_id, session_id, create_token);
        }
    }
}

#[derive(Debug)]
enum TabCreateState {
    Active { pending_url: Option<String> },
    Missing,
    Stale,
}

fn browser_tab_create_state(tab_id: &str, session_id: u64, create_token: u64) -> TabCreateState {
    let state = lock_state();
    match state.tabs.get(tab_id) {
        Some(tab) if tab.session_id == session_id && tab.create_token == create_token => {
            TabCreateState::Active {
                pending_url: tab.pending_url.clone(),
            }
        }
        Some(_) => TabCreateState::Stale,
        None => TabCreateState::Missing,
    }
}

fn browser_remove_tab_if_token_matches(tab_id: &str, session_id: u64, create_token: u64) {
    let mut state = lock_state();
    let should_remove = state
        .tabs
        .get(tab_id)
        .map(|tab| tab.session_id == session_id && tab.create_token == create_token)
        .unwrap_or(false);
    if should_remove {
        state.tabs.remove(tab_id);
    }
}

fn browser_clear_pending_if_token_matches(tab_id: &str, session_id: u64, create_token: u64) {
    let mut state = lock_state();
    if let Some(tab) = state.tabs.get_mut(tab_id)
        && tab.session_id == session_id
        && tab.create_token == create_token
    {
        tab.pending_url = None;
    }
}

fn browser_find_webview(path: &str, session_id: u64) -> Result<Arc<WebView>, LxAppError> {
    let webtag = browser_webtag(path, session_id);
    find_managed_webview(&webtag).ok_or_else(|| {
        LxAppError::ResourceNotFound(format!("browser webview not found: {}", webtag.as_str()))
    })
}

fn browser_load_url(path: &str, session_id: u64, url: &str) -> Result<(), LxAppError> {
    let webview = browser_find_webview(path, session_id)?;
    webview
        .load_url(url.to_string())
        .map_err(|e| LxAppError::WebView(e.to_string()))
}

fn browser_destroy_webview(path: &str, session_id: u64) {
    let webtag = browser_webtag(path, session_id);
    // Follow the same cleanup pattern as normal lxapp pages:
    // 1. Remove delegate to break callback links before destruction
    // 2. Remove from global registry (triggers platform-specific cleanup on Drop)
    if let Some(webview) = find_managed_webview(&webtag) {
        webview.remove_delegate();
    }
    destroy_managed_webview(&webtag);
}

// ---------------------------------------------------------------------------
// Browser startup page
// ---------------------------------------------------------------------------

/// Generate the browser startup page HTML with bridge injection.
///
/// Reads the startup page from the registered browser LxApp, applies bridge
/// config and CSS injection (same pipeline as normal lxapp pages), and returns
/// the HTML bytes together with the `lx://` base URL for asset resolution.
pub fn generate_browser_startup_html() -> Result<(Vec<u8>, String), LxAppError> {
    let browser = crate::try_get(BUILTIN_BROWSER_APPID).ok_or_else(|| {
        LxAppError::ResourceNotFound(format!("{} lxapp not registered", BUILTIN_BROWSER_APPID))
    })?;
    let startup_page = browser.config.get_initial_route();
    if startup_page.is_empty() {
        return Err(LxAppError::InvalidParameter(format!(
            "{} has no startup page configured in lxapp.json",
            BUILTIN_BROWSER_APPID
        )));
    }
    let html = browser.generate_page_html(&startup_page, None);
    let base_url = format!("lx://lxapp/{}/{}", BUILTIN_BROWSER_APPID, startup_page);
    Ok((html, base_url))
}

// ---------------------------------------------------------------------------
// Owner resolution (used by FFI bridge layer)
// ---------------------------------------------------------------------------

pub fn resolve_owner_lxapp(
    owner_appid: &str,
    owner_session_id: u64,
) -> Result<Arc<LxApp>, LxAppError> {
    let owner_appid = owner_appid.trim();
    if owner_appid.is_empty() || owner_session_id == 0 {
        return Err(LxAppError::InvalidParameter(
            "owner_appid and owner_session_id are required".to_string(),
        ));
    }

    let owner = crate::try_get(owner_appid).ok_or_else(|| {
        LxAppError::ResourceNotFound(format!(
            "owner lxapp not found for browser tab operation: {}",
            owner_appid
        ))
    })?;

    if owner.session_id() != owner_session_id {
        return Err(LxAppError::InvalidParameter(format!(
            "owner session mismatch for {}: expected {}, got {}",
            owner_appid,
            owner.session_id(),
            owner_session_id
        )));
    }

    Ok(owner)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub fn browser_tab_path_for_id(tab_id: &str) -> String {
    format!("{INTERNAL_TAB_PATH_PREFIX}{}", sanitize_tab_id(tab_id))
}

pub fn browser_owner_appid_for_tab_id(tab_id: &str) -> Option<String> {
    let normalized = sanitize_tab_id(tab_id);
    if normalized.is_empty() {
        return None;
    }
    lock_state()
        .tabs
        .get(&normalized)
        .map(|state| state.source_appid.clone())
}

pub fn browser_owner_session_id_for_tab_id(tab_id: &str) -> u64 {
    let normalized = sanitize_tab_id(tab_id);
    if normalized.is_empty() {
        return 0;
    }
    lock_state()
        .tabs
        .get(&normalized)
        .map(|state| state.session_id)
        .unwrap_or(0)
}

pub fn open_internal_browser_tab(
    lxapp: &LxApp,
    url: &str,
    tab_id: Option<&str>,
) -> Result<String, LxAppError> {
    let target_url = url.trim();
    let has_target_url = !target_url.is_empty();
    let tab_id = tab_id
        .map(sanitize_tab_id)
        .filter(|v| !v.is_empty())
        .or_else(|| {
            if has_target_url {
                latest_owner_tab_id(lxapp)
            } else {
                None
            }
        })
        .unwrap_or_else(generate_tab_id);
    let path = browser_tab_path_for_id(&tab_id);
    let session_id = lxapp.session_id();
    let mut create_token: Option<u64> = None;
    let mut is_new_tab = false;

    {
        let mut state = lock_state();
        if let Some(existing) = state.tabs.get_mut(&tab_id) {
            existing.verify_owner(lxapp, &tab_id)?;
            if has_target_url {
                existing.pending_url = Some(target_url.to_string());
            }
        } else {
            is_new_tab = true;
            let token = next_browser_create_token();
            create_token = Some(token);
            state.tabs.insert(
                tab_id.clone(),
                BrowserTabState {
                    source_appid: lxapp.appid.clone(),
                    session_id,
                    create_token: token,
                    pending_url: if has_target_url {
                        Some(target_url.to_string())
                    } else {
                        None
                    },
                },
            );
        }
    }

    if is_new_tab {
        let token = create_token.expect("create_token must exist for new tab");
        if let Err(e) = browser_create_webview(&path, session_id, &tab_id, token) {
            lock_state().tabs.remove(&tab_id);
            return Err(e);
        }
        return Ok(tab_id);
    }

    // Existing tab — load target URL if provided.
    if has_target_url {
        match browser_load_url(&path, session_id, target_url) {
            Ok(()) => {
                if let Some(s) = lock_state().tabs.get_mut(&tab_id) {
                    s.pending_url = None;
                }
            }
            Err(LxAppError::ResourceNotFound(_)) => {
                // WebView may still be creating on another thread; keep pending_url for replay.
            }
            Err(e) => return Err(e),
        }
    }

    Ok(tab_id)
}

pub fn close_internal_browser_tab(lxapp: &LxApp, tab_id: &str) -> Result<(), LxAppError> {
    let normalized = sanitize_tab_id(tab_id);
    if normalized.is_empty() {
        return Err(LxAppError::InvalidParameter(
            "close_internal_browser_tab requires tab_id".to_string(),
        ));
    }

    {
        let mut state = lock_state();
        if let Some(tab) = state.tabs.get(&normalized) {
            tab.verify_owner(lxapp, &normalized)?;
        }
        state.tabs.remove(&normalized);
    }

    browser_destroy_webview(&browser_tab_path_for_id(&normalized), lxapp.session_id());
    Ok(())
}

pub fn browser_tab_exists(tab_id: &str) -> bool {
    let normalized = sanitize_tab_id(tab_id);
    if normalized.is_empty() {
        return false;
    }
    lock_state().tabs.contains_key(&normalized)
}

// Tab-id-only operations (resolve owner from stored tab state).
//
// Designed for platform FFI bridges where passing owner params back and forth
// adds unnecessary complexity — the tab state already knows its owner.
fn resolve_tab_owner(tab_id: &str) -> Result<(Arc<LxApp>, String), LxAppError> {
    let normalized = sanitize_tab_id(tab_id);
    if normalized.is_empty() {
        return Err(LxAppError::InvalidParameter(
            "tab_id is required".to_string(),
        ));
    }
    let owner_appid = lock_state()
        .tabs
        .get(&normalized)
        .map(|t| t.source_appid.clone())
        .ok_or_else(|| {
            LxAppError::ResourceNotFound(format!("browser tab not found: {}", normalized))
        })?;
    let owner = crate::try_get(&owner_appid).ok_or_else(|| {
        LxAppError::ResourceNotFound(format!("owner lxapp not found: {}", owner_appid))
    })?;
    Ok((owner, normalized))
}

pub fn close_browser_tab(tab_id: &str) -> Result<(), LxAppError> {
    match resolve_tab_owner(tab_id) {
        Ok((owner, normalized)) => close_internal_browser_tab(&owner, &normalized),
        Err(LxAppError::ResourceNotFound(_)) => Ok(()), // Already closed — idempotent
        Err(e) => Err(e),
    }
}

/// Look up the managed WebView for a browser tab by tab ID.
/// Returns `None` if the tab doesn't exist or the WebView hasn't been created yet.
pub fn find_browser_webview(tab_id: &str) -> Option<Arc<WebView>> {
    let normalized = sanitize_tab_id(tab_id);
    if normalized.is_empty() {
        return None;
    }
    let session_id = lock_state().tabs.get(&normalized).map(|t| t.session_id)?;
    let path = browser_tab_path_for_id(&normalized);
    let webtag = browser_webtag(&path, session_id);
    find_managed_webview(&webtag)
}
