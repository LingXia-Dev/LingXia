//! WebView lifecycle for browser tabs: creation, the per-tab delegate, the
//! ready/replay flow, and find/load/destroy helpers.

use crate::BUILTIN_BROWSER_APPID;
use crate::chooser::browser_choose_files;
use crate::downloads::browser_download_resource;
use crate::policy::{
    BrowserNavigationPolicySession, extract_url_scheme, normalize_browser_target_url,
};
use crate::tabs::{
    TabCreateState, browser_clear_pending_if_token_matches,
    browser_commit_navigation_if_token_matches, browser_create_token_for_path,
    browser_finish_webview_create, browser_pending_url_replaced,
    browser_remove_tab_if_token_matches, browser_tab_create_state, browser_tab_generation_matches,
    ensure_browser_lxapp,
};
use crate::types::{BrowserNavigationPolicyDecision, BrowserNavigationPolicyRequest};
use lingxia_log::{LogBuilder, LogLevel as LxLogLevel, LogTag};
use lingxia_platform::traits::app_runtime::{AppRuntime, OpenUrlRequest, OpenUrlTarget};
use lingxia_webview::runtime::{
    destroy_webview_if_matches as destroy_managed_webview_if_matches,
    find_webview as find_managed_webview,
};
use lingxia_webview::{
    IncomingWebMessage, LoadDataRequest, LoadError, LoadErrorPage, LogLevel, NavigationEvent,
    NavigationPolicy, NavigationProgress, NewWindowPolicy, UserAgentOverride, WebTag, WebView,
    WebViewBuilder, WebViewController, WebViewDataMode, WebViewDelegate, WebViewSession,
    WebViewStateChange, render_load_error_page,
};
use lxapp::LxAppError;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

// ---------------------------------------------------------------------------
// Browser tab WebView delegate
// ---------------------------------------------------------------------------

/// WebView delegate for browser tab WebViews. Tab documents are web content:
/// they update browser tab state and have no LingXia bridge.
struct BrowserTabDelegate {
    tab_id: String,
    page_path: String,
    session_id: u64,
    create_token: u64,
    data_mode: WebViewDataMode,
    url_callback: Arc<AtomicBool>,
    navigation: std::sync::Mutex<BrowserTabNavigationState>,
}

#[derive(Default)]
struct BrowserTabNavigationState {
    /// Canonical attempt-correlation fold over typed navigation events.
    progress: NavigationProgress,
    /// True from a document load failure until the next real navigation
    /// starts. While set, Location/Title changes are suppressed so the tab
    /// keeps showing the failing URL, and the error document never records a
    /// visit.
    showing_error_page: bool,
    /// The error document was handed to the WebView and its own load has
    /// not started yet.
    error_document_pending: bool,
}

/// The error document's URL. Engines report it differently, so the
/// document is told apart by delegate state, not by this URL.
const BROWSER_LOAD_ERROR_URL: &str = "about:blank";

impl BrowserTabDelegate {
    fn records_browser_history(&self) -> bool {
        // Read live: a docked tab promoted into the main browser records
        // history from then on.
        self.data_mode != WebViewDataMode::Ephemeral
            && !self.url_callback.load(Ordering::Acquire)
            && !crate::tabs::is_standalone_tab(&self.tab_id)
    }

