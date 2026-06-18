//! Browser tab state: tab id resolution and scopes, open/close/update/activate,
//! and the create-token machinery shared with WebView creation.

use crate::BUILTIN_BROWSER_APPID;
use crate::policy::{is_lingxia_startup_url, normalize_browser_target_url};
use crate::types::{BrowserAutomationError, BrowserTabInfo};
use crate::webview::{
    browser_create_webview, browser_destroy_webview, browser_find_webview, browser_load_url,
};
use lxapp::{LxApp, LxAppError};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

pub(crate) const INTERNAL_TAB_PATH_PREFIX: &str = "/tabs/";

// Internal browser tab model:
// 1) All tabs are hosted by the built-in browser lxapp (BUILTIN_BROWSER_APPID).
// 2) Callers may provide a stable tab key; the core resolves that key against an
//    explicit scope and maps it to a canonical runtime UUID tab id.
// 3) One canonical runtime tab id maps to one page path: /tabs/{tab_id}.
// 4) One canonical runtime tab id owns one managed WebView instance lifecycle.

#[derive(Clone)]
pub(crate) struct BrowserTabState {
    pub(crate) session_id: u64,
    /// Monotonic token to identify the current create lifecycle of this tab.
    /// Used to ignore stale async callbacks when tab gets recreated quickly.
    pub(crate) create_token: u64,
    /// True while a WebView create for `create_token` is still in-flight.
    /// Cleared once the create resolves; used to detect dead tabs whose
    /// earlier create failed so they can be recreated instead of being
    /// stuck with a `pending_url` that is never replayed.
    pub(crate) create_in_flight: bool,
    /// URL queued for loading while WebView creation is in-flight.
    pub(crate) pending_url: Option<String>,
    pub(crate) current_url: Option<String>,
    pub(crate) title: Option<String>,
    /// PNG-encoded favicon of the current page, as reported by the platform
    /// webview (`WebViewDelegate::on_favicon_changed`). `Arc`'d so shell
    /// layers can mirror it into layout snapshots without copying.
    pub(crate) favicon_png: Option<Arc<Vec<u8>>>,
    /// When true the tab's WebView has been destroyed to free memory
    /// (Chrome-style discard); the entry/metadata is kept and the WebView is
    /// recreated from `current_url` on reactivation.
    pub(crate) discarded: bool,
    /// When true this tab is a standalone browser with no tab strip (e.g. a
    /// docked aside browser). New-window requests (`target=_blank`,
    /// `window.open`) load in the same WebView instead of spawning a new
    /// main-area tab, since there is no tab UI to surface them in.
    pub(crate) standalone: bool,
}

pub(crate) struct BrowserState {
    // tab_id -> tab lifecycle state (single WebView lifecycle per tab_id)
    pub(crate) tabs: HashMap<String, BrowserTabState>,
}

static BROWSER_STATE: OnceLock<Mutex<BrowserState>> = OnceLock::new();
static BROWSER_TAB_COUNTER: AtomicU64 = AtomicU64::new(1);
static BROWSER_CREATE_TOKEN: AtomicU64 = AtomicU64::new(1);
static BROWSER_LOAD_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
static BROWSER_ACTIVE_TAB_ID: OnceLock<Mutex<Option<String>>> = OnceLock::new();
static BROWSER_TABS_CHANGED_HANDLER: OnceLock<Mutex<Option<TabsChangedHandler>>> = OnceLock::new();

/// Process-wide observer invoked when the browser tab set/metadata changes.
type TabsChangedHandler = Arc<dyn Fn() + Send + Sync>;

pub(crate) fn set_tabs_changed_handler(handler: TabsChangedHandler) {
    let slot = BROWSER_TABS_CHANGED_HANDLER.get_or_init(|| Mutex::new(None));
    if let Ok(mut slot) = slot.lock() {
        *slot = Some(handler);
    }
}

/// Invokes the registered tabs-changed handler (if any). Must never be
/// called while a browser state lock is held: handlers typically read the
/// tab list back synchronously.
pub(crate) fn notify_tabs_changed() {
    let handler = BROWSER_TABS_CHANGED_HANDLER
        .get()
        .and_then(|slot| slot.lock().ok())
        .and_then(|slot| slot.clone());
    if let Some(handler) = handler {
        handler();
    }
}

