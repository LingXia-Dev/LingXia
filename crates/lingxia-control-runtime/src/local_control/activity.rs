//! What a product shows while an agent is driving it.
//!
//! The socket is strictly request/response and a command line opens a fresh
//! connection per command, so a connection is not what a person would call a
//! session. A session here is a run of requests: the first one starts it, and
//! it ends after a quiet spell, when the product stops it, or when access is
//! switched off. The product subscribes and draws whatever disclosure fits it
//! — an indicator with a Stop button, a notification while it is in the
//! background, an activity log. LingXia draws nothing for this itself.
//!
//! Only this socket reports. The development websocket is a developer driving
//! their own session and is deliberately left out.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;

/// How long a session survives without a request.
const SESSION_IDLE: Duration = Duration::from_secs(20);

/// What a request did, as far as LingXia can tell from its method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ControlEffect {
    /// Looked at something: a screenshot, a tab list, page content.
    Reads,
    /// Changed something: navigated, clicked, typed, moved a window.
    Changes,
    /// A host-registered namespace. Only the product knows what its own
    /// methods do, so it classifies these itself.
    Unclassified,
}

/// Why a session ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ControlSessionEnd {
    /// No request for a while.
    Idle,
    /// The product stopped it; access stays on.
    Stopped,
    /// Access was switched off.
    Disabled,
}

/// One thing that happened on the control socket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ControlEvent {
    #[serde(rename_all = "camelCase")]
    SessionStarted { session: u64, at_ms: u64 },
    #[serde(rename_all = "camelCase")]
    Activity {
        session: u64,
        at_ms: u64,
        /// The full method, e.g. `browser.click`.
        method: String,
        /// Its first segment, e.g. `browser`, or a host namespace.
        namespace: String,
        effect: ControlEffect,
        /// `None` when the request succeeded, else the error code it was
        /// answered with (`not_declared` for a refused namespace).
        error: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    SessionEnded {
        session: u64,
        at_ms: u64,
        reason: ControlSessionEnd,
    },
}

type Listener = Arc<dyn Fn(&ControlEvent) + Send + Sync>;

struct Live {
    id: u64,
    last: Instant,
}

static LISTENERS: Mutex<Vec<(u64, Listener)>> = Mutex::new(Vec::new());
static NEXT_LISTENER: AtomicU64 = AtomicU64::new(1);
static SESSION: Mutex<Option<Live>> = Mutex::new(None);
/// When a session was last stopped or access switched off. A request that was
/// already running then is not a reason to start a new session.
static LAST_END: Mutex<Option<Instant>> = Mutex::new(None);
static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);

/// Keeps a [`subscribe`] listener registered until dropped.
#[must_use = "the listener is removed when this is dropped"]
pub struct ControlEventSubscription(u64);

impl Drop for ControlEventSubscription {
    fn drop(&mut self) {
        let mut listeners = LISTENERS.lock().unwrap_or_else(|error| error.into_inner());
        listeners.retain(|(id, _)| *id != self.0);
    }
}

/// Receive every [`ControlEvent`] until the returned handle is dropped.
///
/// Listeners run on the thread that handled the request, after the reply is
/// computed, so keep them short and hand real work to the product's own loop.
pub fn subscribe(
    listener: impl Fn(&ControlEvent) + Send + Sync + 'static,
) -> ControlEventSubscription {
    let id = NEXT_LISTENER.fetch_add(1, Ordering::SeqCst);
    LISTENERS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .push((id, Arc::new(listener)));
    ControlEventSubscription(id)
}

/// The session in progress, if an agent is driving the product right now.
pub fn current_session() -> Option<u64> {
    SESSION
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .as_ref()
        .map(|live| live.id)
}

/// Note one answered request that arrived at `began`.
pub(crate) fn record(method: &str, error: Option<&str>, began: Instant) {
    if method == lingxia_control_protocol::methods::ECHO {
        return;
    }
    let cut_off = *LAST_END.lock().unwrap_or_else(|error| error.into_inner());
    if cut_off.is_some_and(|at| began <= at) {
        return;
    }
    let (session, started) = {
        let mut slot = SESSION.lock().unwrap_or_else(|error| error.into_inner());
        match slot.as_mut() {
            Some(live) => {
                live.last = Instant::now();
                (live.id, false)
            }
            None => {
                let id = NEXT_SESSION.fetch_add(1, Ordering::SeqCst);
                *slot = Some(Live {
                    id,
                    last: Instant::now(),
                });
                (id, true)
            }
        }
    };
    if started {
        emit(&ControlEvent::SessionStarted {
            session,
            at_ms: now_ms(),
        });
        watch_for_idle(session);
    }
    let namespace = method
        .split_once('.')
        .map(|(head, _)| head)
        .unwrap_or(method)
        .to_string();
    emit(&ControlEvent::Activity {
        session,
        at_ms: now_ms(),
        method: method.to_string(),
        effect: effect_of(method, &namespace),
        namespace,
        error: error.map(str::to_string),
    });
}

