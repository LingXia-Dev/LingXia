use crate::lxapp::Channel;
use crate::{LxApp, LxAppError};
use lingxia_platform::traits::app_runtime::LxAppOpenMode;
use lingxia_update::default_channel;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Default)]
pub enum Scene {
    #[default]
    System = 8000,
    NavigateTo = 8001,
    NavigateBack = 8002,
    AppLink = 8003,
}

impl From<i32> for Scene {
    fn from(n: i32) -> Self {
        match n {
            8000 => Scene::System,
            8001 => Scene::NavigateTo,
            8002 => Scene::NavigateBack,
            8003 => Scene::AppLink,
            _ => Scene::System,
        }
    }
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct LxAppStartupOptions {
    pub path: String,
    pub query: String,
    /// Author-facing page selector. Resolved against `lxapp.json` when the app
    /// opens; native renderers continue to receive the resolved internal path.
    #[serde(skip)]
    pub page: Option<String>,
    pub release_type: Channel,
    pub scene: Scene,
    /// Original inbound URL for `Scene::AppLink`. Cleared with the scene once
    /// Logic has seen it, so a later plain `onShow` carries no stale link.
    #[serde(skip)]
    pub link_url: String,
    #[serde(skip)]
    pub open_mode: LxAppOpenMode,
    #[serde(skip)]
    pub panel_id: String,
}

impl serde::Serialize for LxAppStartupOptions {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.launch_options_value().serialize(serializer)
    }
}

/// Query key carrying a page's non-string params as one JSON object, so a
/// number, boolean, null, array or object reaches `onLoad` with its type.
const TYPED_PARAMS_KEY: &str = "__lxparams";

/// Largest encoded typed-params value. Params are persisted with the stack
/// for restart replay; anything bigger belongs in storage, passed by key.
pub const PAGE_PARAMS_MAX_BYTES: usize = 8 * 1024;

/// Parse a page query into the object `onLoad` receives. Plain pairs are
/// strings; typed params win over a plain pair of the same name.
pub fn parse_query_string(query_str: &str) -> Result<serde_json::Value, serde_json::Error> {
    let mut query_map = serde_json::Map::new();
    let mut typed = Vec::new();
    for pair in query_str.split('&').filter(|pair| !pair.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        let decoded_value = urlencoding::decode(value).unwrap_or(std::borrow::Cow::Borrowed(value));
        if key == TYPED_PARAMS_KEY {
            // Malformed typed params (an outside URL can carry the key too)
            // are dropped instead of discarding the rest of the query.
            if let Ok(Value::Object(params)) = serde_json::from_str::<Value>(&decoded_value) {
                typed.push(params);
            }
            continue;
        }
        query_map.insert(key.to_string(), Value::String(decoded_value.to_string()));
    }
    for params in typed {
        query_map.extend(params);
    }
    Ok(Value::Object(query_map))
}

/// Splits a full URL (path?query) into path and raw query string (without the '?').
pub fn split_path_query(url: &str) -> (String, Option<String>) {
    if let Some(idx) = url.find('?') {
        let (path, query) = url.split_at(idx);
        (path.to_string(), Some(query[1..].to_string()))
    } else {
        (url.to_string(), None)
    }
}

pub fn parse_channel(tag: &str) -> Result<Channel, String> {
    Channel::parse(tag)
}

pub fn parse_optional_channel(env_version: Option<&str>) -> Result<Channel, String> {
    Ok(parse_requested_channel(env_version)?.unwrap_or_else(default_channel))
}

/// An explicit channel selector, `None` when omitted or blank.
pub fn parse_requested_channel(env_version: Option<&str>) -> Result<Option<Channel>, String> {
    env_version
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(parse_channel)
        .transpose()
}

