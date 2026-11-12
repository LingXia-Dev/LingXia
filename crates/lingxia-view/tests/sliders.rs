use lingxia_view::protocol::field::{
    ACCESSIBILITY_LABEL, ARIA_VALUE_TEXT, BACKGROUND, CHECKED, COLOR, ENABLED, HEIGHT,
    ON_CHECKED_CHANGE, ON_CLICK, ON_SLIDER_CHANGE, PADDING_LEFT, PADDING_RIGHT, PRESSED_STYLE,
    SECONDARY_STYLE, SLIDER_MAX, SLIDER_MIN, SLIDER_SECONDARY, SLIDER_STEP, SLIDER_VALUE,
    THUMB_STYLE, TRACK_STYLE, WIDTH,
};
use lingxia_view::protocol::tag::{SLIDER, SWITCH, TEXT};
use lingxia_view::{Renderer, protocol::decode};
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
        assert_eq!(range.inset, [24.0; 2]);
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

#[test]
fn slider_track_follows_each_side_padding_and_the_thumb_radius() {
    let wire = json!([
        SLIDER,
        -1,
        "",
        "volume",
        "",
        5,
        ACCESSIBILITY_LABEL,
        "Volume",
        SLIDER_VALUE,
        100,
        PADDING_LEFT,
        0,
        PADDING_RIGHT,
        40,
        COLOR,
        "#ff0000"
    ]);
    let frame = Renderer::default()
        .layout(&decode(wire.as_array().unwrap()).unwrap(), 200, 100, 1.0)
        .unwrap();
    let range = frame.nodes[0].range.as_ref().unwrap();
    assert_eq!(range.inset, [12.0, 52.0]);
    // At max the thumb's center sits at 200 - 52; its right edge, 12 further, is the last red column.
    let pixels = frame.rasterize().unwrap().pixels;
    let red = |x: usize| pixels.data_as_u8_slice()[(24 * 200 + x) * 4..][..4] == [255, 0, 0, 255];
    assert!(red(148) && red(158) && !red(162));
}

fn pixels(frame: lingxia_view::DisplayList) -> impl Fn(usize, usize) -> [u8; 4] {
    let pixels = frame.rasterize().unwrap().pixels;
    move |x, y| {
        let at = (y * 200 + x) * 4;
        pixels.data_as_u8_slice()[at..at + 4].try_into().unwrap()
    }
}

#[test]
fn slider_parts_restyle_track_secondary_and_thumb_and_the_widest_thumb_sets_the_ends() {
    let part = |pairs: serde_json::Value| pairs.to_string();
    let wire = json!([
        SLIDER,
        -1,
        "",
        "seek",
        "",
        8,
        ACCESSIBILITY_LABEL,
        "Seek",
        SLIDER_VALUE,
        0,
        SLIDER_SECONDARY,
        50,
        ARIA_VALUE_TEXT,
        "0:00 of 1:00",
        TRACK_STYLE,
        part(json!([HEIGHT, 8, BACKGROUND, "#0000ff"])),
        SECONDARY_STYLE,
        part(json!([BACKGROUND, "#00ff00"])),
        THUMB_STYLE,
        part(json!([WIDTH, 16, HEIGHT, 16, BACKGROUND, "#ff0000"])),
        PRESSED_STYLE,
        json!([
            THUMB_STYLE,
            part(json!([WIDTH, 32, HEIGHT, 32, BACKGROUND, "#ff0000"]))
        ])
        .to_string()
    ]);
    let frame = Renderer::default()
        .layout(&decode(wire.as_array().unwrap()).unwrap(), 200, 100, 1.0)
        .unwrap();
    let range = frame.nodes[0].range.as_ref().unwrap();
    // Padding 12 plus half the pressed thumb, so pressing does not move the ends.
    assert_eq!(range.inset, [28.0; 2]);
    assert_eq!(range.secondary, Some(50.0));
    assert_eq!(range.text.as_deref(), Some("0:00 of 1:00"));
    let pixel = pixels(frame);
    // The thumb sits at the start; the secondary line covers the first half; the track the rest.
    assert_eq!(pixel(28, 24), [255, 0, 0, 255]);
    assert_eq!(pixel(60, 24), [0, 255, 0, 255]);
    assert_eq!(pixel(150, 24), [0, 0, 255, 255]);
    assert_ne!(pixel(150, 29), [0, 0, 255, 255]);
    assert_eq!(pixel(150, 27), [0, 0, 255, 255]);
}

#[test]
fn slider_parts_refuse_fields_a_part_cannot_draw() {
    for pairs in [
        json!([PADDING_LEFT, 4]),
        json!([WIDTH, "50%"]),
        json!([HEIGHT, 4000]),
    ] {
        let wire = json!([
            SLIDER,
            -1,
            "",
            "",
            "",
            2,
            ACCESSIBILITY_LABEL,
            "Volume",
            THUMB_STYLE,
            pairs.to_string()
        ]);
        assert!(decode(wire.as_array().unwrap()).is_err(), "{pairs}");
    }
}

#[test]
fn switch_parts_size_the_track_and_seat_the_thumb_evenly_at_either_end() {
    let part = |pairs: serde_json::Value| pairs.to_string();
    let frame = |checked: u8| {
        let wire = json!([
            SWITCH,
            -1,
            "",
            "wifi",
            "",
            5,
            ACCESSIBILITY_LABEL,
            "Wi-Fi",
            CHECKED,
            checked,
            ON_CHECKED_CHANGE,
            1,
            TRACK_STYLE,
            part(json!([WIDTH, 44, HEIGHT, 24, BACKGROUND, "#0000ff"])),
            THUMB_STYLE,
            part(json!([WIDTH, 20, HEIGHT, 20, BACKGROUND, "#ff0000"]))
        ]);
        let frame = Renderer::default()
            .layout(&decode(wire.as_array().unwrap()).unwrap(), 200, 100, 1.0)
            .unwrap();
        let [x, y, w, h] = frame.nodes[0].bounds;
        let pixel = pixels(frame);
        let (right, middle) = ((x + w) as usize - 12, (y + h / 2.0) as usize);
        move |from_right: usize| pixel(right - from_right, middle)
    };
    // The 44-wide track ends at the padding; the 20 thumb rests 2 inside it.
    let on = frame(1);
    assert_eq!(on(12), [255, 0, 0, 255]);
    assert_eq!(on(38), [0, 0, 255, 255]);
    assert_ne!(on(47), [0, 0, 255, 255]);
    let off = frame(0);
    assert_eq!(off(32), [255, 0, 0, 255]);
    assert_eq!(off(8), [0, 0, 255, 255]);
}

#[test]
fn a_disabled_switch_that_is_on_keeps_its_accent_faintly() {
    let wire = json!([
        SWITCH,
        -1,
        "",
        "",
        "",
        3,
        ACCESSIBILITY_LABEL,
        "Wi-Fi",
        CHECKED,
        1,
        ENABLED,
        0
    ]);
    let frame = Renderer::default()
        .layout(&decode(wire.as_array().unwrap()).unwrap(), 200, 100, 1.0)
        .unwrap();
    let [x, y, w, h] = frame.nodes[0].bounds;
    let at = ((x + w) as usize - 12 - 40, (y + h / 2.0) as usize);
    let pixel = pixels(frame)(at.0, at.1);
    let [r, _, b, _] = pixel.map(i32::from);
    assert!(b > r + 50, "a faded accent, not grey: {pixel:?}");
}
