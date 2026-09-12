use lingxia_update::{Channel, default_channel};
use std::sync::OnceLock;

const LXAPP_PREFIX: &str = "/lxapp/";
const OPEN_ACTION: &str = "open";

/// Parsed inbound AppLink.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppLinkTarget {
    /// The URL exactly as the OS delivered it, fragment included. Logic routes
    /// from this; every other field is a convenience for `/lxapp/*` links.
    pub url: String,
    /// Target lxapp. Empty resolves to the host's home lxapp.
    pub appid: String,
    /// Target page inside the lxapp. Empty means the initial page.
    pub path: String,
    /// Page query. Routing params are stripped only for `/lxapp/open`.
    pub query: String,
    pub release_type: Channel,
    /// The URL is in the `/lxapp/*` namespace, so `appid` / `path` / `query`
    /// were parsed rather than passed through.
    pub lxapp_route: bool,
}

/// Host callback used to open an accepted AppLink.
pub type AppLinkHandler = fn(AppLinkTarget) -> i32;

static APP_LINK_HANDLER: OnceLock<AppLinkHandler> = OnceLock::new();

/// Register the host AppLink opener.
pub fn register_handler(handler: AppLinkHandler) {
    let _ = APP_LINK_HANDLER.set(handler);
}

/// Deliver an inbound URL to the home lxapp. Used by the OS entry points, push
/// links and devtool injection.
///
/// The host is the only gate: any path on a configured host is handed to Logic
/// as `scene: 8003`. Returns:
///
/// - `1` when the link was delivered to the registered handler.
/// - `0` when the URL is not `https://`, or its host is not configured.
/// - `-1` when no handler is registered, or the URL is in the `/lxapp/*`
///   namespace but malformed. A product path never yields `-1`.
pub fn deliver(url: &str) -> i32 {
    dispatch(url, false)
}

/// Deliver only `/lxapp/*` URLs. Used by `scanCode`: a scan is something the
/// user aimed at a code inside an lxapp, so an arbitrary product URL that
/// happens to be on a configured host must not take over the home lxapp.
pub fn deliver_lxapp_only(url: &str) -> i32 {
    dispatch(url, true)
}

fn dispatch(url: &str, lxapp_only: bool) -> i32 {
    match parse(url) {
        Ok(Some(target)) => {
            if lxapp_only && !target.lxapp_route {
                return 0;
            }
            match APP_LINK_HANDLER.get() {
                Some(handler) => handler(target),
                None => -1,
            }
        }
        Ok(None) => 0,
        Err(_) => -1,
    }
}

