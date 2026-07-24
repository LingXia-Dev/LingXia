//! WebView lifecycle for browser tabs: creation, the per-tab delegate, the
//! ready/replay flow, and find/load/destroy helpers.

use crate::BUILTIN_BROWSER_APPID;
use crate::chooser::browser_choose_files;
use crate::downloads::browser_download_resource;
use crate::internal_pages::{
    LingxiaSchemeContext, browser_attach_tab_page, browser_document_scripts_snapshot,
    browser_resolve_delegate_context, browser_resolve_delegate_page, ensure_browser_startup_page,
    handle_browser_lingxia_scheme,
};
use crate::policy::{
    BrowserNavigationPolicySession, LINGXIA_SCHEME, extract_url_scheme,
    normalize_browser_target_url,
};
use crate::tabs::{
    TabCreateState, browser_clear_pending_if_token_matches,
    browser_commit_navigation_if_token_matches, browser_remove_tab_if_token_matches,
    browser_tab_create_state, ensure_browser_lxapp,
};
use crate::types::{BrowserNavigationPolicyDecision, BrowserNavigationPolicyRequest};
use lingxia_log::{LogBuilder, LogLevel as LxLogLevel, LogTag};
use lingxia_platform::traits::app_runtime::{AppRuntime, OpenUrlRequest, OpenUrlTarget};
use lingxia_webview::runtime::{
    destroy_webview as destroy_managed_webview, find_webview as find_managed_webview,
};
use lingxia_webview::{
    LoadDataRequest, LoadError, LoadErrorPage, LogLevel, NavigationEvent, NavigationPolicy,
    NavigationProgress, NewWindowPolicy, WebTag, WebView, WebViewBuilder, WebViewController,
    WebViewDataMode, WebViewDelegate, WebViewSession, WebViewStateChange, render_load_error_page,
};
use lxapp::LxAppError;
use serde_json::Value;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

// ---------------------------------------------------------------------------
// Browser tab WebView delegate
// ---------------------------------------------------------------------------

/// WebView delegate for browser tab WebViews.
///
/// Each tab binds its own headless `PageInstance` (keyed by tab path), and
/// only browser-internal `lingxia://` documents drive that page's lifecycle
/// and receive its bridge. External documents update browser tab state only:
/// they never call `notify_page_started`/`handle_loaded` and never emit an
/// internal page's `onReady`.
struct BrowserTabDelegate {
    tab_id: String,
    page_path: String,
    session_id: u64,
    navigation: std::sync::Mutex<BrowserTabNavigationState>,
}

#[derive(Default)]
struct BrowserTabNavigationState {
    /// Canonical attempt-correlation fold over typed navigation events.
    progress: NavigationProgress,
    /// True from a document load failure until the next real navigation
    /// starts. While set, Location/Title changes are suppressed so the tab
    /// keeps showing the failing URL, and the error document's own lifecycle
    /// never records a visit or drives the internal page.
    showing_error_page: bool,
}

const BROWSER_LOAD_ERROR_URL: &str = "lingxia://browser/load-error";

/// Platforms may normalize the document URL they report (trailing slash,
/// scheme case), so error-document detection compares loosely.
fn is_error_document_url(url: &str) -> bool {
    url.trim_end_matches('/')
        .eq_ignore_ascii_case(BROWSER_LOAD_ERROR_URL)
}

impl BrowserTabDelegate {
    fn document_is_internal(&self) -> bool {
        crate::tabs::browser_tab_document_is_internal(&self.tab_id)
    }

    fn inject_document_scripts(&self) {
        let scripts = browser_document_scripts_snapshot();
        if scripts.is_empty() {
            return;
        }
        // exec_js is same-thread-safe on every backend (the Windows
        // controller queues without waiting on its own UI thread), so the
        // delegate can inject directly from the event callback.
        let Ok(webview) = browser_find_webview(&self.page_path, self.session_id) else {
            return;
        };
        for js in &scripts {
            if let Err(err) = webview.exec_js(js) {
                lxapp::warn!(
                    "[InternalBrowser] document script injection failed for tab {}: {}",
                    self.tab_id,
                    err
                );
            }
        }
    }

