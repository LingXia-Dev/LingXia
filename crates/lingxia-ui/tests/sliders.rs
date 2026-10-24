use lingxia_ui::{Renderer, schema::decode};
use serde_json::json;

#[test]
fn slider_wire_validates_numeric_bounds_labels_and_leaf_shape() {
    for fields in [
        vec![json!(42), json!(null)],
        vec![json!(43), json!(100)],
        vec![json!(44), json!(0)],
        vec![json!(45), json!(0)],
        vec![json!(45), json!(101)],
        vec![json!(45), json!(1e-8)],
        vec![json!(46), json!(0)],
        vec![json!(8), json!(1)],
    ] {
        let mut wire = vec![
            json!(15),
            json!(-1),
            json!(""),
            json!(""),
            json!(""),
            json!(2),
            json!(41),
            json!("Volume"),
        ];
        wire.extend(fields);
        assert!(decode(&wire).is_err(), "{wire:?}");
    }
    for wire in [
        json!([15, -1, "", "", "", 0]),
        json!([15, -1, "", "", "", 1, 41, " "]),
        json!([15, -1, "", "", "", 1, 41, "Range", 2, 0, "", "", "child", 0]),
        json!([2, -1, "", "", "", 1, 42, 1]),
    ] {
        assert!(decode(wire.as_array().unwrap()).is_err());
    }
}

#[test]
fn slider_snapshot_and_pixels_share_normalized_position_and_disable_handles() {
    for (value, expected) in [(-2.0, 0.0), (4.0, 3.0), (9.6, 10.0), (20.0, 10.0)] {
        let wire = json!([
            15, -1, "", "volume", "", 7, 41, "Volume", 42, value, 43, 0, 44, 10, 45, 3, 46, 7, 5,
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
    let wire = json!([15, -1, "", "", "", 3, 41, "Range", 9, 0, 46, 7]);
    let frame = Renderer::default()
        .layout(&decode(wire.as_array().unwrap()).unwrap(), 200, 100, 1.0)
        .unwrap();
    assert!(!frame.nodes[0].range.as_ref().unwrap().enabled);
    assert_eq!(frame.nodes[0].range.as_ref().unwrap().on_change, None);
}
