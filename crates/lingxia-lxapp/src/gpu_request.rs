use crate::bridge::{OutboundContext, PageBridge, RpcError, SessionWorkId};
use crate::page::{GpuVideoRequest, GpuViewRequest, PageInstance};
use lingxia_error::code;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::time;

/// Requests Logic sent a GPU View, waiting on their replies.
static REGISTRY: OnceLock<Registry> = OnceLock::new();

struct Registry {
    pending: Mutex<HashMap<String, PendingEntry>>,
    counter: AtomicU64,
}

struct PendingEntry {
    page_instance_id: String,
    work_id: SessionWorkId,
    /// Kept with the pending call so a future explicit cancel frame has the
    /// same immutable destination as the request it cancels.
    _outbound: Option<OutboundContext>,
    tx: oneshot::Sender<Result<Value, RpcError>>,
    sent_us: u64,
}

pub(crate) struct PendingRequest {
    pub id: String,
    pub rx: oneshot::Receiver<Result<Value, RpcError>>,
}

impl Registry {
    fn new() -> Self {
        Self {
            pending: Mutex::new(HashMap::new()),
            counter: AtomicU64::new(1),
        }
    }
}

fn registry() -> &'static Registry {
    REGISTRY.get_or_init(Registry::new)
}

/// Send a typed video command to a GPU page's player and return a receiver for the reply.
pub(crate) fn call_gpu_video(
    page: &PageInstance,
    request: GpuVideoRequest,
) -> Result<PendingRequest, lingxia_error::Error> {
    pending_call(page, |bridge, work_id, outbound, id| {
        bridge.send_gpu_request_for_context(
            Some(work_id),
            outbound,
            id,
            GpuViewRequest::Video(request),
        )
    })
}

/// Registers a pending call under a fresh id for the page's current document, then `send`s it.
fn pending_call(
    page: &PageInstance,
    send: impl FnOnce(
        &PageBridge,
        SessionWorkId,
        Option<&OutboundContext>,
        String,
    ) -> Result<(), lingxia_error::Error>,
) -> Result<PendingRequest, lingxia_error::Error> {
    let reg = registry();
    let seq = reg
        .counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
        .expect("GPU request id space exhausted");
    let id = format!("lv_{}", seq);
    let page_instance_id = page.instance_id_string();
    let bridge = page.bridge();
    let (work_id, outbound) = bridge.capture_session_work().ok_or_else(|| {
        lingxia_error::Error::internal("View bridge has no active document session")
    })?;

    let (tx, rx) = oneshot::channel();
    reg.pending.lock().unwrap().insert(
        id.clone(),
        PendingEntry {
            page_instance_id,
            work_id,
            sent_us: crate::bridge::probe::now_us(),
            _outbound: outbound.clone(),
            tx,
        },
    );
    // A document revoke can win between taking the immutable snapshot and
    // publishing this pending call. Compensate before sending or returning a
    // receiver that only times out.
    if !bridge.is_current_work(Some(work_id)) {
        reg.pending.lock().unwrap().remove(&id);
        return Err(lingxia_error::Error::internal(
            "View bridge document session was revoked",
        ));
    }

    if let Err(e) = send(&bridge, work_id, outbound.as_ref(), id.clone()) {
        // Remove pending entry on send failure
        reg.pending.lock().unwrap().remove(&id);
        return Err(e);
    }

    Ok(PendingRequest { id, rx })
}

pub(crate) async fn await_pending(
    pending: PendingRequest,
    timeout: Duration,
) -> Result<Value, lingxia_error::Error> {
    match time::timeout(timeout, pending.rx).await {
        Ok(Ok(result)) => result.map_err(|rpc_err| {
            let message = rpc_err
                .message
                .unwrap_or_else(|| "GPU View request failed".to_string());
            let mut error = lingxia_error::Error::new(rpc_err.code, message);
            if let Some(reason) = rpc_err.reason {
                error = error.with_reason(String::from(reason));
            }
            if let Some(data) = rpc_err.data {
                error = error.with_data(*data);
            }
            error
        }),
        Ok(Err(_)) => Err(lingxia_error::Error::internal("GPU View request channel closed")),
        Err(_) => {
            cancel(
                &pending.id,
                Some(format!("GPU View request timed out after {:?}", timeout)),
            );
            Err(lingxia_error::Error::timeout(format!(
                "GPU View request timed out after {timeout:?}"
            )))
        }
    }
}

/// Resolve a pending request with the View's reply; `true` when one matched.
pub(crate) fn resolve(
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
        crate::bridge::probe::record(
            crate::bridge::probe::Probe::RequestAnswer,
            entry
                ._outbound
                .as_ref()
                .is_some_and(OutboundContext::is_gpu),
            entry.sent_us,
        );
        let _ = entry.tx.send(result);
        return true;
    }
    false
}

pub(crate) fn cancel(id: &str, message: Option<String>) {
    let reg = registry();
    let entry = reg.pending.lock().unwrap().remove(id);
    if let Some(entry) = entry {
        let _ = entry.tx.send(Err(RpcError::new(code::ABORT, message)));
    }
}

pub(crate) fn cancel_for_work(work_id: SessionWorkId, page_instance_id: &str, reason: &str) {
    let entries = {
        let mut pending = registry().pending.lock().unwrap();
        // Work ids restart at 1 on every bridge. This map is process-wide, so
        // a match on the id alone cancels another page's in-flight call when
        // a hidden page is reclaimed.
        let ids = pending
            .iter()
            .filter(|(_, entry)| {
                entry.work_id == work_id && entry.page_instance_id == page_instance_id
            })
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        ids.into_iter()
            .filter_map(|id| pending.remove(&id))
            .collect::<Vec<_>>()
    };
    for entry in entries {
        let _ = entry
            .tx
            .send(Err(RpcError::new(code::ABORT, Some(reason.to_string()))));
    }
}