/// End the session in progress, if any.
pub(crate) fn end(reason: ControlSessionEnd) {
    *LAST_END.lock().unwrap_or_else(|error| error.into_inner()) = Some(Instant::now());
    let ended = SESSION
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .take()
        .map(|live| live.id);
    if let Some(session) = ended {
        emit(&ControlEvent::SessionEnded {
            session,
            at_ms: now_ms(),
            reason,
        });
    }
}

fn watch_for_idle(session: u64) {
    let spawned = std::thread::Builder::new()
        .name("lingxia-control-session".to_string())
        .spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(1));
                let idle = {
                    let guard = SESSION.lock().unwrap_or_else(|error| error.into_inner());
                    match guard.as_ref() {
                        Some(live) if live.id == session => live.last.elapsed() >= SESSION_IDLE,
                        // Ended some other way, or a newer session owns the slot.
                        _ => return,
                    }
                };
                if idle {
                    end_if_current(session, ControlSessionEnd::Idle);
                    return;
                }
            }
        });
    if let Err(error) = spawned {
        log::warn!("control session idle watcher unavailable: {error}");
    }
}

fn end_if_current(session: u64, reason: ControlSessionEnd) {
    let mut guard = SESSION.lock().unwrap_or_else(|error| error.into_inner());
    if guard.as_ref().is_some_and(|live| live.id == session) {
        guard.take();
        drop(guard);
        emit(&ControlEvent::SessionEnded {
            session,
            at_ms: now_ms(),
            reason,
        });
    }
}

fn emit(event: &ControlEvent) {
    let listeners: Vec<Listener> = LISTENERS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .iter()
        .map(|(_, listener)| Arc::clone(listener))
        .collect();
    for listener in listeners {
        listener(event);
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}

/// Mirrors the command line's own read/write split, so what the product shows
/// agrees with what the command line would have asked to acknowledge.
fn effect_of(method: &str, namespace: &str) -> ControlEffect {
    use lingxia_control_protocol::methods::{app, browser};

    match namespace {
        "browser" => {
            let reads = matches!(
                method,
                browser::TABS
                    | browser::CURRENT
                    | browser::QUERY
                    | browser::WAIT
                    | browser::WAIT_URL
                    | browser::WAIT_NAVIGATION
                    | browser::SCREENSHOT
                    | browser::UA_SHOW
                    | browser::COOKIES_LIST
                    | browser::NETWORK_LIST
            );
            if reads {
                ControlEffect::Reads
            } else {
                ControlEffect::Changes
            }
        }
        "app" => {
            let reads = matches!(method, app::DOCTOR | app::SCREENSHOT | app::WINDOWS);
            if reads {
                ControlEffect::Reads
            } else {
                ControlEffect::Changes
            }
        }
        "desktop" => desktop_effect(method),
        _ if crate::extra::is_registered_host_namespace(namespace) => ControlEffect::Unclassified,
        // Anything else is refused before dispatch; count the attempt as a
        // change so a disclosure never understates it.
        _ => ControlEffect::Changes,
    }
}

#[cfg(feature = "computer-use")]
fn desktop_effect(method: &str) -> ControlEffect {
    if crate::desktop::changes_machine(method) {
        ControlEffect::Changes
    } else {
        ControlEffect::Reads
    }
}

#[cfg(not(feature = "computer-use"))]
fn desktop_effect(_method: &str) -> ControlEffect {
    ControlEffect::Changes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_reads_and_changes_follow_the_command_line() {
        assert_eq!(effect_of("browser.tabs", "browser"), ControlEffect::Reads);
        assert_eq!(
            effect_of("browser.screenshot", "browser"),
            ControlEffect::Reads
        );
        assert_eq!(
            effect_of("browser.click", "browser"),
            ControlEffect::Changes
        );
        assert_eq!(effect_of("browser.open", "browser"), ControlEffect::Changes);
        assert_eq!(
            effect_of("browser.cookies.clear", "browser"),
            ControlEffect::Changes
        );
        assert_eq!(effect_of("app.windows", "app"), ControlEffect::Reads);
        assert_eq!(effect_of("app.mouse", "app"), ControlEffect::Changes);
        assert_eq!(effect_of("lxapp.eval", "lxapp"), ControlEffect::Changes);
    }

    #[test]
    fn events_serialize_for_a_product_page() {
        let event = ControlEvent::Activity {
            session: 3,
            at_ms: 10,
            method: "browser.click".to_string(),
            namespace: "browser".to_string(),
            effect: ControlEffect::Changes,
            error: None,
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["kind"], "activity");
        assert_eq!(json["atMs"], 10);
        assert_eq!(json["effect"], "changes");

        let ended = ControlEvent::SessionEnded {
            session: 3,
            at_ms: 11,
            reason: ControlSessionEnd::Stopped,
        };
        let json = serde_json::to_value(&ended).unwrap();
        assert_eq!(json["kind"], "sessionEnded");
        assert_eq!(json["reason"], "stopped");
    }
}
