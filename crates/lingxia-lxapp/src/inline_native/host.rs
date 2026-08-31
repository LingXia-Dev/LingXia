use super::apply::{
    ApplyCommitOutcome, HostCapabilities, RootRegistry, apply_root_commit, evaluate_root_ready,
};
use super::geometry::{GeometryPageState, apply_geometry_snapshot, flush_pending_geometry};
use super::lease::{LeaseState, host_can_display, host_grant_lease, host_on_accept};
use super::paint::{
    IslandHitTarget, IslandHostEvent, IslandPointerPhase, IslandPointerTracker, dispatch_pointer,
    pointer_events_from_props, props_with_slider_value,
};
use super::resource::{
    media_urls_from_command_options, media_urls_from_props, validate_media_urls,
};
use super::types::NativeRootOperation;
use super::types::{
    NativeGeometryResult, NativeGeometrySnapshot, NativeRootAck, NativeRootCommit, NodeRef, Rect,
    RootRef,
};
use super::video::{
    VideoCommand, VideoCommandOutcome, VideoCommandQueue, VideoCommandRequest, apply_video_command,
};
use serde_json::Value;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct IslandVideoNode {
    pub node_ref: NodeRef,
    pub author_id: Option<String>,
    pub props: Value,
}

/// One committed island node in composition order, ready for a platform factory.
#[derive(Debug, Clone)]
pub struct IslandPaintNode {
    pub node_ref: NodeRef,
    pub kind: String,
    pub author_id: Option<String>,
    pub author_type: String,
    pub props: Value,
}

/// Platform paint sink. Hosts attach island nodes above the page WebView in
/// committed sibling order. Window z-order (`HWND_TOP`, `SetWindowPos`) is
/// not part of this contract.
pub trait IslandCompositor {
    fn attach_above_webview(&mut self, id: &str, kind: &str, rect: &Rect, props: &Value);
    fn order(&self) -> Vec<String>;
}

/// Shared host session for one page document. Platforms paint from
/// [`IslandSession::composition_order`]; they must not invent HWND_TOP /
/// SurfaceView hole-punch z-order for these nodes.
#[derive(Debug)]
pub struct IslandSession {
    pub registry: RootRegistry,
    pub geometry: GeometryPageState,
    pub command_queue: VideoCommandQueue,
    leases: Vec<(RootRef, LeaseState)>,
    fullscreen_root: Option<RootRef>,
    trusted_domains: Vec<String>,
    dev_session: bool,
    pending_view_messages: Vec<Value>,
    geometry_by_root: HashMap<String, NativeGeometrySnapshot>,
    pointer: IslandPointerTracker,
}

impl IslandSession {
    pub fn new() -> Self {
        Self {
            registry: RootRegistry::new(HostCapabilities::default()),
            geometry: GeometryPageState::default(),
            command_queue: VideoCommandQueue::default(),
            leases: Vec::new(),
            fullscreen_root: None,
            trusted_domains: Vec::new(),
            dev_session: false,
            pending_view_messages: Vec::new(),
            geometry_by_root: HashMap::new(),
            pointer: IslandPointerTracker::default(),
        }
    }

    pub fn set_trusted_domains(&mut self, domains: Vec<String>, dev_session: bool) {
        self.trusted_domains = domains;
        self.dev_session = dev_session;
    }

    /// Island nodes share the WebView composition domain. Windowed HWND z-order
    /// is not a legal implementation path for these nodes.
    pub fn uses_hwnd_zorder(&self) -> bool {
        false
    }

