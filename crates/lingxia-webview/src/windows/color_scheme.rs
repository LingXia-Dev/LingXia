//! Preferred color scheme pinning used by desktop device-preview hosts.
//!
//! Applied at the WebView2 profile level so pages observe the pinned scheme
//! through `prefers-color-scheme` — never through page DOM, which stays the
//! app's own override channel.

use super::*;

/// Color scheme served to WebView2 pages via `prefers-color-scheme`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsPreferredColorScheme {
    /// Follow the host OS appearance.
    Auto,
    Light,
    Dark,
}

static CONFIGURED_SCHEME: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

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
