//! The path from a host run to the dev session's companion, for the
//! `function` rules of `t.app.scenario()`.
//!
//! This crate does not know how the host reaches `lingxia dev`: the dev
//! bridge registers an [`Upstream`] that sends a request over its session
//! connection and resolves with the dev server's answer. A host without a
//! dev session has none, and `function` rules then fail with a clear error.

use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

/// A failed upstream request, as the dev server answered it.
#[derive(Debug, Clone, PartialEq)]
pub struct UpstreamError {
    pub code: String,
    pub message: String,
    pub data: Option<Value>,
}

pub type UpstreamFuture = Pin<Box<dyn Future<Output = Result<Value, UpstreamError>> + Send>>;

/// Send `method` with `params` to the dev server.
pub type Upstream = Arc<dyn Fn(&str, Value) -> UpstreamFuture + Send + Sync>;

static UPSTREAM: Mutex<Option<Upstream>> = Mutex::new(None);

/// Register (or, with `None`, remove) the dev session connection.
pub fn set_upstream(upstream: Option<Upstream>) {
    *UPSTREAM
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = upstream;
}

pub(crate) fn upstream() -> Option<Upstream> {
    UPSTREAM
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

/// Why `function` rules have nowhere to go, or `None` when a dev session
/// is connected (the dev server then says whether its companion takes
/// them).
pub(crate) fn unavailable() -> Option<&'static str> {
    upstream().is_none().then_some(
        "this host has no dev session connection (`lingxia dev`), so no companion can answer them",
    )
}

/// Send one request upstream.
pub(crate) async fn request(method: &str, params: Value) -> Result<Value, UpstreamError> {
    let Some(upstream) = upstream() else {
        return Err(UpstreamError {
            code: "unavailable".into(),
            message: unavailable().unwrap_or_default().into(),
            data: None,
        });
    };
    upstream(method, params).await
}
