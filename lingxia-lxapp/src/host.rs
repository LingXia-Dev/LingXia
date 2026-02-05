use crate::error::LxAppError;
use crate::lxapp::{LxApp, ReleaseType};

use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::{Mutex, OnceLock};
use tokio::sync::oneshot;

#[macro_use]
mod macros;

mod device;
mod navigation;
mod navigator;

pub(crate) type HostCancel = oneshot::Receiver<()>;
pub(crate) type HostFuture<'a> =
    Pin<Box<dyn Future<Output = Result<String, LxAppError>> + Send + 'a>>;

/// Host API handler trait - for view layer to call host app capabilities.
///
/// Design constraints:
/// - `input` is owned to avoid capturing borrows in `'static` futures.
/// - `cancel` is reachable so handlers can stop work early.
pub(crate) trait HostHandler: Send + Sync + 'static {
    fn call<'a>(
        &'a self,
        lxapp: Arc<LxApp>,
        input: Option<String>,
        cancel: HostCancel,
    ) -> HostFuture<'a>;
}

/// Host API registry - stores host handlers.
struct HostRegistry {
    handlers: HashMap<String, Arc<dyn HostHandler>>,
}

impl HostRegistry {
    fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }
}

/// Global host API registry instance.
static GLOBAL_HOST_REGISTRY: OnceLock<Mutex<HostRegistry>> = OnceLock::new();

fn get_host_registry() -> &'static Mutex<HostRegistry> {
    GLOBAL_HOST_REGISTRY.get_or_init(|| Mutex::new(HostRegistry::new()))
}

pub(crate) fn register_host(name: &str, handler: Arc<dyn HostHandler>) {
    let registry = get_host_registry();
    registry
        .lock()
        .unwrap()
        .handlers
        .insert(name.to_string(), handler);
}

pub(crate) fn get_host(name: &str) -> Option<Arc<dyn HostHandler>> {
    let registry = get_host_registry();
    registry.lock().unwrap().handlers.get(name).cloned()
}

pub(crate) fn parse_input<T: DeserializeOwned>(input: Option<&str>) -> Result<T, LxAppError> {
    match input {
        Some(json) => serde_json::from_str(json)
            .map_err(|e| LxAppError::InvalidParameter(format!("Invalid input JSON: {}", e))),
        None => Err(LxAppError::InvalidParameter("Missing input".to_string())),
    }
}

#[allow(dead_code)]
pub(crate) fn parse_input_optional<T: DeserializeOwned>(
    input: Option<&str>,
) -> Result<Option<T>, LxAppError> {
    match input {
        Some(json) => serde_json::from_str(json)
            .map(Some)
            .map_err(|e| LxAppError::InvalidParameter(format!("Invalid input JSON: {}", e))),
        None => Ok(None),
    }
}

pub(crate) fn serialize_result<T: Serialize>(
    result: Result<T, LxAppError>,
) -> Result<String, LxAppError> {
    let value = result?;
    serde_json::to_string(&value).map_err(|e| LxAppError::Bridge(e.to_string()))
}

pub(crate) async fn await_or_cancel<T>(
    cancel: &mut HostCancel,
    fut: impl Future<Output = Result<T, LxAppError>>,
) -> Result<T, LxAppError> {
    tokio::select! {
        _ = cancel => Err(LxAppError::Bridge("Canceled".to_string()))?,
        res = fut => res,
    }
}

/// Register built-in Host API set.
///
/// This is invoked once from lxapp initialization so Host API definitions are owned
/// by `lingxia-lxapp` (not `lingxia-logic` or the host app).
pub(crate) fn register_all() {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        device::register_all();
        navigation::register_all();
        navigator::register_all();
    });
}

fn parse_release_type(env_version: Option<&str>) -> ReleaseType {
    env_version
        .map(crate::startup::parse_env_release_type)
        .unwrap_or(ReleaseType::Release)
}
