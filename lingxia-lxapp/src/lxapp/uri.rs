use http::Uri;
use std::str::FromStr;
use urlencoding::decode;

use crate::page::Page;

pub(crate) const LX_SCHEME: &str = "lx";
pub(crate) const HOST_LXAPP: &str = "lxapp";
pub(crate) const HOST_PLUGIN: &str = "plugin";
pub(crate) const HOST_PROXY: &str = "proxy";
pub(crate) const HOST_ASSETS: &str = "assets";
pub(crate) const PLUGIN_PAGE_PATH_PREFIX: &str = "plugin/";

pub(crate) fn decode_lx_path(path_str: &str) -> String {
    decode(path_str)
        .map(|c| c.to_string())
        .unwrap_or_else(|_| path_str.to_string())
}

pub(crate) fn has_invalid_segment(path: &str) -> bool {
    path.split('/').any(|s| s == "." || s == "..")
}

/// Parse a lx://plugin/<name>/<path> URL into (plugin_name, page_path).
pub(crate) fn parse_plugin_url(url: &str) -> Option<(String, String)> {
    let uri = Uri::from_str(url).ok()?;
    if uri.scheme_str() != Some(LX_SCHEME) || uri.host() != Some(HOST_PLUGIN) {
        return None;
    }

    let rest = uri.path().trim_start_matches('/');
    let (plugin_name, page_path) = if let Some(idx) = rest.find('/') {
        (rest[..idx].to_string(), rest[idx + 1..].to_string())
    } else {
        (rest.to_string(), String::new())
    };

    if plugin_name.is_empty() {
        return None;
    }

    let page_path = page_path
        .trim_start_matches('/')
        .trim_end_matches(&[' ', '\t'][..])
        .to_string();

    Some((plugin_name, page_path))
}

/// Parse a lx://lxapp/<appid>/<path> URL into (appid, page_path).
pub(crate) fn parse_lxapp_url(url: &str) -> Option<(String, String)> {
    let uri = Uri::from_str(url).ok()?;
    if uri.scheme_str() != Some(LX_SCHEME) || uri.host() != Some(HOST_LXAPP) {
        return None;
    }

    let rest = uri.path().trim_start_matches('/');
    let (appid, page_path) = rest.split_once('/')?;
    if appid.is_empty() {
        return None;
    }
    let page_path = page_path
        .trim_start_matches('/')
        .trim_end_matches(&[' ', '\t'][..])
        .to_string();
    if page_path.is_empty() {
        return None;
    }

    Some((appid.to_string(), page_path))
}

/// Parse an internal plugin page path: `plugin/<name>/<path>`.
pub(crate) fn parse_plugin_page_path(path: &str) -> Option<(String, String)> {
    if !path.starts_with(PLUGIN_PAGE_PATH_PREFIX) {
        return None;
    }

    let rest = &path[PLUGIN_PAGE_PATH_PREFIX.len()..];
    let (plugin_name, page_path) = if let Some(idx) = rest.find('/') {
        (rest[..idx].to_string(), rest[idx + 1..].to_string())
    } else {
        (rest.to_string(), String::new())
    };

    if plugin_name.is_empty() {
        return None;
    }

    Some((plugin_name, page_path.trim_start_matches('/').to_string()))
}

/// Build an internal plugin page path: `plugin/<name>` or `plugin/<name>/<path>`.
pub(crate) fn build_plugin_page_path(plugin_name: &str, page_path: &str) -> String {
    let name = plugin_name.trim_matches('/');
    let path = page_path.trim_matches('/');
    if path.is_empty() {
        format!("{}{}", PLUGIN_PAGE_PATH_PREFIX, name)
    } else {
        format!("{}{}/{}", PLUGIN_PAGE_PATH_PREFIX, name, path)
    }
}

pub(crate) fn strip_base_dir(
    page: &Page,
    normalized: &str,
    expected_host: &str,
    expected_owner: &str,
) -> Option<String> {
    let base_uri = Uri::from_str(&page.base_url()).ok()?;
    if base_uri.host() != Some(expected_host) {
        return None;
    }
    let base_path = base_uri.path().trim_start_matches('/');
    let (owner, rest) = base_path.split_once('/')?;
    if owner != expected_owner {
        return None;
    }
    let idx = rest.rfind('/')?;
    let base_dir = &rest[..idx];
    let prefix = format!("{}/", base_dir.trim_matches('/'));
    if normalized.starts_with(&prefix) {
        let stripped = normalized[prefix.len()..].trim_start_matches('/');
        if !stripped.is_empty() {
            return Some(stripped.to_string());
        }
    }
    None
}