pub(crate) fn lock_state() -> MutexGuard<'static, BrowserState> {
    BROWSER_STATE
        .get_or_init(|| {
            Mutex::new(BrowserState {
                tabs: HashMap::new(),
            })
        })
        .lock()
        .unwrap_or_else(|e| {
            lxapp::warn!("[InternalBrowser] recovered poisoned browser state mutex");
            e.into_inner()
        })
}

fn lock_active_tab() -> MutexGuard<'static, Option<String>> {
    BROWSER_ACTIVE_TAB_ID
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// Sets the active tab; returns whether the active tab actually changed.
fn set_active_browser_tab(tab_id: &str) -> bool {
    let mut active = lock_active_tab();
    if active.as_deref() == Some(tab_id) {
        return false;
    }
    *active = Some(tab_id.to_string());
    true
}

#[derive(Clone, Copy)]
pub(crate) enum BrowserTabScope<'a> {
    Global,
    OwnerSession {
        owner_appid: &'a str,
        owner_session_id: u64,
    },
}

fn generate_tab_id() -> String {
    loop {
        let candidate = format!(
            "tab-{}",
            BROWSER_TAB_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        if !lock_state().tabs.contains_key(&candidate) {
            return candidate;
        }
    }
}

fn validate_requested_tab_key(input: &str) -> Result<String, LxAppError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(LxAppError::InvalidParameter(
            "tab_id is required".to_string(),
        ));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(LxAppError::InvalidParameter(
            "tab_id must contain only ASCII letters, digits, '-' or '_'".to_string(),
        ));
    }
    Ok(trimmed.to_ascii_lowercase())
}

pub(crate) fn normalize_runtime_tab_id(input: &str) -> Option<String> {
    validate_requested_tab_key(input).ok()
}

fn resolve_tab_scope_seed(scope: BrowserTabScope<'_>, stable_tab_key: &str) -> String {
    match scope {
        BrowserTabScope::Global => format!("global:{stable_tab_key}"),
        BrowserTabScope::OwnerSession {
            owner_appid,
            owner_session_id,
        } => format!("owner:{owner_appid}:{owner_session_id}:{stable_tab_key}"),
    }
}

fn deterministic_tab_suffix(seed: &str) -> String {
    const FNV_OFFSET_A: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    fn fnv1a64(bytes: &[u8], offset: u64, prime: u64) -> u64 {
        let mut hash = offset;
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(prime);
        }
        hash
    }

    format!(
        "{:08x}",
        fnv1a64(seed.as_bytes(), FNV_OFFSET_A, FNV_PRIME) as u32
    )
}

fn resolve_browser_tab_id(
    requested_tab_key: Option<&str>,
    scope: BrowserTabScope<'_>,
) -> Result<String, LxAppError> {
    match requested_tab_key {
        Some(tab_key) => {
            let stable_tab_key = validate_requested_tab_key(tab_key)?;
            match scope {
                BrowserTabScope::Global => Ok(stable_tab_key),
                BrowserTabScope::OwnerSession { .. } => {
                    let seed = resolve_tab_scope_seed(scope, &stable_tab_key);
                    Ok(format!(
                        "{}-{}",
                        stable_tab_key,
                        deterministic_tab_suffix(&seed)
                    ))
                }
            }
        }
        None => Ok(generate_tab_id()),
    }
}

fn next_browser_create_token() -> u64 {
    BROWSER_CREATE_TOKEN.fetch_add(1, Ordering::Relaxed)
}

// ---------------------------------------------------------------------------
// Owner resolution (used by FFI bridge layer)
// ---------------------------------------------------------------------------

