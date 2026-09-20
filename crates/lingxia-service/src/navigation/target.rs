//! The one navigation target every product entry point resolves to.

use serde_json::{Map, Value};

/// Route names are a persisted contract, so they stay short enough to survive
/// every platform payload that carries a reference to one.
pub const MAX_ROUTE_NAME_CHARS: usize = 64;
pub const MAX_ROUTE_PARAM_COUNT: usize = 16;
pub const MAX_ROUTE_PARAMS_BYTES: usize = 2048;
/// Nested containers, counting the `params` object itself: `{ "orderId": "42" }`
/// is 1, `{ "filter": { "tag": "x" } }` is 2. Params carry resource ids, not
/// business objects, so one level of structure is the whole budget.
pub const MAX_ROUTE_PARAM_DEPTH: usize = 2;

/// Where a tap, menu item, or inbound link goes.
///
/// `Page` and `App` are the same contract as `lx.navigateTo` /
/// `lx.navigateToApp`: a configured page name and a query, ordinary scene.
/// `Route` is a host-registered location that is not a page. `AppLink` is an
/// HTTPS product URL on a configured host, delivered as `scene === 8003`.
#[derive(Debug, Clone, PartialEq)]
pub enum NavigationTarget {
    /// Bring the product forward and go nowhere in particular.
    Activate,
    /// A page of the home lxapp. `page` is the configured name, never a path.
    Page {
        page: String,
        query: Map<String, Value>,
    },
    /// A page of another lxapp. Omit `page` for that app's initial page.
    App {
        appid: String,
        page: Option<String>,
        query: Map<String, Value>,
    },
    Route {
        name: String,
        params: Map<String, Value>,
    },
    AppLink {
        url: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationError {
    /// The request never described a target this build can open. Callers turn
    /// this into a parameter error.
    InvalidTarget(String),
    /// Well-formed, but nothing can open it now: the route was removed, the
    /// token expired, the resource is gone.
    Unavailable(String),
    Internal(String),
}

impl NavigationError {
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::InvalidTarget(message.into())
    }

    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::Unavailable(message.into())
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }

    pub fn message(&self) -> &str {
        match self {
            Self::InvalidTarget(message) | Self::Unavailable(message) | Self::Internal(message) => {
                message
            }
        }
    }
}

impl std::fmt::Display for NavigationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for NavigationError {}

impl NavigationTarget {
    pub fn route(name: impl Into<String>) -> Self {
        Self::Route {
            name: name.into(),
            params: Map::new(),
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::Activate => "activate",
            Self::Page { .. } => "page",
            Self::App { .. } => "app",
            Self::Route { .. } => "route",
            Self::AppLink { .. } => "appLink",
        }
    }

    /// Safe for logs: names and parameter keys, never their values.
    pub fn describe(&self) -> String {
        match self {
            Self::Activate => "activate".to_string(),
            Self::Page { page, query } => format!("page {page}({})", query_keys(query)),
            Self::App { appid, page, query } => match page {
                Some(page) => format!("app {appid}/{page}({})", query_keys(query)),
                None => format!("app {appid}({})", query_keys(query)),
            },
            Self::Route { name, params } => {
                format!("route {name}({})", query_keys(params))
            }
            Self::AppLink { .. } => "appLink".to_string(),
        }
    }

    pub fn to_json(&self) -> Value {
        match self {
            Self::Activate => serde_json::json!({ "kind": "activate" }),
            Self::Page { page, query } => {
                let mut object = serde_json::json!({ "kind": "page", "page": page });
                if !query.is_empty() {
                    object["query"] = Value::Object(query.clone());
                }
                object
            }
            Self::App { appid, page, query } => {
                let mut object = serde_json::json!({ "kind": "app", "appId": appid });
                if let Some(page) = page {
                    object["page"] = Value::from(page.clone());
                }
                if !query.is_empty() {
                    object["query"] = Value::Object(query.clone());
                }
                object
            }
            Self::Route { name, params } => serde_json::json!({
                "kind": "route",
                "name": name,
                "params": Value::Object(params.clone()),
            }),
            Self::AppLink { url } => serde_json::json!({ "kind": "appLink", "url": url }),
        }
    }