/// Append page params to `path`. Strings stay readable `key=value` pairs;
/// every other JSON value travels in one typed-params object. `null` means
/// absent, as it always has here.
pub fn append_page_query(path: String, query: &Value) -> Result<String, String> {
    let Some(object) = query.as_object() else {
        return Err("query must be an object".to_string());
    };
    let mut entries = object.iter().collect::<Vec<_>>();
    entries.sort_unstable_by_key(|(key, _)| *key);
    let mut pairs = Vec::new();
    let mut typed = serde_json::Map::new();
    for (key, value) in entries {
        if key == TYPED_PARAMS_KEY {
            return Err(format!("query key '{TYPED_PARAMS_KEY}' is reserved"));
        }
        match value {
            Value::Null => {}
            Value::String(value) => pairs.push(format!(
                "{}={}",
                urlencoding::encode(key),
                urlencoding::encode(value)
            )),
            other => {
                typed.insert(key.clone(), other.clone());
            }
        }
    }
    if !typed.is_empty() {
        let encoded = Value::Object(typed).to_string();
        if encoded.len() > PAGE_PARAMS_MAX_BYTES {
            return Err(format!(
                "page params are {} bytes as JSON; the limit is {PAGE_PARAMS_MAX_BYTES}. Keep large values in storage and pass their key.",
                encoded.len()
            ));
        }
        pairs.push(format!(
            "{TYPED_PARAMS_KEY}={}",
            urlencoding::encode(&encoded)
        ));
    }
    if pairs.is_empty() {
        return Ok(path);
    }
    let separator = if path.contains('?') { '&' } else { '?' };
    Ok(format!("{path}{separator}{}", pairs.join("&")))
}

impl LxAppStartupOptions {
    /// Creates a new `LxAppStartupOptions` from a path that may contain a query string.
    pub fn new(path_with_query: &str) -> Self {
        let (path, query_str) = if let Some(idx) = path_with_query.find('?') {
            let (p, q) = path_with_query.split_at(idx);
            (p, &q[1..])
        } else {
            (path_with_query, "")
        };

        Self {
            path: path.to_string(),
            query: query_str.to_string(),
            release_type: default_channel(),
            open_mode: LxAppOpenMode::Normal,
            panel_id: String::new(),
            ..Default::default()
        }
    }

    /// Creates startup options from the public page-name + query contract.
    /// Omit `page` to target the app's configured initial page.
    pub fn for_page(page: Option<&str>, query: Option<&Value>) -> Result<Self, String> {
        let page = page
            .map(str::trim)
            .map(|page| {
                if page.is_empty() {
                    Err("page must be a non-empty configured page name".to_string())
                } else {
                    Ok(page.to_string())
                }
            })
            .transpose()?;
        let query = match query {
            Some(query) => append_page_query(String::new(), query)?
                .strip_prefix('?')
                .unwrap_or_default()
                .to_string(),
            None => String::new(),
        };
        Ok(Self {
            page,
            query,
            ..Self::new("")
        })
    }

    /// Resolves an author-facing page selector into the internal URL consumed
    /// by native renderers. Query stays separate until this boundary.
    pub fn resolved_url(&self, app: &LxApp) -> Result<String, LxAppError> {
        let path = match self.page.as_deref() {
            Some(page) => app
                .find_page_path_by_name(page)
                .ok_or_else(|| LxAppError::ResourceNotFound(format!("page name: {page}")))?,
            None if self.path.is_empty() => app.initial_route(),
            None => self.path.clone(),
        };
        if self.query.is_empty() {
            Ok(path)
        } else {
            let separator = if path.contains('?') { '&' } else { '?' };
            Ok(format!("{path}{separator}{}", self.query))
        }
    }

    /// Sets the release type for the startup options.
    pub fn set_release_type(mut self, release_type: Channel) -> Self {
        self.release_type = release_type;
        self
    }

    /// Sets the `scene` for the startup options.
    pub fn set_scene(mut self, scene: Scene) -> Self {
        self.scene = scene;
        self
    }

    /// Sets the `query` for the startup options.
    pub fn set_query(mut self, query: String) -> Self {
        self.query = query;
        self
    }

    /// Sets the original inbound AppLink URL.
    pub fn set_link_url(mut self, link_url: String) -> Self {
        self.link_url = link_url;
        self
    }

