//! Inline native island host for Windows.
//!
//! Island nodes live in the WebView2 composition domain. They never use
//! `HWND_TOP` or a second windowed z-order; the shared [`lxapp::inline_native::IslandSession`]
//! is the source of sibling / document order. Video leaves reuse the existing
//! MFPlay player, keyed by author id so `lx.createVideoContext` reaches them.

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

use lingxia_webview::WebViewController;
use lxapp::inline_native::{
    ApplyCommitOutcome, IslandSession, NativeGeometrySnapshot, NativeRootAck, is_island_action,
};
use serde_json::{Value, json};

use super::{DocRect, PageContext};

static SESSIONS: OnceLock<Mutex<HashMap<String, IslandSession>>> = OnceLock::new();
static ISLAND_COMPONENT_KEYS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

fn sessions() -> &'static Mutex<HashMap<String, IslandSession>> {
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(super) fn island_component_keys() -> std::sync::MutexGuard<'static, HashSet<String>> {
    ISLAND_COMPONENT_KEYS
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

pub(super) fn is_island_component_key(key: &str) -> bool {
    island_component_keys().contains(key)
}

pub(super) fn handle_island_message(context: &PageContext, message: &Value) -> bool {
    let Some(action) = message.get("action").and_then(Value::as_str) else {
        return false;
    };
    if !is_island_action(action) {
        return false;
    }
    let mut sessions = sessions().lock().expect("island session lock");
    let session = sessions.entry(context.page_key.clone()).or_default();
    debug_assert!(!session.uses_hwnd_zorder());
    if let Some(app) = lxapp::try_get(&context.appid) {
        session.set_trusted_domains(app.trusted_network_domains(), lxapp::is_dev_session());
    }
    match action {
        "root.commit" => match session.apply_commit_json(message) {
            Ok(ApplyCommitOutcome::Applied(NativeRootAck::Applied { revision, .. })) => {
                log::debug!(
                    "inline native root applied revision {revision} on {}",
                    context.page_key
                );
                materialize_videos(context, session);
            }
            Ok(ApplyCommitOutcome::ResyncRequired(NativeRootAck::ResyncRequired {
                last_applied_revision,
                ..
            })) => {
                log::warn!(
                    "inline native root on {} needs resync from {last_applied_revision}",
                    context.page_key
                );
            }
            Ok(ApplyCommitOutcome::Rejected(err)) => {
                log::warn!(
                    "inline native commit rejected on {}: {}",
                    context.page_key,
                    err.message
                );
            }
            Ok(_) => {}
            Err(err) => log::warn!(
                "inline native commit parse failed on {}: {err}",
                context.page_key
            ),
        },
        "geometry.snapshot" => {
            if let Ok(snapshot) = serde_json::from_value::<NativeGeometrySnapshot>(message.clone())
            {
                let _ = session.apply_geometry(snapshot);
                materialize_videos(context, session);
            }
        }
        "root.leaseAccept" | "video.command" => {
            let _ = session.handle_view_json(message);
            materialize_videos(context, session);
        }
        other => {
            log::debug!("inline native action '{other}' on {}", context.page_key);
        }
    }
    let outgoing = session.drain_view_messages();
    drop(sessions);
    for payload in outgoing {
        post_island_payload(context, payload);
    }
    true
}

pub(super) fn teardown_island(page_key: &str) {
    if let Ok(mut sessions) = sessions().lock() {
        sessions.remove(page_key);
    }
    island_component_keys().retain(|key| !key.starts_with(page_key));
}

fn materialize_videos(context: &PageContext, session: &IslandSession) {
    if !session.can_display_any() {
        return;
    }
    for video in session.video_nodes() {
        let Some(author_id) = video.author_id.clone() else {
            continue;
        };
        let rect = session
            .last_node_rect(&video.node_ref.node_key)
            .map(|content| DocRect {
                x: content.x,
                y: content.y,
                width: content.width,
                height: content.height,
            });
        super::ensure_island_video(context, &author_id, &video.props, rect);
    }
}

fn post_island_payload(context: &PageContext, payload: Value) {
    let view_message = json!({
        "type": "event",
        "name": "nativecomponent",
        "payload": payload,
    })
    .to_string();
    let page = lxapp::try_get(&context.appid).and_then(|app| app.get_page(&context.path));
    if let Some(page) = page
        && let Some(webview) = page.webview()
    {
        let _ = webview.post_message(&view_message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_a_root_commit_into_the_shared_session() {
        let page = "test-page-island";
        teardown_island(page);
        let context = PageContext {
            page_key: page.to_string(),
            appid: "missing-app".to_string(),
            path: "pages/video/index".to_string(),
        };
        {
            let mut sessions = sessions().lock().unwrap();
            sessions
                .entry(page.to_string())
                .or_default()
                .set_trusted_domains(vec!["cdn.example.com".into()], true);
        }
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
                    "authorId": "lx-video-1",
                    "props": { "src": "https://cdn.example.com/a.mp4" }
                }
            }]
        });
        assert!(handle_island_message(&context, &message));
        {
            let mut guard = sessions().lock().unwrap();
            let session = guard.get_mut(page).unwrap();
            assert!(!session.uses_hwnd_zorder());
            assert_eq!(session.composition_order().len(), 1);
            assert_eq!(session.composition_order()[0].node_key, "video");
            assert_eq!(
                session.video_nodes()[0].author_id.as_deref(),
                Some("lx-video-1")
            );
            assert!(!session.can_display_any());
        }
        let accept = json!({
            "action": "root.leaseAccept",
            "root": {
                "surfaceInstanceId": "s",
                "pageInstanceId": "p",
                "documentInstanceId": "d",
                "rootKey": "player",
                "rootEpoch": 1
            },
            "leaseId": "lease-player",
            "sequence": 1
        });
        assert!(handle_island_message(&context, &accept));
        {
            let guard = sessions().lock().unwrap();
            assert!(guard.get(page).unwrap().can_display_any());
        }
        teardown_island(page);
    }
}
