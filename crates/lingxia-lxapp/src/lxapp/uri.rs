use http::Uri;
use std::ops::Deref;
use std::path::{Component, Path};
use std::str::FromStr;
use urlencoding::{decode, encode};

use super::LxApp;

pub(crate) const LX_SCHEME: &str = "lx";
pub(crate) const HOST_LXAPP: &str = "lxapp";
pub(crate) const HOST_PLUGIN: &str = "plugin";
pub(crate) const HOST_ASSETS: &str = "assets";
pub(crate) const HOST_TEMP: &str = "temp";
pub(crate) const HOST_USER_CACHE: &str = "usercache";
pub(crate) const HOST_USER_DATA: &str = "userdata";
pub(crate) const PLUGIN_PAGE_PATH_PREFIX: &str = "plugin/";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LxUri(String);

impl LxUri {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl Deref for LxUri {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl AsRef<str> for LxUri {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::fmt::Display for LxUri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<LxUri> for String {
    fn from(value: LxUri) -> Self {
        value.into_string()
    }
}

impl FromStr for LxUri {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err("empty uri".to_string());
        }
        let uri = Uri::from_str(trimmed).map_err(|_| "invalid uri".to_string())?;
        if uri.scheme_str() != Some(LX_SCHEME) {
            return Err("invalid scheme".to_string());
        }
        if uri.host().is_none() {
            return Err("missing host".to_string());
        }
        Ok(Self(trimmed.to_string()))
    }
}

pub(crate) fn decode_lx_path(path_str: &str) -> String {
    decode(path_str)
        .map(|c| c.to_string())
        .unwrap_or_else(|_| path_str.to_string())
}

pub(crate) fn has_invalid_segment(path: &str) -> bool {
    path.split('/').any(|s| s == "." || s == "..")
}

fn encode_path_for_lx_uri(relative: &Path) -> Option<String> {
    let mut out: Vec<String> = Vec::new();
    for comp in relative.components() {
        match comp {
            Component::Normal(seg) => {
                let seg = seg.to_string_lossy();
                if seg.is_empty() || seg == "." || seg == ".." {
                    return None;
                }
                out.push(encode(&seg).to_string());
            }
            Component::CurDir | Component::ParentDir => return None,
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    if out.is_empty() {
        return None;
    }
    Some(out.join("/"))
}

pub(crate) fn try_convert_path_to_uri(path: &Path, app: &LxApp) -> Option<LxUri> {
    let cache_rel = path
        .strip_prefix(&app.user_cache_dir)
        .ok()
        .map(Path::to_path_buf);
    let data_rel = path
        .strip_prefix(&app.user_data_dir)
        .ok()
        .map(Path::to_path_buf);

    let (host, rel) = if let Some(rel) = cache_rel {
        (HOST_USER_CACHE, rel)
    } else if let Some(rel) = data_rel {
        (HOST_USER_DATA, rel)
    } else {
        // Fallback for canonical paths (strip_prefix is sensitive to normalization).
        let canonical_path = path.canonicalize().ok()?;
        let canonical_cache_base = app.user_cache_dir.canonicalize().ok();
        if let Some(base) = canonical_cache_base
            && let Ok(rel) = canonical_path.strip_prefix(&base)
        {
            (HOST_USER_CACHE, rel.to_path_buf())
        } else {
            let canonical_data_base = app.user_data_dir.canonicalize().ok()?;
            let rel = canonical_path
                .strip_prefix(&canonical_data_base)
                .ok()?
                .to_path_buf();
            (HOST_USER_DATA, rel)
        }
    };

    let encoded_rel = encode_path_for_lx_uri(&rel)?;
    LxUri::from_str(&format!("{}://{}/{}", LX_SCHEME, host, encoded_rel)).ok()
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
