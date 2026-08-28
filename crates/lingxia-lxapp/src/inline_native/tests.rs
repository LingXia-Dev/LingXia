use super::*;
use serde_json::Value;

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
fn island_session_rejects_untrusted_video_src() {
    let root = root();
    let mut session = IslandSession::new();
    session.set_trusted_domains(vec!["cdn.example.com".into()], false);
    let mut ops = vec![mount(&root, "video", "video", None, 0)];
    if let NativeRootOperation::Mount { node } = &mut ops[0] {
        node.props = serde_json::json!({ "src": "https://evil.example/a.mp4" });
    }
    match session.apply_commit(commit(&root, 0, 1, ops)) {
        ApplyCommitOutcome::Rejected(err) => {
            assert_eq!(err.code, NativeErrorCode::InvalidProps);
            assert!(err.message.contains("trustedDomains"));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn island_session_completes_lease_handshake_and_exposes_video_author() {
    let root = root();
    let mut session = IslandSession::new();
    session.set_trusted_domains(vec!["cdn.example.com".into()], false);
    let mut ops = vec![mount(&root, "hero", "video", None, 0)];
    if let NativeRootOperation::Mount { node } = &mut ops[0] {
        node.author_id = Some("lx-video-1".into());
        node.props = serde_json::json!({ "src": "https://cdn.example.com/a.mp4" });
    }
    assert!(matches!(
        session.apply_commit(commit(&root, 0, 1, ops)),
        ApplyCommitOutcome::Applied(_)
    ));
    let videos = session.video_nodes();
    assert_eq!(videos.len(), 1);
    assert_eq!(videos[0].author_id.as_deref(), Some("lx-video-1"));
    assert!(!session.can_display(&root));

    let grant = session
        .drain_view_messages()
        .into_iter()
        .find(|msg| msg.get("action").and_then(Value::as_str) == Some("root.leaseGranted"))
        .expect("host must send leaseGranted to the View");
    assert_eq!(grant.get("id").and_then(Value::as_str), Some("player"));
    let lease_id = grant
        .get("leaseId")
        .and_then(Value::as_str)
        .expect("leaseGranted must camelCase leaseId for the View handshake");
    let sequence = grant
        .get("sequence")
        .and_then(Value::as_u64)
        .expect("leaseGranted must include sequence");
    assert!(session.handle_view_json(&serde_json::json!({
        "action": "root.leaseAccept",
        "root": {
            "surfaceInstanceId": root.surface_instance_id,
            "pageInstanceId": root.page_instance_id,
            "documentInstanceId": root.document_instance_id,
            "rootKey": root.root_key,
            "rootEpoch": root.root_epoch
        },
        "leaseId": lease_id,
        "sequence": sequence
    })));
    assert!(session.can_display(&root));
    let active = session
        .drain_view_messages()
        .into_iter()
        .find(|msg| msg.get("action").and_then(Value::as_str) == Some("root.leaseActive"));
    assert!(active.is_some());
}

#[test]
fn island_session_accepts_showcase_blender_media_when_domains_listed() {
    let root = root();
    let mut session = IslandSession::new();
    session.set_trusted_domains(
        vec!["download.blender.org".into(), "upload.wikimedia.org".into()],
        false,
    );
    let mut ops = vec![mount(&root, "hero", "video", None, 0)];
    if let NativeRootOperation::Mount { node } = &mut ops[0] {
        node.author_id = Some("lx-video-1".into());
        node.props = serde_json::json!({
            "src": "https://download.blender.org/peach/bigbuckbunny_movies/big_buck_bunny_480p_h264.mov",
            "poster": "https://upload.wikimedia.org/wikipedia/commons/thumb/c/c5/Big_buck_bunny_poster_big.jpg/640px-Big_buck_bunny_poster_big.jpg",
            "qualities": [{
                "label": "720P",
                "url": "https://download.blender.org/peach/bigbuckbunny_movies/big_buck_bunny_720p_h264.mov"
            }]
        });
    }
    assert!(matches!(
        session.apply_commit(commit(&root, 0, 1, ops)),
        ApplyCommitOutcome::Applied(_)
    ));
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
            content_rect: Rect {
                x: 0.0,
                y: 40.0,
                width: 320.0,
                height: 180.0,
            },
            visible: true,
        }],
        nodes: vec![NativeGeometrySnapshotNode {
            node_ref: node(&root, "hero", 1),
            chain_key: "page".into(),
            content_rect: Rect {
                x: 0.0,
                y: 40.0,
                width: 320.0,
                height: 180.0,
            },
            clip_stack: vec![],
            visible: true,
        }],
        chains: vec![ScrollChain {
            chain_key: "page".into(),
            ancestors: vec![],
        }],
    };
    session.apply_geometry(snapshot);
    assert_eq!(session.last_node_rect("hero").unwrap().y, 40.0);
    assert_eq!(
        session.registry.get(&root).unwrap().last_applied_revision,
        1
    );
}

