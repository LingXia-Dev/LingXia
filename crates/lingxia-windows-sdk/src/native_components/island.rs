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
    IslandPointerPhase as SurfacePointerPhase, IslandVideoFrame, IslandVisualSpec,
    find_webview_handler, queue_island_visuals, queued_island_visuals, set_island_pointer_filter,
};
use lxapp::inline_native::{
    ApplyCommitOutcome, IslandCompositor, IslandPointerPhase, IslandSession,
    NativeGeometrySnapshot, NativeRootAck, Rect, is_island_action, plan_island_visual,
    props_with_slider_value, rasterize_island_kind,
};
use serde_json::{Value, json};

use super::{DocRect, PageContext};

static SESSIONS: OnceLock<Mutex<HashMap<String, IslandSession>>> = OnceLock::new();
static CONTEXTS: OnceLock<Mutex<HashMap<String, PageContext>>> = OnceLock::new();
static LAST_ATTACHES: OnceLock<Mutex<HashMap<String, Vec<PendingAttach>>>> = OnceLock::new();
static ISLAND_COMPONENT_KEYS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
static APPLY_SCHEDULED: AtomicBool = AtomicBool::new(false);
static PENDING_APPLIES: OnceLock<Mutex<HashMap<String, DeferredIslandApply>>> = OnceLock::new();
static POINTER_FILTER_READY: AtomicBool = AtomicBool::new(false);

struct DeferredIslandApply {
    context: PageContext,
    nodes: Vec<lxapp::inline_native::IslandPaintNode>,
    pending: Vec<PendingAttach>,
}

fn pending_applies() -> &'static Mutex<HashMap<String, DeferredIslandApply>> {
    PENDING_APPLIES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn sessions() -> &'static Mutex<HashMap<String, IslandSession>> {
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn contexts() -> &'static Mutex<HashMap<String, PageContext>> {
    CONTEXTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn last_attaches() -> &'static Mutex<HashMap<String, Vec<PendingAttach>>> {
    LAST_ATTACHES.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(super) fn install_island_pointer_filter() {
    if POINTER_FILTER_READY.swap(true, Ordering::SeqCst) {
        return;
    }
    set_island_pointer_filter(route_island_pointer);
}

fn route_island_pointer(page_key: &str, phase: SurfacePointerPhase, x: f32, y: f32) -> bool {
    let session_phase = match phase {
        SurfacePointerPhase::Down => IslandPointerPhase::Down,
        SurfacePointerPhase::Move => IslandPointerPhase::Move,
        SurfacePointerPhase::Up => IslandPointerPhase::Up,
        SurfacePointerPhase::Cancel => IslandPointerPhase::Cancel,
    };
    let (css_x, css_y) = surface_point_to_css(page_key, x, y);
    let mut sessions = match sessions().lock() {
        Ok(guard) => guard,
        Err(poison) => poison.into_inner(),
    };
    let Some(session) = sessions.get_mut(page_key) else {
        return false;
    };
    let was_active = session.pointer_sequence_active();
    let events = session.handle_pointer(session_phase, css_x, css_y);
    let consumed = was_active || session.pointer_sequence_active() || !events.is_empty();
    log::debug!(
        "inline native pointer {session_phase:?} on {page_key}: surface=({x:.1},{y:.1}) css=({css_x:.1},{css_y:.1}) active={was_active} events={} consumed={consumed}",
        events.len(),
    );
    let latched = session.latched_slider().or_else(|| {
        events.iter().rev().find_map(|event| {
            if event.event == "valuechange" || event.event == "valuecommit" {
                event
                    .detail
                    .get("value")
                    .and_then(Value::as_f64)
                    .map(|value| (event.id.clone(), value))
            } else {
                None
            }
        })
    });
    drop(sessions);
    match session_phase {
        IslandPointerPhase::Cancel => rematerialize_pending(page_key, None),
        _ => {
            if let Some((id, value)) = latched {
                rematerialize_pending(page_key, Some((id.as_str(), value)));
            }
        }
    }
    if let Some(context) = contexts()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .get(page_key)
        .cloned()
    {
        for event in events {
            post_island_event(&context, &event);
        }
    }
    consumed
}

