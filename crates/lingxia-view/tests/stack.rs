use lingxia_view::protocol::{field, tag};
use lingxia_view::{Renderer, protocol::decode};
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
fn stack_overlaps_in_source_order_and_insets_shift_shared_geometry() {
    let mut wire = vec![];
    node(&mut wire, tag::STACK, -1, "stack", json!([]));
    node(
        &mut wire,
        tag::BUTTON,
        0,
        "back",
        json!([
            field::WIDTH,
            80,
            field::HEIGHT,
            50,
            field::PADDING,
            0,
            field::BACKGROUND,
            "#ff0000",
            field::ON_CLICK,
            1
        ]),
    );
    node(
        &mut wire,
        tag::BUTTON,
        0,
        "front",
        json!([
            field::WIDTH,
            80,
            field::HEIGHT,
            50,
            field::PADDING,
            0,
            field::BACKGROUND,
            "#0000ff",
            field::ON_CLICK,
            2,
            field::INSET_LEFT,
            20,
            field::INSET_TOP,
            10
        ]),
    );
    let frame = Renderer::default()
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
fn nested_stack_sizes_to_largest_unshifted_child_and_relative_insets_do_not_reflow_siblings() {
    let mut wire = vec![];
    node(&mut wire, tag::COLUMN, -1, "root", json!([]));
    let stack = node(&mut wire, tag::STACK, 0, "stack", json!([]));
    node(
        &mut wire,
        tag::COLUMN,
        stack,
        "small",
        json!([field::WIDTH, 20, field::HEIGHT, 20]),
    );
    node(
        &mut wire,
        tag::COLUMN,
        stack,
        "large",
        json!([
            field::WIDTH,
            40,
            field::HEIGHT,
            40,
            field::INSET_LEFT,
            -5,
            field::INSET_TOP,
            10
        ]),
    );
    node(
        &mut wire,
        tag::COLUMN,
        0,
        "after",
        json!([field::HEIGHT, 10]),
    );
    let frame = Renderer::default()
        .layout(&decode(&wire).unwrap(), 200, 100, 1.0)
        .unwrap();
    assert_eq!(frame.nodes[1].bounds[3], 40.0);
    assert_eq!(frame.nodes[3].bounds, [-5.0, 10.0, 40.0, 40.0]);
    assert_eq!(frame.nodes[4].bounds[1], 40.0);
}

#[test]
fn insets_are_bounded_lengths_or_percentages_and_stack_offsets_are_reserved() {
    for value in [
        json!(null),
        json!("1"),
        json!("x%"),
        json!(16385),
        json!(-16385),
    ] {
        let mut wire = vec![];
        node(&mut wire, tag::STACK, -1, "root", json!([]));
        node(
            &mut wire,
            tag::COLUMN,
            0,
            "child",
            json!([field::INSET_TOP, value]),
        );
        assert!(decode(&wire).is_err());
    }
    let mut wire = vec![];
    node(&mut wire, tag::ROW, -1, "root", json!([]));
    node(
        &mut wire,
        tag::COLUMN,
        0,
        "child",
        json!([field::INSET_TOP, "25%", field::POSITION, "absolute"]),
    );
    assert!(decode(&wire).is_ok());
    assert!(
        decode(
            json!([tag::STACK, -1, "", "", "", 1, field::OFFSET_X, 0])
                .as_array()
                .unwrap()
        )
        .is_err()
    );
}
