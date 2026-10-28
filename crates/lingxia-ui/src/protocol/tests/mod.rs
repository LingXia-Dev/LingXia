mod animation;
mod environment;
mod errors;
pub(crate) mod frame_contract;
mod list_keys;

use super::*;
use serde_json::json;
#[test]
fn letter_spacing_accepts_signed_lengths_and_rejects_invalid_wire_values() {
    for value in [json!(-1024), json!(0), json!(1.5), json!(1024)] {
        assert!(
            decode(
                json!([2, -1, "", "", "Title", 1, 89, value])
                    .as_array()
                    .unwrap()
            )
            .is_ok()
        );
    }
    for value in [json!(-1025), json!(1025), json!("1px"), json!(null)] {
        assert!(
            decode(
                json!([2, -1, "", "", "Title", 1, 89, value])
                    .as_array()
                    .unwrap()
            )
            .is_err()
        );
    }
}
#[test]
fn flex_basis_wire_is_bounded_and_typed() {
    for value in [
        json!("auto"),
        json!(0),
        json!(16384),
        json!(".5%"),
        json!("100%"),
    ] {
        assert!(decode(json!([0, -1, "", "", "", 1, 99, value]).as_array().unwrap()).is_ok());
    }
    for value in [
        json!(-1),
        json!(16385),
        json!("-1%"),
        json!("1e2%"),
        json!(".%."),
        json!("auto "),
        json!("10px"),
        json!(null),
    ] {
        assert!(decode(json!([0, -1, "", "", "", 1, 99, value]).as_array().unwrap()).is_err());
    }
}

#[test]
fn rejects_malformed_candidates() {
    for input in [
        json!([]),
        json!([0]),
        json!([99, -1, "", "", "", 0]),
        json!([0, 0, "", "", "", 0]),
        json!([0, -1, "", "", "", 1, FIELDS.len(), 0]),
        json!([0, -1, "", "", "", 1, 0, -1]),
    ] {
        assert!(decode(input.as_array().unwrap()).is_err());
    }
}
#[test]
fn parent_offsets_and_keys_are_validated() {
    let valid = json!([0, -1, "", "", "", 0, 2, 0, "a", "greeting", "Hello", 0]);
    let records = decode(valid.as_array().unwrap()).unwrap();
    assert_eq!(records[1].parent, Some(0));
    let duplicate = json!([
        0, -1, "", "", "", 0, 2, 0, "a", "", "one", 0, 2, 0, "a", "", "two", 0
    ]);
    assert!(decode(duplicate.as_array().unwrap()).is_err());
}
#[test]
fn overflow_wrap_options_are_validated_on_the_wire() {
    for value in [json!("normal"), json!("break-word"), json!("anywhere")] {
        let buffer = json!([2, -1, "", "", "address", 1, 64, value]);
        assert!(decode(buffer.as_array().unwrap()).is_ok());
    }
    for value in [json!("break-all"), json!(0), json!(null)] {
        let buffer = json!([2, -1, "", "", "address", 1, 64, value]);
        assert!(decode(buffer.as_array().unwrap()).is_err());
    }
}

#[test]
fn flex_wrap_options_are_validated_on_the_wire() {
    for value in [json!("nowrap"), json!("wrap"), json!("wrap-reverse")] {
        assert!(decode(json!([1, -1, "", "", "", 1, 87, value]).as_array().unwrap()).is_ok());
    }
    for value in [json!("normal"), json!(0), json!(null)] {
        assert!(decode(json!([1, -1, "", "", "", 1, 87, value]).as_array().unwrap()).is_err());
    }
}

#[test]
fn word_break_options_are_validated_on_the_wire() {
    for value in [json!("normal"), json!("break-all"), json!("keep-all")] {
        let buffer = json!([2, -1, "", "", "address", 1, 63, value]);
        assert!(decode(buffer.as_array().unwrap()).is_ok());
    }
    for value in [
        json!("break-word"),
        json!("anywhere"),
        json!(0),
        json!(null),
    ] {
        let buffer = json!([2, -1, "", "", "address", 1, 63, value]);
        assert!(decode(buffer.as_array().unwrap()).is_err());
    }
}

