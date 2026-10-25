use crate::protocol::*;
use serde_json::json;

fn list(source: Value, count: usize) -> Vec<Value> {
    json!([
        6, -1, "", "", "", 5, 11, "root", 12, count, 13, 0, 14, 0, 102, source
    ])
    .as_array()
    .unwrap()
    .clone()
}

#[test]
fn item_keys_validate_complete_roster_and_realized_row_identity() {
    assert_eq!(FIELDS[102], ("itemKeys", "list-keys"));
    let mut buffer = list(json!("[\"alpha\",\"图🙂\"]"), 2);
    buffer.extend(
        json!([0, 0, "key:\"图🙂\"", "", "", 1, 15, 1])
            .as_array()
            .unwrap()
            .clone(),
    );
    let records = decode(&buffer).unwrap();
    assert_eq!(records[1].number(field::LIST_ROW_INDEX), Some(1.0));
    buffer[18] = json!("key:\"wrong\"");
    assert!(decode(&buffer).err().unwrap().contains("does not match"));
    for (source, count) in [
        (json!(null), 0),
        (json!("null"), 0),
        (json!("[1]"), 1),
        (json!("[\"a\",\"a\"]"), 2),
        (json!("[\"a\"]"), 2),
        (json!(format!("[\"{}\"]", "图".repeat(100))), 1),
        (json!(r#"["\ud800"]"#), 1),
    ] {
        assert!(decode(&list(source, count)).is_err());
    }
    assert!(decode(json!([0, -1, "", "", "", 1, 102, "[]"]).as_array().unwrap()).is_err());
}

#[test]
fn item_key_page_budget_includes_all_lists_not_only_visible_rows() {
    let keys: Vec<String> = (0..1200).map(|i| format!("{i:0200}")).collect();
    let source = serde_json::to_string(&keys).unwrap();
    assert!(decode(&list(json!(source), keys.len())).is_ok());
    let mut buffer = json!([0, -1, "", "", "", 0]).as_array().unwrap().clone();
    for id in ["one", "two"] {
        buffer.extend(
            json!([
                6,
                0,
                id,
                "",
                "",
                5,
                11,
                id,
                12,
                keys.len(),
                13,
                0,
                14,
                0,
                102,
                source
            ])
            .as_array()
            .unwrap()
            .clone(),
        );
    }
    assert!(decode(&buffer).err().unwrap().contains("page budget"));
}
