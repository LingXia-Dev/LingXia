use super::*;

fn root() -> RootRef {
    RootRef {
        surface_instance_id: "s1".into(),
        page_instance_id: "p1".into(),
        document_instance_id: "d1".into(),
        root_key: "player".into(),
        root_epoch: 1,
    }
}

fn node(root: &RootRef, key: &str, epoch: u64) -> NodeRef {
    NodeRef {
        surface_instance_id: root.surface_instance_id.clone(),
        page_instance_id: root.page_instance_id.clone(),
        document_instance_id: root.document_instance_id.clone(),
        root_key: root.root_key.clone(),
        root_epoch: root.root_epoch,
        node_key: key.into(),
        node_epoch: epoch,
    }
}

fn mount(
    root: &RootRef,
    key: &str,
    kind: &str,
    parent: Option<NodeRef>,
    order: u32,
) -> NativeRootOperation {
    NativeRootOperation::Mount {
        node: NativeNode {
            node_ref: node(root, key, 1),
            kind: kind.into(),
            parent,
            order,
            author_type: match kind {
                "video" => "LxVideo",
                "tappable" => "LxNativeButton",
                "slider" => "LxNativeSlider",
                "text" => "LxNativeText",
                _ => "LxNativeView",
            }
            .into(),
            author_id: Some(key.into()),
            automation_id: None,
            props: serde_json::json!({}),
        },
    }
}

fn commit(
    root: &RootRef,
    base: u64,
    revision: u64,
    operations: Vec<NativeRootOperation>,
) -> NativeRootCommit {
    NativeRootCommit {
        action: "root.commit".into(),
        root: root.clone(),
        base_revision: base,
        revision,
        operations,
    }
}

fn rect() -> Rect {
    Rect {
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 80.0,
    }
}

#[test]
fn applies_a_json_commit_through_the_shipped_deserializer() {
    let json = r#"{
        "action":"root.commit",
        "root":{"surfaceInstanceId":"s1","pageInstanceId":"p1","documentInstanceId":"d1","rootKey":"player","rootEpoch":1},
        "baseRevision":0,
        "revision":1,
        "operations":[
            {"op":"mount","node":{
                "ref":{"surfaceInstanceId":"s1","pageInstanceId":"p1","documentInstanceId":"d1","rootKey":"player","rootEpoch":1,"nodeKey":"video","nodeEpoch":1},
                "kind":"video","parent":null,"order":0,"authorType":"LxVideo","authorId":"hero","props":{"src":"https://cdn.example.com/a.mp4"}
            }}
        ]
    }"#;
    let commit: NativeRootCommit = serde_json::from_str(json).expect("commit json");
    let mut registry = RootRegistry::new(HostCapabilities::default());
    match apply_root_commit(&mut registry, &commit) {
        ApplyCommitOutcome::Applied(NativeRootAck::Applied { revision, .. }) => {
            assert_eq!(revision, 1);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        registry.get(&commit.root).unwrap().nodes["video"].kind,
        "video"
    );
}

#[test]
fn applies_an_atomic_first_commit() {
    let root = root();
    let mut registry = RootRegistry::new(HostCapabilities::default());
    let outcome = apply_root_commit(
        &mut registry,
        &commit(
            &root,
            0,
            1,
            vec![
                mount(&root, "video", "video", None, 0),
                mount(&root, "cover", "view", None, 1),
            ],
        ),
    );
    match outcome {
        ApplyCommitOutcome::Applied(NativeRootAck::Applied { revision, .. }) => {
            assert_eq!(revision, 1);
        }
        other => panic!("expected applied, got {other:?}"),
    }
    let state = registry.get(&root).expect("root stored");
    assert_eq!(state.last_applied_revision, 1);
    assert_eq!(state.nodes.len(), 2);
    assert_eq!(state.lifecycle, RootLifecycle::Mounting);
}