    /// Strict: a branch carries its own fields and no others, so a mixed
    /// target fails instead of silently taking one half.
    pub fn from_json(value: &Value) -> Result<Self, NavigationError> {
        let object = value
            .as_object()
            .ok_or_else(|| NavigationError::invalid("target must be an object"))?;
        let kind = object
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| NavigationError::invalid("target.kind is required"))?;
        match kind {
            "activate" => {
                reject_extra_keys(object, &["kind"])?;
                Ok(Self::Activate)
            }
            "page" => {
                reject_extra_keys(object, &["kind", "page", "query"])?;
                let page = object
                    .get("page")
                    .and_then(Value::as_str)
                    .ok_or_else(|| NavigationError::invalid("target.page is required"))?;
                let target = Self::Page {
                    page: validate_page_name(page)?,
                    query: parse_page_query(object.get("query"))?,
                };
                target.check_shape()?;
                Ok(target)
            }
            "app" => {
                reject_extra_keys(object, &["kind", "appId", "page", "query"])?;
                let appid = object
                    .get("appId")
                    .and_then(Value::as_str)
                    .filter(|id| !id.is_empty())
                    .ok_or_else(|| NavigationError::invalid("target.appId is required"))?;
                let page = match object.get("page") {
                    None | Some(Value::Null) => None,
                    Some(Value::String(page)) => Some(validate_page_name(page)?),
                    Some(_) => {
                        return Err(NavigationError::invalid("target.page must be a string"));
                    }
                };
                let target = Self::App {
                    appid: appid.to_string(),
                    page,
                    query: parse_page_query(object.get("query"))?,
                };
                target.check_shape()?;
                Ok(target)
            }
            "route" => {
                reject_extra_keys(object, &["kind", "name", "params"])?;
                let name = object
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| NavigationError::invalid("target.name is required"))?;
                let params = match object.get("params") {
                    None | Some(Value::Null) => Map::new(),
                    Some(Value::Object(params)) => params.clone(),
                    Some(_) => {
                        return Err(NavigationError::invalid("target.params must be an object"));
                    }
                };
                let target = Self::Route {
                    name: validate_route_name(name)?,
                    params,
                };
                target.check_shape()?;
                Ok(target)
            }
            "appLink" => {
                reject_extra_keys(object, &["kind", "url"])?;
                let url = object
                    .get("url")
                    .and_then(Value::as_str)
                    .filter(|url| !url.is_empty())
                    .ok_or_else(|| NavigationError::invalid("target.url is required"))?;
                Ok(Self::AppLink {
                    url: url.to_string(),
                })
            }
            other => Err(NavigationError::invalid(format!(
                "target.kind must be activate, page, app, route, or appLink, not {other:?}"
            ))),
        }
    }

    /// Size, nesting, and count limits. Independent of the route schema, so an
    /// oversized payload fails the same way whatever route it names.
    pub fn check_shape(&self) -> Result<(), NavigationError> {
        match self {
            Self::Route { params, .. } => check_map_budget(params, "params", true),
            Self::Page { query, .. } | Self::App { query, .. } => {
                check_map_budget(query, "query", false)
            }
            Self::Activate | Self::AppLink { .. } => Ok(()),
        }
    }
}

fn reject_extra_keys(object: &Map<String, Value>, allowed: &[&str]) -> Result<(), NavigationError> {
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(NavigationError::invalid(format!(
                "target.{key} does not belong to this kind"
            )));
        }
    }
    Ok(())
}

fn query_keys(map: &Map<String, Value>) -> String {
    map.keys().map(String::as_str).collect::<Vec<_>>().join(",")
}

