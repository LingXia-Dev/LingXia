use lingxia_ui::{
    Renderer,
    schema::{color, decode},
};
use serde_json::{Value, json};

fn node(wire: &mut Vec<Value>, tag: u32, parent: i64, text: &str, fields: Value) {
    let fields = fields.as_array().unwrap();
    wire.extend(
        json!([tag, parent, "", "", text, fields.len() / 2])
            .as_array()
            .unwrap()
            .clone(),
    );
    wire.extend(fields.iter().cloned());
}

#[test]
fn colors_accept_only_rgb_rgba_or_known_tokens() {
    assert_eq!(color("#1234AB").unwrap(), [0x12, 0x34, 0xab, 255]);
    assert_eq!(color("#1234ab80").unwrap(), [0x12, 0x34, 0xab, 128]);
    assert_eq!(color("#ffffff00").unwrap(), [255, 255, 255, 0]);
    assert_eq!(color("surface").unwrap(), [255, 255, 255, 255]);
    for invalid in [
        "#abc",
        "#abcd",
        "#1234567",
        "#123456789",
        "#+00000",
        "#+0000000",
        "#000000gg",
        "#１２３",
        "123456",
        "transparent",
        "unknown",
    ] {
        assert!(color(invalid).is_err(), "{invalid}");
        for field in [5, 6] {
            let wire = json!([2, -1, "", "", "label", 1, field, invalid]);
            assert!(decode(wire.as_array().unwrap()).is_err(), "{invalid}");
        }
    }
}

#[test]
fn translucent_stack_layers_composite_in_order_without_premultiplying_twice() {
    let mut wire = vec![];
    node(&mut wire, 17, -1, "", json!([0, 64, 1, 64, 6, "#204060"]));
    node(&mut wire, 0, 0, "", json!([0, 32, 1, 64, 6, "#00000080"]));
    node(
        &mut wire,
        0,
        0,
        "",
        json!([0, 32, 1, 64, 6, "#ff000000", 48, 32]),
    );
    node(
        &mut wire,
        0,
        0,
        "",
        json!([0, 32, 1, 32, 6, "#ffffff80", 49, 32]),
    );
    let frame = Renderer::default()
        .layout(&decode(&wire).unwrap(), 64, 64, 1.0)
        .unwrap()
        .rasterize()
        .unwrap();
    let pixels = frame.pixels.data_as_u8_slice();
    for (x, y, expected) in [
        (16, 16, [16u8, 32, 48, 255]),
        (48, 16, [32, 64, 96, 255]),
        (16, 48, [136, 144, 152, 255]),
    ] {
        let offset = (y * 64 + x) * 4;
        for channel in 0..4 {
            assert!(
                pixels[offset + channel].abs_diff(expected[channel]) <= 1,
                "pixel ({x},{y}) is {:?}",
                &pixels[offset..offset + 4]
            );
        }
    }
}

#[test]
fn transparent_text_and_search_strokes_keep_geometry_without_visible_ink() {
    let render = |tint: &str| {
        let mut wire = vec![];
        node(&mut wire, 0, -1, "", json!([6, "#ffffff"]));
        node(&mut wire, 2, 0, "Alpha text", json!([4, 24, 5, tint]));
        node(
            &mut wire,
            14,
            0,
            "",
            json!([0, 160, 1, 48, 5, tint, 6, "#ffffff"]),
        );
        Renderer::default()
            .layout(&decode(&wire).unwrap(), 220, 100, 1.0)
            .unwrap()
    };
    let opaque = render("#000000");
    let transparent = render("#00000000");
    assert_eq!(opaque.nodes[1].bounds, transparent.nodes[1].bounds);
    assert_eq!(opaque.nodes[2].bounds, transparent.nodes[2].bounds);
    assert!(
        opaque
            .rasterize()
            .unwrap()
            .pixels
            .data_as_u8_slice()
            .chunks_exact(4)
            .any(|pixel| pixel[0] < 200)
    );
    let transparent = transparent.rasterize().unwrap();
    let ink = transparent
        .pixels
        .data_as_u8_slice()
        .chunks_exact(4)
        .enumerate()
        .find(|(_, pixel)| pixel.iter().any(|channel| *channel < 254));
    assert!(
        ink.is_none(),
        "unexpected ink: {ink:?}; bounds: {:?}",
        transparent
            .nodes
            .iter()
            .map(|node| node.bounds)
            .collect::<Vec<_>>()
    );
}
