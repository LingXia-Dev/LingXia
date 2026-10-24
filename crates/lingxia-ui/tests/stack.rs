use lingxia_ui::{Renderer, schema::decode};
use serde_json::{Value, json};

fn node(wire: &mut Vec<Value>, tag: usize, parent: i64, id: &str, fields: Value) -> i64 {
    let offset = wire.len() as i64;
    let fields = fields.as_array().unwrap();
    wire.extend(
        json!([tag, parent, id, id, "", fields.len() / 2])
            .as_array()
            .unwrap()
            .clone(),
    );
    wire.extend(fields.iter().cloned());
    offset
}

#[test]
fn stack_overlaps_in_source_order_and_offsets_shared_geometry() {
    let mut wire = vec![];
    node(&mut wire, 17, -1, "stack", json!([]));
    node(
        &mut wire,
        3,
        0,
        "back",
        json!([0, 80, 1, 50, 3, 0, 6, "#ff0000", 8, 1]),
    );
    node(
        &mut wire,
        3,
        0,
        "front",
        json!([0, 80, 1, 50, 3, 0, 6, "#0000ff", 8, 2, 48, 20, 49, 10]),
    );
    let mut renderer = Renderer::default();
    let frame = renderer
        .layout(&decode(&wire).unwrap(), 200, 100, 1.0)
        .unwrap();
    assert_eq!(frame.nodes[1].bounds, [0.0, 0.0, 80.0, 50.0]);
    assert_eq!(frame.nodes[2].bounds, [20.0, 10.0, 80.0, 50.0]);
    assert_eq!(frame.nodes[2].on_click, Some(2));
    let pixels = frame.rasterize().unwrap().pixels;
    for (x, y, color) in [(5, 5, [255, 0, 0, 255]), (25, 15, [0, 0, 255, 255])] {
        let offset = (y * 200 + x) * 4;
        assert_eq!(&pixels.data_as_u8_slice()[offset..offset + 4], &color);
    }
}

#[test]
fn nested_stack_sizes_to_largest_unshifted_child_and_offsets_do_not_reflow_siblings() {
    let mut wire = vec![];
    node(&mut wire, 0, -1, "root", json!([]));
    let stack = node(&mut wire, 17, 0, "stack", json!([]));
    node(&mut wire, 0, stack, "small", json!([0, 20, 1, 20]));
    node(
        &mut wire,
        0,
        stack,
        "large",
        json!([0, 40, 1, 40, 48, -5, 49, 10]),
    );
    node(&mut wire, 0, 0, "after", json!([1, 10]));
    let frame = Renderer::default()
        .layout(&decode(&wire).unwrap(), 200, 100, 1.0)
        .unwrap();
    assert_eq!(frame.nodes[1].bounds[3], 40.0);
    assert_eq!(frame.nodes[3].bounds, [-5.0, 10.0, 40.0, 40.0]);
    assert_eq!(frame.nodes[4].bounds[1], 40.0);
}

#[test]
fn offset_wire_requires_direct_stack_parent_and_finite_bounded_values() {
    for parent in [0, 1, 16] {
        let mut wire = vec![];
        node(
            &mut wire,
            parent,
            -1,
            "root",
            if parent == 16 {
                json!([47, 1])
            } else {
                json!([])
            },
        );
        node(&mut wire, 0, 0, "child", json!([48, 1]));
        assert!(decode(&wire).is_err());
    }
    for value in [json!(null), json!("1"), json!(16385), json!(-16385)] {
        let mut wire = vec![];
        node(&mut wire, 17, -1, "root", json!([]));
        node(&mut wire, 0, 0, "child", json!([49, value]));
        assert!(decode(&wire).is_err());
    }
    assert!(decode(json!([17, -1, "", "", "", 1, 48, 0]).as_array().unwrap()).is_err());
}
