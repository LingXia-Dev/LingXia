mod bridge_transport;
pub(crate) mod data_store;
mod schemehandler;
mod webview;

use std::sync::{Mutex, OnceLock};

use crate::{UserAgentOverride, WebViewController, WebViewError};

pub(crate) use webview::WebViewInner;
pub(crate) use webview::apply_http_proxy;

pub const BRIDGE_DOWNSTREAM_CSP_SOURCE: &str = bridge_transport::APPLE_BRIDGE_DOWNSTREAM_CSP_SOURCE;
pub const BRIDGE_DOWNSTREAM_URL: &str = bridge_transport::APPLE_BRIDGE_DOWNSTREAM_URL;

static USER_AGENT_OVERRIDE_FOR_NEW_WEBVIEWS: OnceLock<Mutex<Option<String>>> = OnceLock::new();

pub(crate) fn configured_user_agent_override_for_new_webviews() -> Option<String> {
    USER_AGENT_OVERRIDE_FOR_NEW_WEBVIEWS
        .get()
        .and_then(|state| state.lock().ok())
        .and_then(|state| state.clone())
}

/// Configures the full UA inherited by future Apple WebViews and optionally
/// applies it to the current runtime. Runner hosts call this before opening the
/// first page so the initial request already carries the selected identity.
pub fn configure_user_agent_override_for_webviews(
    user_agent: UserAgentOverride,
    apply_existing: bool,
    reload_existing: bool,
) -> Result<(), WebViewError> {
    user_agent.validate()?;
    let configured = match &user_agent {
        UserAgentOverride::Default => None,
        UserAgentOverride::Custom(value) => Some(value.clone()),
    };
    let state = USER_AGENT_OVERRIDE_FOR_NEW_WEBVIEWS.get_or_init(|| Mutex::new(None));
    *state.lock().map_err(|_| {
        WebViewError::WebView("Apple user-agent configuration is poisoned".into())
    })? = configured;

    if !apply_existing {
        return Ok(());
    }
    let mut failures = Vec::new();
    for webtag in crate::webview::list_webviews() {
        let Some(webview) = crate::webview::find_webview(&webtag) else {
            continue;
        };
        if let Err(err) = webview.set_user_agent_override(user_agent.clone()) {
            failures.push(format!("{}: {err}", webtag.key()));
            continue;
        }
        if reload_existing && let Err(err) = webview.reload() {
            failures.push(format!("{}: {err}", webtag.key()));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(WebViewError::WebView(failures.join("; ")))
    }
}

#[cfg(target_os = "macos")]
pub fn toggle_webview_devtools_by_swift_ptr(swift_ptr: usize, detached: bool) -> bool {
    webview::toggle_devtools_by_swift_ptr(swift_ptr, detached)
}
