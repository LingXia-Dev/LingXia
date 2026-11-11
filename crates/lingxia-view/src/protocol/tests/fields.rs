use crate::protocol::{Fields, decode_wire, field, tag};
use serde_json::{Value, json};

fn wire(values: Value) -> Vec<Value> {
    values.as_array().unwrap().clone()
}

/// A column holding one coloured text, numbered `serial`.
#[rustfmt::skip]
fn text(serial: u64) -> Vec<Value> {
    wire(json!([
        tag::COLUMN, -1, "", "", "", 0,
        tag::TEXT, 0, "", "", "hi", 1, field::COLOR, "#123456",
        -3, serial, 0, 0, 0, 0
    ]))
}

#[test]
fn reused_subtrees_share_fields_with_the_buffer_they_reference() {
    let (first, held) = decode_wire(&text(1), None).unwrap();
    // The text goes as a reference to its nodes in buffer 1.
    #[rustfmt::skip]
    let buffer = wire(json!([
        tag::COLUMN, -1, "", "", "", 0,
        -2, 0, 1, 6, 1, 0,
        -3, 2, 0, 0, 0, 0
    ]));
    let (second, _) = decode_wire(&buffer, Some(&held)).unwrap();
    assert_eq!(second.len(), 2);
    assert_eq!(second[1].parent, Some(0));
    assert!(Fields::ptr_eq(&first[1].fields, &second[1].fields));
    assert!(!Fields::ptr_eq(&first[0].fields, &second[0].fields));
}

#[test]
fn editing_shared_fields_copies_only_the_edited_record() {
    let (decoded, _) = decode_wire(&text(1), None).unwrap();
    let mut edited = decoded.to_vec();
    edited[1].fields.push((field::BACKGROUND, json!("#ffffff")));
    assert_eq!(decoded[1].fields.len(), 1);
    assert_eq!(edited[1].fields.len(), 2);
    assert!(!Fields::ptr_eq(&decoded[1].fields, &edited[1].fields));
    assert!(Fields::ptr_eq(&decoded[0].fields, &edited[0].fields));
    for (_, value) in &mut edited[1].fields {
        *value = json!(null);
    }
    assert_eq!(decoded[1].fields[0].1, json!("#123456"));
}

#[test]
fn fields_print_and_convert_like_the_vec_they_hold() {
    let pairs = vec![(field::COLOR, json!("#123456")), (field::WIDTH, json!(4))];
    let fields = Fields::from(pairs.clone());
    assert_eq!(format!("{fields:?}"), format!("{pairs:?}"));
    assert_eq!(*fields, pairs);
    let shared = fields.clone();
    assert_eq!(shared.into_vec(), pairs);
    assert_eq!(fields.into_iter().collect::<Vec<_>>(), pairs);
}