#[test]
fn composed_buttons_validate_names_and_descendants_on_the_wire() {
    let valid = json!([
        3,
        -1,
        "",
        "button",
        "",
        2,
        41,
        "Open inbox",
        8,
        1,
        0,
        0,
        "",
        "",
        "",
        0,
        2,
        10,
        "",
        "title",
        "Inbox",
        0
    ]);
    assert_eq!(decode(valid.as_array().unwrap()).unwrap().len(), 3);
    let unlabeled = json!([
        3, -1, "", "button", "", 1, 8, 1, 0, 0, "", "", "", 0, 2, 8, "", "", "Inbox", 0
    ]);
    assert_eq!(decode(unlabeled.as_array().unwrap()).unwrap().len(), 3);
    // Controls may nest (each keeps its own target); a scroll or list may not.
    for tag in [5, 6] {
        let mut invalid = valid.clone();
        invalid[16] = json!(tag);
        assert!(
            decode(invalid.as_array().unwrap()).is_err(),
            "nested tag {tag}"
        );
    }
    for label in [json!(""), json!(" "), json!(null)] {
        let mut invalid = valid.clone();
        invalid[7] = label;
        assert!(decode(invalid.as_array().unwrap()).is_err());
    }
    let mut mixed = valid;
    mixed[4] = json!("also text");
    assert!(decode(mixed.as_array().unwrap()).is_err());
}

#[test]
fn composed_checkboxes_validate_names_and_descendants_on_the_wire() {
    let valid = json!([
        10,
        -1,
        "",
        "button",
        "",
        2,
        41,
        "Open inbox",
        40,
        1,
        0,
        0,
        "",
        "",
        "",
        0,
        2,
        10,
        "",
        "title",
        "Inbox",
        0
    ]);
    assert_eq!(decode(valid.as_array().unwrap()).unwrap().len(), 3);
    // Controls may nest (each keeps its own target); a scroll or list may not.
    for tag in [5, 6] {
        let mut invalid = valid.clone();
        invalid[16] = json!(tag);
        assert!(
            decode(invalid.as_array().unwrap()).is_err(),
            "nested tag {tag}"
        );
    }
    for label in [json!(""), json!(" "), json!(null)] {
        let mut invalid = valid.clone();
        invalid[7] = label;
        assert!(decode(invalid.as_array().unwrap()).is_err());
    }
    let mut mixed = valid;
    mixed[4] = json!("also text");
    assert!(decode(mixed.as_array().unwrap()).is_err());
}

#[test]
fn scroll_axis_is_single_axis_and_requires_a_nonvirtual_scroll() {
    for axis in ["horizontal", "vertical"] {
        assert!(decode(json!([5, -1, "", "", "", 1, 100, axis]).as_array().unwrap()).is_ok());
    }
    for tag in [0, 1, 6, 8] {
        assert!(
            decode(
                json!([tag, -1, "", "", "", 1, 100, "horizontal"])
                    .as_array()
                    .unwrap()
            )
            .is_err()
        );
    }
    for axis in [json!("both"), json!(0), json!(null)] {
        assert!(decode(json!([5, -1, "", "", "", 1, 100, axis]).as_array().unwrap()).is_err());
    }
}