    /// JS `AppLaunchOptions`: `{ path, query, scene, url }`. Query is a nested
    /// object so home lxapp Logic can route with `lx.nav.go`. `url` is the
    /// original inbound link and is present only for `scene: 8003`; `path` is
    /// the lxapp page, which for a product link is the initial page.
    pub fn launch_options_value(&self) -> Value {
        let query =
            parse_query_string(&self.query).unwrap_or_else(|_| Value::Object(Default::default()));
        let mut map = serde_json::Map::new();
        if !self.path.is_empty() {
            map.insert("path".to_string(), Value::String(self.path.clone()));
        }
        map.insert("query".to_string(), query);
        if !self.link_url.is_empty() {
            map.insert("url".to_string(), Value::String(self.link_url.clone()));
        }
        map.insert(
            "scene".to_string(),
            Value::Number(serde_json::Number::from(self.scene as u32)),
        );
        Value::Object(map)
    }

    pub fn launch_options_json(&self) -> String {
        self.launch_options_value().to_string()
    }

    /// Sets the open mode for the startup options.
    pub fn set_open_mode(mut self, open_mode: LxAppOpenMode) -> Self {
        self.open_mode = open_mode;
        self
    }

    /// Sets panel slot id (only used when open_mode is panel).
    pub fn set_panel_id(mut self, panel_id: String) -> Self {
        self.panel_id = panel_id;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::{
        LxAppStartupOptions, PAGE_PARAMS_MAX_BYTES, append_page_query, parse_query_string,
    };

    #[test]
    fn empty_query_is_an_options_object() {
        assert_eq!(parse_query_string("").unwrap(), serde_json::json!({}));
    }

    #[test]
    fn typed_params_survive_the_url_round_trip() {
        let params = serde_json::json!({
            "id": "42",
            "count": 3,
            "draft": true,
            "tags": ["a", "b&c"],
            "filter": { "from": "2026-01-01" },
        });
        let url = append_page_query("pages/order/index".to_string(), &params).unwrap();
        assert!(
            url.starts_with("pages/order/index?id=42&__lxparams="),
            "{url}"
        );
        let (_, query) = url.split_once('?').unwrap();
        assert_eq!(parse_query_string(query).unwrap(), params);
        // Re-encoding what onLoad received yields the same URL.
        assert_eq!(
            append_page_query("pages/order/index".to_string(), &params).unwrap(),
            url
        );
    }

    #[test]
    fn typed_params_win_over_a_plain_pair_and_have_a_size_limit() {
        let typed = urlencoding::encode(r#"{"id":7}"#);
        assert_eq!(
            parse_query_string(&format!("id=7&__lxparams={typed}")).unwrap(),
            serde_json::json!({ "id": 7 })
        );

        let big = serde_json::json!({ "blob": vec![0; PAGE_PARAMS_MAX_BYTES] });
        let error = append_page_query("p".to_string(), &big).unwrap_err();
        assert!(error.contains("the limit is 8192"), "{error}");
        assert!(
            append_page_query("p".to_string(), &serde_json::json!({ "__lxparams": 1 })).is_err()
        );
        assert_eq!(
            parse_query_string("id=7&__lxparams=%7Bnot-json").unwrap(),
            serde_json::json!({ "id": "7" })
        );
    }

    #[test]
    fn page_startup_keeps_name_and_encodes_query_separately() {
        let options = LxAppStartupOptions::for_page(
            Some(" tray "),
            Some(&serde_json::json!({ "source": "menu bar", "compact": true })),
        )
        .unwrap();

        assert_eq!(options.page.as_deref(), Some("tray"));
        assert_eq!(options.path, "");
        assert_eq!(
            options.query,
            "source=menu%20bar&__lxparams=%7B%22compact%22%3Atrue%7D"
        );
    }

    #[test]
    fn launch_options_nests_query_and_scene() {
        let options = LxAppStartupOptions::new("pages/home/index")
            .set_query("page=order&id=42".to_string())
            .set_scene(super::Scene::AppLink);
        assert_eq!(
            options.launch_options_value(),
            serde_json::json!({
                "path": "pages/home/index",
                "query": { "page": "order", "id": "42" },
                "scene": 8003,
            })
        );
    }

    #[test]
    fn launch_options_carry_the_inbound_link_url() {
        let options = LxAppStartupOptions::new("pages/home/index")
            .set_query("code=1".to_string())
            .set_scene(super::Scene::AppLink)
            .set_link_url("https://app.example.com/app/auth/reset?code=1#t".to_string());
        assert_eq!(
            options.launch_options_value()["url"],
            serde_json::json!("https://app.example.com/app/auth/reset?code=1#t")
        );
    }
}
