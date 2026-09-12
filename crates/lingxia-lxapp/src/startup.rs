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

/// Parse query string into serde_json::Value
/// This is the centralized query parsing function used by both startup options and page navigation
pub fn parse_query_string(query_str: &str) -> Result<serde_json::Value, serde_json::Error> {
    if query_str.is_empty() {
        return Ok(serde_json::Value::Object(serde_json::Map::new()));
    }

    let mut query_map = serde_json::Map::new();
    for pair in query_str.split('&') {
        if let Some(eq_pos) = pair.find('=') {
            let key = &pair[..eq_pos];
            let value = &pair[eq_pos + 1..];
            let decoded_value =
                urlencoding::decode(value).unwrap_or(std::borrow::Cow::Borrowed(value));
            query_map.insert(
                key.to_string(),
                serde_json::Value::String(decoded_value.to_string()),
            );
        } else {
            query_map.insert(pair.to_string(), serde_json::Value::String("".to_string()));
        }
    }
    Ok(serde_json::Value::Object(query_map))
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
    match env_version.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => parse_channel(value),
        None => Ok(default_channel()),
    }
}

pub fn append_page_query(path: String, query: &Value) -> Result<String, String> {
    let Some(object) = query.as_object() else {
        return Err("query must be an object".to_string());
    };
    let mut entries = object.iter().collect::<Vec<_>>();
    entries.sort_unstable_by_key(|(key, _)| *key);
    let mut pairs = Vec::new();
    for (key, value) in entries {
        if value.is_null() {
            continue;
        }
        let value = match value {
            Value::String(value) => value.clone(),
            Value::Bool(value) => value.to_string(),
            Value::Number(value) => value.to_string(),
            other => other.to_string(),
        };
        pairs.push(format!(
            "{}={}",
            urlencoding::encode(key),
            urlencoding::encode(&value)
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
    /// object so home lxapp Logic can route with `lx.navigateTo`. `url` is the
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
    use super::{LxAppStartupOptions, parse_query_string};

    #[test]
    fn empty_query_is_an_options_object() {
        assert_eq!(parse_query_string("").unwrap(), serde_json::json!({}));
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
        assert_eq!(options.query, "compact=true&source=menu%20bar");
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
