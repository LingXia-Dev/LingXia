use lingxia_browser::{
    BrowserAddressAction, BrowserAddressInputError, BrowserAddressInputRequest,
    BrowserAddressInputResponse, BrowserAddressInputTrigger, BrowserAddressNavigation,
    BrowserAddressState, BrowserAddressValueKind, BrowserNavigationTarget,
};
use std::net::IpAddr;

const DEFAULT_BROWSER_PREFERRED_SCHEME: &str = "https";
const LINGXIA_SCHEME: &str = "lingxia";

#[derive(Debug, Clone)]
struct BrowserUrlResolution {
    url: String,
    inferred_scheme: Option<String>,
}

fn normalize_browser_preferred_scheme(raw: Option<&str>) -> String {
    let candidate = raw
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_BROWSER_PREFERRED_SCHEME);
    let lowered = candidate.to_ascii_lowercase();
    match lowered.as_str() {
        "http" | "https" => lowered,
        _ => DEFAULT_BROWSER_PREFERRED_SCHEME.to_string(),
    }
}

fn extract_host_from_authority(authority: &str) -> Option<&str> {
    let authority = authority.rsplit('@').next()?;
    if authority.is_empty() {
        return None;
    }

    if let Some(rest) = authority.strip_prefix('[') {
        let end = rest.find(']')?;
        let host = &rest[..end];
        if host.is_empty() {
            return None;
        }
        let suffix = &rest[end + 1..];
        if suffix.is_empty() {
            return Some(host);
        }
        if !suffix.starts_with(':') || suffix.len() == 1 {
            return None;
        }
        if suffix[1..].chars().all(|c| c.is_ascii_digit()) {
            return Some(host);
        }
        return None;
    }

    let host = match authority.rsplit_once(':') {
        Some((host, port))
            if !host.is_empty() && !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()) =>
        {
            host
        }
        Some(_) => return None,
        _ => authority,
    };

    if host.is_empty() { None } else { Some(host) }
}

fn is_probable_web_host_without_scheme(host: &str) -> bool {
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    if host.contains('.') {
        return true;
    }
    host.parse::<IpAddr>().is_ok()
}

fn resolve_browser_http_url(raw: &str, preferred_scheme: &str) -> Option<BrowserUrlResolution> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.chars().any(|c| c.is_whitespace()) {
        return None;
    }

    let (candidate, inferred_scheme) = if trimmed.contains("://") {
        (trimmed.to_string(), None)
    } else {
        (
            format!("{preferred_scheme}://{trimmed}"),
            Some(preferred_scheme.to_string()),
        )
    };

    let (scheme_raw, rest) = candidate.split_once("://")?;
    let scheme = scheme_raw.to_ascii_lowercase();
    if !matches!(scheme.as_str(), "http" | "https") {
        return None;
    }
    if rest.is_empty() || rest.starts_with('/') || rest.starts_with('?') || rest.starts_with('#') {
        return None;
    }

    let authority = rest
        .split(|c| matches!(c, '/' | '?' | '#'))
        .next()
        .unwrap_or_default();
    let host = extract_host_from_authority(authority)?;
    if host.trim().is_empty() || host.chars().any(|c| c.is_whitespace()) {
        return None;
    }
    if inferred_scheme.is_some() && !is_probable_web_host_without_scheme(host) {
        return None;
    }

    Some(BrowserUrlResolution {
        url: format!("{scheme}://{rest}"),
        inferred_scheme,
    })
}

fn classify_browser_address_value(raw: &str) -> BrowserAddressValueKind {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        BrowserAddressValueKind::Empty
    } else if trimmed.contains(char::is_whitespace) || !trimmed.contains("://") {
        BrowserAddressValueKind::SearchQuery
    } else {
        BrowserAddressValueKind::Invalid
    }
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

