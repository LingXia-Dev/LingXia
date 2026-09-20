//! Host-registered internal navigation routes.
//!
//! Registration happens once during startup and is then sealed: a route is a
//! trusted host declaration, never something a notification payload, a guest
//! lxapp, or a remote caller can install.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use serde_json::{Map, Value};

use super::dispatch::NavigationRequest;
use super::target::NavigationError;

/// What a parameter is allowed to be. `Json` still obeys the shared size and
/// nesting limits — it is a small structured value, not an arbitrary payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteParamKind {
    String,
    Number,
    Boolean,
    Json,
}

impl RouteParamKind {
    fn accepts(self, value: &Value) -> bool {
        match self {
            Self::String => value.is_string(),
            Self::Number => value.is_number(),
            Self::Boolean => value.is_boolean(),
            Self::Json => true,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Number => "number",
            Self::Boolean => "boolean",
            Self::Json => "json",
        }
    }
}

#[derive(Debug, Clone)]
pub struct RouteParam {
    pub name: String,
    pub kind: RouteParamKind,
    pub required: bool,
}

impl RouteParam {
    pub fn string(name: impl Into<String>) -> Self {
        Self::new(name, RouteParamKind::String)
    }

    pub fn number(name: impl Into<String>) -> Self {
        Self::new(name, RouteParamKind::Number)
    }

    pub fn boolean(name: impl Into<String>) -> Self {
        Self::new(name, RouteParamKind::Boolean)
    }

    pub fn json(name: impl Into<String>) -> Self {
        Self::new(name, RouteParamKind::Json)
    }

    fn new(name: impl Into<String>, kind: RouteParamKind) -> Self {
        Self {
            name: name.into(),
            kind,
            required: true,
        }
    }

    pub fn optional(mut self) -> Self {
        self.required = false;
        self
    }
}

/// Opens the registered location. It navigates and nothing else: a side effect
/// the user has to approve belongs behind the screen this opens, not here.
pub type RouteHandler =
    Arc<dyn Fn(&NavigationRequest) -> Result<(), NavigationError> + Send + Sync>;

pub struct NavigationRoute {
    name: String,
    params: Vec<RouteParam>,
    handler: RouteHandler,
}

impl NavigationRoute {
    pub fn new<F>(name: impl Into<String>, handler: F) -> Self
    where
        F: Fn(&NavigationRequest) -> Result<(), NavigationError> + Send + Sync + 'static,
    {
        Self {
            name: name.into(),
            params: Vec::new(),
            handler: Arc::new(handler),
        }
    }

    pub fn param(mut self, param: RouteParam) -> Self {
        self.params.push(param);
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    fn check(&self, params: &Map<String, Value>) -> Result<(), NavigationError> {
        for declared in &self.params {
            match params.get(&declared.name) {
                Some(value) if declared.kind.accepts(value) => {}
                Some(_) => {
                    return Err(NavigationError::invalid(format!(
                        "route '{}' expects {} to be a {}",
                        self.name,
                        declared.name,
                        declared.kind.as_str()
                    )));
                }
                None if declared.required => {
                    return Err(NavigationError::invalid(format!(
                        "route '{}' requires {}",
                        self.name, declared.name
                    )));
                }
                None => {}
            }
        }
        for key in params.keys() {
            if !self.params.iter().any(|param| &param.name == key) {
                return Err(NavigationError::invalid(format!(
                    "route '{}' has no parameter {key}",
                    self.name
                )));
            }
        }
        Ok(())
    }
}

/// Collects a host's routes before the registry is sealed.
#[derive(Default)]
pub struct NavigationRoutes {
    routes: Vec<NavigationRoute>,
}

impl NavigationRoutes {
    pub fn new() -> Self {
        Self::default()
    }

    /// A duplicate name fails here, at registration, rather than resolving to
    /// whichever handler happened to win.
    pub fn add(&mut self, route: NavigationRoute) -> Result<(), String> {
        if self
            .routes
            .iter()
            .any(|existing| existing.name == route.name)
        {
            return Err(format!(
                "navigation route '{}' is already registered",
                route.name
            ));
        }
        self.routes.push(route);
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.routes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }
}

static ROUTES: OnceLock<HashMap<String, NavigationRoute>> = OnceLock::new();

/// Seal the registry. Called once from the host bootstrap after every addon
/// has contributed; later calls fail rather than swap the table.
pub fn install(routes: NavigationRoutes) -> Result<(), String> {
    let table: HashMap<String, NavigationRoute> = routes
        .routes
        .into_iter()
        .map(|route| (route.name.clone(), route))
        .collect();
    ROUTES
        .set(table)
        .map_err(|_| "navigation routes were already installed".to_string())
}

pub fn is_sealed() -> bool {
    ROUTES.get().is_some()
}

pub fn route_names() -> Vec<String> {
    ROUTES
        .get()
        .map(|routes| {
            let mut names: Vec<String> = routes.keys().cloned().collect();
            names.sort();
            names
        })
        .unwrap_or_default()
}

fn lookup(name: &str) -> Option<&'static NavigationRoute> {
    ROUTES.get()?.get(name)
}

/// Name and parameters against the registered schema. Run at publish time and
/// again at activation: a route can disappear between the two.
pub fn validate(name: &str, params: &Map<String, Value>) -> Result<(), NavigationError> {
    if !is_sealed() {
        return Err(NavigationError::unavailable(
            "navigation routes are not registered yet",
        ));
    }
    let route = lookup(name)
        .ok_or_else(|| NavigationError::invalid(format!("unknown navigation route '{name}'")))?;
    route.check(params)
}

pub(super) fn open(request: &NavigationRequest, name: &str) -> Result<(), NavigationError> {
    let route = lookup(name).ok_or_else(|| {
        NavigationError::unavailable(format!("navigation route '{name}' is no longer registered"))
    })?;
    (route.handler)(request)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(text: &str) -> Map<String, Value> {
        serde_json::from_str(text).unwrap()
    }

    fn route() -> NavigationRoute {
        NavigationRoute::new("downloads.detail", |_| Ok(()))
            .param(RouteParam::string("downloadId"))
            .param(RouteParam::boolean("reveal").optional())
    }

    #[test]
    fn required_params_must_be_present_and_typed() {
        let route = route();
        assert!(route.check(&params(r#"{"downloadId":"1"}"#)).is_ok());
        assert!(
            route
                .check(&params(r#"{"downloadId":"1","reveal":true}"#))
                .is_ok()
        );
        assert!(route.check(&params("{}")).is_err());
        assert!(route.check(&params(r#"{"downloadId":1}"#)).is_err());
        assert!(
            route
                .check(&params(r#"{"downloadId":"1","reveal":"yes"}"#))
                .is_err()
        );
    }

    #[test]
    fn undeclared_params_are_rejected() {
        assert!(
            route()
                .check(&params(r#"{"downloadId":"1","extra":"x"}"#))
                .is_err()
        );
    }

    #[test]
    fn duplicate_names_fail_at_registration() {
        let mut routes = NavigationRoutes::new();
        assert!(routes.add(route()).is_ok());
        assert!(routes.add(route()).is_err());
        assert_eq!(routes.len(), 1);
    }
}
