//! WebView lifecycle for browser tabs: creation, the per-tab delegate, the
//! ready/replay flow, and find/load/destroy helpers.

use crate::BUILTIN_BROWSER_APPID;
use crate::chooser::browser_choose_files;
use crate::downloads::browser_download_resource;
use crate::internal_pages::{
    LingxiaSchemeContext, browser_attach_tab_page, browser_resolve_delegate_context,
    browser_resolve_delegate_page, ensure_browser_startup_page, handle_browser_lingxia_scheme,
};
use crate::policy::{
    LINGXIA_SCHEME, extract_url_scheme, handle_browser_navigation_policy,
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
    LogLevel, NavigationPolicy, NewWindowPolicy, WebTag, WebView, WebViewBuilder,
    WebViewController, WebViewDelegate, WebViewSession,
};
use lxapp::LxAppError;
use serde_json::Value;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Browser tab WebView delegate
// ---------------------------------------------------------------------------

/// WebView delegate for browser tab WebViews.
///
/// All tab WebViews share a single headless startup PageInstance (and its PageSvc).
/// This delegate routes postMessage, page-started, and page-finished events
/// from the currently active tab WebView to that shared startup PageInstance.
struct BrowserTabDelegate {
    tab_id: String,
    page_path: String,
    session_id: u64,
}

impl WebViewDelegate for BrowserTabDelegate {
    fn on_page_started(&self) {
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

    fn on_page_finished(&self) {
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

    fn on_title_changed(&self, title: &str) {
        // Mirror the document title into the tab state (fires the
        // tabs-changed observer for shell sidebars). Platforms whose host
        // layer reports titles separately (e.g. macOS KVO) do not call this.
        let _ = crate::tabs::browser_update_tab_info(&self.tab_id, None, Some(title));
    }

    fn on_favicon_changed(&self, png_bytes: Vec<u8>) {
        // Mirror the page favicon into the tab state (fires the tabs-changed
        // observer for shell sidebars); empty bytes clear a stale favicon.
        let _ = crate::tabs::browser_update_tab_favicon(&self.tab_id, png_bytes);
    }

    fn on_history_changed(&self, can_go_back: bool, can_go_forward: bool) {
        let _ =
            crate::tabs::browser_update_tab_nav_state(&self.tab_id, can_go_back, can_go_forward);
    }

    fn on_url_changed(&self, url: &str) {
        // Mirror the live document URL into the tab state so address
        // displays follow history navigations and redirects.
        let _ = crate::tabs::browser_update_tab_info(&self.tab_id, Some(url), None);
    }

    fn handle_post_message(&self, msg: String) {
        if let Some((level, message)) = decode_console_envelope(&msg) {
            self.log(level, &message);
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

pub(crate) fn browser_create_webview(
    path: &str,
    session_id: u64,
    tab_id: &str,
    create_token: u64,
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
    let session = WebViewBuilder::browser(webtag)
        .delegate(Arc::new(BrowserTabDelegate {
            tab_id: tab_id_owned.clone(),
            page_path: tab_path_owned.clone(),
            session_id,
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
        .on_navigation(move |url| {
            // Keep internal lx:// and lingxia:// browser pages inside this WebView.
            if matches!(extract_url_scheme(url).as_deref(), Some("lx" | "lingxia")) {
                return NavigationPolicy::Allow;
            }
            // This callback only provides the URL string; user-gesture/main-frame
            // metadata is unavailable here, so treat it as a no-gesture navigation.
            // http/https stay in-webview regardless, while external-scheme launches
            // are cancelled in-page: they must come through a platform policy path
            // that carries real gesture data (see handle_browser_navigation_policy).
            let decision = handle_browser_navigation_policy(BrowserNavigationPolicyRequest {
                raw_url: url.to_string(),
                has_user_gesture: false,
                is_main_frame: false,
            });
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
                BrowserNavigationPolicyDecision::Deny => NavigationPolicy::Cancel,
            }
        })
        .on_new_window(move |url| {
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
                // Startup page: attach WebView to shared startup PageInstance, then load with nonce.
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
                    browser_commit_navigation_if_token_matches(
                        &tab_id,
                        session_id,
                        create_token,
                        None,
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
