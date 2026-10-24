use lingxia_ui::{Renderer, schema::decode};
use serde_json::json;

#[test]
fn input_wire_validates_leaf_shape_flags_text_and_callback_kinds() {
    for tag in [8, 9] {
        let valid = json!([
            tag,
            -1,
            "field",
            "draft",
            "",
            7,
            21,
            "你好😀",
            22,
            "Write here",
            23,
            0,
            24,
            "text",
            25,
            1,
            26,
            10,
            27,
            11
        ]);
        assert_eq!(decode(valid.as_array().unwrap()).unwrap()[0].tag, tag);
    }
    for invalid in [
        json!([8, -1, "", "", "", 1, 21, 42]),
        json!([8, -1, "", "", "", 1, 23, 2]),
        json!([8, -1, "", "", "", 1, 24, "unknown"]),
        json!([9, -1, "", "", "", 1, 26, 0]),
        json!([3, -1, "", "", "", 1, 26, 1]),
        json!([8, -1, "", "", "", 1, 8, 1]),
        json!([8, -1, "", "", "child text", 0]),
        json!([8, -1, "", "", "", 0, 2, 0, "", "", "child", 0]),
        json!([8, -1, "", "", "", 1, 21, "中".repeat(22000)]),
    ] {
        assert!(decode(invalid.as_array().unwrap()).is_err(), "{invalid}");
    }
}

#[test]
fn input_preview_masks_passwords_preserves_placeholder_and_clips_text() {
    let values = json!([
        0,
        -1,
        "",
        "",
        "",
        2,
        3,
        10,
        6,
        "#FFFFFF",
        8,
        0,
        "first",
        "field",
        "",
        7,
        0,
        80,
        1,
        40,
        5,
        "#FFFFFF",
        6,
        "#2458DA",
        21,
        "W".repeat(100),
        23,
        1,
        10,
        12,
        9,
        0,
        "second",
        "area",
        "",
        4,
        0,
        80,
        1,
        60,
        21,
        "",
        22,
        "Placeholder"
    ]);
    let records = decode(values.as_array().unwrap()).unwrap();
    let display = Renderer::default().layout(&records, 160, 160, 1.0).unwrap();
    assert_eq!(display.nodes[1].text, "•".repeat(100));
    assert_eq!(display.nodes[2].text, "Placeholder");
    assert!(display.nodes[1].input.as_ref().unwrap().secure);
    let frame = display.rasterize().unwrap();
    let pixel = |x: usize, y: usize| {
        &frame.pixels.data_as_u8_slice()[(y * 160 + x) * 4..(y * 160 + x) * 4 + 4]
    };
    for y in 10..50 {
        for x in 91..150 {
            assert_eq!(pixel(x, y), [255, 255, 255, 255]);
        }
    }
    assert_eq!(pixel(10, 10), [255, 255, 255, 255]);
    assert_eq!(pixel(50, 45), [36, 88, 218, 255]);
}

#[test]
fn single_line_presentation_does_not_mutate_the_controlled_value() {
    let value = "first\nsecond\u{2028}third";
    for tag in [8, 9, 14] {
        let wire = json!([tag, -1, "", "", "", 1, 21, value]);
        let display = Renderer::default()
            .layout(&decode(wire.as_array().unwrap()).unwrap(), 200, 200, 1.0)
            .unwrap();
        assert_eq!(
            display.nodes[0].text,
            if tag != 9 {
                "first second third"
            } else {
                value
            }
        );
        assert_eq!(display.nodes[0].input.as_ref().unwrap().value, value);
    }
}

#[test]
fn search_wire_owns_keyboard_and_rejects_secure_or_container_shapes() {
    let wire = json!([
        14, -1, "query", "query", "", 4, 21, "coffee", 24, "search", 25, 1, 27, 1
    ]);
    let records = decode(wire.as_array().unwrap()).unwrap();
    let display = Renderer::default().layout(&records, 240, 100, 1.0).unwrap();
    assert_eq!(display.nodes[0].tag, "search");
    assert_eq!(display.nodes[0].input.as_ref().unwrap().keyboard, "search");
    assert_eq!(display.nodes[0].bounds[3], 48.0);
    for wire in [
        json!([14, -1, "", "", "", 1, 23, 0]),
        json!([14, -1, "", "", "", 1, 24, "text"]),
        json!([14, -1, "", "", "", 0, 2, 0, "", "", "child", 0]),
    ] {
        assert!(decode(wire.as_array().unwrap()).is_err());
    }
    let wire = json!([14, -1, "", "", "", 0]);
    let display = Renderer::default()
        .layout(&decode(wire.as_array().unwrap()).unwrap(), 240, 100, 1.0)
        .unwrap();
    assert_eq!(display.nodes[0].input.as_ref().unwrap().keyboard, "search");
}