#[test]
fn scroll_accepts_children_but_spacer_remains_a_leaf() {
    let mut buffer = json!([5, -1, "", "", "", 0, 3, 0, "button", "", "Hello", 0]);
    assert_eq!(
        decode(buffer.as_array().unwrap()).unwrap()[1].parent,
        Some(0)
    );
    buffer[0] = json!(4);
    assert!(decode(buffer.as_array().unwrap()).is_err());
}
#[test]
fn layout_style_values_are_validated_on_the_wire() {
    for id in 28..=32 {
        for value in [json!(-1), json!(16385), json!("100"), json!(null)] {
            assert!(decode(json!([0, -1, "", "", "", 1, id, value]).as_array().unwrap()).is_err());
        }
        assert!(decode(json!([0, -1, "", "", "", 1, id, 0]).as_array().unwrap()).is_ok());
    }
    for (id, valid, invalid) in [
        (33, "stretch", "between"),
        (34, "between", "stretch"),
        (35, "end", "auto"),
    ] {
        assert!(decode(json!([0, -1, "", "", "", 1, id, valid]).as_array().unwrap()).is_ok());
        assert!(
            decode(
                json!([0, -1, "", "", "", 1, id, invalid])
                    .as_array()
                    .unwrap()
            )
            .is_err()
        );
        assert!(decode(json!([0, -1, "", "", "", 1, id, 1]).as_array().unwrap()).is_err());
    }
    for (id, value) in [(33, "center"), (34, "between")] {
        assert!(
            decode(json!([6, -1, "", "", "", 1, id, value]).as_array().unwrap())
                .unwrap_err()
                .contains("UI_LIST: rows require")
        );
    }
}

#[test]
fn typography_rejects_unbounded_numbers_and_unrecognized_families() {
    for (id, value) in [
        (36, json!(0)),
        (36, json!(1001)),
        (36, json!("bold")),
        (37, json!(0)),
        (37, json!(-1)),
        (37, json!(16385)),
        (38, json!("Arial")),
        (38, json!(1)),
    ] {
        assert!(
            decode(
                json!([2, -1, "", "", "Hello", 1, id, value])
                    .as_array()
                    .unwrap()
            )
            .is_err()
        );
    }
    for (id, value) in [
        (36, json!(1)),
        (36, json!(1000)),
        (37, json!(32)),
        (38, json!("sans")),
        (38, json!("mono")),
    ] {
        assert!(
            decode(
                json!([2, -1, "", "", "Hello", 1, id, value])
                    .as_array()
                    .unwrap()
            )
            .is_ok()
        );
    }
}

#[test]
fn backdrop_rejects_out_of_range_and_non_numeric_wire_values() {
    for (id, max) in [(95, 64), (96, 16)] {
        for value in [json!(0), json!(1.5), json!(max)] {
            assert!(decode(json!([0, -1, "", "", "", 1, id, value]).as_array().unwrap()).is_ok());
        }
        for value in [
            json!(-1),
            json!(max + 1),
            json!("4"),
            json!(true),
            Value::Null,
        ] {
            assert!(decode(json!([0, -1, "", "", "", 1, id, value]).as_array().unwrap()).is_err());
        }
    }
}

#[test]
fn focus_style_rejects_recursive_geometry_and_invalid_paints() {
    assert!(focus_style(&json!("[6,\"#ffffff\",58,0.5]")).is_ok());
    for value in [
        json!("[98,\"[]\"]"),
        json!("[0,20]"),
        json!("[58,2]"),
        json!("[6,\"bad\"]"),
        json!("[6,\"#fff\",6,\"#000\"]"),
        json!("[6]"),
        json!([]),
    ] {
        assert!(focus_style(&value).is_err(), "{value}");
    }
    assert!(
        decode(
            json!([3, -1, "", "", "", 1, 98, "[6,\"#fff\"]"])
                .as_array()
                .unwrap()
        )
        .is_err()
    );
}

#[test]
fn placeholder_color_validates_wire_colors() {
    for value in [json!("#12345680"), json!("text-muted/60")] {
        assert!(
            decode(
                json!([8, -1, "", "", "", 2, 97, value, 21, ""])
                    .as_array()
                    .unwrap()
            )
            .is_ok()
        );
    }
    for value in [json!(4), json!("invalid-color"), json!(true), Value::Null] {
        assert!(
            decode(
                json!([8, -1, "", "", "", 2, 97, value, 21, ""])
                    .as_array()
                    .unwrap()
            )
            .is_err()
        );
    }
}

