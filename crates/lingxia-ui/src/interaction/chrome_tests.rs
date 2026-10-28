use super::*;

fn node(json: &str) -> ChromeNode {
    ChromeNode::parse(json).unwrap()
}

#[test]
fn independent_own_corners_shape_touches() {
    let n = node(
        r#"{"bounds":[0,0,100,80],"visible_bounds":[0,0,100,80],"radius":24,"radii":[[24,24],[24,24],[6,6],[24,24]]}"#,
    );
    assert!(!contains(&n, 98.0, 2.0, true));
    assert!(contains(&n, 98.0, 78.0, true));
}

#[test]
fn invisible_rounded_corners_cannot_receive_child_touches() {
    let n = node(
        r#"{"bounds":[0,0,100,200],"visible_bounds":[0,0,100,100],"clips":[{"bounds":[0,0,100,100],"radius":30}]}"#,
    );
    assert!(!contains(&n, 1.0, 1.0, false));
    assert!(!contains(&n, 99.0, 99.0, false));
    assert!(contains(&n, 50.0, 1.0, false));
    assert!(contains(&n, 1.0, 50.0, false));
    assert!(!contains(&n, 50.0, 150.0, false));
}

#[test]
fn nested_masks_use_unclipped_geometry_and_all_ancestors() {
    let n = node(
        r#"{"bounds":[0,-20,100,200],"visible_bounds":[0,0,100,60],"clips":[{"bounds":[0,-20,100,80],"radius":30},{"bounds":[0,0,100,100],"radius":10}]}"#,
    );
    assert!(contains(&n, 10.0, 1.0, false));
    assert!(!contains(&n, 1.0, 1.0, false));
    assert!(!contains(&n, 1.0, 59.0, false));
}

