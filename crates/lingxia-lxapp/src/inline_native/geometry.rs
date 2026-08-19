use super::apply::{RootRegistry, evaluate_root_ready};
use super::types::{
    GeometryResultRoot, GeometryRootStatus, NativeGeometryResult, NativeGeometrySnapshot, RootRef,
};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct PendingGeometry {
    pub revision: u64,
    pub snapshot: NativeGeometrySnapshot,
}

#[derive(Debug, Default)]
pub struct GeometryPageState {
    pub last_applied_revision: u64,
    pending_by_root: HashMap<String, PendingGeometry>,
}

pub fn last_applied_geometry_revision(state: &GeometryPageState) -> u64 {
    state.last_applied_revision
}

pub fn apply_geometry_snapshot(
    registry: &mut RootRegistry,
    page: &mut GeometryPageState,
    snapshot: &NativeGeometrySnapshot,
) -> NativeGeometryResult {
    let mut roots = Vec::new();
    for root_entry in &snapshot.roots {
        let status = classify_root(registry, snapshot, root_entry);
        let last_applied = registry
            .get(&root_entry.root_ref)
            .map(|state| state.last_applied_revision)
            .unwrap_or(0);
        if status == GeometryRootStatus::PendingTree {
            page.pending_by_root.insert(
                RootRegistry::slot_key(&root_entry.root_ref),
                PendingGeometry {
                    revision: snapshot.revision,
                    snapshot: snapshot.clone(),
                },
            );
        } else if status == GeometryRootStatus::Applied {
            if let Some(state) = registry.get_mut(&root_entry.root_ref) {
                state.matching_geometry_applied = true;
                evaluate_root_ready(state);
            }
            page.pending_by_root
                .remove(&RootRegistry::slot_key(&root_entry.root_ref));
            page.last_applied_revision = snapshot.revision;
        } else if status == GeometryRootStatus::IdentityInvalid {
            if let Some(state) = registry.get_mut(&root_entry.root_ref) {
                state.lifecycle = super::types::RootLifecycle::Failed;
            }
        }
        roots.push(GeometryResultRoot {
            root: root_entry.root_ref.clone(),
            basis_tree_revision: root_entry.basis_tree_revision,
            last_applied_tree_revision: last_applied,
            status,
        });
    }
    NativeGeometryResult {
        action: "geometry.result".to_string(),
        surface_instance_id: snapshot.surface_instance_id.clone(),
        page_instance_id: snapshot.page_instance_id.clone(),
        document_instance_id: snapshot.document_instance_id.clone(),
        revision: snapshot.revision,
        roots,
    }
}

/// After a tree commit lands, apply any cached geometry whose basis now matches.
pub fn flush_pending_geometry(
    registry: &mut RootRegistry,
    page: &mut GeometryPageState,
    root: &RootRef,
) -> Option<NativeGeometryResult> {
    let key = RootRegistry::slot_key(root);
    let pending = page.pending_by_root.get(&key)?.clone();
    Some(apply_geometry_snapshot(registry, page, &pending.snapshot))
}

fn classify_root(
    registry: &RootRegistry,
    snapshot: &NativeGeometrySnapshot,
    root_entry: &super::types::NativeGeometrySnapshotRoot,
) -> GeometryRootStatus {
    let Some(state) = registry.get_slot(&root_entry.root_ref) else {
        return GeometryRootStatus::StaleGeneration;
    };
    if state.root.document_instance_id != root_entry.root_ref.document_instance_id
        || state.root.root_epoch != root_entry.root_ref.root_epoch
    {
        return GeometryRootStatus::StaleGeneration;
    }
    if snapshot.coordinate_space != "page-unscrolled-css-px" {
        return GeometryRootStatus::IdentityInvalid;
    }
    if identity_invalid(state, snapshot, &root_entry.root_ref) {
        return GeometryRootStatus::IdentityInvalid;
    }
    if root_entry.basis_tree_revision < state.last_applied_revision {
        return GeometryRootStatus::StaleRevision;
    }
    if root_entry.basis_tree_revision > state.last_applied_revision {
        return GeometryRootStatus::PendingTree;
    }
    GeometryRootStatus::Applied
}

fn identity_invalid(
    state: &super::apply::RootState,
    snapshot: &NativeGeometrySnapshot,
    root: &RootRef,
) -> bool {
    let mut seen = HashSet::new();
    for node in &snapshot.nodes {
        if !node.node_ref.same_root(root) {
            // nodes for other roots in the same page snapshot are fine
            continue;
        }
        if !seen.insert((node.node_ref.node_key.clone(), node.node_ref.node_epoch)) {
            return true;
        }
        match state.nodes.get(&node.node_ref.node_key) {
            None => return true,
            Some(existing) if existing.node_ref.node_epoch != node.node_ref.node_epoch => {
                return true;
            }
            Some(_) => {}
        }
        if !snapshot
            .chains
            .iter()
            .any(|chain| chain.chain_key == node.chain_key)
        {
            return true;
        }
    }
    false
}