    fn show_load_error(&self, error: &LoadError) {
        let failing_url = error.failing_url.as_deref().unwrap_or_default();
        let title =
            lingxia_platform::i18n::text("webview.load_error_title", "Couldn't load this page");
        let _ = crate::tabs::browser_update_tab_info_if_token_matches(
            &self.tab_id,
            self.session_id,
            self.create_token,
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
                navigation.error_document_pending = false;
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
            NavigationEvent::Started { .. } => {
                // The first load after the error document was handed over is
                // that document; any later navigation ends the error state.
                navigation.showing_error_page =
                    std::mem::take(&mut navigation.error_document_pending);
            }
            NavigationEvent::Succeeded { final_url, .. } => {
                let is_error_document = navigation.showing_error_page;
                navigation.error_document_pending = false;
                drop(navigation);
                if is_error_document {
                    return;
                }
                // Persisted history: exactly one visit per success, keyed by
                // the authoritative final URL.
                if self.records_browser_history() {
                    crate::tabs::notify_navigation_finished(
                        &self.tab_id,
                        self.session_id,
                        self.create_token,
                        final_url,
                    );
                }
            }
            NavigationEvent::Failed { id, error } => {
                if navigation.showing_error_page {
                    // The error document itself failed to load; give up rather
                    // than looping, and stop suppressing state updates.
                    navigation.showing_error_page = false;
                    navigation.error_document_pending = false;
                    return;
                }
                if !navigation.progress.is_current(*id) {
                    return;
                }
                navigation.showing_error_page = true;
                navigation.error_document_pending = true;
                drop(navigation);
                self.show_load_error(error);
            }
            // Cancellation is control flow (superseded load, intercepted
            // handoff); it never surfaces error UI.
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
                let _ = crate::tabs::browser_update_tab_info_if_token_matches(
                    &self.tab_id,
                    self.session_id,
                    self.create_token,
                    Some(&url),
                    None,
                );
            }
            WebViewStateChange::Title { title } => {
                // Tab titles persist until the next document reports one, so
                // a generation reset (None) is not mirrored into tab state.
                if let Some(title) = title {
                    let _ = crate::tabs::browser_update_tab_info_if_token_matches(
                        &self.tab_id,
                        self.session_id,
                        self.create_token,
                        None,
                        Some(&title),
                    );
                }
            }
            WebViewStateChange::Favicon { png_bytes } => {
                if !crate::tabs::browser_tab_generation_matches(
                    &self.tab_id,
                    self.session_id,
                    self.create_token,
                ) {
                    return;
                }
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

    // Tab documents are web content with no LingXia bridge to receive.
    fn handle_post_message(&self, _message: IncomingWebMessage) {}

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

pub(crate) fn browser_webtag(path: &str, session_id: u64, create_token: u64) -> WebTag {
    #[cfg(all(target_os = "linux", target_env = "ohos"))]
    return browser_instance_webtag(path, session_id, create_token);
    #[cfg(not(all(target_os = "linux", target_env = "ohos")))]
    {
        let _ = create_token;
        WebTag::new(BUILTIN_BROWSER_APPID, path, Some(session_id))
    }
}

#[cfg(any(test, all(target_os = "linux", target_env = "ohos")))]
fn browser_instance_webtag(path: &str, session_id: u64, create_token: u64) -> WebTag {
    // ArkWeb's native controller registry is keyed by tag. A recreated tab
    // must not reuse the retiring controller's key, even in the same session.
    WebTag::new(
        BUILTIN_BROWSER_APPID,
        &format!("{path}#browser-{create_token}"),
        Some(session_id),
    )
}

fn callback_blocks_file_navigation(url_callback: bool, url: &str) -> bool {
    url_callback && extract_url_scheme(url).as_deref() == Some("file")
}

fn callback_policy_blocks_file_navigation(url_callback: &AtomicBool, url: &str) -> bool {
    callback_blocks_file_navigation(url_callback.load(Ordering::Acquire), url)
}

/// A URL-callback surface is the interactive login sheet. `target=_blank` /
/// `window.open` must leave that sheet in place and hand the URL to the OS
/// browser — not replace the authorize page, and not spawn an in-app tab.
fn url_callback_opens_new_window_externally(url_callback: bool) -> bool {
    url_callback
}

pub(crate) fn browser_create_webview(
    path: &str,
    session_id: u64,
    tab_id: &str,
    create_token: u64,
    data_mode: WebViewDataMode,
    url_callback: Arc<AtomicBool>,
) -> Result<(), LxAppError> {
    let webtag = browser_webtag(path, session_id, create_token);
    let browser_owner = ensure_browser_lxapp()?;
    let tab_path_owned = path.to_string();
    let tab_id_owned = tab_id.to_string();

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
    let delegate = Arc::new(BrowserTabDelegate {
        tab_id: tab_id_owned.clone(),
        page_path: tab_path_owned,
        session_id,
        create_token,
        data_mode,
        url_callback: url_callback.clone(),
        navigation: std::sync::Mutex::new(BrowserTabNavigationState::default()),
    });
    let session = WebViewBuilder::browser_tab(webtag)
        .data_mode(data_mode)
        .delegate(delegate)
        .on_navigation(move |request| {
            let url = request.url.as_str();
            if callback_policy_blocks_file_navigation(&url_callback_for_navigation, url) {
                return NavigationPolicy::Cancel;
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
                        want_tab_id: false,
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
            if url_callback_opens_new_window_externally(url_callback.load(Ordering::Acquire)) {
                let _ = runtime_for_new_window.open_url(OpenUrlRequest {
                    owner_appid: owner_appid_for_new_window.clone(),
                    owner_session_id: owner_session_for_new_window,
                    url: normalized,
                    target: OpenUrlTarget::External,
                    want_tab_id: false,
                });
                return NewWindowPolicy::Cancel;
            }
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
                    let request = lxapp::PageSurfaceRequest::new(
                        format!(
                            "surface-aside-newwin-{}",
                            NEW_WINDOW_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                        ),
                        lxapp::PageSurfaceTarget::Url(normalized),
                    )
                    .position(lxapp::SurfacePosition::Right)
                    .role(lxapp::lingxia_surface::Role::Aside);
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
                want_tab_id: false,
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
        browser_on_webview_ready(session_id, tab_id_owned, create_token, session).await;
    });
    Ok(())
}

async fn browser_on_webview_ready(
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
            browser_destroy_webview_if_matches(&webview);
        }
        TabCreateState::Stale => {
            // A newer create lifecycle already took ownership of this tab id.
            // Destroy the orphaned webview from this old create cycle.
            browser_destroy_webview_if_matches(&webview);
        }
        TabCreateState::Active {
            pending_url,
            user_agent_override,
        } => {
            if let Some(user_agent) = user_agent_override
                && let Err(e) =
                    webview.set_user_agent_override(UserAgentOverride::Custom(user_agent))
            {
                lxapp::warn!(
                    "[InternalBrowser] Failed to restore user agent for tab {}: {}",
                    tab_id,
                    e
                );
            }
            // Replay a target that arrived while this load was still running.
            // Same-URL opens are not a second load: commit already dropped them.
            // A tab opened without a target starts blank.
            let mut pending_url = pending_url.or_else(|| Some("about:blank".to_string()));
            while let Some(url) = pending_url {
                if let Err(e) = webview.load_url(&url) {
                    lxapp::warn!(
                        "[InternalBrowser] Failed to load URL for tab {}: {}",
                        tab_id,
                        e
                    );
                    browser_clear_pending_if_token_matches(&tab_id, session_id, create_token);
                    break;
                }
                browser_commit_navigation_if_token_matches(
                    &tab_id,
                    session_id,
                    create_token,
                    Some(&url),
                );
                pending_url = browser_pending_url_replaced(&tab_id, session_id, create_token, &url);
            }
        }
    }
    // Includes a stale callback: its token no longer matches, so this does
    // not clear the replacement create that now owns the tab.
    browser_finish_webview_create(&tab_id, session_id, create_token);
}

pub(crate) fn browser_find_webview(
    path: &str,
    session_id: u64,
) -> Result<Arc<WebView>, LxAppError> {
    let create_token = browser_create_token_for_path(path, session_id)
        .ok_or_else(|| LxAppError::ResourceNotFound(format!("browser tab not found: {path}")))?;
    let webtag = browser_webtag(path, session_id, create_token);
    find_managed_webview(&webtag).ok_or_else(|| {
        LxAppError::ResourceNotFound(format!("browser webview not found: {}", webtag.as_str()))
    })
}

pub(crate) fn browser_load_url(path: &str, session_id: u64, url: &str) -> Result<(), LxAppError> {
    browser_find_webview(path, session_id)?
        .load_url(url)
        .map_err(LxAppError::from)
}

pub(crate) fn browser_reload_document(
    tab_id: &str,
    path: &str,
    session_id: u64,
    create_token: u64,
) -> Result<(), LxAppError> {
    if !browser_tab_generation_matches(tab_id, session_id, create_token) {
        return Err(LxAppError::ResourceNotFound(format!(
            "browser tab generation is stale: {tab_id}"
        )));
    }
    browser_find_webview(path, session_id)?
        .reload()
        .map_err(LxAppError::from)
}

pub(crate) fn browser_destroy_webview_if_matches(expected: &Arc<WebView>) -> bool {
    destroy_managed_webview_if_matches(&expected.webtag(), expected)
}

#[cfg(test)]
mod tests {
    use super::{
        BrowserTabDelegate, BrowserTabNavigationState, callback_blocks_file_navigation,
        callback_policy_blocks_file_navigation, url_callback_opens_new_window_externally,
    };
    use crate::tabs;
    use lingxia_webview::{
        LoadError, LoadErrorKind, NavigationEvent, NavigationId, WebViewDataMode, WebViewDelegate,
    };
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    #[test]
    fn promoted_docked_tab_records_history() {
        let tab_id = "promoted-history-tab".to_string();
        tabs::lock_state().tabs.insert(
            tab_id.clone(),
            tabs::BrowserTabState {
                session_id: 7,
                created_order: 1,
                create_token: 1,
                create_in_flight: false,
                pending_url: None,
                initial_url: None,
                current_url: None,
                title: None,
                title_url: None,
                favicon_png: None,
                can_go_back: false,
                can_go_forward: false,
                discarded: false,
                data_mode: WebViewDataMode::ProfileDefault,
                url_callback: Arc::new(AtomicBool::new(false)),
                standalone: true,
                aside: false,
                owner_appid: None,
                owner_session_id: None,
            },
        );
        let delegate = BrowserTabDelegate {
            tab_id: tab_id.clone(),
            page_path: "/tabs/promoted".to_string(),
            session_id: 7,
            create_token: 1,
            data_mode: WebViewDataMode::ProfileDefault,
            url_callback: Arc::new(AtomicBool::new(false)),
            navigation: Mutex::new(BrowserTabNavigationState::default()),
        };
        assert!(!delegate.records_browser_history());
        tabs::promote_standalone_tab(&tab_id).unwrap();
        assert!(delegate.records_browser_history());
        tabs::lock_state().tabs.remove(&tab_id);
        tabs::clear_active_browser_tab();
    }

    fn error_test_delegate(tab_id: &str) -> BrowserTabDelegate {
        BrowserTabDelegate {
            tab_id: tab_id.to_string(),
            page_path: format!("/tabs/{tab_id}"),
            session_id: 9,
            create_token: 1,
            data_mode: WebViewDataMode::ProfileDefault,
            url_callback: Arc::new(AtomicBool::new(false)),
            navigation: Mutex::new(BrowserTabNavigationState::default()),
        }
    }

    fn started(delegate: &BrowserTabDelegate, raw: u64, url: &str) {
        delegate.on_navigation_event(NavigationEvent::Started {
            id: NavigationId::from_raw(raw),
            requested_url: url.to_string(),
        });
    }

    fn failed(delegate: &BrowserTabDelegate, raw: u64, url: &str) {
        delegate.on_navigation_event(NavigationEvent::Failed {
            id: NavigationId::from_raw(raw),
            error: LoadError {
                failing_url: Some(url.to_string()),
                kind: LoadErrorKind::Network,
                description: "offline".to_string(),
            },
        });
    }

    fn showing_error(delegate: &BrowserTabDelegate) -> bool {
        delegate.navigation.lock().unwrap().showing_error_page
    }

    #[test]
    fn the_error_document_is_the_load_after_a_failure_and_the_next_navigation_ends_it() {
        let delegate = error_test_delegate("error-document-state");
        started(&delegate, 1, "https://down.example");
        failed(&delegate, 1, "https://down.example");
        assert!(showing_error(&delegate));
        started(&delegate, 2, "about:blank");
        delegate.on_navigation_event(NavigationEvent::Succeeded {
            id: NavigationId::from_raw(2),
            final_url: "about:blank".to_string(),
        });
        assert!(showing_error(&delegate));
        started(&delegate, 3, "about:blank");
        assert!(
            !showing_error(&delegate),
            "a later blank load is the user's"
        );
    }

    #[test]
    fn a_failing_error_document_gives_up_instead_of_looping() {
        let delegate = error_test_delegate("error-document-fails");
        started(&delegate, 1, "https://down.example");
        failed(&delegate, 1, "https://down.example");
        started(&delegate, 2, "about:blank");
        failed(&delegate, 2, "about:blank");
        assert!(!showing_error(&delegate));
        started(&delegate, 3, "https://up.example");
        assert!(!showing_error(&delegate));
    }

    #[test]
    fn recreated_tab_has_a_distinct_native_tag_with_the_same_route_and_session() {
        let first = super::browser_instance_webtag("/tabs/newtab", 2, 10);
        let next = super::browser_instance_webtag("/tabs/newtab", 2, 11);
        assert_ne!(first.as_str(), next.as_str());
        assert_eq!(first.extract_parts(), next.extract_parts());
        assert_eq!(next.extract_parts().1, "/tabs/newtab");
        assert_eq!(next.session_id(), Some(2));
    }

    #[test]
    fn login_callback_tabs_open_new_windows_in_the_system_browser() {
        assert!(url_callback_opens_new_window_externally(true));
        assert!(!url_callback_opens_new_window_externally(false));
    }

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