    pub fn apply_commit(&mut self, commit: NativeRootCommit) -> ApplyCommitOutcome {
        if let Err(message) = self.validate_commit_urls(&commit) {
            let error = Box::new(super::types::NativeError {
                code: super::types::NativeErrorCode::InvalidProps,
                scope: super::types::ErrorScope::Root,
                recoverable: true,
                root: commit.root.clone(),
                node: None,
                message,
            });
            self.queue_error(&commit.root, &error);
            return ApplyCommitOutcome::Rejected(error);
        }
        if commit.base_revision == 0 {
            self.leases
                .retain(|(item, _)| !item.same_generation(&commit.root));
        }
        let outcome = apply_root_commit(&mut self.registry, &commit);
        if matches!(outcome, ApplyCommitOutcome::Applied(_)) {
            if self.lease_for(&commit.root).is_none() {
                let (lease, grant) = host_grant_lease(
                    &commit.root,
                    format!("lease-{}", commit.root.root_key),
                    now_ms(),
                );
                self.leases.push((commit.root.clone(), lease));
                self.queue_view_message(&grant);
            }
            flush_pending_geometry(&mut self.registry, &mut self.geometry, &commit.root);
        }
        match &outcome {
            ApplyCommitOutcome::Applied(ack) | ApplyCommitOutcome::ResyncRequired(ack) => {
                self.queue_ack(ack);
            }
            ApplyCommitOutcome::Rejected(error) => {
                self.queue_error(&commit.root, error);
            }
        }
        outcome
    }

    pub fn apply_commit_json(&mut self, value: &Value) -> Result<ApplyCommitOutcome, String> {
        let commit: NativeRootCommit =
            serde_json::from_value(value.clone()).map_err(|err| err.to_string())?;
        Ok(self.apply_commit(commit))
    }

    pub fn apply_geometry(&mut self, snapshot: NativeGeometrySnapshot) -> NativeGeometryResult {
        let result = apply_geometry_snapshot(&mut self.registry, &mut self.geometry, &snapshot);
        for root in &result.roots {
            if root.status == super::types::GeometryRootStatus::Applied {
                self.geometry_by_root
                    .insert(RootRegistry::slot_key(&root.root), snapshot.clone());
            }
        }
        result
    }

    pub fn last_node_rect(&self, node_key: &str) -> Option<Rect> {
        self.geometry_by_root.values().find_map(|snapshot| {
            snapshot
                .nodes
                .iter()
                .find(|node| node.node_ref.node_key == node_key)
                .map(|node| node.content_rect.clone())
        })
    }

