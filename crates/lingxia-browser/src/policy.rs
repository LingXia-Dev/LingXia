//! Navigation policy classification, URL scheme extraction, and URL normalizers.

use crate::types::{
    BrowserNavigationPolicyDecision, BrowserNavigationPolicyRequest,
    BrowserNavigationPolicyResponse,
};

pub(crate) const LINGXIA_SCHEME: &str = "lingxia";
// `file` loads in-webview so the user can open local files from the address bar
// (the WKWebView still gates actual reads to the granted directory).
const BROWSER_IN_WEBVIEW_SCHEMES: &[&str] = &["http", "https", "lx", "lingxia", "file"];
const BROWSER_NON_EXTERNAL_SCHEMES: &[&str] = &["about", "data", "blob", "javascript"];

/// Extract the (lowercased) scheme from a URL-like string, or `None` if the
/// text before the first `:` is not a valid scheme.
pub fn extract_url_scheme(raw: &str) -> Option<String> {
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

/// Whether a `lingxia://` URL maps to the startup/newtab page or another internal browser page.
///
/// - `lingxia://newtab` (or bare `lingxia://`) → `Some(true)`
/// - Registered `lingxia://<route>` values resolve via the browser internal-page registry.
///
/// Returns `None` if `url` is not a `lingxia://` URL.
pub fn is_lingxia_startup_url(url: &str) -> Option<bool> {
    if extract_url_scheme(url).as_deref() != Some(LINGXIA_SCHEME) {
        return None;
    }
    let host = url
        .split_once("://")
        .map(|x| x.1)
        .unwrap_or("")
        .split('/')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    Some(host.is_empty() || host == "newtab")
}

pub(crate) fn normalize_browser_target_url(raw: &str) -> String {
    let trimmed = raw.trim();
    // Byte-wise prefix check: page-supplied URLs may contain multi-byte UTF-8
    // at any position, so slicing by char count would panic.
    if trimmed
        .as_bytes()
        .get(..7)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"http://"))
    {
        // The first 7 bytes are ASCII, so byte offset 7 is a char boundary.
        format!("https://{}", &trimmed[7..])
    } else {
        trimmed.to_string()
    }
}

pub(crate) fn normalize_url_for_wait_compare(raw: &str) -> String {
    let normalized = normalize_browser_target_url(raw);
    let trimmed = normalized.trim();
    let Ok(uri) = trimmed.parse::<http::Uri>() else {
        return trimmed.to_string();
    };
    let Some(scheme) = uri.scheme_str().map(str::to_ascii_lowercase) else {
        return trimmed.to_string();
    };
    if !matches!(scheme.as_str(), "http" | "https") {
        return trimmed.to_string();
    }
    let Some(host) = uri.host() else {
        return trimmed.to_string();
    };
    let host = host.to_ascii_lowercase();
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host
    };
    let port = uri
        .port()
        .map(|port| format!(":{}", port.as_str()))
        .unwrap_or_default();
    let path_and_query = uri
        .path_and_query()
        .map(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("/");
    format!("{scheme}://{host}{port}{path_and_query}")
}

fn scheme_in_list(scheme: &str, candidates: &[&str]) -> bool {
    candidates
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(scheme))
}

fn browser_policy_response(
    decision: BrowserNavigationPolicyDecision,
    reason: Option<&str>,
) -> BrowserNavigationPolicyResponse {
    BrowserNavigationPolicyResponse {
        decision,
        reason: reason.map(str::to_string),
    }
}

/// Classify browser navigation requests into:
/// - `in_webview`: keep loading in current webview.
/// - `open_external`: cancel in-webview load and open externally.
/// - `deny`: cancel navigation.
///
/// Security model:
/// - `http/https/lx` stay in webview.
/// - Potential external schemes require user gesture + main-frame navigation.
/// - Non-external internal schemes (`javascript:`, `data:`, etc.) are denied.
pub(crate) fn handle_browser_navigation_policy(
    request: BrowserNavigationPolicyRequest,
) -> BrowserNavigationPolicyResponse {
    let trimmed = request.raw_url.trim();
    if trimmed.is_empty() {
        return browser_policy_response(BrowserNavigationPolicyDecision::Deny, Some("empty"));
    }

    if trimmed.chars().any(|c| c.is_whitespace()) {
        return browser_policy_response(
            BrowserNavigationPolicyDecision::Deny,
            Some("whitespace_url"),
        );
    }

    let Some(scheme) = extract_url_scheme(trimmed) else {
        return browser_policy_response(
            BrowserNavigationPolicyDecision::Deny,
            Some("missing_scheme"),
        );
    };

    if scheme_in_list(&scheme, BROWSER_IN_WEBVIEW_SCHEMES) {
        return browser_policy_response(BrowserNavigationPolicyDecision::InWebview, None);
    }

    if scheme_in_list(&scheme, BROWSER_NON_EXTERNAL_SCHEMES) {
        return browser_policy_response(
            BrowserNavigationPolicyDecision::Deny,
            Some("non_external_scheme"),
        );
    }

    if !request.is_main_frame {
        return browser_policy_response(
            BrowserNavigationPolicyDecision::Deny,
            Some("non_main_frame_external"),
        );
    }

    if !request.has_user_gesture {
        return browser_policy_response(
            BrowserNavigationPolicyDecision::Deny,
            Some("gesture_required"),
        );
    }

    browser_policy_response(BrowserNavigationPolicyDecision::OpenExternal, None)
}

