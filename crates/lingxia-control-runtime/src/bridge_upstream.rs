//! Requests the runtime sends up its dev session connection, and their
//! answers: how a host run's `t.app.scenario()` reaches the dev session's
//! companion. The bridge thread owns the websocket, so a request is queued
//! here, sent on the bridge's next turn, and resolved when the matching
//! response arrives, the connection drops, or it times out.

use lingxia_automation::runtime::network::{UpstreamError, UpstreamFuture, set_upstream};
use lingxia_control_protocol::{ControlRequest, ControlResponse};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Once, mpsc};
use std::time::{Duration, Instant};
use tokio::sync::oneshot;

/// A dev server that relays to a busy companion answers within this.
const TIMEOUT: Duration = Duration::from_secs(20);

struct Pending {
    reply: oneshot::Sender<Result<Value, UpstreamError>>,
    deadline: Instant,
}

static OUTBOX: Mutex<Option<mpsc::Sender<ControlRequest>>> = Mutex::new(None);
static PENDING: Mutex<Option<HashMap<String, Pending>>> = Mutex::new(None);
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn pending() -> std::sync::MutexGuard<'static, Option<HashMap<String, Pending>>> {
    PENDING
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn error(code: &str, message: impl Into<String>) -> UpstreamError {
    UpstreamError {
        code: code.to_string(),
        message: message.into(),
        data: None,
    }
}

/// Register the upstream once per process; requests fail until a
/// connection is attached.
pub(crate) fn install() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        set_upstream(Some(Arc::new(
            |method: &str, params: Value| -> UpstreamFuture {
                let method = method.to_string();
                Box::pin(async move { request(method, params).await })
            },
        )));
    });
}

async fn request(method: String, params: Value) -> Result<Value, UpstreamError> {
    let id = format!("runtime-{}", NEXT_ID.fetch_add(1, Ordering::Relaxed));
    let (reply, answer) = oneshot::channel();
    {
        let outbox = OUTBOX
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(outbox) = outbox.as_ref() else {
            return Err(error(
                "unavailable",
                "the dev session is not connected (`lingxia dev`)",
            ));
        };
        pending().get_or_insert_with(HashMap::new).insert(
            id.clone(),
            Pending {
                reply,
                deadline: Instant::now() + TIMEOUT,
            },
        );
        let sent = outbox.send(ControlRequest {
            id: id.clone(),
            method,
            params: Some(params),
        });
        if sent.is_err() {
            pending().get_or_insert_with(HashMap::new).remove(&id);
            return Err(error("unavailable", "the dev session connection closed"));
        }
    }
    answer
        .await
        .unwrap_or_else(|_| Err(error("unavailable", "the dev session connection closed")))
}

/// A connection is up: requests queue for it.
pub(crate) fn attach() -> mpsc::Receiver<ControlRequest> {
    let (sender, receiver) = mpsc::channel();
    *OUTBOX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(sender);
    receiver
}

/// The connection dropped: nothing more is sent, and what waits fails.
pub(crate) fn detach() {
    *OUTBOX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
    let waiting = pending().take().unwrap_or_default();
    for (_, pending) in waiting {
        let _ = pending.reply.send(Err(error(
            "unavailable",
            "the dev session connection dropped",
        )));
    }
}

/// Resolve the request a response answers. Returns whether it was ours.
pub(crate) fn resolve(response: ControlResponse) -> bool {
    let Some(pending) = pending()
        .as_mut()
        .and_then(|waiting| waiting.remove(&response.id))
    else {
        return false;
    };
    let result = match response.error {
        Some(err) => Err(UpstreamError {
            code: err.code,
            message: err.message,
            data: err.data,
        }),
        None => Ok(response.result.unwrap_or(Value::Null)),
    };
    let _ = pending.reply.send(result);
    true
}

/// Fail requests past their deadline.
pub(crate) fn expire() {
    let now = Instant::now();
    let mut guard = pending();
    let Some(waiting) = guard.as_mut() else {
        return;
    };
    let late: Vec<String> = waiting
        .iter()
        .filter(|(_, pending)| pending.deadline <= now)
        .map(|(id, _)| id.clone())
        .collect();
    for id in late {
        if let Some(pending) = waiting.remove(&id) {
            let _ = pending.reply.send(Err(error(
                "request_timeout",
                format!(
                    "the dev session did not answer within {}s",
                    TIMEOUT.as_secs()
                ),
            )));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_resolves_with_its_response_and_fails_when_the_connection_drops() {
        let outbox = attach();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let answered = runtime.block_on(async {
            let call = request(
                "session.companion.scenario.use".into(),
                serde_json::json!({ "owner": "test:r" }),
            );
            let respond = async {
                let sent = loop {
                    if let Ok(sent) = outbox.try_recv() {
                        break sent;
                    }
                    tokio::task::yield_now().await;
                };
                assert_eq!(sent.method, "session.companion.scenario.use");
                assert_eq!(sent.params.as_ref().unwrap()["owner"], "test:r");
                assert!(resolve(ControlResponse::success(
                    sent.id,
                    Some(serde_json::json!({ "installed": 1 }))
                )));
            };
            let (answer, ()) = tokio::join!(call, respond);
            answer
        });
        assert_eq!(answered.unwrap()["installed"], 1);
        assert!(!resolve(ControlResponse::success("unknown", None)));

        let failed = runtime.block_on(async {
            let call = request("session.companion.scenario.clear".into(), Value::Null);
            let drop_it = async {
                while outbox.try_recv().is_err() {
                    tokio::task::yield_now().await;
                }
                detach();
            };
            let (answer, ()) = tokio::join!(call, drop_it);
            answer
        });
        assert_eq!(failed.unwrap_err().code, "unavailable");
        let offline = runtime.block_on(request("x".into(), Value::Null));
        assert!(offline.unwrap_err().message.contains("not connected"));
    }
}
