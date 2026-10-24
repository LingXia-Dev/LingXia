use lingxia_ui::{Renderer, schema::decode};
use serde_json::{Value, json};

fn wire(text: &str, lines: Option<u32>) -> Vec<Value> {
    let mut fields = vec![
        json!(4),
        json!(16),
        json!(37),
        json!(24),
        json!(3),
        json!(4),
    ];
    if let Some(lines) = lines {
        fields.extend([json!(50), json!(lines)]);
    }
    let mut wire = json!([
        0,
        -1,
        "",
        "root",
        "",
        0,
        2,
        0,
        "summary",
        "summary",
        text,
        fields.len() / 2
    ])
    .as_array()
    .unwrap()
    .clone();
    wire.extend(fields);
    wire.extend(
        json!([0, 0, "after", "after", "", 1, 1, 10])
            .as_array()
            .unwrap()
            .clone(),
    );
    wire
}

#[test]
fn line_cap_limits_geometry_and_glyphs_but_keeps_the_complete_semantic_text() {
    let text = "First\nSecond\nThird";
    let mut renderer = Renderer::default();
    for (limit, prefix, height) in [(1, "First", 32.0), (2, "First\nSecond", 56.0)] {
        let frame = renderer
            .layout(&decode(&wire(text, Some(limit))).unwrap(), 300, 180, 1.0)
            .unwrap();
        assert_eq!(frame.nodes[1].bounds[3], height);
        assert_eq!(frame.nodes[2].bounds[1], height);
        assert_eq!(frame.nodes[1].text, text);
        let expected = renderer
            .layout(&decode(&wire(prefix, None)).unwrap(), 300, 180, 1.0)
            .unwrap();
        assert_eq!(
            frame.rasterize().unwrap().pixels.data_as_u8_slice(),
            expected.rasterize().unwrap().pixels.data_as_u8_slice()
        );
    }
    let full = renderer
        .layout(&decode(&wire(text, None)).unwrap(), 300, 180, 1.0)
        .unwrap();
    assert_eq!(full.nodes[1].bounds[3], 80.0);
}

#[test]
fn line_cap_reflows_wrapped_unicode_at_each_width_scale_and_reuses_shaping_safely() {
    let text = "English words \u{4e2d}\u{6587} \u{1f600} \u{5e9}\u{5dc}\u{5d5}\u{5dd} ".repeat(10);
    let mut renderer = Renderer::default();
    for scale in [1.0, 2.0] {
        for width in [100, 220] {
            let clipped = renderer
                .layout(&decode(&wire(&text, Some(2))).unwrap(), width, 800, scale)
                .unwrap();
            let all = renderer
                .layout(&decode(&wire(&text, None)).unwrap(), width, 800, scale)
                .unwrap();
            assert_eq!(clipped.nodes[1].bounds[3], 56.0);
            assert!(all.nodes[1].bounds[3] > 56.0);
            assert_eq!(clipped.nodes[1].identity, all.nodes[1].identity);
            assert_eq!(clipped.nodes[1].text, text);
        }
    }
}

#[test]
fn line_limit_requires_a_bounded_integer_and_only_belongs_to_text() {
    for value in [
        json!(0),
        json!(-1),
        json!(1.5),
        json!(1025),
        json!("2"),
        json!(true),
        json!(null),
    ] {
        assert!(
            decode(
                json!([2, -1, "", "", "Text", 1, 50, value])
                    .as_array()
                    .unwrap()
            )
            .is_err()
        );
    }
    for tag in [0, 3, 8, 16, 17] {
        assert!(decode(json!([tag, -1, "", "", "", 1, 50, 2]).as_array().unwrap()).is_err());
    }
    for lines in [1, 1024] {
        assert!(
            decode(
                json!([2, -1, "", "", "Text", 1, 50, lines])
                    .as_array()
                    .unwrap()
            )
            .is_ok()
        );
    }
}

#[test]
fn hidden_lines_do_not_inflate_intrinsic_width() {
    let mut renderer = Renderer::default();
    let mut limited = wire("x\nAn exceptionally long hidden second line", Some(1));
    limited[0] = json!(1);
    let mut prefix = wire("x", None);
    prefix[0] = json!(1);
    let frame = renderer
        .layout(&decode(&limited).unwrap(), 800, 200, 1.0)
        .unwrap();
    let reference = renderer
        .layout(&decode(&prefix).unwrap(), 800, 200, 1.0)
        .unwrap();
    assert_eq!(frame.nodes[1].bounds, reference.nodes[1].bounds);
    assert!(frame.nodes[1].bounds[2] < 50.0);
}