    fn show_load_error(&self, error: &LoadError) {
        crate::internal_pages::detach_internal_tab_page(&self.page_path);
        let failing_url = error.failing_url.as_deref().unwrap_or_default();
        let title =
            lingxia_platform::i18n::text("webview.load_error_title", "Couldn't load this page");
        let _ = crate::tabs::browser_update_tab_info(
            &self.tab_id,
            (!failing_url.is_empty()).then_some(failing_url),
            Some(&title),
        );

        let Ok(webview) = browser_find_webview(&self.page_path, self.session_id) else {
            return;
        };
        let message = lingxia_platform::i18n::text(
            "webview.load_error_message",
            "Check your connection and try again.",
        );
        let retry = lingxia_platform::i18n::text("webview.retry", "Retry");
        let html = render_load_error_page(LoadErrorPage {
            title: &title,
            message: &message,
            retry_label: &retry,
            retry_url: failing_url,
        });
        if let Err(err) = webview.load_data(
            LoadDataRequest::new(&html, BROWSER_LOAD_ERROR_URL)
                .with_history_url(BROWSER_LOAD_ERROR_URL),
        ) {
            if let Ok(mut navigation) = self.navigation.lock() {
                navigation.showing_error_page = false;
            }
            lxapp::warn!(
                "[InternalBrowser] Failed to render load error for tab {}: {}",
                self.tab_id,
                err
            );
        }
    }
}

impl WebViewDelegate for BrowserTabDelegate {
    fn on_navigation_event(&self, event: NavigationEvent) {
        let mut navigation = self.navigation.lock().unwrap_or_else(|e| e.into_inner());
        navigation.progress.apply(&event);
        match &event {
            NavigationEvent::Started { requested_url, .. } => {
                if is_error_document_url(requested_url) {
                    navigation.showing_error_page = true;
                    return;
                }
                navigation.showing_error_page = false;
                drop(navigation);
                if !self.document_is_internal() {
                    return;
                }
                match browser_resolve_delegate_page(&self.page_path, self.session_id) {
                    Ok(page) => page.notify_page_started(),
                    Err(err) => {
                        lxapp::warn!(
                            "[InternalBrowser] Failed to resolve delegate page for tab {} on start: {}",
                            self.tab_id,
                            err
                        );
                    }
                }
            }
            NavigationEvent::Succeeded { id, final_url } => {
                if is_error_document_url(final_url) {
                    return;
                }
                let is_current = navigation.progress.is_current(*id);
                drop(navigation);
                // Browser-owned document scripts (context menu, …) run in
                // every successfully loaded document — internal and external.
                self.inject_document_scripts();
                let internal = extract_url_scheme(final_url).as_deref() == Some(LINGXIA_SCHEME);
                if !internal {
                    // A commit that changes document kind to external
                    // terminates the internal page binding so bridge
                    // responses cannot target a document that is gone.
                    crate::internal_pages::detach_internal_tab_page(&self.page_path);
                }
                // Persisted history: exactly one visit per success, keyed by
                // the authoritative final URL.
                crate::tabs::notify_navigation_finished(&self.tab_id, final_url);
                if internal && is_current {
                    match browser_resolve_delegate_page(&self.page_path, self.session_id) {
                        Ok(page) => page.handle_loaded(),
                        Err(err) => {
                            lxapp::warn!(
                                "[InternalBrowser] Failed to resolve delegate page for tab {} on finish: {}",
                                self.tab_id,
                                err
                            );
                        }
                    }
                }
            }
            NavigationEvent::Failed { id, error } => {
                let failing_url = error.failing_url.as_deref().unwrap_or_default();
                if is_error_document_url(failing_url) || navigation.showing_error_page {
                    // The error document itself failed to load (matched by URL,
                    // or by arriving while it was pending on a platform that
                    // reports no failing URL); give up rather than looping, and
                    // stop suppressing state updates.
                    navigation.showing_error_page = false;
                    return;
                }
                if !navigation.progress.is_current(*id) {
                    return;
                }
                navigation.showing_error_page = true;
                drop(navigation);
                self.show_load_error(error);
            }
            // Cancellation is control flow (superseded load, intercepted
            // handoff); it never surfaces error UI or touches tab state.
            NavigationEvent::Cancelled { .. } => {}
        }
    }

