use lingxia_view::protocol::{decode, field, tag};
use serde_json::json;

#[test]
fn boolean_control_wire_rejects_invalid_values_callbacks_and_children() {
    use field::{CHECKED, ENABLED, ON_CHECKED_CHANGE, ON_CLICK, ON_INPUT};
    use tag::{CHECKBOX, COLUMN, SWITCH, TEXT};
    let text_child = |t: usize| json!([t, -1, "", "", "", 0, TEXT, 0, "", "", "Child", 0]);
    for t in [CHECKBOX, SWITCH] {
        let valid = json!([
            t,
            -1,
            "",
            "preference",
            "A label",
            3,
            CHECKED,
            1,
            ON_CHECKED_CHANGE,
            42,
            ENABLED,
            1
        ]);
        assert_eq!(decode(valid.as_array().unwrap()).unwrap()[0].tag, t);
        for invalid in [
            json!([t, -1, "", "", "", 1, CHECKED, true]),
            json!([t, -1, "", "", "", 1, CHECKED, 2]),
            json!([t, -1, "", "", "", 1, CHECKED, -1]),
            json!([t, -1, "", "", "", 1, ON_CHECKED_CHANGE, 0]),
            json!([t, -1, "", "", "", 1, ON_CHECKED_CHANGE, 9007199254740992u64]),
            json!([t, -1, "", "", "", 1, ON_CLICK, 42]),
            json!([t, -1, "", "", "", 1, ON_INPUT, 42]),
        ] {
            assert!(decode(invalid.as_array().unwrap()).is_err(), "{invalid}");
        }
    }
    // A checkbox composes content; a switch is a leaf; containers carry no inline text.
    assert!(decode(text_child(CHECKBOX).as_array().unwrap()).is_ok());
    assert!(decode(text_child(SWITCH).as_array().unwrap()).is_err());
    let labeled_column = json!([CHECKBOX, -1, "", "", "", 0, COLUMN, 0, "", "", "Child", 0]);
    assert!(decode(labeled_column.as_array().unwrap()).is_err());
    for t in 0..CHECKBOX {
        for f in [CHECKED, ON_CHECKED_CHANGE] {
            let invalid = json!([t, -1, "", "", "", 1, f, 1]);
            assert!(decode(invalid.as_array().unwrap()).is_err(), "{invalid}");
        }
    }
}

fn radio_wire(selected: usize, disabled: bool) -> Vec<serde_json::Value> {
    let mut wire = vec![
        json!(tag::RADIO),
        json!(-1),
        json!("pace"),
        json!("pace"),
        json!(""),
        json!(2),
        json!(field::ACCESSIBILITY_LABEL),
        json!("Pace"),
        json!(field::ENABLED),
        json!(!disabled as u8),
    ];
    for (index, key) in ["daily", "weekly"].iter().enumerate() {
        wire.extend([
            json!(tag::RADIO_OPTION),
            json!(0),
            json!(key),
            json!(key),
            json!(key),
            json!(2),
            json!(field::CHECKED),
            json!((index == selected) as u8),
            json!(field::ON_CHECKED_CHANGE),
            json!(index + 1),
        ]);
    }
    wire
}

#[test]
fn radio_wire_requires_bounded_unique_options_and_a_single_selection() {
    use field::{ACCESSIBILITY_LABEL, CHECKED};
    use tag::{COLUMN, RADIO, RADIO_OPTION, TEXT};
    assert!(decode(&radio_wire(0, false)).is_ok());
    let mut oversized = radio_wire(0, false);
    for index in 2..65 {
        oversized.extend([
            json!(RADIO_OPTION),
            json!(0),
            json!(index.to_string()),
            json!(""),
            json!("Option"),
            json!(1),
            json!(CHECKED),
            json!(0),
        ]);
    }
    assert!(decode(&oversized).is_err());
    let option = |key: &str, text: &str, checked: u8| {
        json!([RADIO_OPTION, 0, key, "", text, 1, CHECKED, checked])
    };
    let group = |options: &[serde_json::Value]| {
        let mut wire = vec![
            json!(RADIO),
            json!(-1),
            json!(""),
            json!(""),
            json!(""),
            json!(0),
        ];
        for option in options {
            wire.extend(option.as_array().unwrap().iter().cloned());
        }
        serde_json::Value::Array(wire)
    };
    for wire in [
        group(&[]),
        json!([RADIO, -1, "", "", "", 0, TEXT, 0, "", "", "Wrong child", 0]),
        json!([RADIO_OPTION, -1, "a", "", "A", 1, CHECKED, 1]),
        json!([
            COLUMN,
            -1,
            "",
            "",
            "",
            0,
            RADIO_OPTION,
            0,
            "a",
            "",
            "A",
            1,
            CHECKED,
            1
        ]),
        group(&[option("a", "A", 1), option("b", "B", 1)]),
        group(&[option("a", "A", 1), option("a", "Again", 0)]),
        group(&[option("", "A", 0)]),
        json!([RADIO, -1, "", "", "", 0, RADIO_OPTION, 0, "a", "", "A", 0]),
        json!([
            tag::SPACER,
            -1,
            "",
            "",
            "",
            1,
            ACCESSIBILITY_LABEL,
            "Wrong role"
        ]),
    ] {
        assert!(decode(wire.as_array().unwrap()).is_err(), "{wire}");
    }
}

#[test]
fn radio_options_share_group_identity_and_render_selection_at_each_scale() {
    for scale in [1.0, 2.75] {
        let mut renderer = lingxia_view::Renderer::default();
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
            Some(first.nodes[0].identity.as_ref())
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