#[test]
fn rejects_unknown_kind_and_leaves_no_half_tree() {
    let root = root();
    let mut registry = RootRegistry::new(HostCapabilities::default());
    let outcome = apply_root_commit(
        &mut registry,
        &commit(
            &root,
            0,
            1,
            vec![
                mount(&root, "video", "video", None, 0),
                mount(&root, "map", "map", None, 1),
            ],
        ),
    );
    match outcome {
        ApplyCommitOutcome::Rejected(err) => {
            assert_eq!(err.code, NativeErrorCode::InvalidStructure);
        }
        other => panic!("expected reject, got {other:?}"),
    }
    let state = registry.get(&root).expect("failed generation kept");
    assert!(state.nodes.is_empty());
    assert_eq!(state.last_applied_revision, 0);
    assert_eq!(state.lifecycle, RootLifecycle::Failed);
}

#[test]
fn rejects_cross_root_parent() {
    let root = root();
    let mut other = node(&root, "foreign", 1);
    other.root_key = "other-root".into();
    let mut registry = RootRegistry::new(HostCapabilities::default());
    let outcome = apply_root_commit(
        &mut registry,
        &commit(
            &root,
            0,
            1,
            vec![mount(&root, "child", "view", Some(other), 0)],
        ),
    );
    assert!(matches!(outcome, ApplyCommitOutcome::Rejected(_)));
}

#[test]
fn rejects_reparent_cycle() {
    let root = root();
    let mut registry = RootRegistry::new(HostCapabilities::default());
    let first = apply_root_commit(
        &mut registry,
        &commit(
            &root,
            0,
            1,
            vec![
                mount(&root, "a", "view", None, 0),
                mount(&root, "b", "view", Some(node(&root, "a", 1)), 0),
            ],
        ),
    );
    assert!(matches!(first, ApplyCommitOutcome::Applied(_)));
    let cycle = apply_root_commit(
        &mut registry,
        &commit(
            &root,
            1,
            2,
            vec![NativeRootOperation::Reparent {
                node: node(&root, "a", 1),
                parent: Some(node(&root, "b", 1)),
            }],
        ),
    );
    match cycle {
        ApplyCommitOutcome::Rejected(err) => {
            assert_eq!(err.code, NativeErrorCode::InvalidStructure);
            assert!(err.message.contains("cycle"));
        }
        other => panic!("expected cycle reject, got {other:?}"),
    }
    let state = registry.get(&root).expect("root");
    assert_eq!(state.last_applied_revision, 1);
    assert_eq!(state.nodes["a"].parent_key, None);
}

#[test]
fn revision_gap_requests_resync_without_applying() {
    let root = root();
    let mut registry = RootRegistry::new(HostCapabilities::default());
    assert!(matches!(
        apply_root_commit(
            &mut registry,
            &commit(&root, 0, 1, vec![mount(&root, "v", "video", None, 0)])
        ),
        ApplyCommitOutcome::Applied(_)
    ));
    let gap = apply_root_commit(
        &mut registry,
        &commit(&root, 4, 5, vec![mount(&root, "x", "view", None, 1)]),
    );
    match gap {
        ApplyCommitOutcome::ResyncRequired(NativeRootAck::ResyncRequired {
            last_applied_revision,
            ..
        }) => assert_eq!(last_applied_revision, 1),
        other => panic!("expected resync, got {other:?}"),
    }
    assert_eq!(registry.get(&root).unwrap().nodes.len(), 1);
}

#[test]
fn duplicate_identity_in_one_commit_is_rejected() {
    let root = root();
    let mut registry = RootRegistry::new(HostCapabilities::default());
    let outcome = apply_root_commit(
        &mut registry,
        &commit(
            &root,
            0,
            1,
            vec![
                mount(&root, "a", "view", None, 0),
                mount(&root, "a", "text", None, 1),
            ],
        ),
    );
    assert!(matches!(outcome, ApplyCommitOutcome::Rejected(_)));
    assert!(registry.get(&root).unwrap().nodes.is_empty());
}

