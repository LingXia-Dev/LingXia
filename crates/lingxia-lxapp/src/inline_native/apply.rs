#![allow(clippy::result_large_err)]

use super::lease::{LeaseState, host_can_display};
use super::types::{
    ALLOWED_HOST_KINDS, ErrorScope, NativeError, NativeErrorCode, NativeNode, NativeRootAck,
    NativeRootCommit, NativeRootOperation, NodeRef, RootLifecycle, RootRef,
};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct HostCapabilities {
    kinds: HashSet<String>,
}

impl Default for HostCapabilities {
    fn default() -> Self {
        Self {
            kinds: ALLOWED_HOST_KINDS
                .iter()
                .map(|kind| (*kind).to_string())
                .collect(),
        }
    }
}

impl HostCapabilities {
    pub fn none() -> Self {
        Self {
            kinds: HashSet::new(),
        }
    }

    pub fn supports(&self, kind: &str) -> bool {
        self.kinds.contains(kind)
    }
}

#[derive(Debug, Clone)]
pub struct ShadowNode {
    pub node_ref: NodeRef,
    pub kind: String,
    pub parent_key: Option<String>,
    pub order: u32,
    pub author_type: String,
    pub author_id: Option<String>,
    pub automation_id: Option<String>,
    pub props: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct RootState {
    pub root: RootRef,
    pub last_applied_revision: u64,
    pub tree_applied: bool,
    pub matching_geometry_applied: bool,
    pub lifecycle: RootLifecycle,
    pub lease: LeaseState,
    pub nodes: HashMap<String, ShadowNode>,
}

impl RootState {
    pub fn new(root: RootRef) -> Self {
        Self {
            root,
            last_applied_revision: 0,
            tree_applied: false,
            matching_geometry_applied: false,
            lifecycle: RootLifecycle::Negotiating,
            lease: LeaseState::default(),
            nodes: HashMap::new(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct RootRegistry {
    pub capabilities: HostCapabilities,
    roots: HashMap<String, RootState>,
}

impl RootRegistry {
    pub fn new(capabilities: HostCapabilities) -> Self {
        Self {
            capabilities,
            roots: HashMap::new(),
        }
    }

    pub fn slot_key(root: &RootRef) -> String {
        format!(
            "{}:{}:{}:{}",
            root.surface_instance_id,
            root.page_instance_id,
            root.document_instance_id,
            root.root_key
        )
    }

    pub fn get(&self, root: &RootRef) -> Option<&RootState> {
        self.roots.get(&Self::slot_key(root)).filter(|state| {
            state.root.document_instance_id == root.document_instance_id
                && state.root.root_epoch == root.root_epoch
        })
    }

    pub fn get_mut(&mut self, root: &RootRef) -> Option<&mut RootState> {
        let key = Self::slot_key(root);
        self.roots.get_mut(&key).filter(|state| {
            state.root.document_instance_id == root.document_instance_id
                && state.root.root_epoch == root.root_epoch
        })
    }

    pub fn get_slot_mut(&mut self, root: &RootRef) -> Option<&mut RootState> {
        self.roots.get_mut(&Self::slot_key(root))
    }

    /// Slot lookup that ignores epoch so geometry can report stale-generation.
    pub fn get_slot(&self, root: &RootRef) -> Option<&RootState> {
        self.roots.get(&Self::slot_key(root))
    }

    pub fn insert(&mut self, state: RootState) {
        let key = Self::slot_key(&state.root);
        self.roots.insert(key, state);
    }

    pub fn destroy(&mut self, root: &RootRef) -> Option<RootState> {
        if let Some(state) = self.roots.get_mut(&Self::slot_key(root)) {
            state.lifecycle = RootLifecycle::Destroyed;
            state.nodes.clear();
        }
        self.roots.remove(&Self::slot_key(root))
    }

    pub fn roots(&self) -> impl Iterator<Item = &RootState> {
        self.roots.values()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ApplyCommitOutcome {
    Applied(NativeRootAck),
    ResyncRequired(NativeRootAck),
    Rejected(Box<NativeError>),
}

pub fn apply_root_commit(
    registry: &mut RootRegistry,
    commit: &NativeRootCommit,
) -> ApplyCommitOutcome {
    if commit.action != "root.commit" {
        return ApplyCommitOutcome::Rejected(Box::new(error(
            NativeErrorCode::InvalidStructure,
            &commit.root,
            None,
            "commit action must be root.commit",
        )));
    }
    if commit.revision == 0 || commit.revision <= commit.base_revision {
        return ApplyCommitOutcome::Rejected(Box::new(error(
            NativeErrorCode::InvalidStructure,
            &commit.root,
            None,
            "commit revision must be greater than baseRevision",
        )));
    }

    let slot_key = RootRegistry::slot_key(&commit.root);
    let existing = registry.roots.get(&slot_key).cloned();
    if let Some(state) = &existing {
        if state.lifecycle == RootLifecycle::Destroyed
            && state.root.root_epoch == commit.root.root_epoch
        {
            return ApplyCommitOutcome::Rejected(Box::new(error(
                NativeErrorCode::RootDestroyed,
                &commit.root,
                None,
                "destroyed root rejects commits",
            )));
        }
        if state.root.document_instance_id != commit.root.document_instance_id
            || state.root.root_epoch != commit.root.root_epoch
        {
            // A new generation replaces the slot after the previous one is destroyed.
            if state.lifecycle != RootLifecycle::Destroyed
                && state.root.document_instance_id == commit.root.document_instance_id
                && state.root.root_epoch != commit.root.root_epoch
            {
                return ApplyCommitOutcome::Rejected(Box::new(error(
                    NativeErrorCode::InvalidStructure,
                    &commit.root,
                    None,
                    "rootEpoch changed without destroying the previous generation",
                )));
            }
        }
    }

    let mut state = match existing {
        Some(state)
            if state.root.same_generation(&commit.root)
                && state.lifecycle != RootLifecycle::Destroyed =>
        {
            state
        }
        _ => RootState::new(commit.root.clone()),
    };

    if state.last_applied_revision != commit.base_revision {
        return ApplyCommitOutcome::ResyncRequired(NativeRootAck::ResyncRequired {
            root: commit.root.clone(),
            last_applied_revision: state.last_applied_revision,
        });
    }

    let mut next_nodes = state.nodes.clone();
    if let Err(err) = apply_operations(
        &registry.capabilities,
        &commit.root,
        &mut next_nodes,
        commit,
    ) {
        state.lifecycle = RootLifecycle::Failed;
        registry.insert(state);
        return ApplyCommitOutcome::Rejected(Box::new(err));
    }

    state.nodes = next_nodes;
    state.last_applied_revision = commit.revision;
    state.tree_applied = true;
    if state.lifecycle == RootLifecycle::Negotiating {
        state.lifecycle = RootLifecycle::Mounting;
    }
    evaluate_root_ready(&mut state);
    registry.insert(state);
    ApplyCommitOutcome::Applied(NativeRootAck::Applied {
        root: commit.root.clone(),
        revision: commit.revision,
    })
}

pub fn evaluate_root_ready(state: &mut RootState) {
    if matches!(
        state.lifecycle,
        RootLifecycle::Failed | RootLifecycle::Unavailable | RootLifecycle::Destroyed
    ) {
        return;
    }
    if state.tree_applied && state.matching_geometry_applied && host_can_display(&state.lease) {
        state.lifecycle = RootLifecycle::Ready;
    }
}

fn apply_operations(
    capabilities: &HostCapabilities,
    root: &RootRef,
    nodes: &mut HashMap<String, ShadowNode>,
    commit: &NativeRootCommit,
) -> Result<(), NativeError> {
    let mut seen = HashSet::new();
    for operation in &commit.operations {
        let node_ref = operation_node_ref(operation);
        if !node_ref.same_root(root) {
            return Err(error(
                NativeErrorCode::InvalidStructure,
                root,
                Some(node_ref.clone()),
                "node identity does not match the commit RootRef",
            ));
        }
        if !seen.insert(node_identity(node_ref)) {
            return Err(error(
                NativeErrorCode::InvalidStructure,
                root,
                Some(node_ref.clone()),
                "duplicate node identity in one commit",
            ));
        }
        match operation {
            NativeRootOperation::Mount { node } => mount_node(capabilities, root, nodes, node)?,
            NativeRootOperation::Update { node, patch } => update_node(root, nodes, node, patch)?,
            NativeRootOperation::Reparent { node, parent } => {
                reparent_node(root, nodes, node, parent.as_ref())?
            }
            NativeRootOperation::Reorder { node, order } => {
                reorder_node(root, nodes, node, *order)?
            }
            NativeRootOperation::Unmount { node } => unmount_node(root, nodes, node)?,
        }
    }
    if has_cycle(nodes) {
        return Err(error(
            NativeErrorCode::InvalidStructure,
            root,
            None,
            "parent graph contains a cycle",
        ));
    }
    Ok(())
}

fn mount_node(
    capabilities: &HostCapabilities,
    root: &RootRef,
    nodes: &mut HashMap<String, ShadowNode>,
    node: &NativeNode,
) -> Result<(), NativeError> {
    if super::types::HostFactoryKind::parse(node.kind.as_str()).is_none() {
        return Err(error(
            NativeErrorCode::InvalidStructure,
            root,
            Some(node.node_ref.clone()),
            &format!("unknown host kind {}", node.kind),
        ));
    }
    if !capabilities.supports(&node.kind) {
        return Err(error(
            NativeErrorCode::MountFailed,
            root,
            Some(node.node_ref.clone()),
            &format!("host cannot factory kind {}", node.kind),
        ));
    }
    if nodes.contains_key(&node.node_ref.node_key) {
        return Err(error(
            NativeErrorCode::InvalidStructure,
            root,
            Some(node.node_ref.clone()),
            "mount of an identity that already exists",
        ));
    }
    if let Some(parent) = &node.parent {
        if !parent.same_root(root) {
            return Err(error(
                NativeErrorCode::InvalidStructure,
                root,
                Some(node.node_ref.clone()),
                "parent ref crosses Root identity",
            ));
        }
        if !nodes.contains_key(&parent.node_key) {
            return Err(error(
                NativeErrorCode::InvalidStructure,
                root,
                Some(node.node_ref.clone()),
                "mount parent does not exist",
            ));
        }
    }
    nodes.insert(
        node.node_ref.node_key.clone(),
        ShadowNode {
            node_ref: node.node_ref.clone(),
            kind: node.kind.clone(),
            parent_key: node.parent.as_ref().map(|parent| parent.node_key.clone()),
            order: node.order,
            author_type: node.author_type.clone(),
            author_id: node.author_id.clone(),
            automation_id: node.automation_id.clone(),
            props: node.props.clone(),
        },
    );
    Ok(())
}

fn update_node(
    root: &RootRef,
    nodes: &mut HashMap<String, ShadowNode>,
    node: &NodeRef,
    patch: &serde_json::Value,
) -> Result<(), NativeError> {
    let Some(existing) = nodes.get_mut(&node.node_key) else {
        return Err(error(
            NativeErrorCode::InvalidStructure,
            root,
            Some(node.clone()),
            "update target does not exist",
        ));
    };
    if existing.node_ref.node_epoch != node.node_epoch {
        return Err(error(
            NativeErrorCode::InvalidStructure,
            root,
            Some(node.clone()),
            "update nodeEpoch does not match the mounted identity",
        ));
    }
    merge_patch(&mut existing.props, patch);
    Ok(())
}

fn reparent_node(
    root: &RootRef,
    nodes: &mut HashMap<String, ShadowNode>,
    node: &NodeRef,
    parent: Option<&NodeRef>,
) -> Result<(), NativeError> {
    if !nodes.contains_key(&node.node_key) {
        return Err(error(
            NativeErrorCode::InvalidStructure,
            root,
            Some(node.clone()),
            "reparent target does not exist",
        ));
    }
    if let Some(parent) = parent {
        if !parent.same_root(root) {
            return Err(error(
                NativeErrorCode::InvalidStructure,
                root,
                Some(node.clone()),
                "reparent parent crosses Root identity",
            ));
        }
        if !nodes.contains_key(&parent.node_key) {
            return Err(error(
                NativeErrorCode::InvalidStructure,
                root,
                Some(node.clone()),
                "reparent parent does not exist",
            ));
        }
        if parent.node_key == node.node_key {
            return Err(error(
                NativeErrorCode::InvalidStructure,
                root,
                Some(node.clone()),
                "node cannot be its own parent",
            ));
        }
        if would_cycle(nodes, &node.node_key, &parent.node_key) {
            return Err(error(
                NativeErrorCode::InvalidStructure,
                root,
                Some(node.clone()),
                "reparent would create a cycle",
            ));
        }
    }
    if let Some(existing) = nodes.get_mut(&node.node_key) {
        existing.parent_key = parent.map(|parent| parent.node_key.clone());
    }
    Ok(())
}

fn reorder_node(
    root: &RootRef,
    nodes: &mut HashMap<String, ShadowNode>,
    node: &NodeRef,
    order: u32,
) -> Result<(), NativeError> {
    let Some(existing) = nodes.get_mut(&node.node_key) else {
        return Err(error(
            NativeErrorCode::InvalidStructure,
            root,
            Some(node.clone()),
            "reorder target does not exist",
        ));
    };
    existing.order = order;
    Ok(())
}

fn unmount_node(
    root: &RootRef,
    nodes: &mut HashMap<String, ShadowNode>,
    node: &NodeRef,
) -> Result<(), NativeError> {
    if !nodes.contains_key(&node.node_key) {
        return Err(error(
            NativeErrorCode::InvalidStructure,
            root,
            Some(node.clone()),
            "unmount target does not exist",
        ));
    }
    let mut remove = vec![node.node_key.clone()];
    let mut i = 0;
    while i < remove.len() {
        let current = remove[i].clone();
        for (key, child) in nodes.iter() {
            if child.parent_key.as_deref() == Some(current.as_str()) {
                remove.push(key.clone());
            }
        }
        i += 1;
    }
    for key in remove {
        nodes.remove(&key);
    }
    Ok(())
}

fn would_cycle(nodes: &HashMap<String, ShadowNode>, node_key: &str, new_parent: &str) -> bool {
    let mut cursor = Some(new_parent);
    let mut guard = 0;
    while let Some(key) = cursor {
        if key == node_key {
            return true;
        }
        cursor = nodes.get(key).and_then(|node| node.parent_key.as_deref());
        guard += 1;
        if guard > nodes.len() + 1 {
            return true;
        }
    }
    false
}

fn has_cycle(nodes: &HashMap<String, ShadowNode>) -> bool {
    for (key, node) in nodes {
        if let Some(parent) = node.parent_key.as_deref()
            && would_cycle(nodes, key, parent)
        {
            return true;
        }
    }
    false
}

fn merge_patch(target: &mut serde_json::Value, patch: &serde_json::Value) {
    match (target, patch) {
        (serde_json::Value::Object(dest), serde_json::Value::Object(src)) => {
            for (key, value) in src {
                if value.is_null() {
                    dest.remove(key);
                } else {
                    merge_patch(
                        dest.entry(key.clone()).or_insert(serde_json::Value::Null),
                        value,
                    );
                }
            }
        }
        (slot, value) => *slot = value.clone(),
    }
}

fn operation_node_ref(operation: &NativeRootOperation) -> &NodeRef {
    match operation {
        NativeRootOperation::Mount { node } => &node.node_ref,
        NativeRootOperation::Update { node, .. }
        | NativeRootOperation::Reparent { node, .. }
        | NativeRootOperation::Reorder { node, .. }
        | NativeRootOperation::Unmount { node } => node,
    }
}

fn node_identity(node: &NodeRef) -> (String, u64) {
    (node.node_key.clone(), node.node_epoch)
}

fn error(
    code: NativeErrorCode,
    root: &RootRef,
    node: Option<NodeRef>,
    message: &str,
) -> NativeError {
    NativeError {
        code,
        scope: ErrorScope::Root,
        recoverable: false,
        root: root.clone(),
        node,
        message: message.to_string(),
    }
}