    fn on_webview_state_change(&self, change: WebViewStateChange) {
        if self
            .navigation
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .showing_error_page
            && matches!(
                &change,
                WebViewStateChange::Location { .. } | WebViewStateChange::Title { .. }
            )
        {
            return;
        }
        match change {
            WebViewStateChange::Location { url } => {
                // Live tab URL for address displays; never a persisted visit.
                let _ = crate::tabs::browser_update_tab_info(&self.tab_id, Some(&url), None);
            }
            WebViewStateChange::Title { title } => {
                // Tab titles persist until the next document reports one, so
                // a generation reset (None) is not mirrored into tab state.
                if let Some(title) = title {
                    let _ = crate::tabs::browser_update_tab_info(&self.tab_id, None, Some(&title));
                }
            }
            WebViewStateChange::Favicon { png_bytes } => {
                // Empty bytes are the tab-state clearing convention; the
                // persisted site favicon cache is deliberately untouched.
                let _ = crate::tabs::browser_update_tab_favicon(
                    &self.tab_id,
                    png_bytes.unwrap_or_default(),
                );
            }
            WebViewStateChange::BackForwardAvailability {
                can_go_back,
                can_go_forward,
            } => {
                let _ = crate::tabs::browser_update_tab_nav_state(
                    &self.tab_id,
                    can_go_back,
                    can_go_forward,
                );
            }
        }
    }

    fn handle_post_message(&self, msg: String) {
        if let Some((level, message)) = decode_console_envelope(&msg) {
            self.log(level, &message);
            return;
        }

        // The page bridge belongs to internal documents; an external document
        // has no nonce and gets no page routing.
        if !self.document_is_internal() {
            lxapp::warn!(
                "[InternalBrowser] Dropping bridge message from external document in tab {}",
                self.tab_id
            );
            return;
        }

        match browser_resolve_delegate_page(&self.page_path, self.session_id) {
            Ok(page) => {
                if let Err(err) = page.handle_incoming_message_json(&msg) {
                    lxapp::warn!(
                        "[InternalBrowser] Failed to handle bridge message for tab {}: {}",
                        self.tab_id,
                        err
                    );
                }
            }
            Err(err) => {
                lxapp::warn!(
                    "[InternalBrowser] Failed to resolve delegate page for tab {}: {}",
                    self.tab_id,
                    err
                );
            }
        }
    }

    fn log(&self, level: LogLevel, message: &str) {
        let log_level = match level {
            LogLevel::Error => LxLogLevel::Error,
            LogLevel::Warn => LxLogLevel::Warn,
            LogLevel::Info => LxLogLevel::Info,
            LogLevel::Debug | LogLevel::Verbose => LxLogLevel::Debug,
        };
        LogBuilder::new(LogTag::BrowserConsole, message)
            .with_level(log_level)
            .with_path(&self.page_path)
            .with_appid(BUILTIN_BROWSER_APPID.to_string());
    }
}

// ---------------------------------------------------------------------------
// WebView helpers — thin wrappers around lingxia-webview cross-platform API
// ---------------------------------------------------------------------------

fn decode_console_envelope(msg: &str) -> Option<(LogLevel, String)> {
    let json = serde_json::from_str::<Value>(msg).ok()?;
    json.get("__lingxia_console__")
        .and_then(Value::as_bool)
        .filter(|enabled| *enabled)?;
    let level = match json.get("level").and_then(Value::as_str) {
        Some("error") => LogLevel::Error,
        Some("warn") => LogLevel::Warn,
        Some("debug") => LogLevel::Debug,
        Some("info") => LogLevel::Info,
        Some("verbose") => LogLevel::Verbose,
        _ => LogLevel::Info,
    };
    let message = json.get("message").and_then(Value::as_str)?.to_string();
    Some((level, message))
}

fn browser_webtag(path: &str, session_id: u64) -> WebTag {
    WebTag::new(BUILTIN_BROWSER_APPID, path, Some(session_id))
}

fn callback_blocks_file_navigation(url_callback: bool, url: &str) -> bool {
    url_callback && extract_url_scheme(url).as_deref() == Some("file")
}

