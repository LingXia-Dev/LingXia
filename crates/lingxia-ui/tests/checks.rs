use lingxia_ui::schema::decode;
use serde_json::json;

#[test]
fn boolean_control_wire_rejects_invalid_values_callbacks_and_children() {
    for tag in [10, 11] {
        let valid = json!([tag, -1, "", "preference", "A label", 3, 39, 1, 40, 42, 9, 1]);
        assert_eq!(decode(valid.as_array().unwrap()).unwrap()[0].tag, tag);
        for invalid in [
            json!([tag, -1, "", "", "", 1, 39, true]),
            json!([tag, -1, "", "", "", 1, 39, 2]),
            json!([tag, -1, "", "", "", 1, 39, -1]),
            json!([tag, -1, "", "", "", 1, 40, 0]),
            json!([tag, -1, "", "", "", 1, 40, 9007199254740992u64]),
            json!([tag, -1, "", "", "", 1, 8, 42]),
            json!([tag, -1, "", "", "", 1, 26, 42]),
            json!([tag, -1, "", "", "", 0, 2, 0, "", "", "Child", 0]),
        ] {
            assert!(decode(invalid.as_array().unwrap()).is_err(), "{invalid}");
        }
    }
    for tag in 0..10 {
        for field in [39, 40] {
            let invalid = json!([tag, -1, "", "", "", 1, field, 1]);
            assert!(decode(invalid.as_array().unwrap()).is_err(), "{invalid}");
        }
    }
}

fn radio_wire(selected: usize, disabled: bool) -> Vec<serde_json::Value> {
    let mut wire = vec![
        json!(12),
        json!(-1),
        json!("pace"),
        json!("pace"),
        json!(""),
        json!(2),
        json!(41),
        json!("Pace"),
        json!(9),
        json!(!disabled as u8),
    ];
    for (index, key) in ["daily", "weekly"].iter().enumerate() {
        wire.extend([
            json!(13),
            json!(0),
            json!(key),
            json!(key),
            json!(key),
            json!(2),
            json!(39),
            json!((index == selected) as u8),
            json!(40),
            json!(index + 1),
        ]);
    }
    wire
}

#[test]
fn radio_wire_requires_bounded_unique_options_and_a_single_selection() {
    assert!(decode(&radio_wire(0, false)).is_ok());
    let mut oversized = radio_wire(0, false);
    for index in 2..65 {
        oversized.extend([
            json!(13),
            json!(0),
            json!(index.to_string()),
            json!(""),
            json!("Option"),
            json!(1),
            json!(39),
            json!(0),
        ]);
    }
    assert!(decode(&oversized).is_err());
    for wire in [
        json!([12, -1, "", "", "", 0]),
        json!([12, -1, "", "", "", 0, 2, 0, "", "", "Wrong child", 0]),
        json!([13, -1, "a", "", "A", 1, 39, 1]),
        json!([0, -1, "", "", "", 0, 13, 0, "a", "", "A", 1, 39, 1]),
        json!([
            12, -1, "", "", "", 0, 13, 0, "a", "", "A", 1, 39, 1, 13, 0, "b", "", "B", 1, 39, 1
        ]),
        json!([
            12, -1, "", "", "", 0, 13, 0, "a", "", "A", 1, 39, 1, 13, 0, "a", "", "Again", 1, 39, 0
        ]),
        json!([12, -1, "", "", "", 0, 13, 0, "", "", "A", 1, 39, 0]),
        json!([12, -1, "", "", "", 0, 13, 0, "a", "", "A", 0]),
        json!([0, -1, "", "", "", 1, 41, "Wrong role"]),
    ] {
        assert!(decode(wire.as_array().unwrap()).is_err(), "{wire}");
    }
}

#[test]
fn radio_options_share_group_identity_and_render_selection_at_each_scale() {
    for scale in [1.0, 2.75] {
        let mut renderer = lingxia_ui::Renderer::default();
        let first = renderer
            .layout(
                &decode(&radio_wire(0, false)).unwrap(),
                (240.0 * scale) as u16,
                (140.0 * scale) as u16,
                scale,
            )
            .unwrap();
        assert_eq!(first.nodes[0].accessibility_label.as_deref(), Some("Pace"));
        assert_eq!(
            first.nodes[1].radio_group.as_deref(),
            Some(first.nodes[0].identity.as_str())
        );
        assert_eq!(first.nodes[1].radio_group, first.nodes[2].radio_group);
        assert_eq!(first.nodes[1].checked, Some(true));
        assert_eq!(first.nodes[2].checked, Some(false));
        assert!(first.nodes[1].bounds[3] >= 44.0);
        let next = renderer
            .layout(
                &decode(&radio_wire(1, false)).unwrap(),
                (240.0 * scale) as u16,
                (140.0 * scale) as u16,
                scale,
            )
            .unwrap();
        assert_eq!(next.nodes[1].checked, Some(false));
        assert_eq!(next.nodes[2].checked, Some(true));
        assert_eq!(first.nodes[1].bounds, next.nodes[1].bounds);
        assert_ne!(
            first.rasterize().unwrap().pixels.data(),
            next.rasterize().unwrap().pixels.data()
        );
        let disabled = renderer
            .layout(
                &decode(&radio_wire(0, true)).unwrap(),
                (240.0 * scale) as u16,
                (140.0 * scale) as u16,
                scale,
            )
            .unwrap();
        assert!(disabled.nodes.iter().all(|node| node.on_click.is_none()));
    }
}