/// Parse an inbound AppLink without opening it.
pub fn parse(url: &str) -> Result<Option<AppLinkTarget>, String> {
    let url = url.trim();
    let Some(rest) = url.strip_prefix("https://") else {
        return Ok(None);
    };
    let (authority, path_and_query) = split_authority(rest);
    let host = host_without_port(authority);
    if host.is_empty() {
        return Err("missing host".to_string());
    }
    if !host_allowed(host) {
        return Ok(None);
    }

    // The fragment survives only in `url`: it is not part of the page query.
    let (url_path, raw_query) = split_path_query(strip_fragment(path_and_query));

    let Some(route) = parse_route(url_path)? else {
        // Product path: nothing here is ours to interpret. Query goes to the
        // page verbatim — no percent validation, no routing params consumed —
        // so a real link is never rejected over its own encoding.
        return Ok(Some(AppLinkTarget {
            url: url.to_string(),
            appid: String::new(),
            path: String::new(),
            query: raw_query.unwrap_or_default().to_string(),
            release_type: default_channel(),
            lxapp_route: false,
        }));
    };

    let uses_query_routing = route.appid.is_none();
    let query_parts = parse_query(raw_query, uses_query_routing)?;
    let appid = match route.appid {
        Some(appid) => appid,
        None => query_parts.appid.unwrap_or_default(),
    };
    let path = match route.path {
        Some(path) => path,
        None => query_parts.path.unwrap_or_default(),
    };
    // Empty appId is resolved to the host's home lxapp when the link is opened.

    Ok(Some(AppLinkTarget {
        url: url.to_string(),
        appid,
        path,
        query: query_parts.page_query,
        release_type: query_parts.release_type,
        lxapp_route: true,
    }))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AppLinkRoute {
    appid: Option<String>,
    path: Option<String>,
}

fn parse_route(path: &str) -> Result<Option<AppLinkRoute>, String> {
    let Some(rest) = path.strip_prefix(LXAPP_PREFIX) else {
        return Ok(None);
    };
    if rest == OPEN_ACTION {
        return Ok(Some(AppLinkRoute {
            appid: None,
            path: None,
        }));
    }
    if rest.starts_with("open/") {
        // Still our namespace: a malformed link must fail, not open home.
        return Err("unsupported /lxapp/open subpath".to_string());
    }

    let (raw_appid, raw_path) = rest.split_once('/').unwrap_or((rest, ""));
    if raw_appid.is_empty() {
        return Err("missing lxapp appId".to_string());
    }
    Ok(Some(AppLinkRoute {
        appid: Some(decode_component(raw_appid)?),
        path: (!raw_path.is_empty())
            .then(|| decode_component(raw_path))
            .transpose()?,
    }))
}

fn split_authority(rest: &str) -> (&str, &str) {
    // A root link may carry a query or fragment with no `/` before it.
    match rest.find(['/', '?', '#']) {
        Some(index) => (&rest[..index], &rest[index..]),
        None => (rest, "/"),
    }
}

fn host_without_port(authority: &str) -> &str {
    authority
        .split('@')
        .next_back()
        .unwrap_or(authority)
        .split(':')
        .next()
        .unwrap_or("")
        .trim()
}

fn strip_fragment(value: &str) -> &str {
    match value.find('#') {
        Some(index) => &value[..index],
        None => value,
    }
}

fn split_path_query(value: &str) -> (&str, Option<&str>) {
    match value.find('?') {
        Some(index) => (&value[..index], Some(&value[index + 1..])),
        None => (value, None),
    }
}

const RUNNER_MARKER_ENV: &str = "LINGXIA_RUNNER";

fn runner_process() -> bool {
    std::env::var(RUNNER_MARKER_ENV)
        .ok()
        .is_some_and(|value| !value.trim().is_empty())
}

fn host_allowed(host: &str) -> bool {
    let Some(config) = lingxia_app_context::app_config() else {
        return true;
    };
    host_allowed_for(
        host,
        config
            .app_links
            .as_ref()
            .map(|links| links.hosts.as_slice())
            .unwrap_or(&[]),
        runner_process(),
    )
}

/// Product hosts must match `appLinks.hosts`. The Runner is not a product and
/// has no hosts — any AppLink URL is accepted so `lxdev app applink` can use
/// the URL under test.
fn host_allowed_for(host: &str, hosts: &[String], is_runner: bool) -> bool {
    if hosts.is_empty() {
        return is_runner;
    }
    hosts
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(host))
}

struct QueryParts {
    release_type: Channel,
    appid: Option<String>,
    path: Option<String>,
    page_query: String,
}

fn parse_query(raw_query: Option<&str>, include_routing: bool) -> Result<QueryParts, String> {
    let Some(raw_query) = raw_query else {
        return Ok(QueryParts {
            release_type: default_channel(),
            appid: None,
            path: None,
            page_query: String::new(),
        });
    };
    let mut release_type = default_channel();
    let mut appid = None;
    let mut path = None;
    let mut page_params = Vec::new();
    for pair in raw_query.split('&').filter(|pair| !pair.is_empty()) {
        let (raw_key, raw_value) = match pair.split_once('=') {
            Some((key, value)) => (key, value),
            None => (pair, ""),
        };
        let key = decode_component(raw_key)?;
        if key == "channel" {
            release_type = parse_channel(&decode_component(raw_value)?)?;
            continue;
        }
        if include_routing && (key == "appId" || key == "appid") {
            appid = Some(decode_component(raw_value)?);
            continue;
        }
        if include_routing && key == "path" {
            path = Some(decode_component(raw_value)?);
            continue;
        }
        page_params.push(pair.to_string());
    }
    Ok(QueryParts {
        release_type,
        appid,
        path,
        page_query: page_params.join("&"),
    })
}

