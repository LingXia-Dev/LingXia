//! Which records may nest, carry inline text, or carry a field on a tag; the
//! tables are generated from the contract schema.
use super::{Children, FIELD_ADMISSION, FIELDS, Record, TAG_ADMISSION, TAGS, field};
use serde_json::Value;

/// A child of `parent` with `child` tag may be appended.
pub(super) fn child(records: &[Record], parent: usize, child: usize) -> Result<(), String> {
    let owner = TAG_ADMISSION[records[parent].tag];
    match owner.children {
        Children::None | Children::Text => return Err("UI_CHILD: leaf node".into()),
        Children::Content if !records[parent].text.is_empty() => {
            return Err("UI_BUTTON: a control with children carries no inline text".into());
        }
        Children::Runs if !records[parent].text.is_empty() => {
            return Err("UI_TEXT: a text with runs carries no inline text".into());
        }
        Children::Runs if TAG_ADMISSION[child].parent != Some(records[parent].tag) => {
            return Err(format!(
                "UI_TEXT: <{}> accepts only text runs",
                TAGS[records[parent].tag]
            ));
        }
        Children::Options if TAG_ADMISSION[child].parent != Some(records[parent].tag) => {
            return Err(format!(
                "UI_CHILD: <{}> accepts only its options",
                TAGS[records[parent].tag]
            ));
        }
        _ => {}
    }
    if !TAG_ADMISSION[child].content {
        let mut ancestor = Some(parent);
        while let Some(owner) = ancestor {
            if TAG_ADMISSION[records[owner].tag].children == Children::Content {
                return Err("UI_BUTTON: descendants must be noninteractive content".into());
            }
            ancestor = records[owner].parent;
        }
    }
    Ok(())
}

pub(super) fn text(tag: usize, text: &str) -> Result<(), String> {
    if !text.is_empty()
        && !matches!(
            TAG_ADMISSION[tag].children,
            Children::Text | Children::Content | Children::Runs
        )
    {
        return Err("UI_TEXT: container".into());
    }
    Ok(())
}

/// Field `id` applies to a record of `tag` under a parent of `parent` tag.
pub(super) fn field(tag: usize, parent: Option<usize>, id: usize) -> Result<(), String> {
    if FIELDS[id].1 == "reserved" {
        return Err(format!("UI_FIELD: {} is not accepted", FIELDS[id].0));
    }
    let admission = FIELD_ADMISSION[id];
    if admission.tags.is_some_and(|tags| !tags.contains(&tag))
        || admission
            .parent
            .is_some_and(|required| parent != Some(required))
    {
        return Err(format!(
            "UI_FIELD: {} is not accepted on <{}>",
            FIELDS[id].0, TAGS[tag]
        ));
    }
    Ok(())
}

fn has(fields: &[(usize, Value)], id: usize) -> bool {
    fields.iter().any(|(field, _)| *field == id)
}

/// Field combinations and placement a complete record must satisfy.
pub(super) fn record(
    tag: usize,
    parent: Option<usize>,
    key: &str,
    fields: &[(usize, Value)],
) -> Result<(), String> {
    for (absolute, percent) in super::DIMENSIONS {
        if has(fields, absolute) && has(fields, percent) {
            return Err("UI_STYLE: conflicting absolute and percentage dimensions".into());
        }
    }
    if has(fields, field::HEIGHT_AUTO)
        && (has(fields, field::HEIGHT) || has(fields, field::HEIGHT_PERCENT))
    {
        return Err("UI_STYLE: conflicting automatic height".into());
    }
    let admission = TAG_ADMISSION[tag];
    if admission.root && parent.is_some() {
        return Err(format!("UI_CHILD: <{}> must be the root", TAGS[tag]));
    }
    if admission
        .parent
        .is_some_and(|required| parent != Some(required))
    {
        return Err(format!(
            "UI_CHILD: <{}> requires a <{}> parent",
            TAGS[tag],
            TAGS[admission.parent.unwrap()]
        ));
    }
    if admission.keyed && key.is_empty() {
        return Err(format!("UI_KEY: <{}> requires a key", TAGS[tag]));
    }
    if let Some(missing) = admission.requires.iter().find(|id| !has(fields, **id)) {
        return Err(format!(
            "UI_FIELD: <{}> requires {}",
            TAGS[tag], FIELDS[*missing].0
        ));
    }
    if parent.is_some_and(|p| TAG_ADMISSION[p].children == Children::Rows)
        && !has(fields, field::LIST_ROW_INDEX)
    {
        return Err("UI_LIST: expected row wrapper".into());
    }
    Ok(())
}