#[test]
fn opacity_rejects_out_of_range_and_non_numeric_wire_values() {
    for value in [json!(0), json!(0.5), json!(1)] {
        assert!(decode(json!([0, -1, "", "", "", 1, 58, value]).as_array().unwrap()).is_ok());
    }
    for value in [
        json!(-0.01),
        json!(1.01),
        json!("0.5"),
        json!(true),
        Value::Null,
    ] {
        assert!(decode(json!([0, -1, "", "", "", 1, 58, value]).as_array().unwrap()).is_err());
    }
}

#[test]
fn padding_and_border_edges_are_bounded_nonnegative_lengths() {
    for field in [59, 60, 61, 62, 65, 67, 68, 69, 70] {
        for value in [json!(0), json!(0.5), json!(16384)] {
            assert!(
                decode(
                    json!([0, -1, "", "", "", 1, field, value])
                        .as_array()
                        .unwrap()
                )
                .is_ok()
            );
        }
        for value in [
            json!(-1),
            json!(16385),
            json!("12"),
            json!(true),
            Value::Null,
        ] {
            assert!(
                decode(
                    json!([0, -1, "", "", "", 1, field, value])
                        .as_array()
                        .unwrap()
                )
                .is_err()
            );
        }
    }
}

#[test]
fn current_color_is_only_accepted_for_borders_and_backgrounds() {
    assert!(
        decode(
            json!([0, -1, "", "", "", 1, 5, "currentColor"])
                .as_array()
                .unwrap()
        )
        .is_err()
    );
    for field in [6, 66] {
        for value in ["currentColor", "#123456", "#12345680"] {
            assert!(
                decode(
                    json!([0, -1, "", "", "", 1, field, value])
                        .as_array()
                        .unwrap()
                )
                .is_ok()
            );
        }
    }
    for value in [json!("invalid"), json!(12), Value::Null] {
        assert!(decode(json!([0, -1, "", "", "", 1, 66, value]).as_array().unwrap()).is_err());
    }
}

#[test]
fn submit_on_enter_requires_a_textarea_boolean() {
    for value in [0, 1] {
        assert!(decode(json!([9, -1, "", "", "", 1, 83, value]).as_array().unwrap()).is_ok());
    }
    for tag in [0, 8, 14] {
        assert!(decode(json!([tag, -1, "", "", "", 1, 83, 1]).as_array().unwrap()).is_err());
    }
    for value in [json!(2), json!(true), json!("true"), Value::Null] {
        assert!(decode(json!([9, -1, "", "", "", 1, 83, value]).as_array().unwrap()).is_err());
    }
}

#[test]
fn automatic_height_rejects_conflicting_dimensions() {
    assert!(decode(json!([9, -1, "", "", "", 1, 82, 1]).as_array().unwrap()).is_ok());
    for value in [json!(0), json!(2), json!(true), json!("auto"), Value::Null] {
        assert!(decode(json!([9, -1, "", "", "", 1, 82, value]).as_array().unwrap()).is_err());
    }
    for field in [1, 75] {
        for fields in [
            vec![json!(82), json!(1), json!(field), json!(100)],
            vec![json!(field), json!(100), json!(82), json!(1)],
        ] {
            let mut record = json!([9, -1, "", "", "", 2]).as_array().unwrap().clone();
            record.extend(fields);
            assert!(decode(&record).unwrap_err().contains("conflicting"));
        }
    }
}

