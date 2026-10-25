use crate::protocol::*;
use serde_json::json;

fn payload() -> String {
    let fixtures: Value = serde_json::from_str(include_str!(
        "../../../../../packages/lingxia-ui/tests/fixtures/animations.json"
    ))
    .unwrap();
    fixtures["blink"].to_string()
}

#[test]
fn animation_field_is_append_only_validated_and_not_silently_rendered() {
    assert_eq!(FIELDS[100].0, "scrollAxis");
    assert_eq!(FIELDS[101], ("animation", "animation"));
    let source = payload();
    let records = decode(
        json!([0, -1, "", "", "", 1, 101, source])
            .as_array()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(records[0].string(field::ANIMATION), Some(source.as_str()));
    assert!(
        crate::Renderer::default()
            .layout(&records, 80, 80, 1.0)
            .err()
            .unwrap()
            .contains("requires animation presentation scheduling")
    );
    for value in [
        Value::Null,
        json!(0),
        json!("null"),
        json!("{}"),
        json!(source.replace("step-end", "step-start")),
    ] {
        assert!(
            decode(
                json!([0, -1, "", "", "", 1, 101, value])
                    .as_array()
                    .unwrap()
            )
            .is_err()
        );
    }
}

#[test]
fn aggregate_animation_payload_is_bounded_before_layout() {
    let source = format!("{}{}", " ".repeat(32000 - payload().len()), payload());
    let mut buffer = json!([0, -1, "", "", "", 0]).as_array().unwrap().clone();
    for index in 0..9 {
        buffer.extend(
            json!([0, 0, index.to_string(), "", "", 1, 101, source])
                .as_array()
                .unwrap()
                .clone(),
        );
    }
    let error = decode(&buffer).err().unwrap();
    assert!(error.contains("page payload budget"), "{error}");
}
