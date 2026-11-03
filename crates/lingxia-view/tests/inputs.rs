use lingxia_view::protocol::field::{
    AUTO_FOCUS, BACKGROUND, COLOR, HEIGHT, INPUT_VALUE, KEYBOARD, ON_CLICK, ON_INPUT, ON_SUBMIT,
    PADDING, PLACEHOLDER, RADIUS, SECURE, WIDTH,
};
use lingxia_view::protocol::tag::{BUTTON, COLUMN, INPUT, SEARCH, TEXT, TEXTAREA};
use lingxia_view::{Renderer, protocol::decode};
use serde_json::json;

#[test]
fn input_wire_validates_leaf_shape_flags_text_and_callback_kinds() {
    for tag in [INPUT, TEXTAREA] {
        let valid = json!([
            tag,
            -1,
            "field",
            "draft",
            "",
            7,
            INPUT_VALUE,
            "你好😀",
            PLACEHOLDER,
            "Write here",
            SECURE,
            0,
            KEYBOARD,
            "text",
            AUTO_FOCUS,
            1,
            ON_INPUT,
            10,
            ON_SUBMIT,
            11
        ]);
        assert_eq!(decode(valid.as_array().unwrap()).unwrap()[0].tag, tag);
    }
    for invalid in [
        json!([INPUT, -1, "", "", "", 1, INPUT_VALUE, 42]),
        json!([INPUT, -1, "", "", "", 1, SECURE, 2]),
        json!([INPUT, -1, "", "", "", 1, KEYBOARD, "unknown"]),
        json!([TEXTAREA, -1, "", "", "", 1, ON_INPUT, 0]),
        json!([BUTTON, -1, "", "", "", 1, ON_INPUT, 1]),
        json!([INPUT, -1, "", "", "", 1, ON_CLICK, 1]),
        json!([INPUT, -1, "", "", "child text", 0]),
        json!([INPUT, -1, "", "", "", 0, TEXT, 0, "", "", "child", 0]),
        json!([INPUT, -1, "", "", "", 1, INPUT_VALUE, "中".repeat(22000)]),
    ] {
        assert!(decode(invalid.as_array().unwrap()).is_err(), "{invalid}");
    }
}

#[test]
fn input_preview_masks_passwords_preserves_placeholder_and_clips_text() {
    let values = json!([
        COLUMN,
        -1,
        "",
        "",
        "",
        2,
        PADDING,
        10,
        BACKGROUND,
        "#FFFFFF",
        INPUT,
        0,
        "first",
        "field",
        "",
        7,
        WIDTH,
        80,
        HEIGHT,
        40,
        COLOR,
        "#FFFFFF",
        BACKGROUND,
        "#2458DA",
        INPUT_VALUE,
        "W".repeat(100),
        SECURE,
        1,
        RADIUS,
        12,
        TEXTAREA,
        0,
        "second",
        "area",
        "",
        4,
        WIDTH,
        80,
        HEIGHT,
        60,
        INPUT_VALUE,
        "",
        PLACEHOLDER,
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
    for tag in [INPUT, TEXTAREA, SEARCH] {
        let wire = json!([tag, -1, "", "", "", 1, INPUT_VALUE, value]);
        let display = Renderer::default()
            .layout(&decode(wire.as_array().unwrap()).unwrap(), 200, 200, 1.0)
            .unwrap();
        assert_eq!(
            display.nodes[0].text,
            if tag != TEXTAREA {
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
        SEARCH,
        -1,
        "query",
        "query",
        "",
        4,
        INPUT_VALUE,
        "coffee",
        KEYBOARD,
        "search",
        AUTO_FOCUS,
        1,
        ON_SUBMIT,
        1
    ]);
    let records = decode(wire.as_array().unwrap()).unwrap();
    let display = Renderer::default().layout(&records, 240, 100, 1.0).unwrap();
    assert_eq!(display.nodes[0].tag, "search");
    assert_eq!(display.nodes[0].input.as_ref().unwrap().keyboard, "search");
    assert_eq!(display.nodes[0].bounds[3], 48.0);
    for wire in [
        json!([SEARCH, -1, "", "", "", 1, SECURE, 0]),
        json!([SEARCH, -1, "", "", "", 1, KEYBOARD, "text"]),
        json!([SEARCH, -1, "", "", "", 0, TEXT, 0, "", "", "child", 0]),
    ] {
        assert!(decode(wire.as_array().unwrap()).is_err());
    }
    let wire = json!([SEARCH, -1, "", "", "", 0]);
    let display = Renderer::default()
        .layout(&decode(wire.as_array().unwrap()).unwrap(), 240, 100, 1.0)
        .unwrap();
    assert_eq!(display.nodes[0].input.as_ref().unwrap().keyboard, "search");
}
