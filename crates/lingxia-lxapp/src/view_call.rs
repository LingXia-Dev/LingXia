use crate::bridge::{
    BRIDGE_CANCELED, BRIDGE_TIMEOUT, OutboundContext, RpcError, SessionWorkId,
    required_cap_for_name,
};
use crate::error::LxAppError;
use crate::page::PageInstance;
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
    page_instance_id: String,
    work_id: SessionWorkId,
    /// Kept with the pending call so a future explicit cancel frame has the
    /// same immutable destination as the request it cancels.
    _outbound: Option<OutboundContext>,
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

/// Send a request to the View (WebView) and return a receiver for the response.
pub(crate) fn call_view(
    page: &PageInstance,
    method: &str,
    params: Option<Value>,
) -> Result<PendingViewCall, LxAppError> {
    let reg = registry();
    let seq = reg
        .counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
        .expect("view-call id space exhausted");
    let id = format!("lv_{}", seq);
    let page_instance_id = page.instance_id_string();
    let bridge = page.bridge();
    let (work_id, outbound) = bridge.capture_session_work().ok_or_else(|| {
        LxAppError::Bridge("View bridge has no active document session".to_string())
    })?;

    let cap = required_cap_for_name(method);
    let (tx, rx) = oneshot::channel();
    reg.pending.lock().unwrap().insert(
        id.clone(),
        PendingViewCallEntry {
            page_instance_id,
            work_id,
            _outbound: outbound.clone(),
            tx,
        },
    );
    // A document revoke can win between taking the immutable snapshot and
    // publishing this pending call. Compensate before sending or returning a
    // receiver that only times out.
    if !bridge.is_current_work(Some(work_id)) {
        reg.pending.lock().unwrap().remove(&id);
        return Err(LxAppError::Bridge(
            "View bridge document session was revoked".to_string(),
        ));
    }

    if let Err(e) = bridge.send_view_request_for_context(
        page,
        Some(work_id),
        outbound.as_ref(),
        id.clone(),
        method.to_string(),
        params,
        cap,
    ) {
        // Remove pending entry on send failure
        reg.pending.lock().unwrap().remove(&id);
        return Err(e);
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
                BRIDGE_TIMEOUT, timeout
            )))
        }
    }
}

/// Resolve a pending view call with the result from the View.
/// Returns `true` if a matching pending call was found and resolved.
pub(crate) fn resolve_view_call(
    id: &str,
    source_page_instance_id: Option<&str>,
    source_work_id: Option<SessionWorkId>,
    result: Result<Value, RpcError>,
) -> bool {
    let reg = registry();
    let entry = {
        let mut pending = reg.pending.lock().unwrap();
        if let Some(instance_id) = source_page_instance_id
            && let Some(existing) = pending.get(id)
            && existing.page_instance_id != instance_id
        {
            return false;
        }
        if let Some(work_id) = source_work_id
            && let Some(existing) = pending.get(id)
            && existing.work_id != work_id
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

pub(crate) fn cancel_view_calls_for_work(work_id: SessionWorkId, reason: &str) {
    let entries = {
        let mut pending = registry().pending.lock().unwrap();
        let ids = pending
            .iter()
            .filter(|(_, entry)| entry.work_id == work_id)
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
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

pub(crate) fn cancel_view_calls_for_page_instances(instance_ids: &[String], reason: &str) {
    if instance_ids.is_empty() {
        return;
    }

    let reg = registry();
    let instance_set: HashSet<&str> = instance_ids.iter().map(String::as_str).collect();

    let entries = {
        let mut pending = reg.pending.lock().unwrap();
        let ids: Vec<String> = pending
            .iter()
            .filter(|(_, entry)| instance_set.contains(entry.page_instance_id.as_str()))
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
                assert!(message.contains(BRIDGE_TIMEOUT));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn stale_document_work_cannot_resolve_successor_view_call() {
        let id = "lv_work_isolation";
        let (tx, mut rx) = oneshot::channel();
        registry().pending.lock().unwrap().insert(
            id.to_string(),
            PendingViewCallEntry {
                page_instance_id: "page-1".to_string(),
                work_id: SessionWorkId::for_test(22),
                _outbound: None,
                tx,
            },
        );

        assert!(!resolve_view_call(
            id,
            Some("page-1"),
            Some(SessionWorkId::for_test(21)),
            Ok(Value::Null),
        ));
        assert!(rx.try_recv().is_err());
        assert!(resolve_view_call(
            id,
            Some("page-1"),
            Some(SessionWorkId::for_test(22)),
            Ok(Value::Null),
        ));
        assert!(matches!(rx.try_recv(), Ok(Ok(Value::Null))));
    }

    #[test]
    fn canceling_retired_work_leaves_successor_view_call_pending() {
        let old_id = "lv_old_work";
        let new_id = "lv_new_work";
        let (old_tx, mut old_rx) = oneshot::channel();
        let (new_tx, mut new_rx) = oneshot::channel();
        let mut pending = registry().pending.lock().unwrap();
        pending.insert(
            old_id.to_string(),
            PendingViewCallEntry {
                page_instance_id: "page-1".to_string(),
                work_id: SessionWorkId::for_test(31),
                _outbound: None,
                tx: old_tx,
            },
        );
        pending.insert(
            new_id.to_string(),
            PendingViewCallEntry {
                page_instance_id: "page-1".to_string(),
                work_id: SessionWorkId::for_test(32),
                _outbound: None,
                tx: new_tx,
            },
        );
        drop(pending);

        cancel_view_calls_for_work(SessionWorkId::for_test(31), "session reset");

        assert!(matches!(old_rx.try_recv(), Ok(Err(_))));
        assert!(new_rx.try_recv().is_err());
        registry().pending.lock().unwrap().remove(new_id);
    }
}
