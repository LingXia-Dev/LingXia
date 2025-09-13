use serde::{Deserialize, Serialize, Serializer, ser::SerializeMap};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum LxAppMode {
    Develop,
    Trial,
    Release,
}

impl Default for LxAppMode {
    fn default() -> Self {
        LxAppMode::Release
    }
}

impl From<&str> for LxAppMode {
    fn from(s: &str) -> Self {
        match s {
            "develop" => LxAppMode::Develop,
            "trial" => LxAppMode::Trial,
            _ => LxAppMode::Release,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum Scene {
    System = 8000,
    NavigateTo = 8001,
    NavigateBack = 8002,
    AppLink = 8003,
}

impl Default for Scene {
    fn default() -> Self {
        Scene::System
    }
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
    pub mode: LxAppMode,
    pub scene: Scene,
}

impl serde::Serialize for LxAppStartupOptions {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(None)?;

        map.serialize_entry("path", &self.path)?;
        map.serialize_entry("scene", &(self.scene as u32))?;

        if let Ok(query_value) = parse_query_string(&self.query) {
            if let Some(query_map) = query_value.as_object() {
                for (k, v) in query_map {
                    map.serialize_entry(k, v)?;
                }
            }
        }

        map.end()
    }
}

/// Parse query string into serde_json::Value
/// This is the centralized query parsing function used by both startup options and page navigation
pub fn parse_query_string(query_str: &str) -> Result<serde_json::Value, serde_json::Error> {
    if query_str.is_empty() {
        return Ok(serde_json::Value::Null);
    }

    let mut query_map = serde_json::Map::new();
    for pair in query_str.split('&') {
        if let Some(eq_pos) = pair.find('=') {
            let key = &pair[..eq_pos];
            let value = &pair[eq_pos + 1..];
            let decoded_value =
                urlencoding::decode(value).unwrap_or_else(|_| std::borrow::Cow::Borrowed(value));
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
            ..Default::default()
        }
    }

    /// Sets the `mode` for the startup options.
    pub fn set_mode(mut self, mode: LxAppMode) -> Self {
        self.mode = mode;
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
}