pub(crate) fn cancel_for_page_instances(instance_ids: &[String], reason: &str) {
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
        let _ = entry
            .tx
            .send(Err(RpcError::new(code::ABORT, Some(reason.to_string()))));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn await_pending_returns_value() {
        let (tx, rx) = oneshot::channel();
        let pending = PendingRequest {
            id: "lv_test_ok".to_string(),
            rx,
        };

        tx.send(Ok(serde_json::json!({ "ok": true }))).unwrap();

        let value = await_pending(pending, Duration::from_millis(50))
            .await
            .unwrap();

        assert_eq!(value, serde_json::json!({ "ok": true }));
    }

    #[tokio::test]
    async fn await_pending_maps_rpc_error() {
        let (tx, rx) = oneshot::channel();
        let pending = PendingRequest {
            id: "lv_test_err".to_string(),
            rx,
        };

        tx.send(Err(RpcError {
            code: "E_NOT_SUPPORTED".to_string(),
            message: Some("view failed".to_string()),
            reason: Some("placementUnsupported".into()),
            data: Some(Box::new(serde_json::json!({ "retryable": false }))),
        }))
        .unwrap();

        let err = await_pending(pending, Duration::from_millis(50))
            .await
            .unwrap_err();

        assert_eq!(err.code(), "E_NOT_SUPPORTED");
        assert_eq!(err.message(), "view failed");
        assert_eq!(err.reason(), Some("placementUnsupported"));
        assert_eq!(err.data(), Some(&serde_json::json!({ "retryable": false })));
    }

    #[tokio::test]
    async fn await_pending_times_out() {
        let (_tx, rx) = oneshot::channel();
        let pending = PendingRequest {
            id: "lv_test_timeout".to_string(),
            rx,
        };

        let err = await_pending(pending, Duration::from_millis(1))
            .await
            .unwrap_err();

        assert_eq!(err.code(), code::TIMEOUT);
    }

    #[test]
    fn stale_document_work_cannot_resolve_successor_request() {
        let id = "lv_work_isolation";
        let (tx, mut rx) = oneshot::channel();
        registry().pending.lock().unwrap().insert(
            id.to_string(),
            PendingEntry {
                page_instance_id: "page-1".to_string(),
                work_id: SessionWorkId::for_test(22),
                _outbound: None,
                sent_us: 0,
                tx,
            },
        );

        assert!(!resolve(
            id,
            Some("page-1"),
            Some(SessionWorkId::for_test(21)),
            Ok(Value::Null),
        ));
        assert!(rx.try_recv().is_err());
        assert!(resolve(
            id,
            Some("page-1"),
            Some(SessionWorkId::for_test(22)),
            Ok(Value::Null),
        ));
        assert!(matches!(rx.try_recv(), Ok(Ok(Value::Null))));
    }

    #[test]
    fn canceling_retired_work_leaves_successor_request_pending() {
        let old_id = "lv_old_work";
        let new_id = "lv_new_work";
        let (old_tx, mut old_rx) = oneshot::channel();
        let (new_tx, mut new_rx) = oneshot::channel();
        let mut pending = registry().pending.lock().unwrap();
        pending.insert(
            old_id.to_string(),
            PendingEntry {
                page_instance_id: "page-1".to_string(),
                work_id: SessionWorkId::for_test(31),
                _outbound: None,
                sent_us: 0,
                tx: old_tx,
            },
        );
        pending.insert(
            new_id.to_string(),
            PendingEntry {
                page_instance_id: "page-1".to_string(),
                work_id: SessionWorkId::for_test(32),
                _outbound: None,
                sent_us: 0,
                tx: new_tx,
            },
        );
        drop(pending);

        cancel_for_work(SessionWorkId::for_test(31), "page-1", "session reset");

        assert!(matches!(old_rx.try_recv(), Ok(Err(_))));
        assert!(new_rx.try_recv().is_err());
        registry().pending.lock().unwrap().remove(new_id);
    }

    #[test]
    fn canceling_one_pages_work_leaves_another_pages_same_work_id() {
        let foreground_id = "lv_foreground_work";
        let hidden_id = "lv_hidden_work";
        let (foreground_tx, mut foreground_rx) = oneshot::channel();
        let (hidden_tx, mut hidden_rx) = oneshot::channel();
        let mut pending = registry().pending.lock().unwrap();
        pending.insert(
            foreground_id.to_string(),
            PendingEntry {
                page_instance_id: "foreground".to_string(),
                work_id: SessionWorkId::for_test(1),
                _outbound: None,
                sent_us: 0,
                tx: foreground_tx,
            },
        );
        pending.insert(
            hidden_id.to_string(),
            PendingEntry {
                page_instance_id: "hidden".to_string(),
                work_id: SessionWorkId::for_test(1),
                _outbound: None,
                sent_us: 0,
                tx: hidden_tx,
            },
        );
        drop(pending);

        cancel_for_work(
            SessionWorkId::for_test(1),
            "hidden",
            "Document session revoked",
        );

        assert!(matches!(hidden_rx.try_recv(), Ok(Err(_))));
        assert!(foreground_rx.try_recv().is_err());
        registry().pending.lock().unwrap().remove(foreground_id);
    }
}