    /// Layout used to paint `node`. Prefers the node's geometry snapshot;
    /// a missing or degenerate (0×0 / 1×1) node rect falls back to the
    /// root content rect so the first attach is not a 1×1 placeholder.
    pub fn paint_rect_for_node(&self, node: &IslandPaintNode) -> Rect {
        if let Some(rect) = self.node_rect(&node.node_ref)
            && rect.is_measured()
        {
            return rect;
        }
        if let Some(rect) = self.last_root_content_rect(&node.node_ref) {
            return rect;
        }
        self.node_rect(&node.node_ref).unwrap_or(Rect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        })
    }

    fn last_root_content_rect(&self, node: &NodeRef) -> Option<Rect> {
        self.geometry_for_root(&node.root()).and_then(|snapshot| {
            snapshot
                .roots
                .iter()
                .find(|root| node.same_root(&root.root_ref))
                .map(|root| root.content_rect.clone())
                .filter(Rect::is_measured)
        })
    }

    fn node_rect(&self, node: &NodeRef) -> Option<Rect> {
        self.geometry_for_root(&node.root()).and_then(|snapshot| {
            snapshot
                .nodes
                .iter()
                .find(|entry| entry.node_ref == *node)
                .map(|entry| entry.content_rect.clone())
        })
    }

    fn geometry_for_root(&self, root: &RootRef) -> Option<&NativeGeometrySnapshot> {
        self.geometry_by_root.get(&RootRegistry::slot_key(root))
    }

    pub fn destroy_root(&mut self, root: &RootRef) -> bool {
        let destroyed = self.registry.destroy(root).is_some();
        self.geometry_by_root.remove(&RootRegistry::slot_key(root));
        self.leases
            .retain(|(candidate, _)| !candidate.same_generation(root));
        if self
            .fullscreen_root
            .as_ref()
            .is_some_and(|candidate| candidate.same_generation(root))
        {
            self.fullscreen_root = None;
        }
        self.pointer.cancel();
        destroyed
    }

    pub fn accept_lease(&mut self, root: &RootRef, lease_id: &str, sequence: u64) -> bool {
        let Some(active) = self
            .lease_for_mut(root)
            .and_then(|lease| host_on_accept(lease, root, lease_id, sequence))
        else {
            return false;
        };
        let lease = self.lease_for(root).cloned();
        self.queue_view_message(&active);
        if let (Some(state), Some(lease)) = (self.registry.get_mut(root), lease) {
            state.lease = lease;
            evaluate_root_ready(state);
        }
        true
    }

    pub fn handle_view_json(&mut self, value: &Value) -> bool {
        match value.get("action").and_then(Value::as_str) {
            Some("root.leaseAccept") => {
                let Some(root) = value
                    .get("root")
                    .and_then(|raw| serde_json::from_value::<RootRef>(raw.clone()).ok())
                else {
                    return false;
                };
                let Some(lease_id) = value.get("leaseId").and_then(Value::as_str) else {
                    return false;
                };
                let sequence = value.get("sequence").and_then(Value::as_u64).unwrap_or(0);
                self.accept_lease(&root, lease_id, sequence)
            }
            Some("video.command") => {
                if let Ok(request) = serde_json::from_value::<VideoCommandRequest>(value.clone()) {
                    let _ = self.apply_video_command(request);
                    return true;
                }
                false
            }
            Some("root.destroy") => {
                let Some(root) = value
                    .get("root")
                    .and_then(|raw| serde_json::from_value::<RootRef>(raw.clone()).ok())
                else {
                    return false;
                };
                self.destroy_root(&root)
            }
            _ => false,
        }
    }

    pub fn drain_view_messages(&mut self) -> Vec<Value> {
        std::mem::take(&mut self.pending_view_messages)
    }

    pub fn composition_nodes(&self) -> Vec<IslandPaintNode> {
        let mut nodes = Vec::new();
        for node_ref in self.composition_order() {
            for state in self.registry.roots() {
                let Some(node) = state.nodes.get(&node_ref.node_key) else {
                    continue;
                };
                if node.node_ref.node_epoch != node_ref.node_epoch {
                    continue;
                }
                nodes.push(IslandPaintNode {
                    node_ref: node.node_ref.clone(),
                    kind: node.kind.clone(),
                    author_id: node.author_id.clone(),
                    author_type: node.author_type.clone(),
                    props: node.props.clone(),
                });
                break;
            }
        }
        nodes
    }

    pub fn video_nodes(&self) -> Vec<IslandVideoNode> {
        let mut nodes = Vec::new();
        for state in self.registry.roots() {
            for node in state.nodes.values() {
                if node.kind != "video" {
                    continue;
                }
                nodes.push(IslandVideoNode {
                    node_ref: node.node_ref.clone(),
                    author_id: node.author_id.clone(),
                    props: node.props.clone(),
                });
            }
        }
        nodes
    }

    pub fn apply_video_command(&mut self, request: VideoCommandRequest) -> VideoCommandOutcome {
        if let VideoCommand::SetStreamSource { options } = &request.command {
            let urls = media_urls_from_command_options(options);
            if let Err(message) =
                validate_media_urls(&urls, &self.trusted_domains, self.dev_session)
            {
                return VideoCommandOutcome::Rejected(Box::new(super::types::NativeError {
                    code: super::types::NativeErrorCode::InvalidProps,
                    scope: super::types::ErrorScope::Node,
                    recoverable: true,
                    root: request.owner.root(),
                    node: Some(request.owner.clone()),
                    message,
                }));
            }
        }
        apply_video_command(&self.registry, &mut self.command_queue, request)
    }

    pub fn set_fullscreen(&mut self, root: &RootRef, enabled: bool) -> Result<(), String> {
        if enabled {
            if let Some(current) = &self.fullscreen_root
                && !current.same_generation(root)
            {
                return Err("a Root already owns fullscreen on this surface".into());
            }
            self.fullscreen_root = Some(root.clone());
        } else if self
            .fullscreen_root
            .as_ref()
            .is_some_and(|current| current.same_generation(root))
        {
            self.fullscreen_root = None;
        }
        Ok(())
    }

    pub fn fullscreen_root(&self) -> Option<&RootRef> {
        self.fullscreen_root.as_ref()
    }

    /// Document order of roots, then committed sibling order inside each root.
    /// A fullscreen root is raised to the end (highest) but stays below host chrome.
    pub fn composition_order(&self) -> Vec<NodeRef> {
        let mut roots: Vec<&super::apply::RootState> = self.registry.roots().collect();
        roots.sort_by(|a, b| {
            let a_fs = self
                .fullscreen_root
                .as_ref()
                .is_some_and(|root| root.same_generation(&a.root));
            let b_fs = self
                .fullscreen_root
                .as_ref()
                .is_some_and(|root| root.same_generation(&b.root));
            a_fs.cmp(&b_fs)
                .then_with(|| self.root_order(&a.root).cmp(&self.root_order(&b.root)))
                .then_with(|| a.root.root_key.cmp(&b.root.root_key))
        });
        let mut order = Vec::new();
        for state in roots {
            append_children_in_order(state, None, &mut order);
        }
        order
    }

    pub fn can_display(&self, root: &RootRef) -> bool {
        self.lease_for(root).is_some_and(host_can_display)
    }

    pub fn can_display_any(&self) -> bool {
        self.leases.iter().any(|(_, lease)| host_can_display(lease))
    }

    /// Pushes every displayable node into `compositor` in
    /// [`IslandSession::composition_nodes`] order. Platforms must not add a
    /// second z-order pass after this.
    pub fn materialize_into(&self, compositor: &mut dyn IslandCompositor) {
        if !self.can_display_any() {
            return;
        }
        for node in self.composition_nodes() {
            if !self.last_node_visible(&node.node_ref) {
                continue;
            }
            let id = node
                .author_id
                .clone()
                .unwrap_or_else(|| node.node_ref.node_key.clone());
            let rect = self.paint_rect_for_node(&node);
            compositor.attach_above_webview(&id, &node.kind, &rect, &node.props);
        }
    }

    /// Hit-test and emit press / valueChange / valueCommit for the current
    /// committed tree. Slider values latch for the duration of the drag.
    pub fn handle_pointer(
        &mut self,
        phase: IslandPointerPhase,
        x: f64,
        y: f64,
    ) -> Vec<IslandHostEvent> {
        if !self.can_display_any() {
            self.pointer.cancel();
            return Vec::new();
        }
        let targets = self.hit_targets();
        dispatch_pointer(&mut self.pointer, &targets, phase, x, y)
    }

    pub fn pointer_sequence_active(&self) -> bool {
        self.pointer.is_active()
    }

    pub fn latched_slider(&self) -> Option<(String, f64)> {
        self.pointer.latched_slider()
    }

    /// Props the compositor should paint for `id`, including a latched slider
    /// value while a drag is in progress.
    pub fn paint_props_for(&self, id: &str) -> Option<Value> {
        let node = self
            .composition_nodes()
            .into_iter()
            .find(|node| node.author_id.as_deref() == Some(id) || node.node_ref.node_key == id)?;
        if node.kind == "slider"
            && let Some((latch_id, value)) = self.latched_slider()
            && (latch_id == id || latch_id == node.node_ref.node_key)
        {
            return Some(props_with_slider_value(&node.props, value));
        }
        Some(node.props)
    }

    pub fn hit_targets(&self) -> Vec<IslandHitTarget> {
        self.composition_nodes()
            .into_iter()
            .map(|node| {
                let id = node
                    .author_id
                    .clone()
                    .unwrap_or_else(|| node.node_ref.node_key.clone());
                let rect = self.paint_rect_for_node(&node);
                let visible = self.last_node_visible(&node.node_ref);
                IslandHitTarget {
                    id,
                    kind: node.kind.clone(),
                    rect,
                    pointer_events: pointer_events_from_props(&node.kind, &node.props),
                    visible,
                    props: node.props,
                }
            })
            .collect()
    }

    fn last_node_visible(&self, node: &NodeRef) -> bool {
        self.geometry_for_root(&node.root())
            .and_then(|snapshot| {
                snapshot
                    .nodes
                    .iter()
                    .find(|entry| entry.node_ref == *node)
                    .map(|entry| entry.visible)
            })
            .unwrap_or(true)
    }

    fn root_order(&self, root: &RootRef) -> u32 {
        self.geometry_for_root(root)
            .and_then(|snapshot| {
                snapshot
                    .roots
                    .iter()
                    .find(|entry| entry.root_ref == *root)
                    .map(|entry| entry.root_order)
            })
            .unwrap_or(u32::MAX)
    }

    fn queue_ack(&mut self, ack: &NativeRootAck) {
        let Ok(mut value) = serde_json::to_value(ack) else {
            return;
        };
        if let Some(object) = value.as_object_mut()
            && let Some(key) = object
                .get("root")
                .and_then(|root| root.get("rootKey"))
                .cloned()
        {
            object.insert("id".to_string(), key);
        }
        self.pending_view_messages.push(value);
    }

    fn queue_error(&mut self, root: &RootRef, error: &super::types::NativeError) {
        if let Ok(mut value) = serde_json::to_value(error) {
            if let Some(object) = value.as_object_mut() {
                object.insert("action".into(), Value::String("root.error".into()));
                object.insert("id".into(), Value::String(root.root_key.clone()));
            }
            self.pending_view_messages.push(value);
        }
    }

    fn queue_view_message(&mut self, message: &super::types::NativeRootLeaseMessage) {
        let Ok(mut value) = serde_json::to_value(message) else {
            return;
        };
        if let Some(object) = value.as_object_mut()
            && let Some(key) = object
                .get("root")
                .and_then(|root| root.get("rootKey"))
                .cloned()
        {
            object.insert("id".to_string(), key);
        }
        self.pending_view_messages.push(value);
    }

    fn validate_commit_urls(&self, commit: &NativeRootCommit) -> Result<(), String> {
        let mut urls = Vec::new();
        for operation in &commit.operations {
            match operation {
                NativeRootOperation::Mount { node } => {
                    urls.extend(media_urls_from_props(&node.props));
                }
                NativeRootOperation::Update { patch, .. } => {
                    urls.extend(media_urls_from_props(patch));
                }
                _ => {}
            }
        }
        validate_media_urls(&urls, &self.trusted_domains, self.dev_session)
    }

    fn lease_for(&self, root: &RootRef) -> Option<&LeaseState> {
        self.leases
            .iter()
            .find(|(item, _)| item.same_generation(root))
            .map(|(_, lease)| lease)
    }

    fn lease_for_mut(&mut self, root: &RootRef) -> Option<&mut LeaseState> {
        self.leases
            .iter_mut()
            .find(|(item, _)| item.same_generation(root))
            .map(|(_, lease)| lease)
    }
}

impl Default for IslandSession {
    fn default() -> Self {
        Self::new()
    }
}

fn append_children_in_order(
    state: &super::apply::RootState,
    parent_key: Option<&str>,
    order: &mut Vec<NodeRef>,
) {
    let mut children: Vec<&super::apply::ShadowNode> = state
        .nodes
        .values()
        .filter(|node| node.parent_key.as_deref() == parent_key)
        .collect();
    children.sort_by_key(|node| (node.order, node.node_ref.node_key.clone()));
    for child in children {
        order.push(child.node_ref.clone());
        append_children_in_order(state, Some(&child.node_ref.node_key), order);
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}

pub fn is_island_action(action: &str) -> bool {
    matches!(
        action,
        "root.commit"
            | "root.destroy"
            | "geometry.snapshot"
            | "root.leaseAccept"
            | "root.leaseRenew"
            | "root.leaseRenewAccept"
            | "video.command"
            | "video.controlsSemanticSnapshot"
    )
}

pub fn parse_applied_revision(ack: &NativeRootAck) -> Option<u64> {
    match ack {
        NativeRootAck::Applied { revision, .. } => Some(*revision),
        _ => None,
    }
}
