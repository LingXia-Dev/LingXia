use crate::{LxAppStartupOptions, ReleaseType, Scene, parse_env_release_type};

const LXAPP_PREFIX: &str = "/lxapp/";
const OPEN_ACTION: &str = "open";

#[derive(Debug, Clone, PartialEq, Eq)]
struct AppLinkTarget {
    appid: String,
    path: String,
    query: String,
    release_type: ReleaseType,
}

pub fn handle_applink(url: &str) -> i32 {
    match parse_applink(url) {
        Ok(Some(target)) => {
            crate::info!(
                "AppLink accepted: appid={}, path={}, releaseType={}",
                target.appid,
                target.path,
                target.release_type
            );
            let appid = target.appid.clone();
            let options = LxAppStartupOptions::new(&target.path)
                .set_query(target.query.clone())
                .set_release_type(target.release_type)
                .set_scene(Scene::AppLink);
            let release_type = target.release_type;
            let _ = rong::RongExecutor::global().spawn(async move {
                if let Err(err) = crate::prepare_lxapp_open(&appid, release_type).await {
                    crate::warn!("AppLink prepare failed for {}: {}", appid, err);
                    return;
                }
                if let Err(err) = crate::open_lxapp(&appid, options) {
                    crate::warn!("AppLink open failed for {}: {}", appid, err);
                }
            });
            1
        }
        Ok(None) => {
            crate::debug!("AppLink ignored: {}", url);
            0
        }
        Err(err) => {
            crate::warn!("AppLink rejected: {} ({})", url, err);
            -1
        }
    }
}

fn parse_applink(url: &str) -> Result<Option<AppLinkTarget>, String> {
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

    let (url_path, raw_query) = split_path_query(path_and_query);
    let route = match parse_route(url_path)? {
        Some(route) => route,
        None => return Ok(None),
    };
    let uses_query_routing = route.appid.is_none();
    let query_parts = parse_query(raw_query, uses_query_routing)?;
    let appid = match route.appid {
        Some(appid) => appid,
        None => query_parts
            .appid
            .ok_or_else(|| "missing lxapp appId".to_string())?,
    };
    let path = match route.path {
        Some(path) => path,
        None => query_parts.path.unwrap_or_default(),
    };
    if appid.trim().is_empty() {
        return Err("empty lxapp appId".to_string());
    }

    Ok(Some(AppLinkTarget {
        appid,
        path,
        query: query_parts.page_query,
        release_type: query_parts.release_type,
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
        return Ok(None);
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
    match rest.find('/') {
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

fn split_path_query(value: &str) -> (&str, Option<&str>) {
    match value.find('?') {
        Some(index) => (&value[..index], Some(&value[index + 1..])),
        None => (value, None),
    }
}

fn host_allowed(host: &str) -> bool {
    let Some(config) = lingxia_app_context::app_config() else {
        return true;
    };
    let Some(app_links) = config.app_links.as_ref() else {
        return false;
    };
    if app_links.hosts.is_empty() {
        return false;
    }
    app_links
        .hosts
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(host))
}

struct QueryParts {
    release_type: ReleaseType,
    appid: Option<String>,
    path: Option<String>,
    page_query: String,
}

fn parse_query(raw_query: Option<&str>, include_routing: bool) -> Result<QueryParts, String> {
    let Some(raw_query) = raw_query else {
        return Ok(QueryParts {
            release_type: ReleaseType::Release,
            appid: None,
            path: None,
            page_query: String::new(),
        });
    };
    let mut release_type = ReleaseType::Release;
    let mut appid = None;
    let mut path = None;
    let mut page_params = Vec::new();
    for pair in raw_query.split('&').filter(|pair| !pair.is_empty()) {
        let (raw_key, raw_value) = match pair.split_once('=') {
            Some((key, value)) => (key, value),
            None => (pair, ""),
        };
        let key = decode_component(raw_key)?;
        if key == "envVersion" {
            release_type = parse_env_release_type(&decode_component(raw_value)?)?;
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
        let target = parse_applink("https://www.lingxia.app/lxapp/open?appId=com.example.shop")
            .unwrap()
            .unwrap();
        assert_eq!(target.appid, "com.example.shop");
        assert_eq!(target.path, "");
        assert_eq!(target.query, "");
        assert_eq!(target.release_type, ReleaseType::Release);
    }

    #[test]
    fn parses_open_page_and_strips_routing_query() {
        let target = parse_applink(
            "https://www.lingxia.app/lxapp/open?appId=com.example.shop&path=pages%2Fdetail%2Findex.html&envVersion=preview&id=42",
        )
        .unwrap()
        .unwrap();
        assert_eq!(target.appid, "com.example.shop");
        assert_eq!(target.path, "pages/detail/index.html");
        assert_eq!(target.query, "id=42");
        assert_eq!(target.release_type, ReleaseType::Preview);
    }

    #[test]
    fn parses_open_query_form() {
        let target = parse_applink(
            "https://www.lingxia.app/lxapp/open?appId=shop&path=pages%2Fdetail%2Findex.html&envVersion=develop&id=42",
        )
        .unwrap()
        .unwrap();
        assert_eq!(target.appid, "shop");
        assert_eq!(target.path, "pages/detail/index.html");
        assert_eq!(target.query, "id=42");
        assert_eq!(target.release_type, ReleaseType::Developer);
    }

    #[test]
    fn parses_path_form() {
        let target = parse_applink(
            "https://www.lingxia.app/lxapp/shop/pages/detail?id=42&envVersion=preview",
        )
        .unwrap()
        .unwrap();
        assert_eq!(target.appid, "shop");
        assert_eq!(target.path, "pages/detail");
        assert_eq!(target.query, "id=42");
        assert_eq!(target.release_type, ReleaseType::Preview);
    }

    #[test]
    fn path_form_keeps_appid_and_path_query_params() {
        let target = parse_applink(
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
        let target = parse_applink(
            "https://www.lingxia.app/lxapp/open?appId=shop&path=pages%2Fhome%2Findex.html&envVersion=preview&releaseType=developer",
        )
        .unwrap()
        .unwrap();
        assert_eq!(target.release_type, ReleaseType::Preview);
        assert_eq!(target.query, "releaseType=developer");
    }

    #[test]
    fn rejects_invalid_env_version() {
        assert!(
            parse_applink("https://www.lingxia.app/lxapp/open?appId=shop&envVersion=trial")
                .is_err()
        );
    }

    #[test]
    fn ignores_non_lxapp_paths() {
        assert!(
            parse_applink("https://www.lingxia.app/oauth/callback")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn rejects_invalid_percent_encoding() {
        assert!(parse_applink("https://www.lingxia.app/lxapp/open?appId=%GG").is_err());
    }
}