#[test]
fn geometry_does_not_bump_tree_revision() {
    let root = root();
    let mut registry = RootRegistry::new(HostCapabilities::default());
    apply_root_commit(
        &mut registry,
        &commit(&root, 0, 1, vec![mount(&root, "v", "video", None, 0)]),
    );
    let mut page = geometry::GeometryPageState::default();
    let snapshot = NativeGeometrySnapshot {
        action: "geometry.snapshot".into(),
        surface_instance_id: root.surface_instance_id.clone(),
        page_instance_id: root.page_instance_id.clone(),
        document_instance_id: root.document_instance_id.clone(),
        revision: 9,
        coordinate_space: "page-unscrolled-css-px".into(),
        roots: vec![NativeGeometrySnapshotRoot {
            root_ref: root.clone(),
            basis_tree_revision: 1,
            root_order: 0,
            chain_key: "page".into(),
            content_rect: rect(),
            visible: true,
        }],
        nodes: vec![NativeGeometrySnapshotNode {
            node_ref: node(&root, "v", 1),
            chain_key: "page".into(),
            content_rect: rect(),
            clip_stack: vec![],
            visible: true,
        }],
        chains: vec![ScrollChain {
            chain_key: "page".into(),
            ancestors: vec![],
        }],
    };
    let result = apply_geometry_snapshot(&mut registry, &mut page, &snapshot);
    assert_eq!(result.roots[0].status, GeometryRootStatus::Applied);
    assert_eq!(registry.get(&root).unwrap().last_applied_revision, 1);
    assert_eq!(last_applied_geometry_revision(&page), 9);
}

#[test]
fn geometry_pending_then_applied_after_tree() {
    let root = root();
    let mut registry = RootRegistry::new(HostCapabilities::default());
    let mut page = geometry::GeometryPageState::default();
    let snapshot = NativeGeometrySnapshot {
        action: "geometry.snapshot".into(),
        surface_instance_id: root.surface_instance_id.clone(),
        page_instance_id: root.page_instance_id.clone(),
        document_instance_id: root.document_instance_id.clone(),
        revision: 1,
        coordinate_space: "page-unscrolled-css-px".into(),
        roots: vec![NativeGeometrySnapshotRoot {
            root_ref: root.clone(),
            basis_tree_revision: 1,
            root_order: 0,
            chain_key: "page".into(),
            content_rect: rect(),
            visible: true,
        }],
        nodes: vec![],
        chains: vec![ScrollChain {
            chain_key: "page".into(),
            ancestors: vec![],
        }],
    };
    let before = apply_geometry_snapshot(&mut registry, &mut page, &snapshot);
    assert_eq!(before.roots[0].status, GeometryRootStatus::StaleGeneration);

    apply_root_commit(
        &mut registry,
        &commit(&root, 0, 1, vec![mount(&root, "v", "video", None, 0)]),
    );
    let after = apply_geometry_snapshot(&mut registry, &mut page, &snapshot);
    assert_eq!(after.roots[0].status, GeometryRootStatus::Applied);
}

#[test]
fn geometry_unknown_node_fails_the_root() {
    let root = root();
    let mut registry = RootRegistry::new(HostCapabilities::default());
    apply_root_commit(
        &mut registry,
        &commit(&root, 0, 1, vec![mount(&root, "v", "video", None, 0)]),
    );
    let mut page = geometry::GeometryPageState::default();
    let snapshot = NativeGeometrySnapshot {
        action: "geometry.snapshot".into(),
        surface_instance_id: root.surface_instance_id.clone(),
        page_instance_id: root.page_instance_id.clone(),
        document_instance_id: root.document_instance_id.clone(),
        revision: 2,
        coordinate_space: "page-unscrolled-css-px".into(),
        roots: vec![NativeGeometrySnapshotRoot {
            root_ref: root.clone(),
            basis_tree_revision: 1,
            root_order: 0,
            chain_key: "page".into(),
            content_rect: rect(),
            visible: true,
        }],
        nodes: vec![NativeGeometrySnapshotNode {
            node_ref: node(&root, "ghost", 1),
            chain_key: "page".into(),
            content_rect: rect(),
            clip_stack: vec![],
            visible: true,
        }],
        chains: vec![ScrollChain {
            chain_key: "page".into(),
            ancestors: vec![],
        }],
    };
    let result = apply_geometry_snapshot(&mut registry, &mut page, &snapshot);
    assert_eq!(result.roots[0].status, GeometryRootStatus::IdentityInvalid);
    assert_eq!(
        registry.get(&root).unwrap().lifecycle,
        RootLifecycle::Failed
    );
}

