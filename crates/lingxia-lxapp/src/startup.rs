use crate::lxapp::ReleaseType;
use lingxia_platform::traits::app_runtime::LxAppOpenMode;
use serde::{Deserialize, Serialize, Serializer, ser::SerializeMap};
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
    pub release_type: ReleaseType,
    pub scene: Scene,
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
        let mut map = serializer.serialize_map(None)?;

        map.serialize_entry("path", &self.path)?;
        map.serialize_entry("scene", &(self.scene as u32))?;

        if let Ok(query_value) = parse_query_string(&self.query)
            && let Some(query_map) = query_value.as_object()
        {
            for (k, v) in query_map {
                map.serialize_entry(k, v)?;
            }
        }

        map.end()
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

pub fn parse_env_release_type(tag: &str) -> Result<ReleaseType, String> {
    match tag.trim() {
        "release" => Ok(ReleaseType::Release),
        "preview" => Ok(ReleaseType::Preview),
        "develop" => Ok(ReleaseType::Developer),
        value => Err(format!("invalid envVersion: {value}")),
    }
}

pub fn parse_optional_env_release_type(env_version: Option<&str>) -> Result<ReleaseType, String> {
    match env_version.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => parse_env_release_type(value),
        None => Ok(ReleaseType::Release),
    }
}

pub fn append_page_query(path: String, query: &Value) -> Result<String, String> {
    let Some(object) = query.as_object() else {
        return Err("query must be an object".to_string());
    };
    let mut pairs = Vec::new();
    for (key, value) in object {
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
            release_type: ReleaseType::Release,
            open_mode: LxAppOpenMode::Normal,
            panel_id: String::new(),
            ..Default::default()
        }
    }

    /// Sets the release type for the startup options.
    pub fn set_release_type(mut self, release_type: ReleaseType) -> Self {
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
    use super::parse_query_string;

    #[test]
    fn empty_query_is_an_options_object() {
        assert_eq!(parse_query_string("").unwrap(), serde_json::json!({}));
    }
}
