use super::apply::{
    ApplyCommitOutcome, HostCapabilities, RootRegistry, apply_root_commit, evaluate_root_ready,
};
use super::geometry::{GeometryPageState, apply_geometry_snapshot, flush_pending_geometry};
use super::lease::{LeaseState, host_can_display, host_grant_lease, host_on_accept};
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
    last_geometry: Option<NativeGeometrySnapshot>,
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
            last_geometry: None,
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
            return ApplyCommitOutcome::Rejected(Box::new(super::types::NativeError {
                code: super::types::NativeErrorCode::InvalidProps,
                scope: super::types::ErrorScope::Root,
                recoverable: true,
                root: commit.root.clone(),
                node: None,
                message,
            }));
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
        outcome
    }

    pub fn apply_commit_json(&mut self, value: &Value) -> Result<ApplyCommitOutcome, String> {
        let commit: NativeRootCommit =
            serde_json::from_value(value.clone()).map_err(|err| err.to_string())?;
        Ok(self.apply_commit(commit))
    }

    pub fn apply_geometry(&mut self, snapshot: NativeGeometrySnapshot) -> NativeGeometryResult {
        let result = apply_geometry_snapshot(&mut self.registry, &mut self.geometry, &snapshot);
        self.last_geometry = Some(snapshot);
        result
    }

    pub fn last_node_rect(&self, node_key: &str) -> Option<Rect> {
        self.last_geometry.as_ref().and_then(|snapshot| {
            snapshot
                .nodes
                .iter()
                .find(|node| node.node_ref.node_key == node_key)
                .map(|node| node.content_rect.clone())
        })
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
                .then_with(|| a.root.root_key.cmp(&b.root.root_key))
        });
        let mut order = Vec::new();
        for state in roots {
            let mut nodes: Vec<&super::apply::ShadowNode> = state.nodes.values().collect();
            nodes.sort_by_key(|node| {
                (
                    parent_rank(node),
                    node.order,
                    node.node_ref.node_key.clone(),
                )
            });
            order.extend(nodes.into_iter().map(|node| node.node_ref.clone()));
        }
        order
    }

    pub fn can_display(&self, root: &RootRef) -> bool {
        self.lease_for(root).is_some_and(host_can_display)
    }

    pub fn can_display_any(&self) -> bool {
        self.leases.iter().any(|(_, lease)| host_can_display(lease))
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

fn parent_rank(node: &super::apply::ShadowNode) -> u32 {
    if node.parent_key.is_none() { 0 } else { 1 }
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
