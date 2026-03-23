use super::bridge::{BRIDGE_CANCELED, Bridge, RpcError};
use crate::error::LxAppError;
use crate::page::Page;
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::time;

static REGISTRY: OnceLock<ViewCallRegistry> = OnceLock::new();

struct ViewCallRegistry {
    pending: Mutex<HashMap<String, PendingViewCallEntry>>,
    counter: AtomicU64,
}

struct PendingViewCallEntry {
    page_path: String,
    tx: oneshot::Sender<Result<Value, RpcError>>,
}

pub(crate) struct PendingViewCall {
    pub id: String,
    pub rx: oneshot::Receiver<Result<Value, RpcError>>,
}

impl ViewCallRegistry {
    fn new() -> Self {
        Self {
            pending: Mutex::new(HashMap::new()),
            counter: AtomicU64::new(1),
        }
    }
}

fn registry() -> &'static ViewCallRegistry {
    REGISTRY.get_or_init(ViewCallRegistry::new)
}

#[derive(Serialize)]
struct ViewReq {
    v: u8,
    kind: &'static str,
    id: String,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
    cap: String,
}

/// Send a request to the View (WebView) and return a receiver for the response.
pub(crate) fn call_view(
    page: &Page,
    method: &str,
    params: Option<Value>,
) -> Result<PendingViewCall, LxAppError> {
    let reg = registry();
    let seq = reg.counter.fetch_add(1, Ordering::Relaxed);
    let id = format!("lv_{}", seq);
    let page_path = page.path();

    let cap = Bridge::required_cap_for_name(method);
    let msg = ViewReq {
        v: 2,
        kind: "req",
        id: id.clone(),
        method: method.to_string(),
        params,
        cap,
    };

    let json = serde_json::to_string(&msg)?;

    let controller = page
        .webview_controller()
        .ok_or_else(|| LxAppError::WebView("WebView not ready".to_string()))?;

    let (tx, rx) = oneshot::channel();
    reg.pending
        .lock()
        .unwrap()
        .insert(id.clone(), PendingViewCallEntry { page_path, tx });

    if let Err(e) = controller.post_message(&json) {
        // Remove pending entry on send failure
        reg.pending.lock().unwrap().remove(&id);
        return Err(LxAppError::WebView(e.to_string()));
    }

    Ok(PendingViewCall { id, rx })
}

pub(crate) async fn await_pending_view_call(
    pending: PendingViewCall,
    timeout: Duration,
) -> Result<Value, LxAppError> {
    match time::timeout(timeout, pending.rx).await {
        Ok(Ok(result)) => result.map_err(|rpc_err| LxAppError::RongJSHost {
            code: rpc_err.code,
            message: rpc_err
                .message
                .unwrap_or_else(|| "View call failed".to_string()),
            data: rpc_err.data,
        }),
        Ok(Err(_)) => Err(LxAppError::ChannelError(
            "View call channel closed".to_string(),
        )),
        Err(_) => {
            cancel_view_call(
                &pending.id,
                Some(format!("View call timed out after {:?}", timeout)),
            );
            Err(LxAppError::Bridge(format!(
                "{}: View call timed out after {:?}",
                super::bridge::BRIDGE_TIMEOUT,
                timeout
            )))
        }
    }
}

/// Resolve a pending view call with the result from the View.
/// Returns `true` if a matching pending call was found and resolved.
pub(crate) fn resolve_view_call(
    id: &str,
    source_page_path: Option<&str>,
    result: Result<Value, RpcError>,
) -> bool {
    let reg = registry();
    let entry = {
        let mut pending = reg.pending.lock().unwrap();
        if let Some(path) = source_page_path
            && let Some(existing) = pending.get(id)
            && existing.page_path != path
        {
            return false;
        }
        pending.remove(id)
    };
    if let Some(entry) = entry {
        let _ = entry.tx.send(result);
        return true;
    }
    false
}

pub(crate) fn cancel_view_call(id: &str, message: Option<String>) {
    let reg = registry();
    let entry = reg.pending.lock().unwrap().remove(id);
    if let Some(entry) = entry {
        let _ = entry.tx.send(Err(RpcError::new(BRIDGE_CANCELED, message)));
    }
}

pub(crate) fn cancel_view_calls_for_pages(paths: &[String], reason: &str) {
    if paths.is_empty() {
        return;
    }

    let reg = registry();
    let path_set: HashSet<&str> = paths.iter().map(String::as_str).collect();

    let entries = {
        let mut pending = reg.pending.lock().unwrap();
        let ids: Vec<String> = pending
            .iter()
            .filter(|(_, entry)| path_set.contains(entry.page_path.as_str()))
            .map(|(id, _)| id.clone())
            .collect();
        ids.into_iter()
            .filter_map(|id| pending.remove(&id))
            .collect::<Vec<_>>()
    };

    for entry in entries {
        let _ = entry.tx.send(Err(RpcError::new(
            BRIDGE_CANCELED,
            Some(reason.to_string()),
        )));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn await_pending_view_call_returns_value() {
        let (tx, rx) = oneshot::channel();
        let pending = PendingViewCall {
            id: "lv_test_ok".to_string(),
            rx,
        };

        tx.send(Ok(serde_json::json!({ "ok": true }))).unwrap();

        let value = await_pending_view_call(pending, Duration::from_millis(50))
            .await
            .unwrap();

        assert_eq!(value, serde_json::json!({ "ok": true }));
    }

    #[tokio::test]
    async fn await_pending_view_call_maps_rpc_error() {
        let (tx, rx) = oneshot::channel();
        let pending = PendingViewCall {
            id: "lv_test_err".to_string(),
            rx,
        };

        tx.send(Err(RpcError {
            code: "E_VIEW".to_string(),
            message: Some("view failed".to_string()),
            data: Some(serde_json::json!({ "retryable": false })),
        }))
        .unwrap();

        let err = await_pending_view_call(pending, Duration::from_millis(50))
            .await
            .unwrap_err();

        match err {
            LxAppError::RongJSHost {
                code,
                message,
                data,
            } => {
                assert_eq!(code, "E_VIEW");
                assert_eq!(message, "view failed");
                assert_eq!(data, Some(serde_json::json!({ "retryable": false })));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn await_pending_view_call_times_out() {
        let (_tx, rx) = oneshot::channel();
        let pending = PendingViewCall {
            id: "lv_test_timeout".to_string(),
            rx,
        };

        let err = await_pending_view_call(pending, Duration::from_millis(1))
            .await
            .unwrap_err();

        match err {
            LxAppError::Bridge(message) => {
                assert!(message.contains(super::super::bridge::BRIDGE_TIMEOUT));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
