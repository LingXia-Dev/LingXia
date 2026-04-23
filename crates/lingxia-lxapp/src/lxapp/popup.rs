use lingxia_platform::traits::app_runtime::{AppRuntime, OpenUrlRequest, OpenUrlTarget};
use lingxia_platform::traits::ui::{PopupPresenter, PopupRequest};
use lingxia_webview::WebViewController;
use lingxia_webview::runtime::destroy_webview;
use lingxia_webview::{NavigationPolicy, NewWindowPolicy, WebTag, WebViewBuilder};
use std::sync::Arc;

use crate::PageLifecycleEvent;
use crate::error::LxAppError;
use crate::lxapp::LxApp;

/// Fixed WebTag path used for the web-page popup WebView.
/// Safe to be a constant because only one popup can be active at a time.
pub(crate) const WEB_POPUP_PATH: &str = "__web_popup__";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExternalWebNavigationDecision {
    InWebview,
    OpenExternal,
    Deny,
}

fn extract_url_scheme(raw: &str) -> Option<String> {
    let (scheme, _) = raw.split_once(':')?;
    if scheme.is_empty() {
        return None;
    }
    let is_valid = scheme
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    if !is_valid {
        return None;
    }
    Some(scheme.to_ascii_lowercase())
}

fn classify_external_web_navigation(raw_url: &str) -> ExternalWebNavigationDecision {
    let trimmed = raw_url.trim();
    if trimmed.is_empty() || trimmed.chars().any(|c| c.is_whitespace()) {
        return ExternalWebNavigationDecision::Deny;
    }

    match extract_url_scheme(trimmed).as_deref() {
        Some("http" | "https" | "lx" | "lingxia") => ExternalWebNavigationDecision::InWebview,
        Some("about" | "data" | "blob" | "javascript" | "file") => {
            ExternalWebNavigationDecision::Deny
        }
        Some(_) => ExternalWebNavigationDecision::OpenExternal,
        None => ExternalWebNavigationDecision::Deny,
    }
}

/// Controls what content is loaded in the popup.
/// Both modes display an in-app popup overlay; they differ only in content source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupMode {
    /// Render an internal lxapp page (`pages/...` or plugin route) — existing behavior.
    LxAppPage,
    /// Load a standard http/https URL in a browser-profile WebView.
    WebPage,
}

/// Tracks what kind of popup is currently active.
#[derive(Debug, Clone)]
pub(crate) enum ActivePopup {
    /// An internal lxapp page popup; holds the resolved page path.
    LxAppPage(String),
    /// A browser-profile WebView popup loading an https URL.
    WebPage,
}

impl LxApp {
    fn show_web_popup_webview(
        self: &Arc<Self>,
        request: PopupRequest,
        target_url: &str,
    ) -> Result<(), LxAppError> {
        let app = self.clone();
        let target_url = target_url.to_string();
        crate::executor::spawn(async move {
            if let Err(err) = app.show_web_popup_webview_async(request, &target_url).await {
                crate::warn!(
                    "[Popup] failed to show external popup url={} err={}",
                    target_url,
                    err
                );
            }
        });
        Ok(())
    }

    async fn show_web_popup_webview_async(
        self: &Arc<Self>,
        request: PopupRequest,
        target_url: &str,
    ) -> Result<(), LxAppError> {
        let owner_appid_for_nav = self.appid.clone();
        let owner_session_for_nav = self.session_id();
        let runtime_for_nav = self.runtime.clone();
        let owner_appid = self.appid.clone();
        let owner_session = self.session_id();

        let webtag = WebTag::new(&self.appid, WEB_POPUP_PATH, Some(self.session_id()));
        let session = WebViewBuilder::browser(webtag)
            .on_navigation(move |url| match classify_external_web_navigation(url) {
                ExternalWebNavigationDecision::InWebview => NavigationPolicy::Allow,
                ExternalWebNavigationDecision::OpenExternal => {
                    let _ = runtime_for_nav.open_url(OpenUrlRequest {
                        owner_appid: owner_appid_for_nav.clone(),
                        owner_session_id: owner_session_for_nav,
                        url: url.to_string(),
                        target: OpenUrlTarget::External,
                    });
                    NavigationPolicy::Cancel
                }
                ExternalWebNavigationDecision::Deny => NavigationPolicy::Cancel,
            })
            .on_new_window(move |url| {
                let _ = url;
                NewWindowPolicy::LoadInSelf
            })
            .create();

        let cleanup = || {
            destroy_webview(&WebTag::new(
                &owner_appid,
                WEB_POPUP_PATH,
                Some(owner_session),
            ));
        };

        let webview = match session.wait_ready().await {
            Ok(webview) => webview,
            Err(wait_err) => {
                cleanup();
                return Err(LxAppError::WebView(format!(
                    "failed to create external popup webview: {}",
                    wait_err
                )));
            }
        };

        if let Err(show_err) = self.runtime.show_popup(request) {
            cleanup();
            return Err(LxAppError::from(show_err));
        }

        if let Ok(mut state) = self.state.lock() {
            state.current_popup = Some(ActivePopup::WebPage);
        }

        if let Err(load_err) = webview.load_url(target_url) {
            let _ = self.runtime.hide_popup(&self.appid);
            if let Ok(mut state) = self.state.lock() {
                state.current_popup = None;
            }
            cleanup();
            return Err(LxAppError::WebView(format!(
                "failed to load external popup url: {}",
                load_err
            )));
        }

        Ok(())
    }

