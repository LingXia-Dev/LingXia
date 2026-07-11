//! Browser automation: element query, wait conditions, input (click/fill/type),
//! cookies, screenshots, and the native input host hook.

use crate::BUILTIN_BROWSER_APPID;
use crate::policy::normalize_url_for_wait_compare;
use crate::tabs::{
    browser_tab_path_for_runtime_id, browser_update_tab_info, lock_state, normalize_runtime_tab_id,
};
use crate::types::{
    BrowserAutomationError, BrowserElementInfo, BrowserNativeInputHost, BrowserWaitCondition,
    BrowserWaitResult,
};
use lingxia_webview::runtime::find_webview as find_managed_webview;
use lingxia_webview::{
    NetworkCaptureSnapshot, WebTag, WebView, WebViewController, WebViewCookie,
    WebViewCookieSetRequest,
};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

const DEFAULT_QUERY_TEXT_LIMIT: usize = 4096;

static BROWSER_NATIVE_INPUT_HOST: OnceLock<Arc<dyn BrowserNativeInputHost>> = OnceLock::new();

pub fn register_native_input_host(host: Arc<dyn BrowserNativeInputHost>) -> bool {
    BROWSER_NATIVE_INPUT_HOST.set(host).is_ok()
}

fn native_input_host() -> Option<&'static Arc<dyn BrowserNativeInputHost>> {
    BROWSER_NATIVE_INPUT_HOST.get()
}

