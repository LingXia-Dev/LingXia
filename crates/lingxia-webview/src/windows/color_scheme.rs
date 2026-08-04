//! Preferred color scheme pinning used by desktop device-preview hosts.
//!
//! Applied at the WebView2 profile level so pages observe the pinned scheme
//! through `prefers-color-scheme` — never through page DOM, which stays the
//! app's own override channel.

use super::*;
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

/// Color scheme served to WebView2 pages via `prefers-color-scheme`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsPreferredColorScheme {
    /// Follow the host OS appearance.
    Auto,
    Light,
    Dark,
}

static CONFIGURED_SCHEME: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static LXAPP_SCHEMES: OnceLock<RwLock<HashMap<String, WindowsPreferredColorScheme>>> =
    OnceLock::new();

pub(crate) fn configured_color_scheme() -> Option<WindowsPreferredColorScheme> {
    match CONFIGURED_SCHEME.load(std::sync::atomic::Ordering::Acquire) {
        1 => Some(WindowsPreferredColorScheme::Auto),
        2 => Some(WindowsPreferredColorScheme::Light),
        3 => Some(WindowsPreferredColorScheme::Dark),
        _ => None,
    }
}

/// Configures the scheme inherited by WebViews created afterwards.
pub fn set_windows_preferred_color_scheme_for_new_webviews(scheme: WindowsPreferredColorScheme) {
    CONFIGURED_SCHEME.store(scheme as u8 + 1, std::sync::atomic::Ordering::Release);
}

/// Configure the scheme for one lxapp without changing browser or shell
/// WebViews. The registry is also consulted when later page WebViews start.
pub fn set_windows_lxapp_preferred_color_scheme(appid: &str, scheme: WindowsPreferredColorScheme) {
    if let Ok(mut schemes) = LXAPP_SCHEMES
        .get_or_init(|| RwLock::new(HashMap::new()))
        .write()
    {
        schemes.insert(appid.to_string(), scheme);
    }
}

/// Remove a closed lxapp's retained scheme so a future install/session starts cleanly.
pub fn clear_windows_lxapp_preferred_color_scheme(appid: &str) {
    if let Some(schemes) = LXAPP_SCHEMES.get()
        && let Ok(mut schemes) = schemes.write()
    {
        schemes.remove(appid);
    }
}

pub(crate) fn lxapp_color_scheme(webtag: &WebTag) -> Option<WindowsPreferredColorScheme> {
    LXAPP_SCHEMES
        .get()
        .and_then(|schemes| schemes.read().ok())
        .and_then(|schemes| schemes.get(&webtag.extract_appid()).copied())
}

pub(crate) fn apply_color_scheme(
    webview: &ICoreWebView2,
    scheme: WindowsPreferredColorScheme,
) -> StdResult<()> {
    let webview13: ICoreWebView2_13 = webview
        .cast()
        .map_err(|err| WebViewError::WebView(format!("WebView profile cast failed: {err}")))?;
    let profile = unsafe {
        webview13
            .Profile()
            .map_err(|err| WebViewError::WebView(format!("Profile failed: {err}")))?
    };
    let value = match scheme {
        WindowsPreferredColorScheme::Auto => COREWEBVIEW2_PREFERRED_COLOR_SCHEME_AUTO,
        WindowsPreferredColorScheme::Light => COREWEBVIEW2_PREFERRED_COLOR_SCHEME_LIGHT,
        WindowsPreferredColorScheme::Dark => COREWEBVIEW2_PREFERRED_COLOR_SCHEME_DARK,
    };
    unsafe {
        profile
            .SetPreferredColorScheme(value)
            .map_err(|err| WebViewError::WebView(format!("SetPreferredColorScheme failed: {err}")))
    }
}