#[test]
fn own_radius_is_capped_to_half_the_shorter_edge() {
    let n = node(r#"{"bounds":[10,20,100,40],"visible_bounds":[10,20,100,40],"radius":1000}"#);
    assert!(!contains(&n, 11.0, 21.0, true));
    assert!(contains(&n, 60.0, 21.0, true));
    assert!(contains(&n, 11.0, 40.0, true));
    assert!(contains(&n, 11.0, 21.0, false));
}

#[test]
fn asymmetric_inner_corners_mask_touches() {
    let n = node(
        r#"{"bounds":[0,0,100,80],"visible_bounds":[8,2,88,72],"clips":[{"bounds":[8,2,88,72],"radius":0,"radii":[[12,18],[16,18],[16,14],[12,14]]}]}"#,
    );
    assert!(!contains(&n, 9.0, 3.0, false));
    assert!(contains(&n, 20.0, 3.0, false));
    assert!(contains(&n, 9.0, 20.0, false));
    assert!(!contains(&n, 95.0, 73.0, false));
    assert!(contains(&n, 80.0, 73.0, false));
}

#[test]
fn malformed_inner_radii_cannot_receive_touches() {
    for radii in ["[]", "[[12,18]]", "[[-1,2],[0,0],[0,0],[0,0]]"] {
        let n = node(&format!(
            r#"{{"bounds":[0,0,100,80],"clips":[{{"bounds":[0,0,100,80],"radii":{radii}}}]}}"#
        ));
        assert!(!contains(&n, 50.0, 40.0, false), "{radii}");
    }
}

/// An editor 120x70 with content [10,10,100,40]; anchor and caret line rects.
fn editor(anchor_x: f64, caret_x: f64, clips: &str) -> ChromeNode {
    node(&format!(
        r#"{{"bounds":[0,0,120,70],"visible_bounds":[0,0,120,70],"clips":{clips},"input":{{"geometry":{{"anchor":[{anchor_x},15,1.5,20],"caret":[{caret_x},15,1.5,20],"content_bounds":[10,10,100,40]}}}}}}"#
    ))
}

#[test]
fn handles_sit_below_their_line_and_offscreen_ends_have_none() {
    let n = editor(20.0, 80.0, "[]");
    assert_eq!(endpoint(&n, 0), Some([20.75, 35.0, 25.0]));
    assert_eq!(endpoint(&n, 1), Some([80.75, 35.0, 25.0]));
    let scrolled = editor(-30.0, 80.0, "[]");
    assert_eq!(endpoint(&scrolled, 0), None);
    assert_eq!(handle_hit(&scrolled, 10.0, 43.0, false), None);
}

#[test]
fn hits_take_the_nearest_knob_and_never_steal_taps_on_the_text_line() {
    let n = editor(20.0, 80.0, "[]");
    assert_eq!(handle_hit(&n, 80.75, 43.0, false).map(|h| h.0), Some(1));
    assert_eq!(handle_hit(&n, 20.75, 43.0, false).map(|h| h.0), Some(0));
    assert_eq!(
        handle_hit(&n, 20.75, 43.0, true),
        None,
        "a caret has no anchor knob"
    );
    assert_eq!(
        handle_hit(&n, 80.75, 25.0, false),
        None,
        "above the line bottom"
    );
    assert_eq!(
        handle_hit(&n, 80.75 + 23.0, 43.0, false),
        None,
        "beyond the radius"
    );
}

#[test]
fn handle_hits_respect_ancestor_clips() {
    let clipped = editor(20.0, 80.0, r#"[{"bounds":[0,0,120,40],"radius":0}]"#);
    assert_eq!(handle_hit(&clipped, 80.75, 25.0, false), None);
    assert_eq!(handle_hit(&clipped, 80.75, 50.0, false), None);
    assert_eq!(
        handle_hit(&clipped, 80.75, 38.0, false).map(|h| h.0),
        Some(1)
    );
}

#[test]
fn drags_clamp_into_visible_content_and_report_the_edge() {
    let n = editor(20.0, 80.0, "[]");
    let inside = drag_point(&n, 90.0, 43.0).unwrap();
    assert_eq!(inside.point, [90.0, 43.0]);
    assert_eq!(inside.edge, [0, 0]);
    assert!(inside.visible);
    let beyond = drag_point(&n, -30.0, 43.0).unwrap();
    assert_eq!(beyond.point, [10.5, 43.0]);
    assert_eq!(beyond.edge, [-1, 0]);
    let below = drag_point(&n, 50.0, 90.0).unwrap();
    assert_eq!(below.point, [50.0, 49.5]);
    assert_eq!(below.edge, [0, 1]);
    let gone = node(
        r#"{"visible_bounds":[0,0,120,0.5],"input":{"geometry":{"content_bounds":[10,10,100,40]}}}"#,
    );
    assert_eq!(drag_point(&gone, 50.0, 20.0), None);
}

#[test]
fn feedback_shape_is_the_own_shape_inside_visible_bounds_and_clips() {
    let n = node(
        r#"{"bounds":[10,20,100,40],"visible_bounds":[10,20,100,30],"radius":50,"clips":[{"bounds":[0,0,200,50],"radius":8}]}"#,
    );
    let shapes = feedback_shape(&n).unwrap();
    assert_eq!(shapes.len(), 3);
    assert_eq!(&shapes[0][..6], &[10.0, 20.0, 100.0, 40.0, 20.0, 20.0]);
    assert_eq!(&shapes[1][..5], &[10.0, 20.0, 100.0, 30.0, 0.0]);
    assert_eq!(&shapes[2][4..6], &[8.0, 8.0]);
}

#[test]
fn feedback_shape_is_none_when_clipped_away_or_malformed() {
    assert!(
        feedback_shape(&node(
            r#"{"bounds":[0,0,10,10],"visible_bounds":[0,0,0,0]}"#
        ))
        .is_none()
    );
    assert!(
        feedback_shape(&node(
            r#"{"bounds":[0,0,10,10],"clips":[{"bounds":[0,0,10,10],"radii":[[1,1]]}]}"#
        ))
        .is_none()
    );
}
