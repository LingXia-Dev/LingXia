mod bridge_transport;
pub(crate) mod data_store;
mod schemehandler;
#[cfg(target_os = "macos")]
mod session;
mod webview;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::{UserAgentOverride, WebViewController, WebViewError};

#[cfg(target_os = "macos")]
pub use session::screen_locked;
pub(crate) use webview::WebViewInner;
pub(crate) use webview::{apply_http_proxy, run_on_main_blocking};

pub const BRIDGE_DOWNSTREAM_CSP_SOURCE: &str = bridge_transport::APPLE_BRIDGE_DOWNSTREAM_CSP_SOURCE;
pub const BRIDGE_DOWNSTREAM_URL: &str = bridge_transport::APPLE_BRIDGE_DOWNSTREAM_URL;

static KEEP_RESPONSIVE_FOR_DEVELOPMENT: AtomicBool = AtomicBool::new(false);

/// Development hosts (the Runner, an app in a `lingxia dev` session): keep
/// pages scheduled while their window is covered or the app is in the
/// background, where specs drive it from a terminal or editor.
///
/// Public API only: WebViews created afterwards get
/// `WKPreferences.inactiveSchedulingPolicy = .none` (macOS 14 / iOS 17 and
/// later), and the process holds a user-initiated, latency-critical
/// `NSProcessInfo` activity so App Nap does not throttle its timers. It still
/// allows idle system sleep. WebKit keeps pausing `requestAnimationFrame` for
/// a page in a fully covered window; nothing public changes that.
pub fn keep_responsive_for_development() {
    if KEEP_RESPONSIVE_FOR_DEVELOPMENT.swap(true, Ordering::AcqRel) {
        return;
    }
    webview::begin_development_activity();
}

/// Wakes the display and keeps it awake while it lives. An automation run
/// holds one: WebKit stops animation frames for a page whose display is
/// asleep, so rAF- and transition-driven UI would never move under a spec.
#[cfg(target_os = "macos")]
pub struct DisplayAwake {
    _activity: webview::DisplayAwakeActivity,
    _user_activity: Option<session::UserActivity>,
}

/// Public API only: a user-activity assertion (`IOPMAssertionDeclareUserActivity`)
/// wakes a display that is already asleep -- the display-sleep activity alone
/// only keeps an awake one awake -- and an `NSProcessInfo` activity with
/// `NSActivityIdleDisplaySleepDisabled` keeps it on; both end when the guard
/// drops. Neither unlocks a locked screen, and neither helps a page in a
/// fully covered window: WebKit pauses its animation frames and nothing
/// public changes that.
#[cfg(target_os = "macos")]
pub fn keep_display_awake(reason: &str) -> Option<DisplayAwake> {
    let user_activity = session::declare_user_activity(reason);
    webview::begin_display_awake_activity(reason).map(|activity| DisplayAwake {
        _activity: activity,
        _user_activity: user_activity,
    })
}

pub(crate) fn keeps_responsive_for_development() -> bool {
    KEEP_RESPONSIVE_FOR_DEVELOPMENT.load(Ordering::Acquire)
}

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
