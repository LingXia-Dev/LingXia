use lingxia_view::protocol::field::{
    BACKGROUND, FONT_SIZE, GAP, GRID_COLUMNS, HEIGHT, ON_CLICK, PADDING,
};
use lingxia_view::protocol::tag::{BUTTON, COLUMN, GRID, TEXT};
use lingxia_view::{Renderer, protocol::decode};
use serde_json::{Value, json};

#[test]
fn grid_columns_are_bounded_and_tag_specific() {
    for value in [
        json!(0),
        json!(65),
        json!(1.5),
        json!("3"),
        json!(null),
        json!(-1),
    ] {
        assert!(
            decode(
                json!([GRID, -1, "", "", "", 1, GRID_COLUMNS, value])
                    .as_array()
                    .unwrap()
            )
            .is_err()
        );
    }
    // A grid without columns is one implicit column, as in CSS.
    assert!(decode(json!([GRID, -1, "", "", "", 0]).as_array().unwrap()).is_ok());
    assert!(
        decode(
            json!([COLUMN, -1, "", "", "", 1, GRID_COLUMNS, 3])
                .as_array()
                .unwrap()
        )
        .is_err()
    );
    for columns in [1, 64] {
        assert!(
            decode(
                json!([GRID, -1, "", "", "", 1, GRID_COLUMNS, columns])
                    .as_array()
                    .unwrap()
            )
            .is_ok()
        );
    }
}

fn cards(columns: u32) -> Vec<Value> {
    let mut wire = json!([
        COLUMN,
        -1,
        "",
        "root",
        "",
        0,
        GRID,
        0,
        "grid",
        "grid",
        "",
        3,
        GRID_COLUMNS,
        columns,
        GAP,
        10,
        PADDING,
        10
    ])
    .as_array()
    .unwrap()
    .clone();
    for (i, height) in [20, 40, 30, 25].iter().enumerate() {
        wire.extend(
            json!([
                BUTTON,
                6,
                format!("card-{i}"),
                format!("card-{i}"),
                "",
                4,
                HEIGHT,
                height,
                PADDING,
                0,
                BACKGROUND,
                "#ff0000",
                ON_CLICK,
                i + 1
            ])
            .as_array()
            .unwrap()
            .clone(),
        );
    }
    wire
}

#[test]
fn equal_grid_tracks_wrap_in_source_order_and_keep_identity_after_reflow() {
    let mut renderer = Renderer::default();
    let frame = renderer
        .layout(&decode(&cards(3)).unwrap(), 340, 200, 1.0)
        .unwrap();
    assert_eq!(frame.nodes[1].bounds, [0.0, 0.0, 340.0, 95.0]);
    for (index, expected) in [
        [10.0, 10.0, 100.0, 20.0],
        [120.0, 10.0, 100.0, 40.0],
        [230.0, 10.0, 100.0, 30.0],
        [10.0, 60.0, 100.0, 25.0],
    ]
    .iter()
    .enumerate()
    {
        assert_eq!(&frame.nodes[index + 2].bounds, expected);
        assert_eq!(frame.nodes[index + 2].on_click, Some((index + 1) as u64));
    }
    let nodes = frame.nodes.clone();
    let pixels = frame.rasterize().unwrap().pixels;
    assert_eq!(
        &pixels.data_as_u8_slice()[(15 * 340 + 15) * 4..(15 * 340 + 15) * 4 + 4],
        &[255, 0, 0, 255]
    );
    let next = renderer
        .layout(&decode(&cards(2)).unwrap(), 230, 200, 1.0)
        .unwrap();
    assert_eq!(next.nodes[4].bounds, [10.0, 60.0, 100.0, 30.0]);
    for (before, after) in nodes.iter().zip(&next.nodes) {
        assert_eq!(before.identity, after.identity);
    }
}

#[test]
fn grid_text_wraps_without_widening_equal_columns() {
    let wire = json!([
        COLUMN,
        -1,
        "",
        "",
        "",
        0,
        GRID,
        0,
        "",
        "",
        "",
        2,
        GRID_COLUMNS,
        2,
        GAP,
        8,
        TEXT,
        6,
        "",
        "long",
        "A long card title that must wrap across several lines",
        1,
        FONT_SIZE,
        20,
        TEXT,
        6,
        "",
        "short",
        "Short",
        1,
        FONT_SIZE,
        20
    ]);
    let frame = Renderer::default()
        .layout(&decode(wire.as_array().unwrap()).unwrap(), 208, 400, 1.0)
        .unwrap();
    assert_eq!(frame.nodes[2].bounds[2], 100.0);
    assert_eq!(frame.nodes[3].bounds[2], 100.0);
    assert!(frame.nodes[2].bounds[3] > 40.0);
    assert!(frame.nodes[1].bounds[3] >= frame.nodes[2].bounds[3]);
}
