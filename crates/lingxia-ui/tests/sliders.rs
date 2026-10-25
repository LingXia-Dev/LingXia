use lingxia_ui::protocol::field::{
    ACCESSIBILITY_LABEL, COLOR, ENABLED, ON_CLICK, ON_SLIDER_CHANGE, SLIDER_MAX, SLIDER_MIN,
    SLIDER_STEP, SLIDER_VALUE,
};
use lingxia_ui::protocol::tag::{SLIDER, TEXT};
use lingxia_ui::{Renderer, protocol::decode};
use serde_json::json;

#[test]
fn slider_wire_validates_numeric_bounds_labels_and_leaf_shape() {
    for fields in [
        vec![json!(SLIDER_VALUE), json!(null)],
        vec![json!(SLIDER_MIN), json!(100)],
        vec![json!(SLIDER_MAX), json!(0)],
        vec![json!(SLIDER_STEP), json!(0)],
        vec![json!(SLIDER_STEP), json!(101)],
        vec![json!(SLIDER_STEP), json!(1e-8)],
        vec![json!(ON_SLIDER_CHANGE), json!(0)],
        vec![json!(ON_CLICK), json!(1)],
    ] {
        let mut wire = vec![
            json!(SLIDER),
            json!(-1),
            json!(""),
            json!(""),
            json!(""),
            json!(2),
            json!(ACCESSIBILITY_LABEL),
            json!("Volume"),
        ];
        wire.extend(fields);
        assert!(decode(&wire).is_err(), "{wire:?}");
    }
    for wire in [
        json!([SLIDER, -1, "", "", "", 0]),
        json!([SLIDER, -1, "", "", "", 1, ACCESSIBILITY_LABEL, " "]),
        json!([
            SLIDER,
            -1,
            "",
            "",
            "",
            1,
            ACCESSIBILITY_LABEL,
            "Range",
            TEXT,
            0,
            "",
            "",
            "child",
            0
        ]),
        json!([TEXT, -1, "", "", "", 1, SLIDER_VALUE, 1]),
    ] {
        assert!(decode(wire.as_array().unwrap()).is_err());
    }
}

#[test]
fn slider_snapshot_and_pixels_share_normalized_position_and_disable_handles() {
    for (value, expected) in [(-2.0, 0.0), (4.0, 3.0), (9.6, 10.0), (20.0, 10.0)] {
        let wire = json!([
            SLIDER,
            -1,
            "",
            "volume",
            "",
            7,
            ACCESSIBILITY_LABEL,
            "Volume",
            SLIDER_VALUE,
            value,
            SLIDER_MIN,
            0,
            SLIDER_MAX,
            10,
            SLIDER_STEP,
            3,
            ON_SLIDER_CHANGE,
            7,
            COLOR,
            "#ff0000"
        ]);
        let frame = Renderer::default()
            .layout(&decode(wire.as_array().unwrap()).unwrap(), 200, 100, 1.0)
            .unwrap();
        let range = frame.nodes[0].range.as_ref().unwrap();
        assert_eq!(range.value, expected);
        assert_eq!(range.inset, 24.0);
        assert_eq!(range.on_change, Some(7));
        assert_eq!(frame.nodes[0].bounds[3], 48.0);
        let thumb = (24.0 + 152.0 * expected / 10.0) as usize;
        let pixels = frame.rasterize().unwrap().pixels;
        let offset = (24 * 200 + thumb) * 4;
        assert_eq!(
            &pixels.data_as_u8_slice()[offset..offset + 4],
            &[255, 0, 0, 255]
        );
    }
    let wire = json!([
        SLIDER,
        -1,
        "",
        "",
        "",
        3,
        ACCESSIBILITY_LABEL,
        "Range",
        ENABLED,
        0,
        ON_SLIDER_CHANGE,
        7
    ]);
    let frame = Renderer::default()
        .layout(&decode(wire.as_array().unwrap()).unwrap(), 200, 100, 1.0)
        .unwrap();
    assert!(!frame.nodes[0].range.as_ref().unwrap().enabled);
    assert_eq!(frame.nodes[0].range.as_ref().unwrap().on_change, None);
}