fn parse_page_query(value: Option<&Value>) -> Result<Map<String, Value>, NavigationError> {
    match value {
        None | Some(Value::Null) => Ok(Map::new()),
        Some(Value::Object(query)) => {
            for (key, entry) in query {
                match entry {
                    Value::String(_) | Value::Number(_) | Value::Bool(_) | Value::Null => {}
                    _ => {
                        return Err(NavigationError::invalid(format!(
                            "target.query.{key} must be a string, number, boolean, or null"
                        )));
                    }
                }
            }
            Ok(query.clone())
        }
        Some(_) => Err(NavigationError::invalid("target.query must be an object")),
    }
}

fn validate_page_name(page: &str) -> Result<String, NavigationError> {
    let page = page.trim();
    if page.is_empty() || page.chars().count() > MAX_ROUTE_NAME_CHARS {
        return Err(NavigationError::invalid(format!(
            "target.page must be 1–{MAX_ROUTE_NAME_CHARS} characters"
        )));
    }
    if page.contains('/') || page.contains('?') || page.contains('#') {
        return Err(NavigationError::invalid(
            "target.page must be a configured page name, not a path",
        ));
    }
    Ok(page.to_string())
}

fn check_map_budget(
    map: &Map<String, Value>,
    field: &str,
    allow_one_object_level: bool,
) -> Result<(), NavigationError> {
    if map.len() > MAX_ROUTE_PARAM_COUNT {
        return Err(NavigationError::invalid(format!(
            "target.{field} takes at most {MAX_ROUTE_PARAM_COUNT} entries"
        )));
    }
    for (key, value) in map {
        let depth = depth_of(value);
        let max = if allow_one_object_level {
            MAX_ROUTE_PARAM_DEPTH
        } else {
            1
        };
        if 1 + depth > max {
            return Err(NavigationError::invalid(format!(
                "target.{field}.{key} nests deeper than {max} levels"
            )));
        }
    }
    let encoded = serde_json::to_string(&Value::Object(map.clone()))
        .map_err(|error| NavigationError::invalid(format!("target.{field}: {error}")))?;
    if encoded.len() > MAX_ROUTE_PARAMS_BYTES {
        return Err(NavigationError::invalid(format!(
            "target.{field} serializes to {} bytes, over the {MAX_ROUTE_PARAMS_BYTES} byte limit",
            encoded.len()
        )));
    }
    Ok(())
}