#[test]
fn lease_is_fail_closed_until_accept() {
    let root = root();
    let (mut host, grant) = host_grant_lease(&root, "lease-1".into(), 0);
    assert!(!host_can_display(&host));
    match grant {
        NativeRootLeaseMessage::LeaseGranted {
            lease_id, sequence, ..
        } => {
            assert_eq!(lease_id, "lease-1");
            assert_eq!(sequence, 1);
        }
        other => panic!("{other:?}"),
    }
    let mut view = view_on_grant(&root, "lease-1", 1, 8000, 10, false);
    assert!(view_can_show_fallback(&view, 10));
    let accept = view_send_accept(&mut view, &root).expect("accept");
    match accept {
        NativeRootLeaseMessage::LeaseAccept {
            lease_id, sequence, ..
        } => {
            let active = host_on_accept(&mut host, &root, &lease_id, sequence);
            assert!(matches!(
                active,
                Some(NativeRootLeaseMessage::LeaseActive { .. })
            ));
        }
        other => panic!("{other:?}"),
    }
    assert!(host_can_display(&host));
    assert!(!view_can_show_fallback(&view, 10));
    assert!(view_can_show_fallback(&view, 10 + 8000));
}

#[test]
fn lost_grant_never_displays_and_allows_fallback() {
    let root = root();
    let (host, _) = host_grant_lease(&root, "lease-lost".into(), 0);
    assert!(!host_can_display(&host));
    let view = LeaseState::default();
    assert!(view_can_show_fallback(&view, 0));
}

#[test]
fn ready_requires_tree_geometry_and_active_lease() {
    let root = root();
    let mut registry = RootRegistry::new(HostCapabilities::default());
    apply_root_commit(
        &mut registry,
        &commit(&root, 0, 1, vec![mount(&root, "v", "video", None, 0)]),
    );
    assert_eq!(
        registry.get(&root).unwrap().lifecycle,
        RootLifecycle::Mounting
    );

    let mut page = GeometryPageState::default();
    let snapshot = NativeGeometrySnapshot {
        action: "geometry.snapshot".into(),
        surface_instance_id: root.surface_instance_id.clone(),
        page_instance_id: root.page_instance_id.clone(),
        document_instance_id: root.document_instance_id.clone(),
        revision: 1,
        coordinate_space: "page-unscrolled-css-px".into(),
        roots: vec![NativeGeometrySnapshotRoot {
            root_ref: root.clone(),
            basis_tree_revision: 1,
            root_order: 0,
            chain_key: "page".into(),
            content_rect: rect(),
            visible: true,
        }],
        nodes: vec![NativeGeometrySnapshotNode {
            node_ref: node(&root, "v", 1),
            chain_key: "page".into(),
            content_rect: rect(),
            clip_stack: vec![],
            visible: true,
        }],
        chains: vec![ScrollChain {
            chain_key: "page".into(),
            ancestors: vec![],
        }],
    };
    apply_geometry_snapshot(&mut registry, &mut page, &snapshot);
    assert_eq!(
        registry.get(&root).unwrap().lifecycle,
        RootLifecycle::Mounting
    );

    {
        let state = registry.get_mut(&root).unwrap();
        let (mut lease, _) = host_grant_lease(&root, "l".into(), 0);
        host_on_accept(&mut lease, &root, "l", 1);
        state.lease = lease;
        evaluate_root_ready(state);
    }
    assert_eq!(registry.get(&root).unwrap().lifecycle, RootLifecycle::Ready);
}