fn surface_point_to_css(page_key: &str, x: f32, y: f32) -> (f64, f64) {
    let view = super::page_views().get(page_key).copied();
    let scale = view
        .map(|view| view.target.scale)
        .filter(|scale| *scale > 0.0)
        .unwrap_or(1.0);
    let scroll_x = view.map(|view| view.scroll_x).unwrap_or(0.0);
    let scroll_y = view.map(|view| view.scroll_y).unwrap_or(0.0);
    (
        f64::from(x) / scale + scroll_x,
        f64::from(y) / scale + scroll_y,
    )
}

fn post_island_event(context: &PageContext, event: &lxapp::inline_native::IslandHostEvent) {
    let context = context.clone();
    let component_id = event.id.clone();
    let payload = json!({
        "action": "component.event",
        "id": component_id.clone(),
        "componentId": component_id.clone(),
        "event": event.event,
        "detail": event.detail,
        "pageId": format!("{}:{}", context.appid, context.path),
    });
    let thread_name = format!("lingxia-island-event-{component_id}");
    if let Err(err) = std::thread::Builder::new()
        .name(thread_name)
        .spawn(move || {
            post_island_payload(&context, payload);
        })
    {
        log::warn!("failed to spawn inline-native event thread: {err}");
    }
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
    if let Ok(mut contexts) = contexts().lock() {
        contexts.insert(context.page_key.clone(), context.clone());
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
        "root.destroy" | "root.leaseAccept" | "video.command" => {
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
    if let Ok(mut contexts) = contexts().lock() {
        contexts.remove(page_key);
    }
    if let Ok(mut attaches) = last_attaches().lock() {
        attaches.remove(page_key);
    }
    pending_applies()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .remove(page_key);
    let prefix = format!("{page_key}\u{1}");
    island_component_keys().retain(|key| !key.starts_with(&prefix));
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
    props: Value,
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
    let pending = preserve_measured_rects(&context.page_key, pending);
    let mut specs = Vec::with_capacity(pending.len());
    for attach in &pending {
        let props = nodes
            .iter()
            .find(|node| {
                node.author_id.as_deref() == Some(attach.id.as_str())
                    || node.node_ref.node_key == attach.id
            })
            .map(|node| node.props.clone())
            .unwrap_or_else(|| attach.props.clone());
        let (offset_x, offset_y, dest_width, dest_height) =
            surface_rect(&context.page_key, &attach.rect);
        specs.push(build_island_visual_spec(
            &attach.id,
            &attach.kind,
            &attach.rect,
            &props,
            offset_x,
            offset_y,
            dest_width as f32,
            dest_height as f32,
        ));
    }
    // Queue only. MFPlay and the geometry replay run after WebView2 has
    // left this web-message callback — a same-turn DComp Commit or EVR
    // HWND on this stack stalls eval and the dev websocket.
    last_attaches()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .insert(context.page_key.clone(), pending.clone());
    queue_island_visuals(&context.page_key, specs);
    let queued = queued_island_visuals(&context.page_key).len();
    log::info!(
        "queued {queued} island visuals for {} (deferred apply)",
        context.page_key
    );
    schedule_island_apply(context.clone(), nodes.to_vec(), pending);
}

fn schedule_island_apply(
    context: PageContext,
    nodes: Vec<lxapp::inline_native::IslandPaintNode>,
    pending: Vec<PendingAttach>,
) {
    let page_key = context.page_key.clone();
    pending_applies()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .insert(
            page_key,
            DeferredIslandApply {
                context,
                nodes,
                pending,
            },
        );
    if APPLY_SCHEDULED.swap(true, Ordering::SeqCst) {
        return;
    }
    if let Err(err) = std::thread::Builder::new()
        .name("lingxia-island-apply".into())
        .spawn(|| {
            std::thread::sleep(std::time::Duration::from_millis(48));
            loop {
                let jobs = {
                    let mut jobs = pending_applies()
                        .lock()
                        .unwrap_or_else(|poison| poison.into_inner());
                    if jobs.is_empty() {
                        // Publish the idle state while holding the queue lock.
                        // A racing producer inserts only after this unlock and
                        // will therefore observe `false` and start a new drain.
                        APPLY_SCHEDULED.store(false, Ordering::SeqCst);
                        return;
                    }
                    std::mem::take(&mut *jobs)
                };
                for (_, job) in jobs {
                    let page_key = job.context.page_key.clone();
                    let Some(parent) = super::parent_window_for_page(&page_key) else {
                        log::debug!("no host window for {page_key}; deferred island apply dropped");
                        continue;
                    };
                    let posted = super::run_on_window_thread(parent, move || {
                        let active = contexts()
                            .lock()
                            .unwrap_or_else(|poison| poison.into_inner())
                            .contains_key(&job.context.page_key);
                        if !active {
                            return;
                        }
                        apply_queued_island_visuals(&job.context.page_key);
                        mount_pending_island_videos(&job.context, &job.nodes, &job.pending);
                    });
                    if !posted {
                        log::debug!("host window disappeared before island apply on {page_key}");
                    }
                }
            }
        })
    {
        APPLY_SCHEDULED.store(false, Ordering::SeqCst);
        log::warn!("failed to spawn deferred island apply worker: {err}");
    }
}

fn apply_queued_island_visuals(page_key: &str) {
    let visuals = queued_island_visuals(page_key);
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
    let live_ids: HashSet<String> = pending
        .iter()
        .filter(|attach| attach.kind == "video")
        .map(|attach| attach.id.clone())
        .collect();
    super::retain_island_videos(&context.page_key, &live_ids);
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
    let tex_w = dest_w.clamp(1, 640) as u32;
    let tex_h = dest_h.clamp(1, 360) as u32;
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

fn rematerialize_pending(page_key: &str, latch: Option<(&str, f64)>) {
    let pending = last_attaches()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .get(page_key)
        .cloned()
        .unwrap_or_default();
    if pending.is_empty() {
        return;
    }
    let specs = island_visuals_from_pending(page_key, &pending, latch);
    queue_island_visuals(page_key, specs);
    apply_queued_island_visuals(page_key);
}

fn island_visuals_from_pending(
    page_key: &str,
    pending: &[PendingAttach],
    latch: Option<(&str, f64)>,
) -> Vec<IslandVisualSpec> {
    pending
        .iter()
        .map(|attach| {
            let props = match latch {
                Some((id, value)) if attach.id == id && attach.kind == "slider" => {
                    props_with_slider_value(&attach.props, value)
                }
                _ => attach.props.clone(),
            };
            let (offset_x, offset_y, dest_width, dest_height) =
                surface_rect(page_key, &attach.rect);
            build_island_visual_spec(
                &attach.id,
                &attach.kind,
                &attach.rect,
                &props,
                offset_x,
                offset_y,
                dest_width as f32,
                dest_height as f32,
            )
        })
        .collect()
}

pub(crate) fn build_island_visual_spec(
    id: &str,
    kind: &str,
    css: &Rect,
    props: &Value,
    offset_x: f32,
    offset_y: f32,
    dest_width: f32,
    dest_height: f32,
) -> IslandVisualSpec {
    let plan = plan_island_visual(kind, css, props);
    let width = dest_width.round().clamp(1.0, 640.0) as i32;
    let height = dest_height.round().clamp(1.0, 360.0) as i32;
    let pixels = match kind {
        "video" => None,
        _ => Some(rasterize_island_kind(kind, width, height, props)),
    };
    IslandVisualSpec {
        id: id.to_string(),
        kind: kind.to_string(),
        offset_x,
        offset_y,
        width,
        height,
        dest_width,
        dest_height,
        color: plan.color,
        text: plan.text,
        hwnd: None,
        pixels,
    }
}

fn rect_is_measured(rect: &Rect) -> bool {
    rect.width >= 2.0 && rect.height >= 2.0
}

/// A later geometry snapshot can report 0×0 / 1×1 (hidden document, or
/// measure-before-layout). Keep the last measured dest so the visual does
/// not collapse.
fn preserve_measured_rects(page_key: &str, pending: Vec<PendingAttach>) -> Vec<PendingAttach> {
    let previous = last_attaches()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .get(page_key)
        .cloned()
        .unwrap_or_default();
    pending
        .into_iter()
        .map(|mut attach| {
            if !rect_is_measured(&attach.rect)
                && let Some(prev) = previous.iter().find(|item| item.id == attach.id)
                && rect_is_measured(&prev.rect)
            {
                attach.rect = prev.rect.clone();
            }
            attach
        })
        .collect()
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
    fn attach_above_webview(&mut self, id: &str, kind: &str, rect: &Rect, props: &Value) {
        self.pending.push(PendingAttach {
            id: id.to_string(),
            kind: kind.to_string(),
            rect: rect.clone(),
            props: props.clone(),
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
        && let Err(err) = webview.post_message(&view_message)
    {
        log::debug!("failed to post inline-native event to view: {err}");
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

    #[test]
    fn preserve_measured_rects_keeps_last_good_size() {
        let page = "test-page-preserve-rect";
        teardown_island(page);
        last_attaches().lock().unwrap().insert(
            page.to_string(),
            vec![PendingAttach {
                id: "lx-video-1".into(),
                kind: "video".into(),
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 640.0,
                    height: 360.0,
                },
                props: json!({}),
            }],
        );
        let next = preserve_measured_rects(
            page,
            vec![PendingAttach {
                id: "lx-video-1".into(),
                kind: "video".into(),
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 0.0,
                    height: 0.0,
                },
                props: json!({}),
            }],
        );
        assert_eq!(next[0].rect.width, 640.0);
        assert_eq!(next[0].rect.height, 360.0);
        teardown_island(page);
    }

    #[test]
    fn island_visual_builder_uses_committed_css_rects() {
        let rect = Rect {
            x: 0.0,
            y: 40.0,
            width: 320.0,
            height: 80.0,
        };
        let cover = build_island_visual_spec(
            "cover",
            "view",
            &rect,
            &json!({ "scrimPaint": { "scrim": "bottom", "opacity": 0.6 } }),
            0.0,
            40.0,
            320.0,
            80.0,
        );
        assert!((cover.dest_width - 320.0).abs() < f32::EPSILON);
        assert!((cover.dest_height - 80.0).abs() < f32::EPSILON);
        assert_ne!((cover.width, cover.height), (16, 16));
        assert!(
            cover
                .pixels
                .as_ref()
                .is_some_and(|pixels| !pixels.is_empty())
        );

        let button = build_island_visual_spec(
            "play",
            "tappable",
            &Rect {
                x: 16.0,
                y: 180.0,
                width: 48.0,
                height: 32.0,
            },
            &json!({ "content": { "text": "Play" } }),
            16.0,
            180.0,
            48.0,
            32.0,
        );
        assert!((button.dest_width - 48.0).abs() < f32::EPSILON);
        assert_eq!(button.text.as_deref(), Some("Play"));

        let slider = build_island_visual_spec(
            "seek",
            "slider",
            &Rect {
                x: 80.0,
                y: 188.0,
                width: 200.0,
                height: 16.0,
            },
            &json!({ "min": 0, "max": 100, "value": 25, "step": 1, "valueLabel": "value" }),
            80.0,
            188.0,
            200.0,
            16.0,
        );
        assert!((slider.dest_width - 200.0).abs() < f32::EPSILON);
        assert_ne!((slider.width, slider.height), (16, 16));
        assert_eq!(slider.text.as_deref(), Some("25"));

        let committed =
            json!({ "min": 0, "max": 100, "value": 10, "step": 1, "valueLabel": "value" });
        let pending = vec![PendingAttach {
            id: "seek".into(),
            kind: "slider".into(),
            rect: Rect {
                x: 80.0,
                y: 188.0,
                width: 200.0,
                height: 16.0,
            },
            props: committed.clone(),
        }];
        let latched =
            island_visuals_from_pending("test-page-latch", &pending, Some(("seek", 80.0)));
        let committed_specs = island_visuals_from_pending("test-page-latch", &pending, None);
        assert_eq!(latched[0].text.as_deref(), Some("80"));
        assert_eq!(committed_specs[0].text.as_deref(), Some("10"));
        assert_ne!(
            latched[0].pixels.as_ref(),
            committed_specs[0].pixels.as_ref(),
            "latched slider raster must move the thumb without a new root.commit"
        );
        let overlaid = props_with_slider_value(&committed, 80.0);
        let planned = plan_island_visual("slider", &pending[0].rect, &overlaid);
        assert_eq!(planned.text.as_deref(), Some("80"));
    }

    struct AttachRecorder {
        calls: Vec<(String, String, Rect)>,
    }

    impl IslandCompositor for AttachRecorder {
        fn attach_above_webview(&mut self, id: &str, kind: &str, rect: &Rect, _props: &Value) {
            self.calls
                .push((id.to_string(), kind.to_string(), rect.clone()));
        }

        fn order(&self) -> Vec<String> {
            self.calls.iter().map(|(id, _, _)| id.clone()).collect()
        }
    }
}
