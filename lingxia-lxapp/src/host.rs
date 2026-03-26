//! Host API runtime and extension surface.
//!
//! Built-in host capabilities and third-party host extensions share the same
//! registry. External crates can define handlers and register them here.

use crate::error::LxAppError;
use crate::lxapp::{LxApp, ReleaseType};

use futures::{Stream, StreamExt};
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

pub type HostResult<T> = Result<T, LxAppError>;

pub type HostCancel = oneshot::Receiver<()>;
pub type HostStream =
    Pin<Box<dyn Stream<Item = Result<HostStreamItem, LxAppError>> + Send + 'static>>;
pub type HostFuture<'a> = Pin<Box<dyn Future<Output = Result<HostOutput, LxAppError>> + Send + 'a>>;

pub enum HostStreamValue<TEvent, TResult> {
    Event(TEvent),
    Return(TResult),
}

pub type HostTypedStreamItem<TEvent, TResult> = HostResult<HostStreamValue<TEvent, TResult>>;

pub enum HostStreamItem {
    Event(String),
    Return(String),
}

pub enum HostOutput {
    Json(String),
    Stream(HostStream),
}

/// Wire-level method kind, serialized into the handshake schema so the JS
/// bridge can automatically choose `call` vs `callStream`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostMethodKind {
    Call,
    Stream,
}

pub struct HostRegistration {
    pub namespace: &'static str,
    pub method: &'static str,
    pub handler: Arc<dyn HostHandler>,
    pub kind: HostMethodKind,
}

impl HostRegistration {
    pub fn new(
        namespace: &'static str,
        method: &'static str,
        handler: Arc<dyn HostHandler>,
    ) -> Self {
        Self {
            namespace,
            method,
            handler,
            kind: HostMethodKind::Call,
        }
    }

    pub fn stream(
        namespace: &'static str,
        method: &'static str,
        handler: Arc<dyn HostHandler>,
    ) -> Self {
        Self {
            namespace,
            method,
            handler,
            kind: HostMethodKind::Stream,
        }
    }
}

/// Host API handler trait - for view layer to call host app capabilities.
///
/// Design constraints:
/// - `input` is owned to avoid capturing borrows in `'static` futures.
/// - `cancel` is reachable so handlers can stop work early.
pub trait HostHandler: Send + Sync + 'static {
    fn call<'a>(
        &'a self,
        lxapp: Arc<LxApp>,
        input: Option<String>,
        cancel: HostCancel,
    ) -> HostFuture<'a>;
}

/// Host API registry - stores host handlers and their method kinds.
struct HostRegistry {
    handlers: HashMap<String, Arc<dyn HostHandler>>,
    kinds: HashMap<String, HostMethodKind>,
}

impl HostRegistry {
    fn new() -> Self {
        Self {
            handlers: HashMap::new(),
            kinds: HashMap::new(),
        }
    }
}

/// Global host API registry instance.
static GLOBAL_HOST_REGISTRY: OnceLock<Mutex<HostRegistry>> = OnceLock::new();

fn get_host_registry() -> &'static Mutex<HostRegistry> {
    GLOBAL_HOST_REGISTRY.get_or_init(|| Mutex::new(HostRegistry::new()))
}

pub fn register_host_route(namespace: &str, method: &str, handler: Arc<dyn HostHandler>) {
    let key = format!("{namespace}.{method}");
    let registry = get_host_registry();
    let mut reg = registry.lock().unwrap();
    reg.kinds.insert(key.clone(), HostMethodKind::Call);
    reg.handlers.insert(key, handler);
}

pub fn register_host(registration: HostRegistration) {
    let key = format!("{}.{}", registration.namespace, registration.method);
    let registry = get_host_registry();
    let mut reg = registry.lock().unwrap();
    reg.kinds.insert(key.clone(), registration.kind);
    reg.handlers.insert(key, registration.handler);
}

pub(crate) fn get_host(name: &str) -> Option<Arc<dyn HostHandler>> {
    let registry = get_host_registry();
    registry.lock().unwrap().handlers.get(name).cloned()
}

/// Returns a map of `"namespace.method"` → `"call"` | `"stream"` for all
/// registered host methods. Included in the handshake `Ready` message so
/// the JS bridge can automatically choose the right wire protocol.
pub fn host_method_schema() -> HashMap<String, &'static str> {
    let registry = get_host_registry();
    let reg = registry.lock().unwrap();
    reg.kinds
        .iter()
        .map(|(k, v)| {
            let kind_str = match v {
                HostMethodKind::Call => "call",
                HostMethodKind::Stream => "stream",
            };
            (k.clone(), kind_str)
        })
        .collect()
}

pub fn parse_input<T: DeserializeOwned>(input: Option<&str>) -> HostResult<T> {
    match input {
        Some(json) => serde_json::from_str(json)
            .map_err(|e| LxAppError::InvalidParameter(format!("Invalid input JSON: {}", e))),
        None => Err(LxAppError::InvalidParameter("Missing input".to_string())),
    }
}

pub fn serialize_result<T: Serialize>(result: HostResult<T>) -> HostResult<HostOutput> {
    let value = result?;
    serde_json::to_string(&value)
        .map(HostOutput::Json)
        .map_err(|e| LxAppError::Bridge(e.to_string()))
}

pub fn stream_event<TEvent, TResult>(event: TEvent) -> HostStreamValue<TEvent, TResult> {
    HostStreamValue::Event(event)
}

pub fn stream_return<TEvent, TResult>(result: TResult) -> HostStreamValue<TEvent, TResult> {
    HostStreamValue::Return(result)
}

pub fn serialize_stream<S, TEvent, TResult>(result: HostResult<S>) -> HostResult<HostOutput>
where
    S: Stream<Item = HostTypedStreamItem<TEvent, TResult>> + Send + 'static,
    TEvent: Serialize,
    TResult: Serialize,
{
    let stream = result?.map(|item| match item? {
        HostStreamValue::Event(payload) => serde_json::to_string(&payload)
            .map(HostStreamItem::Event)
            .map_err(|e| LxAppError::Bridge(e.to_string())),
        HostStreamValue::Return(result) => serde_json::to_string(&result)
            .map(HostStreamItem::Return)
            .map_err(|e| LxAppError::Bridge(e.to_string())),
    });
    Ok(HostOutput::Stream(Box::pin(stream)))
}

pub async fn await_or_cancel<T>(
    cancel: &mut HostCancel,
    fut: impl Future<Output = HostResult<T>>,
) -> HostResult<T> {
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