fn parse_channel(tag: &str) -> Result<Channel, String> {
    Channel::parse(tag)
}

fn decode_component(value: &str) -> Result<String, String> {
    percent_decode(value).ok_or_else(|| format!("invalid percent encoding in {value:?}"))
}

fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' => {
                if index + 2 >= bytes.len() {
                    return None;
                }
                let hi = hex_value(bytes[index + 1])?;
                let lo = hex_value(bytes[index + 2])?;
                out.push((hi << 4) | lo);
                index += 3;
            }
            ch => {
                out.push(ch);
                index += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_open_without_page_path() {
        let target = parse("https://www.lingxia.app/lxapp/open?appId=com.example.shop")
            .unwrap()
            .unwrap();
        assert_eq!(target.appid, "com.example.shop");
        assert_eq!(target.path, "");
        assert_eq!(target.query, "");
        assert_eq!(target.release_type, Channel::Release);
        assert!(target.lxapp_route);
    }

    #[test]
    fn parses_open_without_appid_for_home_routing() {
        let target = parse("https://www.lingxia.app/lxapp/open?page=order&id=42")
            .unwrap()
            .unwrap();
        assert_eq!(target.appid, "");
        assert_eq!(target.path, "");
        assert_eq!(target.query, "page=order&id=42");
    }

    #[test]
    fn parses_open_page_and_strips_routing_query() {
        let target = parse(
            "https://www.lingxia.app/lxapp/open?appId=com.example.shop&path=pages%2Fdetail%2Findex.html&channel=preview&id=42",
        )
        .unwrap()
        .unwrap();
        assert_eq!(target.appid, "com.example.shop");
        assert_eq!(target.path, "pages/detail/index.html");
        assert_eq!(target.query, "id=42");
        assert_eq!(target.release_type, Channel::Preview);
    }

    #[test]
    fn parses_open_query_form() {
        let target = parse(
            "https://www.lingxia.app/lxapp/open?appId=shop&path=pages%2Fdetail%2Findex.html&channel=draft&id=42",
        )
        .unwrap()
        .unwrap();
        assert_eq!(target.appid, "shop");
        assert_eq!(target.path, "pages/detail/index.html");
        assert_eq!(target.query, "id=42");
        assert_eq!(target.release_type, Channel::Draft);
    }

    #[test]
    fn parses_path_form() {
        let target = parse("https://www.lingxia.app/lxapp/shop/pages/detail?id=42&channel=preview")
            .unwrap()
            .unwrap();
        assert_eq!(target.appid, "shop");
        assert_eq!(target.path, "pages/detail");
        assert_eq!(target.query, "id=42");
        assert_eq!(target.release_type, Channel::Preview);
    }

    #[test]
    fn path_form_keeps_appid_and_path_query_params() {
        let target = parse(
            "https://www.lingxia.app/lxapp/shop/pages/detail?appId=cart&path=pages%2Fcheckout&id=42",
        )
        .unwrap()
        .unwrap();
        assert_eq!(target.appid, "shop");
        assert_eq!(target.path, "pages/detail");
        assert_eq!(target.query, "appId=cart&path=pages%2Fcheckout&id=42");
    }

    #[test]
    fn release_type_query_is_forwarded_to_page() {
        let target = parse(
            "https://www.lingxia.app/lxapp/open?appId=shop&path=pages%2Fhome%2Findex.html&channel=preview&releaseType=developer",
        )
        .unwrap()
        .unwrap();
        assert_eq!(target.release_type, Channel::Preview);
        assert_eq!(target.query, "releaseType=developer");
    }

    #[test]
    fn rejects_invalid_env_version() {
        assert!(parse("https://www.lingxia.app/lxapp/open?appId=shop&channel=trial").is_err());
        assert!(parse("https://www.lingxia.app/lxapp/open?appId=shop&channel=develop").is_err());
        assert!(parse("https://www.lingxia.app/lxapp/open?appId=shop&channel=developer").is_err());
    }

    #[test]
    fn product_path_is_delivered_untouched() {
        let target = parse("https://www.lingxia.app/app/auth/reset-password?code=abc&email=a%40b")
            .unwrap()
            .unwrap();
        assert!(!target.lxapp_route);
        assert_eq!(target.appid, "");
        assert_eq!(target.path, "");
        assert_eq!(target.query, "code=abc&email=a%40b");
        assert_eq!(
            target.url,
            "https://www.lingxia.app/app/auth/reset-password?code=abc&email=a%40b"
        );
    }

    #[test]
    fn product_query_keeps_routing_names_and_bad_encoding() {
        // `path` and `envVersion` are the product's own params here, and a lone
        // `%` must not turn a real link into a rejection.
        let target =
            parse("https://www.lingxia.app/app/auth?path=/home&envVersion=trial&code=100%")
                .unwrap()
                .unwrap();
        assert_eq!(target.path, "");
        assert_eq!(target.release_type, default_channel());
        assert_eq!(target.query, "path=/home&envVersion=trial&code=100%");
    }

    #[test]
    fn fragment_stays_in_url_only() {
        let target = parse("https://www.lingxia.app/app/auth?code=1#token=x")
            .unwrap()
            .unwrap();
        assert_eq!(target.query, "code=1");
        assert_eq!(
            target.url,
            "https://www.lingxia.app/app/auth?code=1#token=x"
        );
    }

    #[test]
    fn root_url_is_a_product_path() {
        let target = parse("https://www.lingxia.app").unwrap().unwrap();
        assert!(!target.lxapp_route);
        assert_eq!(target.query, "");
    }

    #[test]
    fn root_url_with_query_or_fragment_keeps_its_host() {
        let target = parse("https://www.lingxia.app?ref=mail").unwrap().unwrap();
        assert!(!target.lxapp_route);
        assert_eq!(target.query, "ref=mail");

        let target = parse("https://www.lingxia.app#hero").unwrap().unwrap();
        assert_eq!(target.query, "");
        assert_eq!(target.url, "https://www.lingxia.app#hero");
    }

    #[test]
    fn rejects_open_subpath_instead_of_opening_home() {
        assert!(parse("https://www.lingxia.app/lxapp/open/?appId=shop").is_err());
        assert!(parse("https://www.lingxia.app/lxapp/open/extra").is_err());
    }

    #[test]
    fn rejects_invalid_percent_encoding_in_lxapp_namespace() {
        assert!(parse("https://www.lingxia.app/lxapp/open?appId=%GG").is_err());
    }

    #[test]
    fn runner_without_hosts_allows_any_host() {
        assert!(host_allowed_for("app.example.com", &[], true));
    }

    #[test]
    fn product_without_hosts_rejects() {
        assert!(!host_allowed_for("app.example.com", &[], false));
    }

    #[test]
    fn configured_hosts_must_match() {
        let hosts = vec!["app.example.com".to_string()];
        assert!(host_allowed_for("app.example.com", &hosts, false));
        assert!(host_allowed_for("APP.EXAMPLE.COM", &hosts, true));
        assert!(!host_allowed_for("evil.example", &hosts, false));
        assert!(!host_allowed_for("evil.example", &hosts, true));
    }
}
