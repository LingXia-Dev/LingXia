//! Inline native island host for Windows.
//!
//! Island nodes share the page protocol ([`lxapp::inline_native::IslandSession`])
//! and never use `HWND_TOP` restacking. Visuals are queued for the WebView
//! DComp tree and committed on a later geometry pass — never from inside a
//! WebView2 web-message callback. Video leaves use windowless MFPlay keyed
//! by author id so `lx.createVideoContext` still reaches them.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use lingxia_webview::WebTag;
use lingxia_webview::WebViewController;
use lingxia_webview::platform::windows::{
    IslandVideoFrame, IslandVisualSpec, find_webview_handler, queue_island_visuals,
    queued_island_visuals,
};
use lxapp::inline_native::{
    ApplyCommitOutcome, IslandCompositor, IslandSession, NativeGeometrySnapshot, NativeRootAck,
    Rect, is_island_action,
};
use serde_json::{Value, json};

use super::{DocRect, PageContext};

static SESSIONS: OnceLock<Mutex<HashMap<String, IslandSession>>> = OnceLock::new();
static ISLAND_COMPONENT_KEYS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
static APPLY_SCHEDULED: AtomicBool = AtomicBool::new(false);
static NEXT_APPLY: OnceLock<Mutex<Option<DeferredIslandApply>>> = OnceLock::new();

struct DeferredIslandApply {
    context: PageContext,
    nodes: Vec<lxapp::inline_native::IslandPaintNode>,
    pending: Vec<PendingAttach>,
}

fn next_apply() -> &'static Mutex<Option<DeferredIslandApply>> {
    NEXT_APPLY.get_or_init(|| Mutex::new(None))
}

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

#[derive(Clone)]
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
    let mut specs = Vec::with_capacity(pending.len());
    for attach in &pending {
        let props = nodes
            .iter()
            .find(|node| {
                node.author_id.as_deref() == Some(attach.id.as_str())
                    || node.node_ref.node_key == attach.id
            })
            .map(|node| &node.props);
        let (offset_x, offset_y, full_width, full_height) =
            surface_rect(&context.page_key, &attach.rect);
        let (width, height) = match attach.kind.as_str() {
            "video" => (full_width.min(640).max(1), full_height.min(360).max(1)),
            "text" => (full_width.min(128).max(1), full_height.min(24).max(1)),
            _ => (16, 16),
        };
        let text = props
            .and_then(|props| props.get("text"))
            .and_then(Value::as_str)
            .map(str::to_string);
        specs.push(IslandVisualSpec {
            id: attach.id.clone(),
            kind: attach.kind.clone(),
            offset_x,
            offset_y,
            width,
            height,
            dest_width: if attach.kind == "video" {
                full_width as f32
            } else {
                width as f32
            },
            dest_height: if attach.kind == "video" {
                full_height as f32
            } else {
                height as f32
            },
            color: island_fill_color(&attach.kind),
            text,
            hwnd: None,
            pixels: None,
        });
    }
    // Queue only. MFPlay and the geometry replay run after WebView2 has
    // left this web-message callback — a same-turn DComp Commit or EVR
    // HWND on this stack stalls eval and the dev websocket.
    queue_island_visuals(&context.page_key, specs);
    let queued = queued_island_visuals(&context.page_key).len();
    log::info!(
        "queued {queued} island visuals for {} (deferred apply)",
        context.page_key
    );
    if queued == 0 {
        return;
    }
    schedule_island_apply(context.clone(), nodes.to_vec(), pending);
}

fn schedule_island_apply(
    context: PageContext,
    nodes: Vec<lxapp::inline_native::IslandPaintNode>,
    pending: Vec<PendingAttach>,
) {
    *next_apply()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner()) = Some(DeferredIslandApply {
        context,
        nodes,
        pending,
    });
    if APPLY_SCHEDULED.swap(true, Ordering::SeqCst) {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("lingxia-island-apply".into())
        .spawn(|| {
            std::thread::sleep(std::time::Duration::from_millis(48));
            APPLY_SCHEDULED.store(false, Ordering::SeqCst);
            let job = next_apply()
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .take();
            let Some(job) = job else {
                return;
            };
            apply_queued_island_visuals(&job.context.page_key);
            let Some(parent) = super::parent_window_for_page(&job.context.page_key) else {
                return;
            };
            super::run_on_window_thread(parent, move || {
                mount_pending_island_videos(&job.context, &job.nodes, &job.pending);
            });
        });
}

