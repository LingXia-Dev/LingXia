//! Inline native island host for Windows.
//!
//! Island nodes share the page protocol ([`lxapp::inline_native::IslandSession`])
//! and never use `HWND_TOP` restacking. Video leaves reuse MFPlay keyed by
//! author id so `lx.createVideoContext` reaches them. Live DComp attach of
//! those nodes exists on the WebView DComp tree API but is not driven from
//! this path: mutating that shared device stalls the page UI thread.

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

use lingxia_webview::WebViewController;
use lxapp::inline_native::{
    ApplyCommitOutcome, IslandCompositor, IslandSession, NativeGeometrySnapshot, NativeRootAck,
    Rect, is_island_action,
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
                materialize_island(context, session);
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
                materialize_island(context, session);
            }
        }
        "root.leaseAccept" | "video.command" => {
            let _ = session.handle_view_json(message);
            materialize_island(context, session);
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

fn materialize_island(context: &PageContext, session: &IslandSession) {
    let mut compositor = DcompIslandCompositor::new(context, session);
    session.materialize_into(&mut compositor);
    compositor.commit();
}

struct PendingAttach {
    id: String,
    kind: String,
    rect: Rect,
}

struct DcompIslandCompositor<'a> {
    context: &'a PageContext,
    session: &'a IslandSession,
    pending: Vec<PendingAttach>,
}

impl<'a> DcompIslandCompositor<'a> {
    fn new(context: &'a PageContext, session: &'a IslandSession) -> Self {
        Self {
            context,
            session,
            pending: Vec::new(),
        }
    }

    fn commit(self) {
        let context = self.context.clone();
        let pending = self.pending;
        let nodes = self.session.composition_nodes();
        let Some(parent) = super::parent_window_for_page(&context.page_key) else {
            if !pending.is_empty() {
                log::debug!(
                    "no host window for {}; island visuals not attached",
                    context.page_key
                );
            }
            return;
        };
        super::run_on_window_thread(parent, move || {
            finish_island_commit(&context, &nodes, pending);
        });
    }
}

fn finish_island_commit(
    context: &PageContext,
    nodes: &[lxapp::inline_native::IslandPaintNode],
    pending: Vec<PendingAttach>,
) {
    for attach in &pending {
        if attach.kind != "video" {
            continue;
        }
        let props = nodes
            .iter()
            .find(|node| {
                node.author_id.as_deref() == Some(attach.id.as_str())
                    || node.node_ref.node_key == attach.id
            })
            .map(|node| &node.props);
        let _ = super::ensure_island_video(
            context,
            &attach.id,
            props.unwrap_or(&Value::Null),
            Some(DocRect {
                x: attach.rect.x,
                y: attach.rect.y,
                width: attach.rect.width,
                height: attach.rect.height,
            }),
        );
    }
}

impl IslandCompositor for DcompIslandCompositor<'_> {
    fn attach_above_webview(&mut self, id: &str, kind: &str, rect: &Rect) {
        self.pending.push(PendingAttach {
            id: id.to_string(),
            kind: kind.to_string(),
            rect: rect.clone(),
        });
    }

    fn order(&self) -> Vec<String> {
        self.pending.iter().map(|item| item.id.clone()).collect()
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
        let cover = json!({
            "action": "root.commit",
            "root": {
                "surfaceInstanceId": "s",
                "pageInstanceId": "p",
                "documentInstanceId": "d",
                "rootKey": "player",
                "rootEpoch": 1
            },
            "baseRevision": 1,
            "revision": 2,
            "operations": [
                {
                    "op": "mount",
                    "node": {
                        "ref": {
                            "surfaceInstanceId": "s",
                            "pageInstanceId": "p",
                            "documentInstanceId": "d",
                            "rootKey": "player",
                            "rootEpoch": 1,
                            "nodeKey": "cover",
                            "nodeEpoch": 1
                        },
                        "kind": "view",
                        "parent": null,
                        "order": 1,
                        "authorType": "LxNativeCover",
                        "authorId": "cover",
                        "props": {}
                    }
                },
                {
                    "op": "mount",
                    "node": {
                        "ref": {
                            "surfaceInstanceId": "s",
                            "pageInstanceId": "p",
                            "documentInstanceId": "d",
                            "rootKey": "player",
                            "rootEpoch": 1,
                            "nodeKey": "title",
                            "nodeEpoch": 1
                        },
                        "kind": "text",
                        "parent": {
                            "surfaceInstanceId": "s",
                            "pageInstanceId": "p",
                            "documentInstanceId": "d",
                            "rootKey": "player",
                            "rootEpoch": 1,
                            "nodeKey": "cover",
                            "nodeEpoch": 1
                        },
                        "order": 0,
                        "authorType": "LxNativeText",
                        "authorId": "title",
                        "props": { "text": "Inline native" }
                    }
                }
            ]
        });
        assert!(handle_island_message(&context, &cover));
        {
            let mut guard = sessions().lock().unwrap();
            let session = guard.get_mut(page).unwrap();
            session.apply_geometry(NativeGeometrySnapshot {
                action: "geometry.snapshot".into(),
                surface_instance_id: "s".into(),
                page_instance_id: "p".into(),
                document_instance_id: "d".into(),
                revision: 3,
                coordinate_space: "page-unscrolled-css-px".into(),
                roots: vec![],
                nodes: vec![],
                chains: vec![],
            });
            let mut recorder = AttachRecorder { calls: Vec::new() };
            session.materialize_into(&mut recorder);
            assert_eq!(
                recorder.order(),
                vec![
                    "lx-video-1".to_string(),
                    "cover".to_string(),
                    "title".to_string()
                ]
            );
            let kinds: Vec<&str> = recorder
                .calls
                .iter()
                .map(|(_, kind, _)| kind.as_str())
                .collect();
            assert_eq!(kinds, ["video", "view", "text"]);
        }
        teardown_island(page);
    }

    struct AttachRecorder {
        calls: Vec<(String, String, Rect)>,
    }

    impl IslandCompositor for AttachRecorder {
        fn attach_above_webview(&mut self, id: &str, kind: &str, rect: &Rect) {
            self.calls
                .push((id.to_string(), kind.to_string(), rect.clone()));
        }

        fn order(&self) -> Vec<String> {
            self.calls.iter().map(|(id, _, _)| id.clone()).collect()
        }
    }
}
