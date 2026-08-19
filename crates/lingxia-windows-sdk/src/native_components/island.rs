//! Inline native island host for Windows.
//!
//! Island nodes live in the WebView2 composition domain. They never use
//! `HWND_TOP` or a second windowed z-order; the shared [`lxapp::inline_native::IslandSession`]
//! is the source of sibling / document order.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use lxapp::inline_native::{ApplyCommitOutcome, IslandSession, NativeRootAck, is_island_action};
use serde_json::Value;

static SESSIONS: OnceLock<Mutex<HashMap<String, IslandSession>>> = OnceLock::new();

fn sessions() -> &'static Mutex<HashMap<String, IslandSession>> {
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(super) fn handle_island_message(page_key: &str, message: &Value) -> bool {
    let Some(action) = message.get("action").and_then(Value::as_str) else {
        return false;
    };
    if !is_island_action(action) {
        return false;
    }
    let mut sessions = sessions().lock().expect("island session lock");
    let session = sessions
        .entry(page_key.to_string())
        .or_insert_with(IslandSession::new);
    debug_assert!(!session.uses_hwnd_zorder());
    match action {
        "root.commit" => match session.apply_commit_json(message) {
            Ok(ApplyCommitOutcome::Applied(NativeRootAck::Applied { revision, .. })) => {
                log::debug!("inline native root applied revision {revision} on {page_key}");
            }
            Ok(ApplyCommitOutcome::ResyncRequired(NativeRootAck::ResyncRequired {
                last_applied_revision,
                ..
            })) => {
                log::warn!(
                    "inline native root on {page_key} needs resync from {last_applied_revision}"
                );
            }
            Ok(ApplyCommitOutcome::Rejected(err)) => {
                log::warn!(
                    "inline native commit rejected on {page_key}: {}",
                    err.message
                );
            }
            Ok(_) => {}
            Err(err) => log::warn!("inline native commit parse failed on {page_key}: {err}"),
        },
        "geometry.snapshot" => {
            if let Ok(snapshot) = serde_json::from_value(message.clone()) {
                let _ = session.apply_geometry(snapshot);
            }
        }
        other => {
            log::debug!("inline native action '{other}' on {page_key}");
        }
    }
    true
}

pub(super) fn teardown_island(page_key: &str) {
    if let Ok(mut sessions) = sessions().lock() {
        sessions.remove(page_key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn routes_a_root_commit_into_the_shared_session() {
        let page = "test-page-island";
        teardown_island(page);
        let message = json!({
            "action": "root.commit",
            "root": {
                "surfaceInstanceId": "s",
                "pageInstanceId": "p",
                "documentInstanceId": "d",
                "rootKey": "player",
                "rootEpoch": 1
            },
            "baseRevision": 0,
            "revision": 1,
            "operations": [{
                "op": "mount",
                "node": {
                    "ref": {
                        "surfaceInstanceId": "s",
                        "pageInstanceId": "p",
                        "documentInstanceId": "d",
                        "rootKey": "player",
                        "rootEpoch": 1,
                        "nodeKey": "video",
                        "nodeEpoch": 1
                    },
                    "kind": "video",
                    "parent": null,
                    "order": 0,
                    "authorType": "LxVideo",
                    "props": { "src": "./clip.mp4" }
                }
            }]
        });
        assert!(handle_island_message(page, &message));
        let sessions = sessions().lock().unwrap();
        let session = sessions.get(page).unwrap();
        assert!(!session.uses_hwnd_zorder());
        assert_eq!(session.composition_order().len(), 1);
        assert_eq!(session.composition_order()[0].node_key, "video");
        drop(sessions);
        teardown_island(page);
    }
}
