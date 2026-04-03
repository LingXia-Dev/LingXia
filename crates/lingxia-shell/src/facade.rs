use lingxia_browser::{
    BrowserNavigationPolicyRequest, BrowserNavigationPolicyResponse, LxAppError,
};

pub const APP_ID: &str = lingxia_browser::BUILTIN_BROWSER_APPID;
const LINGXIA_SCHEME: &str = "lingxia";

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

fn is_lingxia_startup_url(url: &str) -> Option<bool> {
    if extract_url_scheme(url).as_deref() != Some(LINGXIA_SCHEME) {
        return None;
    }
    let host = url
        .splitn(2, "://")
        .nth(1)
        .unwrap_or("")
        .split('/')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    Some(host.is_empty() || host == "newtab")
}

pub fn classify_navigation(
    request: BrowserNavigationPolicyRequest,
) -> BrowserNavigationPolicyResponse {
    lingxia_browser::classify_navigation(request)
}

pub fn classify_navigation_json(request_json: &str) -> Option<String> {
    lingxia_browser::classify_navigation_json(request_json)
}

pub fn should_hide_url(raw: &str) -> bool {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return true;
    }
    let lowered = trimmed.to_ascii_lowercase();
    if lowered.starts_with("lx:")
        || lowered.starts_with("data:")
        || lowered.starts_with("javascript:")
        || lowered.starts_with("blob:")
        || lowered == "about:blank"
    {
        return true;
    }
    matches!(is_lingxia_startup_url(trimmed), Some(true))
}

pub fn open(url: &str, tab_id: Option<&str>) -> Result<String, LxAppError> {
    lingxia_browser::open(url, tab_id)
}

pub fn open_for_app(
    appid: &str,
    session_id: u64,
    url: &str,
    tab_id: Option<&str>,
) -> Result<String, LxAppError> {
    lingxia_browser::open_for_app(appid, session_id, url, tab_id)
}

pub fn close(tab_id: &str) -> Result<(), LxAppError> {
    lingxia_browser::close(tab_id)
}

pub fn tab_path(tab_id: &str) -> String {
    lingxia_browser::tab_path(tab_id)
}

pub fn update_tab(tab_id: &str, current_url: Option<&str>, title: Option<&str>) -> bool {
    lingxia_browser::update_tab(tab_id, current_url, title)
}

pub fn download(
    tab_id: &str,
    url: &str,
    user_agent: Option<&str>,
    suggested_filename: Option<&str>,
    source_page_url: Option<&str>,
    cookie: Option<&str>,
) -> Result<(), LxAppError> {
    lingxia_browser::start_download(
        tab_id,
        url,
        user_agent,
        suggested_filename,
        source_page_url,
        cookie,
    )
}

#[cfg(test)]
mod tests {
    use super::{classify_navigation, classify_navigation_json, should_hide_url};
    use lingxia_browser::{BrowserNavigationPolicyDecision, BrowserNavigationPolicyRequest};

    #[test]
    fn browser_nav_policy_allows_lark_with_gesture() {
        let response = classify_navigation(BrowserNavigationPolicyRequest {
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
    fn browser_nav_policy_denies_non_external_scheme() {
        let response = classify_navigation(BrowserNavigationPolicyRequest {
            raw_url: "javascript:alert(1)".to_string(),
            has_user_gesture: true,
            is_main_frame: true,
        });

        assert_eq!(response.decision, BrowserNavigationPolicyDecision::Deny);
        assert_eq!(response.reason.as_deref(), Some("non_external_scheme"));
    }

    #[test]
    fn startup_page_url_is_hidden() {
        assert!(should_hide_url("lingxia://newtab"));
        assert!(should_hide_url("lingxia://"));
        assert!(!should_hide_url("lingxia://downloads"));
    }

    #[test]
    fn nav_policy_json_round_trips() {
        let json = serde_json::to_string(&BrowserNavigationPolicyRequest {
            raw_url: "lingxia://settings".to_string(),
            has_user_gesture: false,
            is_main_frame: true,
        })
        .unwrap();
        let out = classify_navigation_json(&json).unwrap();
        assert!(out.contains("\"in_webview\""));
    }
}
