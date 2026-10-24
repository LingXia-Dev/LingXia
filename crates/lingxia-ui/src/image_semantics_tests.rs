use crate::{ViewSession, schema, session::tests::source};
use serde_json::{Value, json};

#[test]
fn alternative_text_is_image_only_bounded_and_preserves_legacy_records() {
    assert_eq!(schema::FIELDS[105], ("alt", "image-alt"));
    for alt in [
        json!(""),
        json!("A mountain lake"),
        json!("图片".repeat(170)),
    ] {
        let values = json!([7, -1, "", "", "", 2, 19, "photo.png", 105, alt]);
        assert!(schema::decode(values.as_array().unwrap()).is_ok());
    }
    for (tag, alt) in [
        (7, json!(1)),
        (7, Value::Null),
        (7, json!("  ")),
        (7, json!("\0")),
        (7, json!("图片".repeat(171))),
        (2, json!("Wrong tag")),
    ] {
        let values = json!([tag, -1, "", "", "", 1, 105, alt]);
        assert!(
            schema::decode(values.as_array().unwrap())
                .unwrap_err()
                .contains("UI_IMAGE")
        );
    }
}

#[test]
fn image_descriptions_and_decoration_share_layout_and_pixels() {
    let mut baseline: Option<crate::Frame> = None;
    for (props, label, hidden) in [
        ("", None, None),
        (",alt:'A mountain lake'", Some("A mountain lake"), None),
        (",alt:''", None, Some(true)),
    ] {
        let module = source(&format!(
            "function render(){{return h('image',{{src:'photo.png',testId:'photo',style:{{width:64,height:48,background:'#4488cc'}}{props}}})}}"
        ));
        let mut page = ViewSession::new(&module).unwrap();
        let frame = page.render(160, 120, 1.0).unwrap();
        assert_eq!(frame.nodes[0].accessibility_label.as_deref(), label);
        assert_eq!(frame.nodes[0].accessibility_hidden, hidden);
        if let Some(previous) = &baseline {
            assert_eq!(frame.nodes[0].bounds, previous.nodes[0].bounds);
            assert_eq!(frame.pixels.data(), previous.pixels.data());
        } else {
            baseline = Some(frame);
        }
    }
}

#[test]
fn composed_buttons_own_the_accessible_label_of_their_images() {
    let module = source(
        "function render(){return h('button',{accessibilityLabel:'Open photo'},h('image',{src:'photo.png',alt:'A mountain lake'}))}",
    );
    let mut page = ViewSession::new(&module).unwrap();
    let frame = page.layout(160, 120, 1.0).unwrap();
    assert_eq!(
        frame.nodes[0].accessibility_label.as_deref(),
        Some("Open photo")
    );
    assert_eq!(frame.nodes[1].accessibility_hidden, Some(true));
}