    pub async fn show_popup_with_mode_async(
        self: &Arc<Self>,
        mode: PopupMode,
        mut request: PopupRequest,
    ) -> Result<(), LxAppError> {
        self.hide_popup()?;
        request.app_id = self.appid.clone();

        if !request.width_ratio.is_nan() {
            request.width_ratio = request.width_ratio.clamp(0.0, 1.0);
        }
        if !request.height_ratio.is_nan() {
            request.height_ratio = request.height_ratio.clamp(0.0, 1.0);
        }

        match mode {
            PopupMode::LxAppPage => self.show_popup_with_mode(mode, request),
            PopupMode::WebPage => {
                let target_url = request.path.trim().to_string();
                let scheme = extract_url_scheme(&target_url);
                if !matches!(scheme.as_deref(), Some("http" | "https")) {
                    return Err(LxAppError::InvalidParameter(format!(
                        "WebPage popup only supports http/https URLs, got '{}'",
                        request.path
                    )));
                }

                request.path = WEB_POPUP_PATH.to_string();
                self.show_web_popup_webview_async(request, &target_url)
                    .await
            }
        }
    }

    /// Show a popup with explicit mode control.
    ///
    /// - `LxAppPage`: resolves an lxapp route, creates/reuses its PageInstance, dispatches lifecycle.
    /// - `WebPage`: validates the URL is http/https, creates a browser-profile WebView popup.
    pub fn show_popup_with_mode(
        self: &Arc<Self>,
        mode: PopupMode,
        mut request: PopupRequest,
    ) -> Result<(), LxAppError> {
        self.hide_popup()?;
        request.app_id = self.appid.clone();

        if !request.width_ratio.is_nan() {
            request.width_ratio = request.width_ratio.clamp(0.0, 1.0);
        }
        if !request.height_ratio.is_nan() {
            request.height_ratio = request.height_ratio.clamp(0.0, 1.0);
        }

        let active = match mode {
            PopupMode::LxAppPage => {
                let resolved = crate::route::resolve_route(self, &request.path)?;
                let path = resolved.internal_path();
                let query_str = resolved.query.unwrap_or_default();

                let popup_page = self.get_or_create_page(&path);
                popup_page.mark_active();

                if !query_str.is_empty() {
                    popup_page.set_query(query_str);
                }

                popup_page.dispatch_lifecycle_event(PageLifecycleEvent::OnLoad);
                request.path = path.clone();
                ActivePopup::LxAppPage(path)
            }
            PopupMode::WebPage => {
                let target_url = request.path.trim().to_string();
                let scheme = extract_url_scheme(&target_url);
                if !matches!(scheme.as_deref(), Some("http" | "https")) {
                    return Err(LxAppError::InvalidParameter(format!(
                        "WebPage popup only supports http/https URLs, got '{}'",
                        request.path
                    )));
                }

                request.path = WEB_POPUP_PATH.to_string();
                self.show_web_popup_webview(request, &target_url)?;
                return Ok(());
            }
        };

        self.runtime.show_popup(request).map_err(LxAppError::from)?;
        if let Ok(mut state) = self.state.lock() {
            state.current_popup = Some(active);
        }

        Ok(())
    }

    /// Show popup content rendered via WebView (internal lxapp page).
    pub fn show_popup(self: &Arc<Self>, request: PopupRequest) -> Result<(), LxAppError> {
        self.show_popup_with_mode(PopupMode::LxAppPage, request)
    }

    /// Hide the currently displayed popup, if any.
    pub fn hide_popup(self: &Arc<Self>) -> Result<(), LxAppError> {
        let active = {
            let mut state = self.state.lock().unwrap();
            state.current_popup.take()
        };

        let Some(active) = active else {
            return Ok(());
        };

        if let ActivePopup::LxAppPage(ref path) = active {
            if let Some(page) = self.get_page(path) {
                page.dispatch_lifecycle_event(PageLifecycleEvent::OnHide);
                page.dispatch_lifecycle_event(PageLifecycleEvent::OnUnload);
            }
        }

        self.runtime
            .hide_popup(&self.appid)
            .map_err(LxAppError::from)?;

        if let ActivePopup::WebPage = active {
            destroy_webview(&WebTag::new(
                &self.appid,
                WEB_POPUP_PATH,
                Some(self.session.id),
            ));
        }

        Ok(())
    }
}