fn validate_route_name(name: &str) -> Result<String, NavigationError> {
    if name.is_empty() || name.chars().count() > MAX_ROUTE_NAME_CHARS {
        return Err(NavigationError::invalid(format!(
            "target.name must be 1–{MAX_ROUTE_NAME_CHARS} characters"
        )));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    {
        return Err(NavigationError::invalid(
            "target.name may use letters, digits, '.', '_', and '-' only",
        ));
    }
    Ok(name.to_string())
}

/// Nested containers in `value`. A scalar is 0, `{ "a": 1 }` is 1.
fn depth_of(value: &Value) -> usize {
    match value {
        Value::Array(items) => 1 + items.iter().map(depth_of).max().unwrap_or(0),
        Value::Object(entries) => 1 + entries.values().map(depth_of).max().unwrap_or(0),
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<NavigationTarget, NavigationError> {
        NavigationTarget::from_json(&serde_json::from_str(text).unwrap())
    }

    #[test]
    fn activate_takes_no_other_fields() {
        assert_eq!(
            parse(r#"{"kind":"activate"}"#).unwrap(),
            NavigationTarget::Activate
        );
        assert!(parse(r#"{"kind":"activate","url":"https://x.test/"}"#).is_err());
    }

    #[test]
    fn mixed_branches_are_rejected() {
        assert!(parse(r#"{"kind":"route","name":"a","url":"https://x.test/"}"#).is_err());
        assert!(parse(r#"{"kind":"appLink","url":"https://x.test/","name":"a"}"#).is_err());
        assert!(parse(r#"{"kind":"page","page":"order","url":"https://x.test/"}"#).is_err());
        assert!(parse(r#"{"kind":"app","appId":"shop","name":"a"}"#).is_err());
    }

    #[test]
    fn page_takes_a_configured_name_and_scalar_query() {
        let target = parse(r#"{"kind":"page","page":"order","query":{"orderId":"42"}}"#).unwrap();
        assert_eq!(
            target,
            NavigationTarget::Page {
                page: "order".into(),
                query: serde_json::json!({ "orderId": "42" })
                    .as_object()
                    .cloned()
                    .unwrap(),
            }
        );
        assert!(parse(r#"{"kind":"page","page":"/pages/order/index"}"#).is_err());
        assert!(parse(r#"{"kind":"page","page":"order","query":{"filter":{"tag":"x"}}}"#).is_err());
    }

    #[test]
    fn app_page_is_optional() {
        let target = parse(r#"{"kind":"app","appId":"lingxia-chat"}"#).unwrap();
        assert_eq!(
            target,
            NavigationTarget::App {
                appid: "lingxia-chat".into(),
                page: None,
                query: Map::new(),
            }
        );
        assert!(parse(r#"{"kind":"app","appId":"chat","page":"room?x=1"}"#).is_err());
    }

    #[test]
    fn route_params_default_to_empty() {
        let target = parse(r#"{"kind":"route","name":"downloads.detail"}"#).unwrap();
        assert_eq!(target, NavigationTarget::route("downloads.detail"));
    }

    #[test]
    fn route_name_is_restricted() {
        assert!(parse(r#"{"kind":"route","name":""}"#).is_err());
        assert!(parse(r#"{"kind":"route","name":"pages/detail?x=1"}"#).is_err());
        assert!(parse(r#"{"kind":"route","name":"orders.detail"}"#).is_ok());
    }

    #[test]
    fn params_respect_count_depth_and_size() {
        assert!(parse(r#"{"kind":"route","name":"a","params":{"x":{"y":"z"}}}"#).is_ok());
        assert!(parse(r#"{"kind":"route","name":"a","params":{"x":{"y":{"z":1}}}}"#).is_err());
        assert!(parse(r#"{"kind":"route","name":"a","params":{"x":[["y"]]}}"#).is_err());

        let wide: String = (0..MAX_ROUTE_PARAM_COUNT + 1)
            .map(|index| format!("\"k{index}\":1"))
            .collect::<Vec<_>>()
            .join(",");
        assert!(
            parse(&format!(
                r#"{{"kind":"route","name":"a","params":{{{wide}}}}}"#
            ))
            .is_err()
        );

        let big = "x".repeat(MAX_ROUTE_PARAMS_BYTES);
        assert!(
            parse(&format!(
                r#"{{"kind":"route","name":"a","params":{{"k":"{big}"}}}}"#
            ))
            .is_err()
        );
    }

    #[test]
    fn json_round_trips() {
        for target in [
            NavigationTarget::Activate,
            NavigationTarget::Page {
                page: "order".into(),
                query: serde_json::json!({ "orderId": "42" })
                    .as_object()
                    .cloned()
                    .unwrap(),
            },
            NavigationTarget::App {
                appid: "lingxia-chat".into(),
                page: Some("room".into()),
                query: Map::new(),
            },
            NavigationTarget::Route {
                name: "orders.detail".into(),
                params: serde_json::json!({ "orderId": "42" })
                    .as_object()
                    .cloned()
                    .unwrap(),
            },
            NavigationTarget::AppLink {
                url: "https://app.example.com/orders/42".into(),
            },
        ] {
            let encoded = target.to_json();
            assert_eq!(NavigationTarget::from_json(&encoded).unwrap(), target);
        }
    }

    #[test]
    fn describe_keeps_param_values_out_of_logs() {
        let target = NavigationTarget::Route {
            name: "orders.detail".into(),
            params: serde_json::json!({ "orderId": "secret-42" })
                .as_object()
                .cloned()
                .unwrap(),
        };
        assert_eq!(target.describe(), "route orders.detail(orderId)");
        let page = NavigationTarget::Page {
            page: "order".into(),
            query: serde_json::json!({ "orderId": "secret-42" })
                .as_object()
                .cloned()
                .unwrap(),
        };
        assert_eq!(page.describe(), "page order(orderId)");
    }
}