async fn prepare_browser_tab_for_input(tab_id: &str) -> Result<(), BrowserAutomationError> {
    if let Some(host) = native_input_host() {
        let mut last_error = None;
        for _ in 0..10 {
            match host.prepare_for_input(tab_id) {
                Ok(()) => return Ok(()),
                Err(error) => last_error = Some(error),
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        return Err(BrowserAutomationError::NativeInput(
            last_error.unwrap_or_else(|| format!("failed to prepare browser tab: {tab_id}")),
        ));
    }
    Ok(())
}

fn browser_tab_webview(tab_id: &str) -> Result<Arc<WebView>, BrowserAutomationError> {
    let normalized_tab_id = normalize_runtime_tab_id(tab_id)
        .ok_or_else(|| BrowserAutomationError::TabNotFound(tab_id.to_string()))?;
    let session_id = {
        let state = lock_state();
        state
            .tabs
            .get(&normalized_tab_id)
            .map(|tab| tab.session_id)
            .ok_or_else(|| BrowserAutomationError::TabNotFound(tab_id.to_string()))?
    };
    let path = browser_tab_path_for_runtime_id(&normalized_tab_id);
    let webtag = WebTag::new(BUILTIN_BROWSER_APPID, &path, Some(session_id));
    find_managed_webview(&webtag)
        .ok_or_else(|| BrowserAutomationError::WebViewNotFound(tab_id.to_string()))
}

fn build_browser_query_script(
    selector: &str,
    max_text_chars: Option<usize>,
) -> Result<String, BrowserAutomationError> {
    let selector_json = serde_json::to_string(selector)
        .map_err(|err| BrowserAutomationError::NativeInput(format!("invalid selector: {err}")))?;
    let max_text_json = serde_json::to_string(&max_text_chars).map_err(|err| {
        BrowserAutomationError::NativeInput(format!("invalid query limit: {err}"))
    })?;
    Ok(format!(
        r#"
(() => {{
  const selector = {selector_json};
  const maxText = {max_text_json};
  const truncate = (value) => {{
    const text = String(value ?? "");
    if (typeof maxText === "number" && maxText >= 0 && text.length > maxText) {{
      return {{ value: text.slice(0, maxText), truncated: true }};
    }}
    return {{ value: text, truncated: false }};
  }};
  if (typeof selector !== "string" || selector.trim() === "") {{
    throw new Error("selector must not be empty");
  }}
  let el;
  try {{
    el = document.querySelector(selector);
  }} catch (err) {{
    throw new Error("invalid selector: " + String(err && err.message ? err.message : err));
  }}
  if (!el) {{
    return {{
      exists: false,
      visible: false,
      enabled: false,
      editable: false
    }};
  }}

  const rect = el.getBoundingClientRect();
  const style = window.getComputedStyle(el);
  const disabled = !!el.disabled || el.getAttribute("aria-disabled") === "true";
  const tag = (el.tagName || "").toLowerCase();
  const inputType = tag === "input" ? String(el.type || "text").toLowerCase() : "";
  const blockedInputTypes = new Set([
    "button", "checkbox", "color", "file", "hidden", "image", "radio",
    "range", "reset", "submit"
  ]);
  const editable = !!el.isContentEditable ||
    (tag === "textarea" && !disabled && !el.readOnly) ||
    (tag === "input" && !disabled && !el.readOnly && !blockedInputTypes.has(inputType));
  const visible = rect.width > 0 &&
    rect.height > 0 &&
    rect.bottom > 0 &&
    rect.right > 0 &&
    rect.top < window.innerHeight &&
    rect.left < window.innerWidth &&
    style.visibility !== "hidden" &&
    style.display !== "none" &&
    Number(style.opacity || "1") !== 0;
  const hasValue = "value" in el;
  const text = truncate(el.innerText || el.textContent || "");
  const value = hasValue ? truncate(el.value ?? "") : null;
  return {{
    exists: true,
    visible,
    enabled: !disabled,
    editable,
    text: text.value,
    text_truncated: text.truncated,
    value: value ? value.value : null,
    value_truncated: value ? value.truncated : false,
    rect: {{
      left: rect.left,
      top: rect.top,
      width: rect.width,
      height: rect.height,
      right: rect.right,
      bottom: rect.bottom,
      center_x: rect.left + (rect.width / 2),
      center_y: rect.top + (rect.height / 2),
      viewport_width: window.innerWidth,
      viewport_height: window.innerHeight
    }}
  }};
}})()
"#
    ))
}

fn browser_tab_current_url(tab_id: &str) -> Result<Option<String>, BrowserAutomationError> {
    let normalized_tab_id = normalize_runtime_tab_id(tab_id)
        .ok_or_else(|| BrowserAutomationError::TabNotFound(tab_id.to_string()))?;
    let state = lock_state();
    state
        .tabs
        .get(&normalized_tab_id)
        .map(|tab| tab.current_url.clone().or_else(|| tab.pending_url.clone()))
        .ok_or_else(|| BrowserAutomationError::TabNotFound(tab_id.to_string()))
}

async fn browser_live_current_url(tab_id: &str) -> Result<Option<String>, BrowserAutomationError> {
    let state_url = browser_tab_current_url(tab_id)?;
    let webview = browser_tab_webview(tab_id)?;

    match webview.current_url().await {
        Ok(Some(url)) => {
            let _ = browser_update_tab_info(tab_id, Some(url.as_str()), None);
            Ok(Some(url))
        }
        Ok(None) => Ok(state_url),
        Err(_) => Ok(state_url),
    }
}

pub async fn browser_current_url(tab_id: &str) -> Result<Option<String>, BrowserAutomationError> {
    browser_live_current_url(tab_id).await
}

fn wait_condition_label(condition: &BrowserWaitCondition) -> String {
    match condition {
        BrowserWaitCondition::Loaded => "loaded".to_string(),
        BrowserWaitCondition::SelectorExists { selector } => format!("selector exists {selector}"),
        BrowserWaitCondition::SelectorVisible { selector } => {
            format!("selector visible {selector}")
        }
        BrowserWaitCondition::SelectorHidden { selector } => {
            format!("selector hidden {selector}")
        }
        BrowserWaitCondition::SelectorEditable { selector } => {
            format!("selector editable {selector}")
        }
        BrowserWaitCondition::JsTrue { .. } => "js returns true".to_string(),
        BrowserWaitCondition::UrlEquals { url } => format!("url equals {url}"),
        BrowserWaitCondition::UrlContains { text } => format!("url contains {text}"),
        BrowserWaitCondition::Navigation { .. } => "navigation".to_string(),
    }
}

fn duration_ms_u64(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

struct BrowserWaitCheck {
    matched: bool,
    current_url: Option<String>,
    element: Option<BrowserElementInfo>,
    value: Option<serde_json::Value>,
}

async fn check_wait_condition(
    tab_id: &str,
    condition: &BrowserWaitCondition,
) -> Result<BrowserWaitCheck, BrowserAutomationError> {
    match condition {
        BrowserWaitCondition::Loaded => {
            let value = browser_evaluate_javascript(tab_id, "document.readyState").await?;
            let matched = value.as_str() == Some("complete");
            Ok(BrowserWaitCheck {
                matched,
                current_url: browser_live_current_url(tab_id).await?,
                element: None,
                value: Some(value),
            })
        }
        BrowserWaitCondition::SelectorExists { selector } => {
            let element = browser_query(tab_id, selector).await?;
            Ok(BrowserWaitCheck {
                matched: element.exists,
                current_url: browser_live_current_url(tab_id).await?,
                element: Some(element),
                value: None,
            })
        }
        BrowserWaitCondition::SelectorVisible { selector } => {
            let element = browser_query(tab_id, selector).await?;
            Ok(BrowserWaitCheck {
                matched: element.exists && element.visible,
                current_url: browser_live_current_url(tab_id).await?,
                element: Some(element),
                value: None,
            })
        }
        BrowserWaitCondition::SelectorHidden { selector } => {
            let element = browser_query(tab_id, selector).await?;
            Ok(BrowserWaitCheck {
                matched: !element.exists || !element.visible,
                current_url: browser_live_current_url(tab_id).await?,
                element: Some(element),
                value: None,
            })
        }
        BrowserWaitCondition::SelectorEditable { selector } => {
            let element = browser_query(tab_id, selector).await?;
            Ok(BrowserWaitCheck {
                matched: element.exists && element.visible && element.enabled && element.editable,
                current_url: browser_live_current_url(tab_id).await?,
                element: Some(element),
                value: None,
            })
        }
        BrowserWaitCondition::JsTrue { js } => {
            let value = browser_evaluate_javascript(tab_id, js).await?;
            Ok(BrowserWaitCheck {
                matched: value.as_bool().unwrap_or(false),
                current_url: browser_live_current_url(tab_id).await?,
                element: None,
                value: Some(value),
            })
        }
        BrowserWaitCondition::UrlEquals { url } => {
            let current_url = browser_live_current_url(tab_id).await?;
            let expected = normalize_url_for_wait_compare(url);
            Ok(BrowserWaitCheck {
                matched: current_url
                    .as_deref()
                    .is_some_and(|url| normalize_url_for_wait_compare(url) == expected),
                current_url,
                element: None,
                value: None,
            })
        }
        BrowserWaitCondition::UrlContains { text } => {
            let current_url = browser_live_current_url(tab_id).await?;
            Ok(BrowserWaitCheck {
                matched: current_url
                    .as_deref()
                    .is_some_and(|url| url.contains(text.as_str())),
                current_url,
                element: None,
                value: None,
            })
        }
        BrowserWaitCondition::Navigation {
            initial_url,
            wait_until_complete,
        } => {
            let current_url = browser_live_current_url(tab_id).await?;
            let changed = current_url.as_deref().map(normalize_url_for_wait_compare)
                != initial_url.as_deref().map(normalize_url_for_wait_compare);
            let loaded = if changed && *wait_until_complete {
                browser_evaluate_javascript(tab_id, "document.readyState")
                    .await?
                    .as_str()
                    == Some("complete")
            } else {
                true
            };
            Ok(BrowserWaitCheck {
                matched: changed && loaded,
                current_url,
                element: None,
                value: None,
            })
        }
    }
}

pub async fn browser_evaluate_javascript(
    tab_id: &str,
    js: &str,
) -> Result<serde_json::Value, BrowserAutomationError> {
    browser_tab_webview(tab_id)?
        .evaluate_javascript(js)
        .await
        .map_err(BrowserAutomationError::from)
}

pub async fn browser_take_screenshot(tab_id: &str) -> Result<Vec<u8>, BrowserAutomationError> {
    browser_tab_webview(tab_id)?
        .take_screenshot()
        .await
        .map_err(BrowserAutomationError::from)
}

pub fn browser_reload(tab_id: &str) -> Result<(), BrowserAutomationError> {
    browser_tab_webview(tab_id)?.reload()?;
    Ok(())
}

pub fn browser_go_back(tab_id: &str) -> Result<(), BrowserAutomationError> {
    browser_tab_webview(tab_id)?.go_back()?;
    Ok(())
}

pub fn browser_go_forward(tab_id: &str) -> Result<(), BrowserAutomationError> {
    browser_tab_webview(tab_id)?.go_forward()?;
    Ok(())
}

pub async fn browser_list_cookies(
    tab_id: &str,
) -> Result<Vec<WebViewCookie>, BrowserAutomationError> {
    let current_url = browser_live_current_url(tab_id).await?;
    let cookies = browser_tab_webview(tab_id)?
        .list_cookies()
        .await
        .map_err(BrowserAutomationError::from)?;
    Ok(
        match current_url
            .as_deref()
            .and_then(cookie_filter_context_for_url)
        {
            Some((host, path)) => cookies
                .into_iter()
                .filter(|cookie| cookie_matches_url(cookie, &host, &path))
                .collect(),
            None => cookies,
        },
    )
}

pub async fn browser_list_all_cookies(
    tab_id: &str,
) -> Result<Vec<WebViewCookie>, BrowserAutomationError> {
    browser_tab_webview(tab_id)?
        .list_cookies()
        .await
        .map_err(BrowserAutomationError::from)
}

pub async fn browser_set_cookie(
    tab_id: &str,
    mut request: WebViewCookieSetRequest,
) -> Result<(), BrowserAutomationError> {
    if request.url.trim().is_empty() {
        request.url = browser_live_current_url(tab_id).await?.ok_or_else(|| {
            BrowserAutomationError::NativeInput(
                "cookie url is required when tab has no current URL".to_string(),
            )
        })?;
    }
    browser_tab_webview(tab_id)?
        .set_cookie(request)
        .await
        .map_err(BrowserAutomationError::from)
}

pub async fn browser_delete_cookie(
    tab_id: &str,
    name: &str,
    domain: &str,
    path: &str,
) -> Result<(), BrowserAutomationError> {
    browser_tab_webview(tab_id)?
        .delete_cookie(name, domain, path)
        .await
        .map_err(BrowserAutomationError::from)
}

pub async fn browser_clear_cookies(tab_id: &str) -> Result<(), BrowserAutomationError> {
    browser_tab_webview(tab_id)?
        .clear_cookies()
        .await
        .map_err(BrowserAutomationError::from)
}

pub async fn browser_clear_site_data(
    tab_id: &str,
    options: lingxia_webview::ClearSiteDataOptions,
) -> Result<lingxia_webview::ClearSiteDataResult, BrowserAutomationError> {
    let url = browser_live_current_url(tab_id).await?.ok_or_else(|| {
        BrowserAutomationError::NativeInput("current tab has no website URL".to_string())
    })?;
    let uri = url.parse::<http::Uri>().map_err(|_| {
        BrowserAutomationError::NativeInput("current tab URL is not a website".to_string())
    })?;
    if !matches!(uri.scheme_str(), Some("http" | "https")) || uri.host().is_none() {
        return Err(BrowserAutomationError::NativeInput(
            "current tab URL is not a website".to_string(),
        ));
    }
    browser_tab_webview(tab_id)?
        .clear_site_data(&url, options)
        .await
        .map_err(BrowserAutomationError::from)
}

pub async fn browser_start_network_capture(tab_id: &str) -> Result<(), BrowserAutomationError> {
    browser_tab_webview(tab_id)?
        .start_network_capture()
        .await
        .map_err(BrowserAutomationError::from)
}

pub async fn browser_stop_network_capture(tab_id: &str) -> Result<(), BrowserAutomationError> {
    browser_tab_webview(tab_id)?
        .stop_network_capture()
        .await
        .map_err(BrowserAutomationError::from)
}

pub async fn browser_network_entries(
    tab_id: &str,
) -> Result<NetworkCaptureSnapshot, BrowserAutomationError> {
    browser_tab_webview(tab_id)?
        .network_entries()
        .await
        .map_err(BrowserAutomationError::from)
}

pub async fn browser_clear_network_capture(tab_id: &str) -> Result<(), BrowserAutomationError> {
    browser_tab_webview(tab_id)?
        .clear_network_capture()
        .await
        .map_err(BrowserAutomationError::from)
}

fn cookie_filter_context_for_url(url: &str) -> Option<(String, String)> {
    let uri = url.parse::<http::Uri>().ok()?;
    let host = normalize_cookie_host(uri.host()?);
    if host.is_empty() {
        None
    } else {
        let path = uri
            .path_and_query()
            .map(|value| value.path())
            .filter(|value| !value.is_empty())
            .unwrap_or("/")
            .to_string();
        Some((host, path))
    }
}

fn cookie_matches_url(cookie: &WebViewCookie, host: &str, path: &str) -> bool {
    let domain = normalize_cookie_host(cookie.domain.trim_start_matches('.'));
    if domain.is_empty() {
        return false;
    }
    let domain_matches = if cookie.host_only {
        host == domain
    } else {
        host == domain || host.ends_with(&format!(".{domain}"))
    };
    if !domain_matches {
        return false;
    }
    let cookie_path = if cookie.path.trim().is_empty() {
        "/"
    } else {
        cookie.path.as_str()
    };
    if cookie_path == "/" || path == cookie_path {
        return true;
    }
    if cookie_path.ends_with('/') {
        return path.starts_with(cookie_path);
    }
    path.strip_prefix(cookie_path)
        .is_some_and(|suffix| suffix.starts_with('/'))
}

fn normalize_cookie_host(host: &str) -> String {
    host.trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_ascii_lowercase()
}

pub async fn browser_query(
    tab_id: &str,
    selector: &str,
) -> Result<BrowserElementInfo, BrowserAutomationError> {
    browser_query_with_max_text(tab_id, selector, Some(DEFAULT_QUERY_TEXT_LIMIT)).await
}

pub async fn browser_query_with_max_text(
    tab_id: &str,
    selector: &str,
    max_text_chars: Option<usize>,
) -> Result<BrowserElementInfo, BrowserAutomationError> {
    let script = build_browser_query_script(selector, max_text_chars)?;
    let value = browser_evaluate_javascript(tab_id, &script).await?;
    serde_json::from_value(value).map_err(|err| {
        BrowserAutomationError::NativeInput(format!("failed to decode element info: {err}"))
    })
}

pub async fn browser_wait(
    tab_id: &str,
    condition: BrowserWaitCondition,
    timeout: Duration,
) -> Result<BrowserWaitResult, BrowserAutomationError> {
    let started = Instant::now();
    let timeout_ms = duration_ms_u64(timeout);

    loop {
        let check = check_wait_condition(tab_id, &condition).await?;
        if check.matched {
            return Ok(BrowserWaitResult {
                elapsed_ms: duration_ms_u64(started.elapsed()),
                current_url: check.current_url,
                element: check.element,
                value: check.value,
            });
        }

        if started.elapsed() >= timeout {
            return Err(BrowserAutomationError::WaitTimeout {
                condition: wait_condition_label(&condition),
                timeout_ms,
            });
        }

        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

pub async fn browser_wait_for_url(
    tab_id: &str,
    url: &str,
    timeout: Duration,
) -> Result<BrowserWaitResult, BrowserAutomationError> {
    browser_wait(
        tab_id,
        BrowserWaitCondition::UrlEquals {
            url: url.to_string(),
        },
        timeout,
    )
    .await
}

pub async fn browser_wait_for_url_contains(
    tab_id: &str,
    text: &str,
    timeout: Duration,
) -> Result<BrowserWaitResult, BrowserAutomationError> {
    browser_wait(
        tab_id,
        BrowserWaitCondition::UrlContains {
            text: text.to_string(),
        },
        timeout,
    )
    .await
}

pub async fn browser_wait_for_navigation(
    tab_id: &str,
    timeout: Duration,
    wait_until_complete: bool,
) -> Result<BrowserWaitResult, BrowserAutomationError> {
    let initial_url = browser_live_current_url(tab_id).await?;
    browser_wait(
        tab_id,
        BrowserWaitCondition::Navigation {
            initial_url,
            wait_until_complete,
        },
        timeout,
    )
    .await
}

pub async fn browser_click(tab_id: &str, selector: &str) -> Result<(), BrowserAutomationError> {
    prepare_browser_tab_for_input(tab_id).await?;
    browser_tab_webview(tab_id)?
        .click(selector, lingxia_webview::ClickOptions::default())
        .await
        .map_err(BrowserAutomationError::from)
}

pub async fn browser_fill(
    tab_id: &str,
    selector: &str,
    text: &str,
) -> Result<(), BrowserAutomationError> {
    prepare_browser_tab_for_input(tab_id).await?;
    browser_tab_webview(tab_id)?
        .fill(selector, text, lingxia_webview::FillOptions::default())
        .await
        .map_err(BrowserAutomationError::from)
}

pub async fn browser_type_text(
    tab_id: &str,
    selector: &str,
    text: &str,
) -> Result<(), BrowserAutomationError> {
    prepare_browser_tab_for_input(tab_id).await?;
    browser_tab_webview(tab_id)?
        .type_text(selector, text, lingxia_webview::TypeOptions::default())
        .await
        .map_err(BrowserAutomationError::from)
}

pub async fn browser_press(tab_id: &str, key: &str) -> Result<(), BrowserAutomationError> {
    prepare_browser_tab_for_input(tab_id).await?;
    browser_tab_webview(tab_id)?
        .press(key, lingxia_webview::PressOptions)
        .await
        .map_err(BrowserAutomationError::from)
}

pub async fn browser_scroll(tab_id: &str, dx: f64, dy: f64) -> Result<(), BrowserAutomationError> {
    prepare_browser_tab_for_input(tab_id).await?;
    browser_tab_webview(tab_id)?
        .scroll(dx, dy, lingxia_webview::ScrollOptions)
        .await
        .map_err(BrowserAutomationError::from)
}

pub async fn browser_scroll_to(tab_id: &str, selector: &str) -> Result<(), BrowserAutomationError> {
    prepare_browser_tab_for_input(tab_id).await?;
    browser_tab_webview(tab_id)?
        .scroll_to(selector, lingxia_webview::ScrollOptions)
        .await
        .map_err(BrowserAutomationError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cookie_filter_context_handles_ipv6_urls() {
        assert_eq!(
            cookie_filter_context_for_url("https://[::1]:8443/path?q=1"),
            Some(("::1".to_string(), "/path".to_string()))
        );
    }

    #[test]
    fn cookie_matches_url_respects_host_only_and_path_rules() {
        let host_only = WebViewCookie {
            name: "a".to_string(),
            value: "1".to_string(),
            domain: "example.com".to_string(),
            path: "/foo".to_string(),
            host_only: true,
            secure: false,
            http_only: false,
            session: true,
            expires_unix_ms: None,
            same_site: None,
        };
        assert!(cookie_matches_url(&host_only, "example.com", "/foo/bar"));
        assert!(!cookie_matches_url(
            &host_only,
            "sub.example.com",
            "/foo/bar"
        ));
        assert!(!cookie_matches_url(&host_only, "example.com", "/foobar"));

        let domain_cookie = WebViewCookie {
            host_only: false,
            domain: ".example.com".to_string(),
            ..host_only
        };
        assert!(cookie_matches_url(
            &domain_cookie,
            "sub.example.com",
            "/foo/bar"
        ));
    }
}
