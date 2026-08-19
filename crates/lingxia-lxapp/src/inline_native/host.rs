use super::apply::{ApplyCommitOutcome, HostCapabilities, RootRegistry, apply_root_commit};
use super::geometry::{GeometryPageState, apply_geometry_snapshot};
use super::lease::{LeaseState, host_can_display, host_grant_lease, host_on_accept};
use super::types::{
    NativeGeometryResult, NativeGeometrySnapshot, NativeRootAck, NativeRootCommit, NodeRef, RootRef,
};
use super::video::{
    VideoCommandOutcome, VideoCommandQueue, VideoCommandRequest, apply_video_command,
};
use serde_json::Value;

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
}

impl IslandSession {
    pub fn new() -> Self {
        Self {
            registry: RootRegistry::new(HostCapabilities::default()),
            geometry: GeometryPageState::default(),
            command_queue: VideoCommandQueue::default(),
            leases: Vec::new(),
            fullscreen_root: None,
        }
    }

    /// Island nodes share the WebView composition domain. Windowed HWND z-order
    /// is not a legal implementation path for these nodes.
    pub fn uses_hwnd_zorder(&self) -> bool {
        false
    }

    pub fn apply_commit(&mut self, commit: NativeRootCommit) -> ApplyCommitOutcome {
        let outcome = apply_root_commit(&mut self.registry, &commit);
        if matches!(outcome, ApplyCommitOutcome::Applied(_))
            && self.lease_for(&commit.root).is_none()
        {
            let (lease, _) =
                host_grant_lease(&commit.root, format!("lease-{}", commit.root.root_key), 0);
            self.leases.push((commit.root.clone(), lease));
        }
        outcome
    }

    pub fn apply_commit_json(&mut self, value: &Value) -> Result<ApplyCommitOutcome, String> {
        let commit: NativeRootCommit =
            serde_json::from_value(value.clone()).map_err(|err| err.to_string())?;
        Ok(self.apply_commit(commit))
    }

    pub fn apply_geometry(&mut self, snapshot: NativeGeometrySnapshot) -> NativeGeometryResult {
        apply_geometry_snapshot(&mut self.registry, &mut self.geometry, &snapshot)
    }

    pub fn accept_lease(&mut self, root: &RootRef, lease_id: &str, sequence: u64) -> bool {
        if let Some(lease) = self.lease_for_mut(root) {
            return host_on_accept(lease, root, lease_id, sequence).is_some();
        }
        false
    }

    pub fn apply_video_command(&mut self, request: VideoCommandRequest) -> VideoCommandOutcome {
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