#[test]
fn island_session_orders_committed_siblings_without_hwnd_zorder() {
    let root = root();
    let mut session = IslandSession::new();
    assert!(!session.uses_hwnd_zorder());
    let outcome = session.apply_commit(commit(
        &root,
        0,
        1,
        vec![
            mount(&root, "video", "video", None, 0),
            mount(&root, "cover", "view", None, 1),
            mount(&root, "btn", "tappable", Some(node(&root, "cover", 1)), 0),
        ],
    ));
    assert!(matches!(outcome, ApplyCommitOutcome::Applied(_)));
    let order = session.composition_order();
    let keys: Vec<&str> = order.iter().map(|node| node.node_key.as_str()).collect();
    assert_eq!(keys, ["video", "cover", "btn"]);
    let painted = session.composition_nodes();
    let kinds: Vec<&str> = painted.iter().map(|node| node.kind.as_str()).collect();
    assert_eq!(kinds, ["video", "view", "tappable"]);
    session.set_fullscreen(&root, true).unwrap();
    assert_eq!(session.fullscreen_root().unwrap().root_key, "player");
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
fn base_zero_commit_replaces_a_stale_same_slot_tree() {
    let root = root();
    let mut registry = RootRegistry::new(HostCapabilities::default());
    assert!(matches!(
        apply_root_commit(
            &mut registry,
            &commit(&root, 0, 1, vec![mount(&root, "old", "video", None, 0)])
        ),
        ApplyCommitOutcome::Applied(_)
    ));
    match apply_root_commit(
        &mut registry,
        &commit(&root, 0, 1, vec![mount(&root, "fresh", "video", None, 0)]),
    ) {
        ApplyCommitOutcome::Applied(NativeRootAck::Applied { revision, .. }) => {
            assert_eq!(revision, 1);
        }
        other => panic!("{other:?}"),
    }
    let state = registry.get(&root).unwrap();
    assert!(state.nodes.contains_key("fresh"));
    assert!(!state.nodes.contains_key("old"));
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
fn video_command_requires_applied_video_node() {
    let root = root();
    let mut registry = RootRegistry::new(HostCapabilities::default());
    apply_root_commit(
        &mut registry,
        &commit(&root, 0, 1, vec![mount(&root, "v", "video", None, 0)]),
    );
    let mut queue = VideoCommandQueue::default();
    let request = VideoCommandRequest {
        action: "video.command".into(),
        owner: node(&root, "v", 1),
        request_id: "r1".into(),
        command: VideoCommand::Play,
    };
    match apply_video_command(&registry, &mut queue, request) {
        VideoCommandOutcome::Queued { request_id } => assert_eq!(request_id, "r1"),
        other => panic!("{other:?}"),
    }
    {
        let state = registry.get_mut(&root).unwrap();
        state.lifecycle = RootLifecycle::Ready;
    }
    let play = VideoCommandRequest {
        action: "video.command".into(),
        owner: node(&root, "v", 1),
        request_id: "r2".into(),
        command: VideoCommand::Seek { seconds: 4.0 },
    };
    assert!(matches!(
        apply_video_command(&registry, &mut queue, play),
        VideoCommandOutcome::Applied { .. }
    ));
    let missing = VideoCommandRequest {
        action: "video.command".into(),
        owner: node(&root, "missing", 1),
        request_id: "r3".into(),
        command: VideoCommand::Pause,
    };
    assert!(matches!(
        apply_video_command(&registry, &mut queue, missing),
        VideoCommandOutcome::Rejected(_)
    ));
}

#[test]
fn video_controls_snapshot_rejects_stale_and_bad_slider() {
    let root = root();
    let owner = node(&root, "v", 1);
    let good = VideoControlsSemanticSnapshot {
        action: "video.controlsSemanticSnapshot".into(),
        owner: owner.clone(),
        revision: 1,
        controls: vec![VideoControlDescriptor {
            control_id: "play".into(),
            label: "Play".into(),
            role: "button".into(),
            visible: true,
            disabled: false,
            value: None,
            min: None,
            max: None,
            actions: vec!["activate".into()],
        }],
    };
    assert_eq!(apply_video_controls_snapshot(0, &good).unwrap(), 1);
    assert!(apply_video_controls_snapshot(1, &good).is_err());
    let bad = VideoControlsSemanticSnapshot {
        action: "video.controlsSemanticSnapshot".into(),
        owner,
        revision: 2,
        controls: vec![VideoControlDescriptor {
            control_id: "seek".into(),
            label: "Seek".into(),
            role: "slider".into(),
            visible: true,
            disabled: false,
            value: Some(50.0),
            min: Some(0.0),
            max: Some(10.0),
            actions: vec!["setValue".into()],
        }],
    };
    assert!(apply_video_controls_snapshot(1, &bad).is_err());
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

struct AttachRecorder {
    calls: Vec<(String, String, Rect, Value)>,
}

impl IslandCompositor for AttachRecorder {
    fn attach_above_webview(&mut self, id: &str, kind: &str, rect: &Rect, props: &Value) {
        self.calls.push((
            id.to_string(),
            kind.to_string(),
            rect.clone(),
            props.clone(),
        ));
    }

    fn order(&self) -> Vec<String> {
        self.calls.iter().map(|(id, _, _, _)| id.clone()).collect()
    }
}

#[test]
fn materialize_into_attaches_root_video_cover_in_composition_order() {
    let root = root();
    let mut session = IslandSession::new();
    session.set_trusted_domains(vec!["cdn.example.com".into()], true);
    let mut ops = vec![
        mount(&root, "lx-video-1", "video", None, 0),
        mount(&root, "cover", "view", None, 1),
        mount(&root, "title", "text", Some(node(&root, "cover", 1)), 0),
    ];
    if let NativeRootOperation::Mount { node } = &mut ops[0] {
        node.props = serde_json::json!({ "src": "https://cdn.example.com/a.mp4" });
    }
    if let NativeRootOperation::Mount { node } = &mut ops[2] {
        node.props = serde_json::json!({ "text": "Inline native" });
    }
    assert!(matches!(
        session.apply_commit(commit(&root, 0, 1, ops)),
        ApplyCommitOutcome::Applied(_)
    ));
    let grant = session
        .drain_view_messages()
        .into_iter()
        .find(|msg| msg.get("action").and_then(Value::as_str) == Some("root.leaseGranted"))
        .expect("leaseGranted");
    let lease_id = grant.get("leaseId").and_then(Value::as_str).unwrap();
    let sequence = grant.get("sequence").and_then(Value::as_u64).unwrap();
    assert!(session.handle_view_json(&serde_json::json!({
        "action": "root.leaseAccept",
        "root": {
            "surfaceInstanceId": root.surface_instance_id,
            "pageInstanceId": root.page_instance_id,
            "documentInstanceId": root.document_instance_id,
            "rootKey": root.root_key,
            "rootEpoch": root.root_epoch
        },
        "leaseId": lease_id,
        "sequence": sequence
    })));

    let video_rect = Rect {
        x: 8.0,
        y: 40.0,
        width: 320.0,
        height: 180.0,
    };
    let cover_rect = video_rect.clone();
    let text_rect = Rect {
        x: 20.0,
        y: 52.0,
        width: 120.0,
        height: 18.0,
    };
    session.apply_geometry(NativeGeometrySnapshot {
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
            content_rect: video_rect.clone(),
            visible: true,
        }],
        nodes: vec![
            NativeGeometrySnapshotNode {
                node_ref: node(&root, "lx-video-1", 1),
                chain_key: "page".into(),
                content_rect: video_rect.clone(),
                clip_stack: vec![],
                visible: true,
            },
            NativeGeometrySnapshotNode {
                node_ref: node(&root, "cover", 1),
                chain_key: "page".into(),
                content_rect: cover_rect,
                clip_stack: vec![],
                visible: true,
            },
            NativeGeometrySnapshotNode {
                node_ref: node(&root, "title", 1),
                chain_key: "page".into(),
                content_rect: text_rect.clone(),
                clip_stack: vec![],
                visible: true,
            },
        ],
        chains: vec![ScrollChain {
            chain_key: "page".into(),
            ancestors: vec![],
        }],
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
        .map(|(_, kind, _, _)| kind.as_str())
        .collect();
    assert_eq!(kinds, ["video", "view", "text"]);
    assert_eq!(recorder.calls[0].2, video_rect);
    assert_eq!(recorder.calls[2].2, text_rect);
    assert!(!session.uses_hwnd_zorder());
}

#[test]
fn materialize_into_uses_root_rect_when_node_geometry_is_missing_or_degenerate() {
    let root = root();
    let mut session = IslandSession::new();
    session.set_trusted_domains(vec!["cdn.example.com".into()], true);
    let mut ops = vec![mount(&root, "lx-video-1", "video", None, 0)];
    if let NativeRootOperation::Mount { node } = &mut ops[0] {
        node.props = serde_json::json!({ "src": "https://cdn.example.com/a.mp4" });
    }
    assert!(matches!(
        session.apply_commit(commit(&root, 0, 1, ops)),
        ApplyCommitOutcome::Applied(_)
    ));
    activate_lease(&mut session, &root);

    let root_rect = Rect {
        x: 8.0,
        y: 40.0,
        width: 320.0,
        height: 180.0,
    };
    session.apply_geometry(NativeGeometrySnapshot {
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
            content_rect: root_rect.clone(),
            visible: true,
        }],
        nodes: vec![NativeGeometrySnapshotNode {
            node_ref: node(&root, "lx-video-1", 1),
            chain_key: "page".into(),
            content_rect: Rect {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            },
            clip_stack: vec![],
            visible: true,
        }],
        chains: vec![ScrollChain {
            chain_key: "page".into(),
            ancestors: vec![],
        }],
    });

    let mut recorder = AttachRecorder { calls: Vec::new() };
    session.materialize_into(&mut recorder);
    assert_eq!(recorder.calls.len(), 1);
    assert_eq!(recorder.calls[0].2, root_rect);
}

fn activate_lease(session: &mut IslandSession, root: &RootRef) {
    let grant = session
        .drain_view_messages()
        .into_iter()
        .find(|msg| msg.get("action").and_then(Value::as_str) == Some("root.leaseGranted"))
        .expect("leaseGranted");
    let lease_id = grant.get("leaseId").and_then(Value::as_str).unwrap();
    let sequence = grant.get("sequence").and_then(Value::as_u64).unwrap();
    assert!(session.handle_view_json(&serde_json::json!({
        "action": "root.leaseAccept",
        "root": {
            "surfaceInstanceId": root.surface_instance_id,
            "pageInstanceId": root.page_instance_id,
            "documentInstanceId": root.document_instance_id,
            "rootKey": root.root_key,
            "rootEpoch": root.root_epoch
        },
        "leaseId": lease_id,
        "sequence": sequence
    })));
}

#[test]
fn paints_cover_button_slider_and_dispatches_pointer() {
    let root = root();
    let mut session = IslandSession::new();
    session.set_trusted_domains(vec!["cdn.example.com".into()], true);
    let mut ops = vec![
        mount(&root, "hero", "video", None, 0),
        mount(&root, "cover", "view", None, 1),
        mount(&root, "play", "tappable", Some(node(&root, "cover", 1)), 0),
        mount(&root, "seek", "slider", Some(node(&root, "cover", 1)), 1),
    ];
    if let NativeRootOperation::Mount { node } = &mut ops[0] {
        node.props = serde_json::json!({ "src": "https://cdn.example.com/a.mp4" });
    }
    if let NativeRootOperation::Mount { node } = &mut ops[1] {
        node.author_type = "LxNativeCover".into();
        node.props = serde_json::json!({
            "scrimPaint": { "scrim": "bottom", "opacity": 0.6 },
            "pointerEvents": "box-none"
        });
    }
    if let NativeRootOperation::Mount { node } = &mut ops[2] {
        node.props = serde_json::json!({
            "content": { "icon": { "kind": "semantic", "name": "play" }, "text": "Play" },
            "pointerEvents": "auto"
        });
    }
    if let NativeRootOperation::Mount { node } = &mut ops[3] {
        node.props = serde_json::json!({
            "min": 0,
            "max": 100,
            "value": 10,
            "step": 5,
            "valueLabel": "value",
            "pointerEvents": "auto"
        });
    }
    assert!(matches!(
        session.apply_commit(commit(&root, 0, 1, ops)),
        ApplyCommitOutcome::Applied(_)
    ));
    activate_lease(&mut session, &root);

    let video_rect = Rect {
        x: 0.0,
        y: 40.0,
        width: 320.0,
        height: 180.0,
    };
    let button_rect = Rect {
        x: 16.0,
        y: 180.0,
        width: 48.0,
        height: 32.0,
    };
    let slider_rect = Rect {
        x: 80.0,
        y: 188.0,
        width: 200.0,
        height: 16.0,
    };
    session.apply_geometry(NativeGeometrySnapshot {
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
            content_rect: video_rect.clone(),
            visible: true,
        }],
        nodes: vec![
            NativeGeometrySnapshotNode {
                node_ref: node(&root, "hero", 1),
                chain_key: "page".into(),
                content_rect: video_rect.clone(),
                clip_stack: vec![],
                visible: true,
            },
            NativeGeometrySnapshotNode {
                node_ref: node(&root, "cover", 1),
                chain_key: "page".into(),
                content_rect: video_rect.clone(),
                clip_stack: vec![],
                visible: true,
            },
            NativeGeometrySnapshotNode {
                node_ref: node(&root, "play", 1),
                chain_key: "page".into(),
                content_rect: button_rect.clone(),
                clip_stack: vec![],
                visible: true,
            },
            NativeGeometrySnapshotNode {
                node_ref: node(&root, "seek", 1),
                chain_key: "page".into(),
                content_rect: slider_rect.clone(),
                clip_stack: vec![],
                visible: true,
            },
        ],
        chains: vec![],
    });

    let mut recorder = AttachRecorder { calls: Vec::new() };
    session.materialize_into(&mut recorder);
    let kinds: Vec<&str> = recorder
        .calls
        .iter()
        .map(|(_, kind, _, _)| kind.as_str())
        .collect();
    assert_eq!(kinds, ["video", "view", "tappable", "slider"]);
    assert_eq!(recorder.calls[1].2.width, 320.0);
    assert_eq!(recorder.calls[2].2, button_rect);
    assert_eq!(recorder.calls[3].2, slider_rect);
    assert!(
        recorder.calls[1].3.get("scrimPaint").is_some(),
        "Cover wire field must reach the compositor"
    );
    assert!(
        recorder.calls[2].3.get("content").is_some(),
        "Button content must reach the compositor"
    );

    let cover_plan = plan_island_visual("view", &video_rect, &recorder.calls[1].3);
    assert!((cover_plan.dest_width - 320.0).abs() < f32::EPSILON);
    assert!((cover_plan.dest_height - 180.0).abs() < f32::EPSILON);
    assert_ne!(
        (cover_plan.texture_width, cover_plan.texture_height),
        (16, 16)
    );
    let button_plan = plan_island_visual("tappable", &button_rect, &recorder.calls[2].3);
    assert_eq!(button_plan.text.as_deref(), Some("Play"));
    let slider_plan = plan_island_visual("slider", &slider_rect, &recorder.calls[3].3);
    assert_eq!(slider_plan.text.as_deref(), Some("10"));
    let pixels = rasterize_island_kind("slider", 80, 16, &recorder.calls[3].3);
    assert!(
        pixels.contains(&0xffff_ffff),
        "slider raster must paint a thumb or valueLabel"
    );

    let press = session.handle_pointer(IslandPointerPhase::Down, 24.0, 190.0);
    assert!(press.is_empty());
    let press = session.handle_pointer(IslandPointerPhase::Up, 24.0, 190.0);
    assert_eq!(press.len(), 1);
    assert_eq!(press[0].id, "play");
    assert_eq!(press[0].event, "press");
    assert_eq!(press[0].detail["source"], "pointer");

    let start = session.handle_pointer(IslandPointerPhase::Down, 180.0, 196.0);
    assert_eq!(start.len(), 1);
    assert_eq!(start[0].event, "valuechange");
    let start_value = start[0].detail["value"].as_f64().unwrap();
    let drag = session.handle_pointer(IslandPointerPhase::Move, 280.0, 196.0);
    assert_eq!(drag.len(), 1);
    let drag_value = drag[0].detail["value"].as_f64().unwrap();
    assert!(drag_value > start_value);
    let (latch_id, latch_value) = session
        .latched_slider()
        .expect("slider drag must latch locally");
    assert_eq!(latch_id, "seek");
    assert_eq!(latch_value, drag_value);
    let latched_props = session
        .paint_props_for("seek")
        .expect("slider paint props during drag");
    assert_eq!(latched_props["value"].as_f64(), Some(drag_value));
    assert_ne!(
        latched_props["value"].as_f64(),
        recorder.calls[3].3.get("value").and_then(Value::as_f64),
        "latched paint must not wait on the committed Logic value"
    );
    let latched_plan = plan_island_visual("slider", &slider_rect, &latched_props);
    let committed_plan = plan_island_visual("slider", &slider_rect, &recorder.calls[3].3);
    assert_ne!(latched_plan.text, committed_plan.text);
    let latched_pixels = rasterize_island_kind("slider", 80, 16, &latched_props);
    let committed_pixels = rasterize_island_kind("slider", 80, 16, &recorder.calls[3].3);
    assert_ne!(
        latched_pixels, committed_pixels,
        "raster must move the thumb from the latched value without a new commit"
    );
    let commit_events = session.handle_pointer(IslandPointerPhase::Up, 280.0, 196.0);
    assert_eq!(commit_events.len(), 1);
    assert_eq!(commit_events[0].event, "valuecommit");
    assert_eq!(commit_events[0].detail["value"], drag[0].detail["value"]);

    let through_cover = hit_test_island(&session.hit_targets(), 40.0, 80.0);
    assert!(
        matches!(through_cover, IslandHit::Video { ref id } if id == "hero"),
        "Cover box-none must not swallow hits meant for video, got {through_cover:?}"
    );
}

#[test]
fn unmounting_a_parent_and_its_child_in_one_commit_is_applied() {
    // The view enumerates every removed node, ancestors first. Unmounting the
    // parent cascades the child away, so the child's own op must not reject the
    // commit and latch the root into Failed.
    let root = root();
    let mut registry = RootRegistry::new(HostCapabilities::default());
    let cover = node(&root, "cover", 1);
    let caption = node(&root, "caption", 1);
    let first = commit(
        &root,
        0,
        1,
        vec![
            mount(&root, "cover", "view", None, 0),
            mount(&root, "caption", "text", Some(cover.clone()), 0),
        ],
    );
    assert!(matches!(
        apply_root_commit(&mut registry, &first),
        ApplyCommitOutcome::Applied(_)
    ));

    let second = commit(
        &root,
        1,
        2,
        vec![
            NativeRootOperation::Unmount { node: cover },
            NativeRootOperation::Unmount { node: caption },
        ],
    );
    match apply_root_commit(&mut registry, &second) {
        ApplyCommitOutcome::Applied(_) => {}
        other => panic!("{other:?}"),
    }
    let state = registry.get(&root).unwrap();
    assert!(state.nodes.is_empty());
    assert_ne!(state.lifecycle, RootLifecycle::Failed);
}