fn apply_queued_island_visuals(page_key: &str) {
    let visuals = queued_island_visuals(page_key);
    if visuals.is_empty() {
        return;
    }
    let Some(handler) = find_webview_handler(&WebTag::from(page_key)) else {
        log::debug!("no webview handler for {page_key}; island visuals stay queued");
        return;
    };
    match handler.sync_island_visuals(visuals) {
        Ok(()) => log::info!(
            "applied {} island visuals on {page_key}",
            queued_island_visuals(page_key).len()
        ),
        Err(err) => log::warn!("island visual apply failed on {page_key}: {err}"),
    }
}

fn mount_pending_island_videos(
    context: &PageContext,
    nodes: &[lxapp::inline_native::IslandPaintNode],
    pending: &[PendingAttach],
) {
    for attach in pending {
        if attach.kind != "video" {
            continue;
        }
        let props = nodes
            .iter()
            .find(|node| {
                node.author_id.as_deref() == Some(attach.id.as_str())
                    || node.node_ref.node_key == attach.id
            })
            .map(|node| node.props.clone())
            .unwrap_or(Value::Null);
        let _ = super::ensure_island_video(
            context,
            &attach.id,
            &props,
            Some(DocRect {
                x: attach.rect.x,
                y: attach.rect.y,
                width: attach.rect.width,
                height: attach.rect.height,
            }),
        );
    }
}

pub(super) fn present_decoded_island_frame(
    context: &PageContext,
    author_id: &str,
    css: &DocRect,
    player: &crate::video_player::VideoPlayer,
    evr: isize,
) {
    let hwnd = windows::Win32::Foundation::HWND(evr as *mut _);
    let Some((src_w, src_h, pixels)) = crate::video_player::VideoPlayer::capture_window_bgra(hwnd)
        .or_else(|| player.current_frame())
    else {
        return;
    };
    let (offset_x, offset_y, dest_w, dest_h) = surface_rect(
        &context.page_key,
        &Rect {
            x: css.x,
            y: css.y,
            width: css.width,
            height: css.height,
        },
    );
    let tex_w = dest_w.min(640).max(1) as u32;
    let tex_h = dest_h.min(360).max(1) as u32;
    let pixels = crate::video_player::scale_bgra_nearest(&pixels, src_w, src_h, tex_w, tex_h);
    log::debug!(
        "island video frame {} {}x{} -> {}x{} dest {}x{}",
        author_id,
        src_w,
        src_h,
        tex_w,
        tex_h,
        dest_w,
        dest_h
    );
    let Some(handler) = find_webview_handler(&WebTag::from(context.page_key.as_str())) else {
        return;
    };
    let _ = handler.present_island_video_frame(IslandVideoFrame {
        id: author_id.to_string(),
        offset_x,
        offset_y,
        dest_width: dest_w as f32,
        dest_height: dest_h as f32,
        width: tex_w as i32,
        height: tex_h as i32,
        pixels,
    });
}

fn island_fill_color(kind: &str) -> u32 {
    match kind {
        "video" => 0xff10_1010,
        _ => 0x0000_0000,
    }
}

fn surface_rect(page_key: &str, css: &Rect) -> (f32, f32, i32, i32) {
    let view = super::page_views().get(page_key).copied();
    let scale = view
        .map(|view| view.target.scale)
        .filter(|scale| *scale > 0.0)
        .unwrap_or(1.0);
    let scroll_x = view.map(|view| view.scroll_x).unwrap_or(0.0);
    let scroll_y = view.map(|view| view.scroll_y).unwrap_or(0.0);
    let x = ((css.x - scroll_x) * scale) as f32;
    let y = ((css.y - scroll_y) * scale) as f32;
    let width = (css.width * scale).round().max(1.0) as i32;
    let height = (css.height * scale).round().max(1.0) as i32;
    (x, y, width, height)
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