#[test]
fn percentage_dimensions_reject_conflicts_and_invalid_wire_values() {
    for (index, absolute) in [0, 1, 28, 29, 30, 31].into_iter().enumerate() {
        let field = 74 + index;
        assert!(decode(json!([0, -1, "", "", "", 1, field, 72]).as_array().unwrap()).is_ok());
        for value in [json!(-1), json!(16385), json!("72%"), Value::Null] {
            assert!(
                decode(
                    json!([0, -1, "", "", "", 1, field, value])
                        .as_array()
                        .unwrap()
                )
                .is_err()
            );
        }
        for fields in [
            vec![json!(absolute), json!(100), json!(field), json!(72)],
            vec![json!(field), json!(72), json!(absolute), json!(100)],
        ] {
            let mut record = json!([0, -1, "", "", "", 2]).as_array().unwrap().clone();
            record.extend(fields);
            assert!(decode(&record).unwrap_err().contains("conflicting"));
        }
    }
}

#[test]
fn vector_wire_requires_source_and_enforces_frame_geometry_budget() {
    let source =
        serde_json::json!({"viewBox":[0,0,24,24],"paths":[{"d":"M0 0h24v24H0z"}]}).to_string();
    assert!(
        decode(
            json!([19, -1, "", "", "", 1, 80, source])
                .as_array()
                .unwrap()
        )
        .is_ok()
    );
    assert!(
        decode(
            json!([0, -1, "", "", "", 1, 80, source])
                .as_array()
                .unwrap()
        )
        .is_err()
    );
    assert!(decode(json!([19, -1, "", "", "", 0]).as_array().unwrap()).is_err());
    assert!(
        decode(
            json!([19, -1, "", "", "", 1, 80, source, 2, 0, "", "", "child", 0])
                .as_array()
                .unwrap()
        )
        .is_err()
    );
    let path = format!("M0 0{}", "l1 0".repeat(1000));
    let source = serde_json::json!({"viewBox":[0,0,24,24],"paths":[{"d":path}]}).to_string();
    let mut records = json!([0, -1, "", "", "", 0]).as_array().unwrap().clone();
    for _ in 0..33 {
        records.extend(
            json!([19, 0, "", "", "", 1, 80, source])
                .as_array()
                .unwrap()
                .iter()
                .cloned(),
        );
    }
    assert!(decode(&records).unwrap_err().contains("geometry budget"));
}

#[test]
fn text_alignment_wire_values_are_validated() {
    for value in ["start", "end", "left", "center", "right", "justify"] {
        assert!(
            decode(
                json!([2, -1, "", "", "Text", 1, 71, value])
                    .as_array()
                    .unwrap()
            )
            .is_ok()
        );
    }
    for value in [json!("stretch"), json!(0), Value::Null] {
        assert!(
            decode(
                json!([2, -1, "", "", "Text", 1, 71, value])
                    .as_array()
                    .unwrap()
            )
            .is_err()
        );
    }
}

#[test]
fn compiled_wire_table_matches_the_contract_schema() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packages/lingxia-ui/contracts/schema.json"
    );
    // Published crates ship without the package tree; the repository gate runs this.
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let schema: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(contract_drift(&schema), Vec::<String>::new());
}

#[test]
fn contract_drift_reports_fields_missing_on_either_side() {
    let current = json!({
        "uiAbi": UI_ABI,
        "tags": TAGS,
        "options": {},
        "fields": FIELDS.iter().enumerate().map(|(id, (wire, kind))| json!({"id": id, "wire": wire, "kind": kind})).collect::<Vec<_>>(),
    });
    let mut extra = current.clone();
    extra["fields"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id": FIELDS.len(), "wire": "added", "kind": "number"}));
    assert!(contract_drift(&extra).iter().any(|e| e.contains("added")));
    let mut missing = current.clone();
    missing["fields"].as_array_mut().unwrap().pop();
    assert!(!contract_drift(&missing).is_empty());
    let mut renamed = current;
    renamed["tags"][0] = json!("box");
    assert!(contract_drift(&renamed).iter().any(|e| e.contains("box")));
}

#[test]
fn contract_drift_reports_admission_and_limit_changes() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packages/lingxia-ui/contracts/schema.json"
    );
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let schema: Value = serde_json::from_str(&text).unwrap();
    let mut limit = schema.clone();
    limit["limits"]["nodes"] = json!(limits::NODES + 1);
    assert!(
        contract_drift(&limit)
            .iter()
            .any(|e| e.contains("limit nodes"))
    );
    let mut leaf = schema.clone();
    leaf["admission"]["text"]["children"] = json!("any");
    assert!(
        contract_drift(&leaf)
            .iter()
            .any(|e| e.contains("admission text"))
    );
    let mut widened = schema.clone();
    widened["fields"][field::ON_CLICK]["tags"] = json!(["button", "row"]);
    assert!(
        contract_drift(&widened)
            .iter()
            .any(|e| e.contains(&format!("field admission {}", field::ON_CLICK)))
    );
    let mut opened = schema;
    opened["fields"][field::LIST_ROW_INDEX]
        .as_object_mut()
        .unwrap()
        .remove("parent");
    assert!(!contract_drift(&opened).is_empty());
}