fn callback_policy_blocks_file_navigation(url_callback: &AtomicBool, url: &str) -> bool {
    callback_blocks_file_navigation(url_callback.load(Ordering::Acquire), url)
}

pub(crate) fn browser_create_webview(
    path: &str,
    session_id: u64,
    tab_id: &str,
    create_token: u64,
    data_mode: WebViewDataMode,
    url_callback: Arc<AtomicBool>,
) -> Result<(), LxAppError> {
    let webtag = browser_webtag(path, session_id);
    let browser_owner = ensure_browser_lxapp()?;
    let tab_path_owned = path.to_string();
    let tab_id_owned = tab_id.to_string();

    // Ensure the JS worker and browser startup page exist before creating the tab WebView.
    ensure_browser_startup_page(&browser_owner)?;

    let tab_id_for_lx = tab_id_owned.clone();
    let tab_path_for_lx = tab_path_owned.clone();
    let lingxia_ctx = Arc::new(LingxiaSchemeContext {
        browser: browser_owner.clone(),
        startup_path: browser_owner.initial_route(),
        tab_id: tab_id_owned.clone(),
        tab_path: tab_path_owned.clone(),
        session_id,
    });
    let runtime_for_nav = browser_owner.runtime.clone();
    let owner_appid_for_nav = browser_owner.appid.clone();
    let owner_session_for_nav = browser_owner.session_id();
    let runtime_for_new_window = browser_owner.runtime.clone();
    let owner_appid_for_new_window = browser_owner.appid.clone();
    let owner_session_for_new_window = browser_owner.session_id();
    let tab_id_for_new_window = tab_id_owned.clone();
    let tab_id_for_download = tab_id_owned.clone();
    let owner_for_download = browser_owner.clone();
    let owner_for_file_chooser = browser_owner.clone();
    let url_callback_for_navigation = url_callback.clone();
    let policy_session_for_navigation = Arc::new(std::sync::Mutex::new(
        BrowserNavigationPolicySession::default(),
    ));
    let session = WebViewBuilder::browser(webtag)
        .data_mode(data_mode)
        .delegate(Arc::new(BrowserTabDelegate {
            tab_id: tab_id_owned.clone(),
            page_path: tab_path_owned.clone(),
            session_id,
            navigation: std::sync::Mutex::new(BrowserTabNavigationState::default()),
        }))
        .on_scheme("lx", move |req| {
            let tab_id = tab_id_for_lx.clone();
            let tab_path = tab_path_for_lx.clone();
            async move {
                match browser_resolve_delegate_context(&tab_path, session_id) {
                    Ok((owner, page)) => owner.handle_lingxia_request(&page, req).into(),
                    Err(err) => {
                        lxapp::warn!(
                            "[InternalBrowser] Failed to resolve lx:// owner for tab {}: {}",
                            tab_id,
                            err
                        );
                        None.into()
                    }
                }
            }
        })
        .on_scheme(LINGXIA_SCHEME, move |req| {
            let ctx = lingxia_ctx.clone();
            async move { handle_browser_lingxia_scheme(&ctx, req).await.into() }
        })
        .on_navigation(move |request| {
            let url = request.url.as_str();
            if callback_policy_blocks_file_navigation(&url_callback_for_navigation, url) {
                return NavigationPolicy::Cancel;
            }
            // Keep internal lx:// and lingxia:// browser pages inside this WebView.
            if matches!(extract_url_scheme(url).as_deref(), Some("lx" | "lingxia")) {
                return NavigationPolicy::Allow;
            }
            let policy_request = BrowserNavigationPolicyRequest {
                raw_url: url.to_string(),
                has_user_gesture: request.has_user_gesture,
                is_main_frame: request.is_main_frame,
            };
            let evaluation = match policy_session_for_navigation.lock() {
                Ok(mut session) => session.evaluate(policy_request),
                Err(_) => {
                    lxapp::warn!("[InternalBrowser] Navigation policy session poisoned");
                    return NavigationPolicy::Cancel;
                }
            };
            if evaluation.inherited_user_activation {
                lxapp::debug!(
                    "[InternalBrowser] Inherited transient user activation url={}",
                    url
                );
            }
            let decision = evaluation.response;
            match decision.decision {
                BrowserNavigationPolicyDecision::InWebview => NavigationPolicy::Allow,
                BrowserNavigationPolicyDecision::OpenExternal => {
                    let _ = runtime_for_nav.open_url(OpenUrlRequest {
                        owner_appid: owner_appid_for_nav.clone(),
                        owner_session_id: owner_session_for_nav,
                        url: url.to_string(),
                        target: OpenUrlTarget::External,
                    });
                    NavigationPolicy::Cancel
                }
                BrowserNavigationPolicyDecision::Deny => {
                    lxapp::debug!(
                        "[InternalBrowser] Denied navigation url={} reason={}",
                        url,
                        decision.reason.as_deref().unwrap_or("unspecified")
                    );
                    NavigationPolicy::Cancel
                }
            }
        })
        .on_new_window(move |url| {
            if callback_policy_blocks_file_navigation(&url_callback, url) {
                return NewWindowPolicy::Cancel;
            }
            let normalized = normalize_browser_target_url(url);
            // A docked aside browser tab: surface the target as ANOTHER aside
            // tab in the same panel — the same open-in-new-tab behavior as the
            // self browser. Deferred onto the executor so we never re-enter
            // the createWebView delegate, which aborts the process.
            if crate::tabs::is_standalone_tab(&tab_id_for_new_window) {
                let owner_appid = crate::tabs::tab_owner_appid(&tab_id_for_new_window);
                rong::RongExecutor::global().spawn(async move {
                    let Some(owner) = owner_appid.as_deref().and_then(lxapp::try_get) else {
                        return;
                    };
                    static NEW_WINDOW_SEQ: std::sync::atomic::AtomicU64 =
                        std::sync::atomic::AtomicU64::new(1);
                    let request = lxapp::PageSurfaceRequest {
                        id: format!(
                            "surface-aside-newwin-{}",
                            NEW_WINDOW_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                        ),
                        target: lxapp::PageSurfaceTarget::Url(normalized),
                        query: None,
                        kind: lxapp::SurfaceKind::Overlay,
                        width: None,
                        height: None,
                        width_ratio: None,
                        height_ratio: None,
                        position: lxapp::SurfacePosition::Right,
                        role: lxapp::lingxia_surface::Role::Aside,
                        interaction: None,
                    };
                    let _ = owner.open_surface(request);
                });
                return NewWindowPolicy::Cancel;
            }
            // An aside tab in the shared browser (compact) spawns another
            // aside tab; a self tab spawns a self tab.
            let target = if crate::tabs::is_aside_tab(&tab_id_for_new_window) {
                OpenUrlTarget::AsideBrowser
            } else {
                OpenUrlTarget::NewBrowserTab
            };
            let _ = runtime_for_new_window.open_url(OpenUrlRequest {
                owner_appid: owner_appid_for_new_window.clone(),
                owner_session_id: owner_session_for_new_window,
                url: normalized,
                target,
            });
            NewWindowPolicy::Cancel
        })
        .on_download(move |request| {
            let tab_id = tab_id_for_download.clone();
            let owner = owner_for_download.clone();
            rong::RongExecutor::global().spawn(async move {
                browser_download_resource(owner, tab_id, request).await;
            });
        })
        .on_file_chooser(move |request| {
            let owner = owner_for_file_chooser.clone();
            async move { browser_choose_files(owner, request).await }
        })
        .create();

    rong::RongExecutor::global().spawn(async move {
        browser_on_webview_ready(
            tab_path_owned,
            session_id,
            tab_id_owned,
            create_token,
            session,
        )
        .await;
    });
    Ok(())
}