pub(crate) fn handle_browser_navigation_policy_json(request_json: &str) -> Option<String> {
    let request: BrowserNavigationPolicyRequest = serde_json::from_str(request_json).ok()?;
    serde_json::to_string(&handle_browser_navigation_policy(request)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_browser_target_url_upgrades_http_case_insensitively() {
        assert_eq!(
            normalize_browser_target_url("  HTTP://Example.com/path?q=1 "),
            "https://Example.com/path?q=1"
        );
        assert_eq!(
            normalize_browser_target_url("http://example.com"),
            "https://example.com"
        );
        assert_eq!(
            normalize_browser_target_url("https://example.com"),
            "https://example.com"
        );
    }

    #[test]
    fn normalize_browser_target_url_handles_multibyte_input() {
        // Multi-byte UTF-8 within the first bytes must not panic.
        assert_eq!(normalize_browser_target_url("http🌐//x"), "http🌐//x");
        assert_eq!(normalize_browser_target_url("中文网址"), "中文网址");
        assert_eq!(
            normalize_browser_target_url("http://例え.jp/路径"),
            "https://例え.jp/路径"
        );
    }

    #[test]
    fn normalize_url_for_wait_compare_canonicalizes_browser_urls() {
        assert_eq!(
            normalize_url_for_wait_compare("https://Example.com"),
            "https://example.com/"
        );
        assert_eq!(
            normalize_url_for_wait_compare("http://example.com"),
            "https://example.com/"
        );
        assert_eq!(
            normalize_url_for_wait_compare("https://[::1]:8443/path?q=1"),
            "https://[::1]:8443/path?q=1"
        );
    }

    #[test]
    fn browser_nav_policy_allows_lark_with_gesture() {
        let response = handle_browser_navigation_policy(BrowserNavigationPolicyRequest {
            raw_url: "lark://client/auth?code=1".to_string(),
            has_user_gesture: true,
            is_main_frame: true,
        });

        assert_eq!(
            response.decision,
            BrowserNavigationPolicyDecision::OpenExternal
        );
    }

    #[test]
    fn browser_nav_policy_denies_lark_without_gesture() {
        let response = handle_browser_navigation_policy(BrowserNavigationPolicyRequest {
            raw_url: "lark://client/auth?code=1".to_string(),
            has_user_gesture: false,
            is_main_frame: true,
        });

        assert_eq!(response.decision, BrowserNavigationPolicyDecision::Deny);
        assert_eq!(response.reason.as_deref(), Some("gesture_required"));
    }

    #[test]
    fn browser_nav_policy_allows_unknown_custom_scheme_with_gesture() {
        let response = handle_browser_navigation_policy(BrowserNavigationPolicyRequest {
            raw_url: "customxyz://hello".to_string(),
            has_user_gesture: true,
            is_main_frame: true,
        });

        assert_eq!(
            response.decision,
            BrowserNavigationPolicyDecision::OpenExternal
        );
    }

    #[test]
    fn browser_nav_policy_denies_non_external_scheme() {
        let response = handle_browser_navigation_policy(BrowserNavigationPolicyRequest {
            raw_url: "javascript:alert(1)".to_string(),
            has_user_gesture: true,
            is_main_frame: true,
        });

        assert_eq!(response.decision, BrowserNavigationPolicyDecision::Deny);
        assert_eq!(response.reason.as_deref(), Some("non_external_scheme"));
    }

    #[test]
    fn browser_nav_policy_denies_external_in_subframe() {
        let response = handle_browser_navigation_policy(BrowserNavigationPolicyRequest {
            raw_url: "lark://client/auth".to_string(),
            has_user_gesture: true,
            is_main_frame: false,
        });

        assert_eq!(response.decision, BrowserNavigationPolicyDecision::Deny);
        assert_eq!(response.reason.as_deref(), Some("non_main_frame_external"));
    }

    #[test]
    fn browser_nav_policy_allows_lingxia_in_webview() {
        // `lingxia://` is served natively by the browser scheme handler — stay in-webview.
        let response = handle_browser_navigation_policy(BrowserNavigationPolicyRequest {
            raw_url: "lingxia://settings".to_string(),
            has_user_gesture: false,
            is_main_frame: true,
        });
        assert_eq!(
            response.decision,
            BrowserNavigationPolicyDecision::InWebview
        );
    }

    #[test]
    fn lingxia_newtab_is_startup_url() {
        assert_eq!(is_lingxia_startup_url("lingxia://newtab"), Some(true));
        assert_eq!(is_lingxia_startup_url("lingxia://"), Some(true));
        assert_eq!(is_lingxia_startup_url("lingxia://downloads"), Some(false));
        assert_eq!(is_lingxia_startup_url("https://example.com"), None);
    }

    #[test]
    fn browser_nav_policy_allows_file_in_webview() {
        // Local files load in-webview (file is no longer a denied non-external scheme).
        let response = handle_browser_navigation_policy(BrowserNavigationPolicyRequest {
            raw_url: "file:///Users/me/page.html".to_string(),
            has_user_gesture: false,
            is_main_frame: true,
        });
        assert_eq!(response.decision, BrowserNavigationPolicyDecision::InWebview);
    }
}
