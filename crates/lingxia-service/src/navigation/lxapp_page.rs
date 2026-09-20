//! `{ kind: 'page' | 'app' }` resolve the way `navigateTo` / `navigateToApp` do.
//!
//! The service crate does not load lxapps. The host facade installs a
//! validator and an opener once at bootstrap.

use std::sync::{Arc, OnceLock};

use serde_json::{Map, Value};

use super::target::NavigationError;

type Validator = Arc<dyn Fn(&str, Option<&str>) -> Result<(), NavigationError> + Send + Sync>;
type Opener = Arc<
    dyn Fn(&str, Option<&str>, &Map<String, Value>) -> Result<(), NavigationError> + Send + Sync,
>;

static VALIDATE: OnceLock<Validator> = OnceLock::new();
static OPEN: OnceLock<Opener> = OnceLock::new();

pub fn install(
    validate: impl Fn(&str, Option<&str>) -> Result<(), NavigationError> + Send + Sync + 'static,
    open: impl Fn(&str, Option<&str>, &Map<String, Value>) -> Result<(), NavigationError>
    + Send
    + Sync
    + 'static,
) {
    let _ = VALIDATE.set(Arc::new(validate));
    let _ = OPEN.set(Arc::new(open));
}

pub fn home_appid() -> Result<&'static str, NavigationError> {
    lingxia_app_context::home_app_id()
        .filter(|id| !id.is_empty())
        .ok_or_else(|| NavigationError::invalid("this product has no home lxapp"))
}

/// Shape is already checked. When no host hook is installed (unit tests)
/// existence is not re-checked here.
pub fn validate(appid: &str, page: Option<&str>) -> Result<(), NavigationError> {
    match VALIDATE.get() {
        Some(validate) => validate(appid, page),
        None => Ok(()),
    }
}

pub fn open(
    appid: &str,
    page: Option<&str>,
    query: &Map<String, Value>,
) -> Result<(), NavigationError> {
    match OPEN.get() {
        Some(open) => open(appid, page, query),
        None => Err(NavigationError::unavailable(
            "no lxapp page opener is installed",
        )),
    }
}