pub fn resolve_input(request: BrowserAddressInputRequest) -> BrowserAddressInputResponse {
    let preferred_scheme =
        normalize_browser_preferred_scheme(request.context.preferred_scheme.as_deref());
    let trimmed = request.raw_input.trim();

    if extract_url_scheme(trimmed).as_deref() == Some(LINGXIA_SCHEME) {
        let url = trimmed.to_string();
        let action = match request.trigger {
            BrowserAddressInputTrigger::Submit => BrowserAddressAction::Navigate,
            BrowserAddressInputTrigger::Edit => BrowserAddressAction::Suggest,
        };
        let state = BrowserAddressState {
            raw_input: request.raw_input,
            normalized_input: url.clone(),
            display_text: url.clone(),
            value_kind: BrowserAddressValueKind::Url,
            canonical_url: Some(url.clone()),
            inferred_scheme: None,
        };
        let navigation = BrowserAddressNavigation {
            url,
            target: BrowserNavigationTarget::CurrentTab,
        };
        return BrowserAddressInputResponse {
            action,
            state,
            navigation: matches!(action, BrowserAddressAction::Navigate).then_some(navigation),
            suggestions: None,
            error: None,
        };
    }

    if let Some(resolved) = resolve_browser_http_url(trimmed, &preferred_scheme) {
        let BrowserUrlResolution {
            url: resolved_url,
            inferred_scheme,
        } = resolved;

        let action = match request.trigger {
            BrowserAddressInputTrigger::Submit => BrowserAddressAction::Navigate,
            BrowserAddressInputTrigger::Edit => BrowserAddressAction::Suggest,
        };

        let display_text = resolved_url.clone();
        let state = BrowserAddressState {
            raw_input: request.raw_input,
            normalized_input: display_text.clone(),
            display_text,
            value_kind: BrowserAddressValueKind::Url,
            canonical_url: Some(resolved_url.clone()),
            inferred_scheme,
        };

        let navigation = BrowserAddressNavigation {
            url: resolved_url,
            target: BrowserNavigationTarget::CurrentTab,
        };

        return BrowserAddressInputResponse {
            action,
            state,
            navigation: matches!(action, BrowserAddressAction::Navigate).then_some(navigation),
            suggestions: None,
            error: None,
        };
    }

    let value_kind = classify_browser_address_value(trimmed);
    let normalized = trimmed.to_string();
    let state = BrowserAddressState {
        raw_input: request.raw_input,
        display_text: normalized.clone(),
        normalized_input: normalized,
        value_kind,
        canonical_url: None,
        inferred_scheme: None,
    };

    let should_suggest = matches!(request.trigger, BrowserAddressInputTrigger::Edit)
        || (matches!(value_kind, BrowserAddressValueKind::SearchQuery)
            && request.context.allow_search_fallback);

    if should_suggest {
        return BrowserAddressInputResponse {
            action: BrowserAddressAction::Suggest,
            state,
            navigation: None,
            suggestions: None,
            error: None,
        };
    }

    let error = match value_kind {
        BrowserAddressValueKind::Empty => BrowserAddressInputError {
            code: "empty_input".to_string(),
            message: "Address input is empty".to_string(),
        },
        BrowserAddressValueKind::SearchQuery => BrowserAddressInputError {
            code: "search_fallback_unavailable".to_string(),
            message: "Search fallback is not enabled for this browser input".to_string(),
        },
        BrowserAddressValueKind::Invalid | BrowserAddressValueKind::Url => {
            BrowserAddressInputError {
                code: "invalid_url".to_string(),
                message: "Address input is not a supported URL".to_string(),
            }
        }
    };

    BrowserAddressInputResponse {
        action: BrowserAddressAction::Reject,
        state,
        navigation: None,
        suggestions: None,
        error: Some(error),
    }
}

pub fn resolve_input_json(request_json: &str) -> Option<String> {
    let request: BrowserAddressInputRequest = serde_json::from_str(request_json).ok()?;
    serde_json::to_string(&resolve_input(request)).ok()
}

#[cfg(test)]
mod tests {
    use super::resolve_input;
    use lingxia_browser::{
        BrowserAddressAction, BrowserAddressInputContext, BrowserAddressInputRequest,
        BrowserAddressInputTrigger, BrowserAddressValueKind,
    };