fn resolve_owner_lxapp(owner_appid: &str, owner_session_id: u64) -> Result<Arc<LxApp>, LxAppError> {
    let owner_appid = owner_appid.trim();
    if owner_appid.is_empty() || owner_session_id == 0 {
        return Err(LxAppError::InvalidParameter(
            "owner_appid and owner_session_id are required".to_string(),
        ));
    }

    let owner = lxapp::try_get(owner_appid).ok_or_else(|| {
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

pub(crate) fn register_builtin_browser_host() {
    // Synthetic host: just owns tab session_id + page lifecycle. shell-runtime
    // upgrades this to a real asset bundle later (see lingxia-shell).
    lxapp::register_synthetic_lxapp(BUILTIN_BROWSER_APPID);
}

/// Ensure browser lxapp instance exists in manager.
pub(crate) fn ensure_browser_lxapp() -> Result<Arc<LxApp>, LxAppError> {
    let _load_guard = BROWSER_LOAD_MUTEX
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    if let Some(browser) = lxapp::try_get(BUILTIN_BROWSER_APPID) {
        return Ok(browser);
    }

    lxapp::ensure_builtin_lxapp(BUILTIN_BROWSER_APPID)
}

pub(crate) fn browser_tab_path_for_runtime_id(tab_id: &str) -> String {
    format!("{INTERNAL_TAB_PATH_PREFIX}{tab_id}")
}

pub(crate) fn browser_tab_path_for_id(tab_id: &str) -> String {
    normalize_runtime_tab_id(tab_id)
        .map(|tab_id| browser_tab_path_for_runtime_id(&tab_id))
        .unwrap_or_else(|| INTERNAL_TAB_PATH_PREFIX.to_string())
}

pub(crate) fn normalize_optional_string(value: Option<&str>) -> Option<String> {
    let text = value.unwrap_or_default().trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

fn build_tab_info(tab_id: &str, state: &BrowserTabState) -> BrowserTabInfo {
    BrowserTabInfo {
        tab_id: tab_id.to_string(),
        path: browser_tab_path_for_runtime_id(tab_id),
        session_id: state.session_id,
        current_url: state.current_url.clone(),
        title: state.title.clone(),
    }
}

pub fn browser_tab_info(tab_id: &str) -> Option<BrowserTabInfo> {
    let normalized = normalize_runtime_tab_id(tab_id)?;
    let state = lock_state();
    state
        .tabs
        .get(&normalized)
        .map(|tab| build_tab_info(&normalized, tab))
}

pub fn browser_tabs() -> Vec<BrowserTabInfo> {
    let state = lock_state();
    let mut tabs: Vec<BrowserTabInfo> = state
        .tabs
        .iter()
        .map(|(tab_id, tab)| build_tab_info(tab_id, tab))
        .collect();
    tabs.sort_by(|a, b| a.tab_id.cmp(&b.tab_id));
    tabs
}

pub fn browser_current_tab() -> Option<BrowserTabInfo> {
    if let Some(tab_id) = lock_active_tab().clone()
        && let Some(info) = browser_tab_info(&tab_id)
    {
        return Some(info);
    }
    browser_tabs().into_iter().next()
}

pub fn browser_activate_tab(tab_id: &str) -> Result<BrowserTabInfo, BrowserAutomationError> {
    let normalized_tab_id = normalize_runtime_tab_id(tab_id)
        .ok_or_else(|| BrowserAutomationError::TabNotFound(tab_id.to_string()))?;
    let info = browser_tab_info(&normalized_tab_id)
        .ok_or_else(|| BrowserAutomationError::TabNotFound(tab_id.to_string()))?;
    if set_active_browser_tab(&normalized_tab_id) {
        notify_tabs_changed();
    }
    Ok(info)
}

pub(crate) fn browser_update_tab_info(
    tab_id: &str,
    current_url: Option<&str>,
    title: Option<&str>,
) -> bool {
    let Some(normalized) = normalize_runtime_tab_id(tab_id) else {
        return false;
    };
    let changed = {
        let mut state = lock_state();
        let Some(tab) = state.tabs.get_mut(&normalized) else {
            return false;
        };
        let mut changed = false;
        if current_url.is_some() {
            let value = normalize_optional_string(current_url);
            if tab.current_url != value {
                tab.current_url = value;
                changed = true;
            }
        }
        if title.is_some() {
            let value = normalize_optional_string(title);
            if tab.title != value {
                tab.title = value;
                changed = true;
            }
        }
        changed
    };
    if changed {
        notify_tabs_changed();
    }
    true
}

/// Stores the PNG favicon reported by the platform webview for `tab_id`
/// (empty bytes clear it) and fires the tabs-changed observer when it
/// actually changed. Returns `false` when the tab does not exist.
pub(crate) fn browser_update_tab_favicon(tab_id: &str, png_bytes: Vec<u8>) -> bool {
    let Some(normalized) = normalize_runtime_tab_id(tab_id) else {
        return false;
    };
    let value = if png_bytes.is_empty() {
        None
    } else {
        Some(Arc::new(png_bytes))
    };
    let changed = {
        let mut state = lock_state();
        let Some(tab) = state.tabs.get_mut(&normalized) else {
            return false;
        };
        let same = match (&tab.favicon_png, &value) {
            (None, None) => true,
            (Some(old), Some(new)) => old.as_ref() == new.as_ref(),
            _ => false,
        };
        if !same {
            tab.favicon_png = value;
        }
        !same
    };
    if changed {
        notify_tabs_changed();
    }
    true
}

/// PNG favicon currently stored for `tab_id`, if any.
pub(crate) fn browser_tab_favicon(tab_id: &str) -> Option<Arc<Vec<u8>>> {
    let normalized = normalize_runtime_tab_id(tab_id)?;
    lock_state()
        .tabs
        .get(&normalized)
        .and_then(|tab| tab.favicon_png.clone())
}

// ---------------------------------------------------------------------------
// Create-token machinery (shared with the WebView creation flow)
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub(crate) enum TabCreateState {
    Active { pending_url: Option<String> },
    Missing,
    Stale,
}

pub(crate) fn browser_tab_create_state(
    tab_id: &str,
    session_id: u64,
    create_token: u64,
) -> TabCreateState {
    let mut state = lock_state();
    match state.tabs.get_mut(tab_id) {
        Some(tab) if tab.session_id == session_id && tab.create_token == create_token => {
            // This create cycle now owns a live WebView; clear the in-flight
            // marker so a missing WebView later means the tab must be recreated.
            tab.create_in_flight = false;
            TabCreateState::Active {
                pending_url: tab.pending_url.clone(),
            }
        }
        Some(_) => TabCreateState::Stale,
        None => TabCreateState::Missing,
    }
}

pub(crate) fn browser_remove_tab_if_token_matches(
    tab_id: &str,
    session_id: u64,
    create_token: u64,
) {
    let removed = {
        let mut state = lock_state();
        let should_remove = state
            .tabs
            .get(tab_id)
            .map(|tab| tab.session_id == session_id && tab.create_token == create_token)
            .unwrap_or(false);
        if should_remove {
            state.tabs.remove(tab_id);
        }
        should_remove
    };
    if removed {
        notify_tabs_changed();
    }
}

pub(crate) fn browser_clear_pending_if_token_matches(
    tab_id: &str,
    session_id: u64,
    create_token: u64,
) {
    let mut state = lock_state();
    if let Some(tab) = state.tabs.get_mut(tab_id)
        && tab.session_id == session_id
        && tab.create_token == create_token
    {
        tab.pending_url = None;
    }
}

pub(crate) fn browser_commit_navigation_if_token_matches(
    tab_id: &str,
    session_id: u64,
    create_token: u64,
    current_url: Option<&str>,
) {
    let committed = {
        let mut state = lock_state();
        if let Some(tab) = state.tabs.get_mut(tab_id)
            && tab.session_id == session_id
            && tab.create_token == create_token
        {
            tab.pending_url = None;
            tab.current_url = normalize_optional_string(current_url);
            true
        } else {
            false
        }
    };
    if committed {
        notify_tabs_changed();
    }
}

fn browser_clear_pending_url(tab_id: &str) {
    let mut state = lock_state();
    if let Some(tab) = state.tabs.get_mut(tab_id) {
        tab.pending_url = None;
    }
}

// ---------------------------------------------------------------------------
// Open / close
// ---------------------------------------------------------------------------

fn open_internal_browser_tab_with_scope(
    url: &str,
    requested_tab_key: Option<&str>,
    scope: BrowserTabScope<'_>,
    standalone: bool,
) -> Result<String, LxAppError> {
    let browser = ensure_browser_lxapp()?;
    let browser_session_id = browser.session_id();

    let raw_url = url.trim();

    // `lingxia://newtab` (and bare `lingxia://`) → startup page (no URL).
    // Other `lingxia://` pages stay as-is and are served by the lingxia:// scheme handler.
    let effective_url: String = match is_lingxia_startup_url(raw_url) {
        Some(true) => String::new(),
        _ => raw_url.to_string(),
    };
    let target_url = effective_url.as_str();

    let normalized_target_url = normalize_browser_target_url(target_url);
    let has_target_url = !normalized_target_url.is_empty();
    let tab_id = resolve_browser_tab_id(requested_tab_key, scope)?;
    let path = browser_tab_path_for_runtime_id(&tab_id);
    let session_id = browser_session_id;
    let mut create_token: Option<u64> = None;
    let mut is_new_tab = false;

    {
        let mut state = lock_state();
        if let Some(existing) = state.tabs.get_mut(&tab_id) {
            existing.session_id = session_id;
            if has_target_url {
                existing.pending_url = Some(normalized_target_url.clone());
            }
        } else {
            is_new_tab = true;
            let token = next_browser_create_token();
            create_token = Some(token);
            state.tabs.insert(
                tab_id.clone(),
                BrowserTabState {
                    session_id,
                    create_token: token,
                    create_in_flight: true,
                    pending_url: if has_target_url {
                        Some(normalized_target_url.clone())
                    } else {
                        None
                    },
                    current_url: None,
                    title: None,
                    favicon_png: None,
                    discarded: false,
                    standalone,
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
        // A standalone (docked aside) browser is independent of the main tab
        // model — it must not become the process-wide active browser tab, which
        // drives the main coordinator's active-tab and memory policy.
        if !standalone {
            let _ = set_active_browser_tab(&tab_id);
        }
        notify_tabs_changed();
        return Ok(tab_id);
    }

    // Existing tab — load target URL if provided.
    if has_target_url {
        match browser_load_url(&path, session_id, &normalized_target_url) {
            Ok(()) => {
                if let Some(s) = lock_state().tabs.get_mut(&tab_id) {
                    s.pending_url = None;
                    s.current_url = Some(normalized_target_url.clone());
                }
            }
            Err(LxAppError::ResourceNotFound(_)) => {
                // WebView is missing. If a create is still in-flight, keep
                // pending_url for replay once the WebView becomes ready.
                // Otherwise the earlier create failed (or the WebView is gone),
                // so start a fresh create cycle instead of leaving the tab dead.
                let retry_token = {
                    let mut state = lock_state();
                    match state.tabs.get_mut(&tab_id) {
                        Some(tab) if !tab.create_in_flight => {
                            let token = next_browser_create_token();
                            tab.create_token = token;
                            tab.create_in_flight = true;
                            Some(token)
                        }
                        _ => None,
                    }
                };
                if let Some(token) = retry_token
                    && let Err(e) = browser_create_webview(&path, session_id, &tab_id, token)
                {
                    lock_state().tabs.remove(&tab_id);
                    return Err(e);
                }
            }
            Err(e) => {
                browser_clear_pending_url(&tab_id);
                return Err(e);
            }
        }
    }

    let _ = set_active_browser_tab(&tab_id);
    notify_tabs_changed();
    Ok(tab_id)
}

pub(crate) fn open_internal_browser_tab(
    url: &str,
    tab_id: Option<&str>,
) -> Result<String, LxAppError> {
    open_internal_browser_tab_with_scope(url, tab_id, BrowserTabScope::Global, false)
}

pub(crate) fn open_internal_browser_tab_for_owner(
    owner_appid: &str,
    owner_session_id: u64,
    url: &str,
    tab_id: Option<&str>,
    standalone: bool,
) -> Result<String, LxAppError> {
    let _owner = resolve_owner_lxapp(owner_appid, owner_session_id)?;
    open_internal_browser_tab_with_scope(
        url,
        tab_id,
        BrowserTabScope::OwnerSession {
            owner_appid,
            owner_session_id,
        },
        standalone,
    )
}

/// Whether `tab_id` is a standalone (no-tab-strip) browser tab whose new-window
/// requests should load inline rather than spawn a new main-area tab.
pub(crate) fn is_standalone_tab(tab_id: &str) -> bool {
    let Some(normalized) = normalize_runtime_tab_id(tab_id) else {
        return false;
    };
    lock_state()
        .tabs
        .get(&normalized)
        .map(|tab| tab.standalone)
        .unwrap_or(false)
}

pub fn browser_tab_exists(tab_id: &str) -> bool {
    let Some(normalized) = normalize_runtime_tab_id(tab_id) else {
        return false;
    };
    lock_state().tabs.contains_key(&normalized)
}

pub(crate) fn close_browser_tab(tab_id: &str) -> Result<(), LxAppError> {
    let normalized = normalize_runtime_tab_id(tab_id).ok_or_else(|| {
        LxAppError::InvalidParameter("tab_id must be a valid runtime browser tab id".to_string())
    })?;

    let removed = {
        let mut state = lock_state();
        state.tabs.remove(&normalized)
    };
    let removed_any = removed.is_some();
    if let Some(tab) = removed {
        let tab_path = browser_tab_path_for_runtime_id(&normalized);
        // Detach only when this tab currently backs the startup page bridge.
        // Closing a background tab must not break the active tab bridge.
        if let Ok(browser) = ensure_browser_lxapp() {
            let startup_path = browser.initial_route();
            if let Some(page) = browser.get_page(&startup_path) {
                let startup_webview = page.webview();
                let closing_tab_webview = browser_find_webview(&tab_path, tab.session_id).ok();
                if let (Some(startup_webview), Some(closing_tab_webview)) =
                    (startup_webview, closing_tab_webview)
                    && Arc::ptr_eq(&startup_webview, &closing_tab_webview)
                {
                    page.detach_webview();
                }
            }
            if let Some(page) = browser.get_page(&tab_path) {
                page.detach_webview();
            }
            browser.remove_pages(std::slice::from_ref(&tab_path));
        }
        browser_destroy_webview(&tab_path, tab.session_id);
    }
    let active_matches_closed = lock_active_tab().as_deref() == Some(normalized.as_str());
    if active_matches_closed {
        let next = browser_tabs().into_iter().next().map(|tab| tab.tab_id);
        *lock_active_tab() = next;
    }
    if removed_any || active_matches_closed {
        notify_tabs_changed();
    }
    Ok(())
}

/// Chrome-style tab discard: destroy the tab's WebView to free its native
/// memory while keeping the tab entry (`current_url` / `title`) so the sidebar
/// still shows it. Reactivation recreates the WebView and reloads the URL.
/// Refuses to discard the active tab.
pub(crate) fn discard_browser_tab(tab_id: &str) -> Result<(), LxAppError> {
    let normalized = normalize_runtime_tab_id(tab_id).ok_or_else(|| {
        LxAppError::InvalidParameter("tab_id must be a valid runtime browser tab id".to_string())
    })?;
    if lock_active_tab().as_deref() == Some(normalized.as_str()) {
        return Err(LxAppError::InvalidParameter(
            "cannot discard the active browser tab".to_string(),
        ));
    }
    let tab = match lock_state().tabs.get(&normalized).cloned() {
        // Unknown or already discarded — nothing to free.
        Some(tab) if !tab.discarded => tab,
        _ => return Ok(()),
    };
    let tab_path = browser_tab_path_for_runtime_id(&normalized);

    // Bump the create token BEFORE destroying the WebView. If the WebView is
    // still being created, its in-flight `browser_on_webview_ready` holds the
    // old token; once `wait_ready()` errors after the destroy below, its
    // `browser_remove_tab_if_token_matches(old)` no longer matches and the
    // kept entry survives (otherwise reactivate would hit ResourceNotFound).
    if let Some(state) = lock_state().tabs.get_mut(&normalized) {
        state.create_token = next_browser_create_token();
    }

    // Detach from the shared startup bridge if this tab backs it, and drop any
    // per-tab internal page — same dance as close_browser_tab.
    if let Ok(browser) = ensure_browser_lxapp() {
        let startup_path = browser.initial_route();
        if let Some(page) = browser.get_page(&startup_path) {
            let startup_webview = page.webview();
            let tab_webview = browser_find_webview(&tab_path, tab.session_id).ok();
            if let (Some(startup_webview), Some(tab_webview)) = (startup_webview, tab_webview)
                && Arc::ptr_eq(&startup_webview, &tab_webview)
            {
                page.detach_webview();
            }
        }
        if let Some(page) = browser.get_page(&tab_path) {
            page.detach_webview();
        }
        browser.remove_pages(std::slice::from_ref(&tab_path));
    }
    browser_destroy_webview(&tab_path, tab.session_id);

    // Keep the entry; remember where to reload from on reactivation. Preserve
    // an in-flight `pending_url` (WebView not yet loaded / mid-navigation);
    // only fall back to `current_url` when there is no pending target.
    if let Some(state) = lock_state().tabs.get_mut(&normalized) {
        state.discarded = true;
        state.create_in_flight = false;
        if state.pending_url.is_none() {
            state.pending_url = state.current_url.clone();
        }
    }
    Ok(())
}

/// Mark a tab as the active one without touching its WebView. Lets the SDK keep
/// the Rust-side active tab in sync when switching to an already-live tab, so
/// the discard policy doesn't mistake a backgrounded tab for the active one.
pub(crate) fn mark_browser_tab_active(tab_id: &str) {
    let Some(normalized) = normalize_runtime_tab_id(tab_id) else {
        return;
    };
    let exists = lock_state().tabs.contains_key(&normalized);
    if exists && set_active_browser_tab(&normalized) {
        notify_tabs_changed();
    }
}

/// Recreate a discarded tab's WebView and reload its saved URL, then make it
/// the active tab. No-op if the tab is already live.
pub(crate) fn reactivate_browser_tab(tab_id: &str) -> Result<(), LxAppError> {
    let normalized = normalize_runtime_tab_id(tab_id).ok_or_else(|| {
        LxAppError::InvalidParameter("tab_id must be a valid runtime browser tab id".to_string())
    })?;
    // Returns the create params when the tab needs its WebView rebuilt, or
    // `None` when it is already live (just needs (re)activating below).
    let recreate = {
        let mut state = lock_state();
        let Some(tab) = state.tabs.get_mut(&normalized) else {
            return Err(LxAppError::ResourceNotFound(
                "browser tab not found".to_string(),
            ));
        };
        if tab.discarded {
            let token = next_browser_create_token();
            tab.create_token = token;
            tab.discarded = false;
            tab.create_in_flight = true;
            // `pending_url` already holds the saved `current_url` from discard.
            Some((tab.session_id, token))
        } else {
            None
        }
    };

    if let Some((session_id, token)) = recreate {
        let path = browser_tab_path_for_runtime_id(&normalized);
        if let Err(error) = browser_create_webview(&path, session_id, &normalized, token) {
            if let Some(tab) = lock_state().tabs.get_mut(&normalized) {
                tab.discarded = true;
                tab.create_in_flight = false;
            }
            return Err(error);
        }
    }

    if set_active_browser_tab(&normalized) {
        notify_tabs_changed();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_browser_tab_ids_are_deterministic_per_scope() {
        let global_a = resolve_browser_tab_id(Some("settings"), BrowserTabScope::Global).unwrap();
        let global_b = resolve_browser_tab_id(Some("settings"), BrowserTabScope::Global).unwrap();
        let owner_a = resolve_browser_tab_id(
            Some("settings"),
            BrowserTabScope::OwnerSession {
                owner_appid: "app.demo",
                owner_session_id: 1,
            },
        )
        .unwrap();
        let owner_b = resolve_browser_tab_id(
            Some("settings"),
            BrowserTabScope::OwnerSession {
                owner_appid: "app.demo",
                owner_session_id: 2,
            },
        )
        .unwrap();

        assert_eq!(global_a, global_b);
        assert_ne!(global_a, owner_a);
        assert_ne!(owner_a, owner_b);
    }

    #[test]
    fn stable_browser_tab_ids_reject_invalid_keys() {
        let result = resolve_browser_tab_id(Some("settings/main"), BrowserTabScope::Global);
        assert!(matches!(result, Err(LxAppError::InvalidParameter(_))));
    }

    #[test]
    fn runtime_tab_id_lookup_normalizes_stable_keys() {
        assert_eq!(
            normalize_runtime_tab_id("settings"),
            Some("settings".to_string())
        );
        assert_eq!(
            normalize_runtime_tab_id("SeTtings"),
            Some("settings".to_string())
        );
        assert!(normalize_runtime_tab_id("settings/main").is_none());
    }
}