fn contract_drift(schema: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    if schema["uiAbi"] != UI_ABI {
        errors.push(format!("uiAbi {} != {UI_ABI}", schema["uiAbi"]));
    }
    let tags: Vec<&str> = schema["tags"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    if tags != TAGS {
        errors.push(format!("tags {tags:?} != {TAGS:?}"));
    }
    let fields = schema["fields"].as_array().unwrap();
    for index in 0..fields.len().max(FIELDS.len()) {
        let schema_field = fields.get(index).map(|f| {
            (
                f["wire"].as_str().unwrap_or(""),
                f["kind"].as_str().unwrap_or(""),
            )
        });
        if schema_field != FIELDS.get(index).copied() {
            errors.push(format!(
                "field {index}: schema {schema_field:?}, native {:?}",
                FIELDS.get(index)
            ));
        }
    }
    if let Some(kinds) = schema["options"].as_object() {
        for (kind, values) in kinds {
            let values: Vec<&str> = values
                .as_array()
                .unwrap()
                .iter()
                .filter_map(Value::as_str)
                .collect();
            if options(kind) != Some(values.as_slice()) {
                errors.push(format!("options {kind}"));
            }
        }
    }
    if let Some(table) = schema["limits"].as_object() {
        let native = [
            ("nodes", limits::NODES),
            ("depth", limits::DEPTH),
            ("animationBytes", limits::ANIMATION_BYTES),
            ("itemKeyBytes", limits::ITEM_KEY_BYTES),
            ("imageBytes", limits::IMAGE_BYTES),
            ("sourceBytes", limits::SOURCE_BYTES),
            ("visibleImageBytes", limits::VISIBLE_IMAGE_BYTES),
            ("decodedImageBytes", limits::DECODED_IMAGE_BYTES),
            ("runs", limits::RUNS),
            ("transitionMs", limits::TRANSITION_MS),
            ("evalBudgetMs", limits::EVAL_BUDGET_MS),
            (
                "evalBudgetDebugMultiplier",
                limits::EVAL_BUDGET_DEBUG_MULTIPLIER,
            ),
            ("editorTextBytes", limits::EDITOR_TEXT_BYTES),
            ("inputRequestBytes", limits::INPUT_REQUEST_BYTES),
            ("inputBatchEdits", limits::INPUT_BATCH_EDITS),
            ("clipboardItems", limits::CLIPBOARD_ITEMS),
            ("wireValues", limits::WIRE_VALUES),
            ("textBytes", limits::TEXT_BYTES),
            ("identifierBytes", limits::IDENTIFIER_BYTES),
            ("listRows", limits::LIST_ROWS),
            ("gridTracks", limits::GRID_TRACKS),
            ("vectorSourceBytes", limits::VECTOR_SOURCE_BYTES),
            ("vectorPathBytes", limits::VECTOR_PATH_BYTES),
            ("vectorPathWork", limits::VECTOR_PATH_WORK),
            ("vectorWork", limits::VECTOR_WORK),
            ("plotWork", limits::PLOT_WORK),
            ("shadowMaskBytes", limits::SHADOW_MASK_BYTES),
            ("compositeFrames", limits::COMPOSITE_FRAMES),
            ("leaseFrames", limits::LEASE_FRAMES),
            ("compositeBytes", limits::COMPOSITE_BYTES),
            ("outlineGlyphs", limits::OUTLINE_GLYPHS),
            ("ghosts", limits::GHOSTS),
            ("layoutMotions", limits::LAYOUT_MOTIONS),
            ("dataAttributeBytes", limits::DATA_ATTRIBUTE_BYTES),
            ("blurLayers", limits::BLUR_LAYERS),
        ];
        let names: Vec<&str> = native.iter().map(|(name, _)| *name).collect();
        for (name, value) in table {
            match native.iter().find(|(native, _)| native == name) {
                Some((_, native)) if Some(*native as u64) == value.as_u64() => {}
                _ => errors.push(format!("limit {name}: schema {value}, native {names:?}")),
            }
        }
        if table.len() != native.len() {
            errors.push(format!("limits: schema {table:?}, native {names:?}"));
        }
    }
    let tag_id = |name: &Value| TAGS.iter().position(|tag| Some(*tag) == name.as_str());
    let field_id = |name: &Value| {
        fields
            .iter()
            .position(|field| field["key"] == *name)
            .unwrap_or(usize::MAX)
    };
    if let Some(table) = schema["admission"].as_object() {
        for (index, name) in TAGS.iter().enumerate() {
            let Some(entry) = table.get(*name) else {
                errors.push(format!("admission {name}: missing in schema"));
                continue;
            };
            let native = TAG_ADMISSION[index];
            let children = format!("{:?}", native.children).to_lowercase();
            let requires: Vec<usize> = entry["requires"]
                .as_array()
                .map(|r| r.iter().map(field_id).collect())
                .unwrap_or_default();
            if entry["children"].as_str() != Some(children.as_str())
                || entry["content"].as_bool().unwrap_or(false) != native.content
                || entry["root"].as_bool().unwrap_or(false) != native.root
                || entry["keyed"].as_bool().unwrap_or(false) != native.keyed
                || entry.get("parent").and_then(tag_id) != native.parent
                || requires != native.requires
            {
                errors.push(format!(
                    "admission {name}: schema {entry}, native {native:?}"
                ));
            }
        }
        if table.len() != TAGS.len() || TAG_ADMISSION.len() != TAGS.len() {
            errors.push("admission: tag count differs".into());
        }
    }
    // Field admission is part of the admission section; table-only schemas omit both.
    let admitted_fields = if schema["admission"].is_object() {
        fields.as_slice()
    } else {
        &[]
    };
    for (index, entry) in admitted_fields.iter().enumerate() {
        let Some(native) = FIELD_ADMISSION.get(index) else {
            errors.push(format!("field admission {index}: missing natively"));
            continue;
        };
        let tags: Option<Vec<usize>> = entry["tags"]
            .as_array()
            .map(|tags| tags.iter().filter_map(tag_id).collect());
        if tags.as_deref() != native.tags || entry.get("parent").and_then(tag_id) != native.parent {
            errors.push(format!(
                "field admission {index}: schema {entry}, native {native:?}"
            ));
        }
    }
    errors
}
#[test]
fn every_truncation_and_unknown_token_is_safe() {
    let valid = json!([0, -1, "", "", "", 1, 2, 12, 2, 0, "", "", "hello", 1, 4, 24]);
    let tokens = valid.as_array().unwrap();
    for n in 0..tokens.len() {
        let _ = decode(&tokens[..n]);
    }
    for index in 0..tokens.len() {
        for replacement in [
            Value::Null,
            json!({}),
            json!([]),
            json!(-999),
            json!("invalid"),
        ] {
            let mut changed = tokens.clone();
            changed[index] = replacement;
            let _ = decode(&changed);
        }
    }
}