    #[test]
    fn submit_without_scheme_navigates_with_https() {
        let response = resolve_input(BrowserAddressInputRequest {
            raw_input: "example.com/docs".to_string(),
            trigger: BrowserAddressInputTrigger::Submit,
            context: BrowserAddressInputContext::default(),
        });

        assert_eq!(response.action, BrowserAddressAction::Navigate);
        assert_eq!(
            response.navigation.as_ref().map(|value| value.url.as_str()),
            Some("https://example.com/docs")
        );
        assert_eq!(response.state.value_kind, BrowserAddressValueKind::Url);
        assert_eq!(response.state.inferred_scheme.as_deref(), Some("https"));
    }

    #[test]
    fn submit_keeps_http_fragments() {
        let response = resolve_input(BrowserAddressInputRequest {
            raw_input: "http://example.com/path?q=1#frag".to_string(),
            trigger: BrowserAddressInputTrigger::Submit,
            context: BrowserAddressInputContext::default(),
        });

        assert_eq!(response.action, BrowserAddressAction::Navigate);
        assert_eq!(
            response.navigation.as_ref().map(|value| value.url.as_str()),
            Some("http://example.com/path?q=1#frag")
        );
        assert_eq!(response.state.inferred_scheme, None);
    }

    #[test]
    fn submit_supports_localhost() {
        let response = resolve_input(BrowserAddressInputRequest {
            raw_input: "localhost:3000".to_string(),
            trigger: BrowserAddressInputTrigger::Submit,
            context: BrowserAddressInputContext::default(),
        });

        assert_eq!(response.action, BrowserAddressAction::Navigate);
        assert_eq!(
            response.navigation.as_ref().map(|value| value.url.as_str()),
            Some("https://localhost:3000")
        );
    }

    #[test]
    fn edit_search_query_returns_suggest_action() {
        let response = resolve_input(BrowserAddressInputRequest {
            raw_input: "openai docs".to_string(),
            trigger: BrowserAddressInputTrigger::Edit,
            context: BrowserAddressInputContext::default(),
        });

        assert_eq!(response.action, BrowserAddressAction::Suggest);
        assert_eq!(
            response.state.value_kind,
            BrowserAddressValueKind::SearchQuery
        );
        assert!(response.navigation.is_none());
    }

    #[test]
    fn submit_search_query_rejects_when_fallback_is_disabled() {
        let response = resolve_input(BrowserAddressInputRequest {
            raw_input: "openai".to_string(),
            trigger: BrowserAddressInputTrigger::Submit,
            context: BrowserAddressInputContext::default(),
        });

        assert_eq!(response.action, BrowserAddressAction::Reject);
        assert_eq!(
            response.error.as_ref().map(|value| value.code.as_str()),
            Some("search_fallback_unavailable")
        );
        assert_eq!(
            response.state.value_kind,
            BrowserAddressValueKind::SearchQuery
        );
    }

    #[test]
    fn address_input_submit_lingxia_newtab_navigates() {
        let response = resolve_input(BrowserAddressInputRequest {
            raw_input: "lingxia://newtab".to_string(),
            trigger: BrowserAddressInputTrigger::Submit,
            context: BrowserAddressInputContext::default(),
        });
        assert_eq!(response.action, BrowserAddressAction::Navigate);
        assert_eq!(response.state.value_kind, BrowserAddressValueKind::Url);
        assert_eq!(
            response.navigation.as_ref().map(|n| n.url.as_str()),
            Some("lingxia://newtab")
        );
    }

    #[test]
    fn address_input_submit_lingxia_transfer_navigates() {
        let response = resolve_input(BrowserAddressInputRequest {
            raw_input: "lingxia://downloads".to_string(),
            trigger: BrowserAddressInputTrigger::Submit,
            context: BrowserAddressInputContext::default(),
        });
        assert_eq!(response.action, BrowserAddressAction::Navigate);
        assert_eq!(
            response.navigation.as_ref().map(|n| n.url.as_str()),
            Some("lingxia://downloads")
        );
    }
}