async fn browser_on_webview_ready(
    path: String,
    session_id: u64,
    tab_id: String,
    create_token: u64,
    session: WebViewSession,
) {
    let webview = match session.wait_ready().await {
        Ok(webview) => webview,
        Err(e) => {
            lxapp::warn!(
                "[InternalBrowser] Failed to create webview for tab {}: {}",
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
        }
        TabCreateState::Stale => {
            // A newer create lifecycle already took ownership of this tab id.
            // Destroy the orphaned webview from this old create cycle.
            browser_destroy_webview(&path, session_id);
        }
        TabCreateState::Active { pending_url } => {
            if let Some(url) = pending_url {
                // Internal browser pages (`lingxia://X`) need the startup bridge attached
                // so they can communicate with the JS app service worker.
                let is_browser_internal =
                    extract_url_scheme(&url).as_deref() == Some(LINGXIA_SCHEME);
                if is_browser_internal {
                    if let Err(e) = browser_attach_tab_page(
                        webview.clone(),
                        &path,
                        session_id,
                        &tab_id,
                        Some(url.as_str()),
                    )
                    .await
                    {
                        lxapp::warn!(
                            "[InternalBrowser] Failed to attach startup page for internal tab {}: {}",
                            tab_id,
                            e
                        );
                        browser_clear_pending_if_token_matches(&tab_id, session_id, create_token);
                        let _ = webview.load_url("about:blank");
                    } else {
                        browser_commit_navigation_if_token_matches(
                            &tab_id,
                            session_id,
                            create_token,
                            Some(&url),
                        );
                    }
                } else {
                    // Direct URL load — no bridge handshake needed, just navigate.
                    if let Err(e) = webview.load_url(&url) {
                        lxapp::warn!(
                            "[InternalBrowser] Failed to load URL for tab {}: {}",
                            tab_id,
                            e
                        );
                        browser_clear_pending_if_token_matches(&tab_id, session_id, create_token);
                    } else {
                        browser_commit_navigation_if_token_matches(
                            &tab_id,
                            session_id,
                            create_token,
                            Some(&url),
                        );
                    }
                }
            } else {
                // Startup page: attach WebView to the tab's headless PageInstance, then load with nonce.
                if let Err(e) =
                    browser_attach_tab_page(webview.clone(), &path, session_id, &tab_id, None).await
                {
                    lxapp::warn!(
                        "[InternalBrowser] Failed to load startup page for tab {}: {}",
                        tab_id,
                        e
                    );
                    let _ = webview.load_url("about:blank");
                } else {
                    // Commit the real startup URL: document-kind gating reads
                    // the committed URL, and platforms without URL-change
                    // reporting never backfill it.
                    let startup_url = format!("{}://newtab", LINGXIA_SCHEME);
                    browser_commit_navigation_if_token_matches(
                        &tab_id,
                        session_id,
                        create_token,
                        Some(&startup_url),
                    );
                }
            }
        }
    }
}

pub(crate) fn browser_find_webview(
    path: &str,
    session_id: u64,
) -> Result<Arc<WebView>, LxAppError> {
    let webtag = browser_webtag(path, session_id);
    find_managed_webview(&webtag).ok_or_else(|| {
        LxAppError::ResourceNotFound(format!("browser webview not found: {}", webtag.as_str()))
    })
}

pub(crate) fn browser_load_url(path: &str, session_id: u64, url: &str) -> Result<(), LxAppError> {
    let webview = browser_find_webview(path, session_id)?;
    webview
        .load_url(url)
        .map_err(|e| LxAppError::WebView(e.to_string()))
}

pub(crate) fn browser_destroy_webview(path: &str, session_id: u64) {
    let webtag = browser_webtag(path, session_id);
    // Remove from global registry (triggers platform-specific cleanup on Drop).
    destroy_managed_webview(&webtag);
}

#[cfg(test)]
mod tests {
    use super::{callback_blocks_file_navigation, callback_policy_blocks_file_navigation};
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn callback_tabs_block_file_navigation_only() {
        assert!(callback_blocks_file_navigation(
            true,
            "file:///tmp/auth.html"
        ));
        assert!(callback_blocks_file_navigation(
            true,
            "FILE:///tmp/auth.html"
        ));
        assert!(!callback_blocks_file_navigation(
            true,
            "https://auth.example.com/callback"
        ));
        assert!(!callback_blocks_file_navigation(
            false,
            "file:///tmp/document.html"
        ));
    }

    #[test]
    fn callback_policy_updates_apply_to_an_existing_webview_handler() {
        let policy = AtomicBool::new(false);
        assert!(!callback_policy_blocks_file_navigation(
            &policy,
            "file:///tmp/document.html"
        ));

        policy.store(true, Ordering::Release);
        assert!(callback_policy_blocks_file_navigation(
            &policy,
            "file:///tmp/document.html"
        ));
    }
}
