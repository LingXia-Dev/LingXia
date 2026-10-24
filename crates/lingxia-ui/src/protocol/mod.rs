use serde_json::Value;

#[rustfmt::skip]
mod generated;
pub use generated::{FIELDS, TAGS, UI_ABI, field, options, tag};

/// Absolute dimension fields paired with their percentage forms.
pub const DIMENSIONS: [(usize, usize); 6] = [
    (field::WIDTH, field::WIDTH_PERCENT),
    (field::HEIGHT, field::HEIGHT_PERCENT),
    (field::MIN_WIDTH, field::MIN_WIDTH_PERCENT),
    (field::MIN_HEIGHT, field::MIN_HEIGHT_PERCENT),
    (field::MAX_WIDTH, field::MAX_WIDTH_PERCENT),
    (field::MAX_HEIGHT, field::MAX_HEIGHT_PERCENT),
];
/// Top, right, bottom, left.
pub const PADDING_EDGES: [usize; 4] = [
    field::PADDING_TOP,
    field::PADDING_RIGHT,
    field::PADDING_BOTTOM,
    field::PADDING_LEFT,
];
/// Top, right, bottom, left.
pub const BORDER_EDGES: [usize; 4] = [
    field::BORDER_TOP_WIDTH,
    field::BORDER_RIGHT_WIDTH,
    field::BORDER_BOTTOM_WIDTH,
    field::BORDER_LEFT_WIDTH,
];
/// Top-left, top-right, bottom-right, bottom-left.
pub const RADIUS_CORNERS: [usize; 4] = [
    field::RADIUS_TOP_LEFT,
    field::RADIUS_TOP_RIGHT,
    field::RADIUS_BOTTOM_RIGHT,
    field::RADIUS_BOTTOM_LEFT,
];
/// Paint-only fields an editor focus delta may carry.
pub const FOCUS_PAINT_FIELDS: [usize; 7] = [
    field::COLOR,
    field::BACKGROUND,
    field::OPACITY,
    field::BORDER_COLOR,
    field::BACKGROUND_GRADIENT,
    field::BOX_SHADOW,
    field::PLACEHOLDER_COLOR,
];

#[cfg(test)]
mod list_key_wire_tests {
    use super::*;
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
}

#[cfg(test)]
mod animation_wire_tests {
    use super::*;
    use serde_json::json;

    fn payload() -> String {
        let fixtures: Value = serde_json::from_str(include_str!(
            "../../../../packages/lingxia-ui/tests/fixtures/animations.json"
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
}

pub(crate) fn flex_basis(value: &Value) -> Result<Option<(f32, bool)>, String> {
    if value.as_str() == Some("auto") {
        return Ok(None);
    }
    let percent = value.as_str().and_then(|v| v.strip_suffix('%'));
    let number = if let Some(text) = percent {
        if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
            return Err("UI_STYLE: invalid flex basis".into());
        }
        text.parse::<f64>().ok()
    } else {
        value.as_f64()
    };
    let number = number
        .filter(|v| v.is_finite() && (0.0..=16384.0).contains(v))
        .ok_or("UI_STYLE: invalid flex basis")?;
    Ok(Some((number as f32, percent.is_some())))
}

pub(crate) fn focus_style(value: &Value) -> Result<Vec<(usize, Value)>, String> {
    let text = value
        .as_str()
        .filter(|s| s.len() <= 16384)
        .ok_or("UI_STYLE: invalid focus style")?;
    let pairs: Vec<Value> =
        serde_json::from_str(text).map_err(|_| "UI_STYLE: invalid focus style")?;
    if pairs.len() > 14
        || !pairs.len().is_multiple_of(2)
        || pairs.chunks_exact(2).any(|p| {
            p[0].as_u64()
                .is_none_or(|id| !FOCUS_PAINT_FIELDS.contains(&(id as usize)))
        })
    {
        return Err("UI_STYLE: focus supports paint properties only".into());
    }
    let mut record = vec![
        serde_json::json!(0),
        serde_json::json!(-1),
        serde_json::json!(""),
        serde_json::json!(""),
        serde_json::json!(""),
        serde_json::json!(pairs.len() / 2),
    ];
    record.extend(pairs);
    Ok(decode(&record)?.remove(0).fields)
}

#[derive(Clone, Debug)]
pub struct Record {
    pub tag: usize,
    pub parent: Option<usize>,
    pub key: String,
    pub test_id: String,
    pub text: String,
    pub fields: Vec<(usize, Value)>,
}

impl Record {
    pub fn number(&self, field: usize) -> Option<f32> {
        self.fields
            .iter()
            .find(|(id, _)| *id == field)?
            .1
            .as_f64()
            .map(|v| v as f32)
    }
    pub fn string(&self, field: usize) -> Option<&str> {
        self.fields.iter().find(|(id, _)| *id == field)?.1.as_str()
    }
}

pub(crate) fn identities(records: &[Record]) -> Vec<String> {
    let mut identities: Vec<String> = Vec::with_capacity(records.len());
    let mut children = vec![0; records.len()];
    for record in records {
        let ordinal = record
            .parent
            .map(|p| {
                let n = children[p];
                children[p] += 1;
                n
            })
            .unwrap_or(0);
        let segment = if record.key.is_empty() {
            format!("i:{ordinal}")
        } else {
            format!("k:{}:{}", record.key.len(), record.key)
        };
        identities.push(format!(
            "{}/{segment}:{}",
            record.parent.map(|p| identities[p].as_str()).unwrap_or(""),
            record.tag
        ));
    }
    identities
}

pub(crate) const MAX_BUFFER_VALUES: usize = 100_000;

pub fn decode(buffer: &[Value]) -> Result<Vec<Record>, String> {
    let mut animation_bytes = 0usize;
    let mut item_key_bytes = 0usize;
    let mut item_keys = std::collections::HashMap::new();
    let mut records: Vec<Record> = Vec::new();
    let mut offsets: std::collections::HashMap<i64, usize> = std::collections::HashMap::new();
    let mut keys = std::collections::HashSet::new();
    let mut lists = std::collections::HashSet::new();
    let mut cursor = 0;
    let mut depths = Vec::new();
    if buffer.len() > MAX_BUFFER_VALUES {
        return Err("UI_LIMIT: buffer too large".into());
    }
    while cursor < buffer.len() {
        if records.len() >= 4096 {
            return Err("UI_LIMIT: too many nodes".into());
        }
        let start = cursor;
        let header = buffer
            .get(cursor..cursor + 6)
            .ok_or("UI_BUFFER: truncated header")?;
        let tag = header[0]
            .as_u64()
            .filter(|v| *v < TAGS.len() as u64)
            .ok_or("UI_TAG")? as usize;
        let parent_offset = header[1].as_i64().ok_or("UI_PARENT")?;
        let parent = if records.is_empty() && parent_offset == -1 {
            None
        } else {
            let index = *offsets
                .get(&parent_offset)
                .ok_or("UI_PARENT: must precede child")?;
            if matches!(
                records[index].tag,
                tag::TEXT
                    | tag::SPACER
                    | tag::IMAGE
                    | tag::INPUT
                    | tag::TEXTAREA
                    | tag::SWITCH
                    | tag::RADIO_OPTION
                    | tag::SEARCH
                    | tag::SLIDER
                    | tag::PLOT
                    | tag::VECTOR
            ) {
                return Err("UI_CHILD: leaf node".into());
            }
            if matches!(records[index].tag, tag::BUTTON | tag::CHECKBOX)
                && (!records[index].text.is_empty()
                    || records[index]
                        .string(field::ACCESSIBILITY_LABEL)
                        .is_none_or(|label| label.trim().is_empty()))
            {
                return Err(
                    "UI_BUTTON: composed button requires an explicit label and no inline text"
                        .into(),
                );
            }
            let mut ancestor = Some(index);
            while let Some(owner) = ancestor {
                if matches!(records[owner].tag, tag::BUTTON | tag::CHECKBOX)
                    && !matches!(
                        tag,
                        tag::COLUMN
                            | tag::ROW
                            | tag::TEXT
                            | tag::SPACER
                            | tag::IMAGE
                            | tag::GRID
                            | tag::STACK
                            | tag::VECTOR
                    )
                {
                    return Err("UI_BUTTON: descendants must be noninteractive content".into());
                }
                ancestor = records[owner].parent;
            }
            Some(index)
        };
        let key = header[2].as_str().ok_or("UI_KEY")?.to_owned();
        let depth = parent.map(|p| depths[p] + 1).unwrap_or(0);
        if depth > 128 {
            return Err("UI_LIMIT: tree too deep".into());
        }
        depths.push(depth);
        if !key.is_empty() && !keys.insert((parent, key.clone())) {
            return Err("UI_KEY: duplicate sibling".into());
        }
        let test_id = header[3].as_str().ok_or("UI_TEST_ID")?.to_owned();
        let text = header[4].as_str().ok_or("UI_TEXT")?.to_owned();
        if text.len() > 65536 || test_id.len() > 256 || key.len() > 256 {
            return Err("UI_LIMIT: string too large".into());
        }
        if !matches!(
            tag,
            tag::TEXT | tag::BUTTON | tag::CHECKBOX | tag::SWITCH | tag::RADIO_OPTION
        ) && !text.is_empty()
        {
            return Err("UI_TEXT: container".into());
        }
        let count = header[5]
            .as_u64()
            .filter(|v| *v <= FIELDS.len() as u64)
            .ok_or("UI_FIELDS")? as usize;
        cursor += 6;
        let pairs = buffer
            .get(cursor..cursor + count * 2)
            .ok_or("UI_BUFFER: truncated fields")?;
        let mut fields = Vec::new();
        for pair in pairs.chunks_exact(2) {
            let id = pair[0]
                .as_u64()
                .filter(|v| *v < FIELDS.len() as u64)
                .ok_or("UI_FIELD")? as usize;
            if fields.iter().any(|(old, _)| *old == id) {
                return Err("UI_FIELD: duplicate".into());
            }
            if let Some(options) = options(FIELDS[id].1) {
                if matches!(id, field::FLEX_DIRECTION | field::DISPLAY)
                    && !matches!(tag, tag::COLUMN | tag::ROW)
                {
                    return Err(
                        "UI_LAYOUT: flex direction/display requires a flex container".into(),
                    );
                }
                if id == field::SCROLL_AXIS && tag != tag::SCROLL {
                    return Err("UI_SCROLL: scrollAxis requires a scroll node".into());
                }
                let value = pair[1]
                    .as_str()
                    .filter(|v| options.contains(v))
                    .ok_or("UI_STYLE: invalid style option")?;
                if tag == tag::LIST
                    && ((id == field::ALIGN && value != "stretch")
                        || (id == field::JUSTIFY && value != "start"))
                {
                    return Err(
                        "UI_LIST: rows require stretch alignment and start justification".into(),
                    );
                }
            } else if id == field::LIST_ITEM_KEYS {
                if tag != tag::LIST {
                    return Err("UI_LIST: itemKeys requires a list".into());
                }
                let source = pair[1].as_str().ok_or("UI_LIST: invalid itemKeys")?;
                item_key_bytes = item_key_bytes.saturating_add(source.len());
                if item_key_bytes > 262_144 {
                    return Err("UI_LIST: itemKeys page budget exceeded".into());
                }
                item_keys.insert(
                    records.len(),
                    crate::layout::list::decode_item_keys(source)?,
                );
            } else if id == field::ANIMATION {
                let source = pair[1]
                    .as_str()
                    .ok_or("UI_ANIMATION: expected encoded definition")?;
                animation_bytes = animation_bytes
                    .checked_add(source.len())
                    .ok_or("UI_ANIMATION: page payload budget exceeded")?;
                if animation_bytes > 262144 {
                    return Err("UI_ANIMATION: page payload budget exceeded".into());
                }
                crate::animation::decode_definition(source)?;
            } else if id == field::FLEX_BASIS {
                flex_basis(&pair[1])?;
            } else if id == field::FOCUS_STYLE {
                if !matches!(tag, tag::INPUT | tag::TEXTAREA | tag::SEARCH) {
                    return Err("UI_STYLE: focus styles require an editor".into());
                }
                focus_style(&pair[1])?;
            } else if id == field::VECTOR_SOURCE {
                if tag != tag::VECTOR || !pair[1].is_string() {
                    return Err("UI_VECTOR: source requires vector node".into());
                }
            } else if id == field::SUBMIT_ON_ENTER {
                if tag != tag::TEXTAREA || pair[1].as_u64().is_none_or(|v| v > 1) {
                    return Err("UI_INPUT: invalid submitOnEnter".into());
                }
            } else if id == field::HEIGHT_AUTO {
                if pair[1].as_u64() != Some(1) {
                    return Err("UI_STYLE: invalid automatic height".into());
                }
            } else if DIMENSIONS.iter().any(|(_, percent)| *percent == id) {
                pair[1]
                    .as_f64()
                    .filter(|v| v.is_finite() && (0.0..=16384.0).contains(v))
                    .ok_or("UI_STYLE: invalid percentage")?;
            } else if id == field::BOX_SHADOW {
                crate::scene::shadow::BoxShadow::parse(
                    pair[1]
                        .as_str()
                        .ok_or("UI_SHADOW: expected encoded shadows")?,
                )?;
            } else if id == field::BACKGROUND_GRADIENT {
                crate::scene::gradient::LinearGradient::parse(
                    pair[1]
                        .as_str()
                        .ok_or("UI_GRADIENT: expected encoded gradient")?,
                )?;
            } else if id == field::LETTER_SPACING {
                if pair[1]
                    .as_f64()
                    .is_none_or(|v| !v.is_finite() || v.abs() > 1024.0)
                {
                    return Err("UI_STYLE: invalid letter spacing".into());
                }
            } else if id == field::PLOT_MARKER_EMPHASIS_SCALE {
                if tag != tag::PLOT
                    || pair[1]
                        .as_f64()
                        .is_none_or(|v| !v.is_finite() || !(1.0..=4.0).contains(&v))
                {
                    return Err("UI_PLOT: invalid marker emphasis scale".into());
                }
            } else if id == field::PLOT_PIE_LAYOUT {
                if tag != tag::PLOT || pair[1].as_str().is_none_or(|s| s.len() > 1024) {
                    return Err("UI_PLOT: invalid pie layout".into());
                }
            } else if matches!(id, field::PLOT_DOMAIN_MIN | field::PLOT_DOMAIN_MAX) {
                if tag != tag::PLOT
                    || pair[1]
                        .as_f64()
                        .is_none_or(|v| !v.is_finite() || v.abs() > 1e9)
                {
                    return Err("UI_PLOT: invalid domain bound".into());
                }
            } else if id == field::ON_PLOT_SELECT {
                if tag != tag::PLOT
                    || pair[1]
                        .as_u64()
                        .is_none_or(|v| v == 0 || v > 9_007_199_254_740_991)
                {
                    return Err("UI_PLOT: invalid selection callback".into());
                }
            } else if id == field::PLOT_SELECTED_INDEX {
                if tag != tag::PLOT || pair[1].as_i64().is_none_or(|v| !(-1..128).contains(&v)) {
                    return Err("UI_PLOT: invalid selected index".into());
                }
            } else if matches!(id, field::PLOT_SMOOTH | field::PLOT_AREA) {
                if tag != tag::PLOT || !matches!(pair[1].as_u64(), Some(0 | 1)) {
                    return Err("UI_PLOT: invalid boolean property".into());
                }
            } else if matches!(
                id,
                field::PLOT_VALUES | field::PLOT_KIND | field::PLOT_COLORS
            ) {
                if tag != tag::PLOT || pair[1].as_str().is_none_or(|s| s.len() > 8192) {
                    return Err("UI_PLOT: invalid encoded property".into());
                }
            } else if id == field::TEXT_LINES {
                if tag != tag::TEXT || pair[1].as_u64().is_none_or(|v| !(1..=1024).contains(&v)) {
                    return Err(
                        "UI_TEXT: lines must be an integer between 1 and 1024 on text".into(),
                    );
                }
            } else if matches!(id, field::OFFSET_X | field::OFFSET_Y) {
                if parent.is_none_or(|p| records[p].tag != tag::STACK)
                    || pair[1]
                        .as_f64()
                        .is_none_or(|v| !v.is_finite() || v.abs() > 16384.0)
                {
                    return Err(
                        "UI_STACK: offset requires a stack parent and bounded number".into(),
                    );
                }
            } else if id == field::GRID_COLUMNS {
                if tag != tag::GRID || pair[1].as_u64().is_none_or(|v| !(1..=64).contains(&v)) {
                    return Err(
                        "UI_GRID: columns must be an integer between 1 and 64 on a grid".into(),
                    );
                }
            } else if matches!(
                id,
                field::SLIDER_VALUE
                    | field::SLIDER_MIN
                    | field::SLIDER_MAX
                    | field::SLIDER_STEP
                    | field::ON_SLIDER_CHANGE
            ) {
                if tag != tag::SLIDER {
                    return Err("UI_SLIDER: property on another tag".into());
                }
                if id == field::ON_SLIDER_CHANGE {
                    pair[1]
                        .as_u64()
                        .filter(|v| *v > 0 && *v <= 9_007_199_254_740_991)
                        .ok_or("UI_SLIDER: invalid callback")?;
                } else {
                    pair[1]
                        .as_f64()
                        .filter(|v| v.is_finite() && v.abs() <= 1e9)
                        .ok_or("UI_SLIDER: invalid number")?;
                }
            } else if id == field::IMAGE_RETRY_KEY {
                if tag != tag::IMAGE
                    || pair[1]
                        .as_str()
                        .is_none_or(|s| s.len() > 256 || s.contains('\0'))
                {
                    return Err("UI_IMAGE: invalid retry key".into());
                }
            } else if matches!(id, field::ON_IMAGE_LOAD | field::ON_IMAGE_ERROR) {
                if tag != tag::IMAGE
                    || pair[1]
                        .as_u64()
                        .is_none_or(|v| v == 0 || v > 9_007_199_254_740_991)
                {
                    return Err("UI_IMAGE: invalid callback".into());
                }
            } else if id == field::IMAGE_ALT {
                if tag != tag::IMAGE
                    || pair[1].as_str().is_none_or(|s| {
                        s.len() > 1024 || s.contains('\0') || (!s.is_empty() && s.trim().is_empty())
                    })
                {
                    return Err("UI_IMAGE: invalid alternative text".into());
                }
            } else if id == field::ACCESSIBILITY_LABEL {
                if !matches!(
                    tag,
                    tag::BUTTON
                        | tag::INPUT
                        | tag::TEXTAREA
                        | tag::CHECKBOX
                        | tag::RADIO
                        | tag::SEARCH
                        | tag::SLIDER
                        | tag::PLOT
                ) || pair[1].as_str().is_none_or(|s| {
                    s.len() > 1024
                        || s.contains('\0')
                        || (matches!(tag, tag::BUTTON | tag::CHECKBOX) && s.trim().is_empty())
                }) {
                    return Err("UI_RADIO: invalid accessibility label".into());
                }
            } else if matches!(id, field::CHECKED | field::ON_CHECKED_CHANGE) {
                if !matches!(tag, tag::CHECKBOX | tag::SWITCH | tag::RADIO_OPTION) {
                    return Err("UI_CHECK: property on another tag".into());
                }
                let value = pair[1].as_u64().ok_or("UI_CHECK: invalid flag or handle")?;
                if (id == field::CHECKED && value > 1)
                    || (id == field::ON_CHECKED_CHANGE
                        && (value == 0 || value > 9_007_199_254_740_991))
                {
                    return Err("UI_CHECK: invalid flag or handle".into());
                }
            } else if (field::LIST_ID..=field::ON_SUBMIT).contains(&id) {
                match id {
                    field::INPUT_VALUE | field::PLACEHOLDER
                        if matches!(tag, tag::INPUT | tag::TEXTAREA | tag::SEARCH) =>
                    {
                        pair[1]
                            .as_str()
                            .filter(|s| s.len() <= crate::text::input::MAX_TEXT_BYTES)
                            .ok_or("UI_INPUT: invalid text")?;
                    }
                    field::SECURE | field::AUTO_FOCUS
                        if matches!(tag, tag::INPUT | tag::TEXTAREA | tag::SEARCH)
                            && !(tag == tag::SEARCH && id == field::SECURE) =>
                    {
                        pair[1]
                            .as_u64()
                            .filter(|v| *v <= 1)
                            .ok_or("UI_INPUT: invalid flag")?;
                    }
                    field::KEYBOARD if matches!(tag, tag::INPUT | tag::TEXTAREA | tag::SEARCH) => {
                        if tag == tag::SEARCH && pair[1].as_str() != Some("search") {
                            return Err("UI_INPUT: search requires search keyboard".into());
                        }
                        if !matches!(
                            pair[1].as_str(),
                            Some("text" | "number" | "email" | "phone" | "search")
                        ) {
                            return Err("UI_INPUT: invalid keyboard".into());
                        }
                    }
                    field::ON_INPUT | field::ON_SUBMIT
                        if matches!(tag, tag::INPUT | tag::TEXTAREA | tag::SEARCH) =>
                    {
                        pair[1]
                            .as_u64()
                            .filter(|v| *v > 0 && *v <= 9_007_199_254_740_991)
                            .ok_or("UI_INPUT: invalid callback")?;
                    }
                    field::IMAGE_SRC if tag == tag::IMAGE => {
                        pair[1]
                            .as_str()
                            .filter(|s| !s.is_empty() && s.len() <= 8192 && !s.contains('\0'))
                            .ok_or("UI_IMAGE: invalid source")?;
                    }
                    field::IMAGE_FIT if tag == tag::IMAGE => {
                        if !matches!(pair[1].as_str(), Some("cover" | "contain" | "fill")) {
                            return Err("UI_IMAGE: invalid fit".into());
                        }
                    }
                    field::ON_REACH_START | field::ON_REACH_END
                        if matches!(tag, tag::SCROLL | tag::LIST) =>
                    {
                        pair[1]
                            .as_u64()
                            .filter(|v| *v > 0 && *v <= 9_007_199_254_740_991)
                            .ok_or("UI_SCROLL: invalid endpoint handle")?;
                    }
                    field::LIST_ID if tag == tag::LIST => {
                        let value = pair[1]
                            .as_str()
                            .filter(|s| !s.is_empty() && s.len() <= 8192)
                            .ok_or("UI_LIST: invalid identity")?;
                        if !lists.insert(value.to_owned()) {
                            return Err("UI_LIST: duplicate identity".into());
                        }
                    }
                    field::LIST_COUNT if tag == tag::LIST => {
                        pair[1]
                            .as_u64()
                            .filter(|n| *n <= crate::layout::list::MAX_ROWS as u64)
                            .ok_or("UI_LIST: invalid count")?;
                    }
                    field::LIST_LEADING | field::LIST_TRAILING if tag == tag::LIST => {
                        pair[1]
                            .as_f64()
                            .filter(|v| v.is_finite() && *v >= 0.0 && *v <= 1_638_400_000.0)
                            .ok_or("UI_LIST: invalid extent")?;
                    }
                    field::LIST_ROW_INDEX
                        if tag == tag::COLUMN
                            && parent.is_some_and(|p| records[p].tag == tag::LIST) =>
                    {
                        let count = records[parent.unwrap()]
                            .number(field::LIST_COUNT)
                            .unwrap_or(0.0) as u64;
                        pair[1]
                            .as_u64()
                            .filter(|n| *n < count)
                            .ok_or("UI_LIST: invalid row index")?;
                    }
                    field::LIST_ANCHOR if tag == tag::LIST => {
                        let text = pair[1]
                            .as_str()
                            .filter(|s| s.len() <= 256)
                            .ok_or("UI_LIST: invalid anchor")?;
                        let value: Value =
                            serde_json::from_str(text).map_err(|_| "UI_LIST: invalid anchor")?;
                        if value != "end" {
                            let object = value.as_object().ok_or("UI_LIST: invalid anchor")?;
                            if object
                                .keys()
                                .any(|k| !matches!(k.as_str(), "index" | "align" | "seq"))
                                || object.get("index").and_then(Value::as_u64).is_none()
                                || !matches!(
                                    object.get("align").and_then(Value::as_str),
                                    Some("start" | "end")
                                )
                                || object.get("seq").is_some_and(|v| v.as_u64().is_none())
                            {
                                return Err("UI_LIST: invalid anchor".into());
                            }
                        }
                    }
                    _ => return Err("UI_LIST: property on another tag".into()),
                }
            } else if id == field::ON_CLICK || id == field::ENABLED {
                if tag != tag::BUTTON
                    && !(id == field::ENABLED
                        && matches!(
                            tag,
                            tag::INPUT
                                | tag::TEXTAREA
                                | tag::CHECKBOX
                                | tag::SWITCH
                                | tag::RADIO
                                | tag::RADIO_OPTION
                                | tag::SEARCH
                                | tag::SLIDER
                        ))
                {
                    return Err("UI_PROP: button property on another tag".into());
                }
                let value = pair[1].as_u64().ok_or("UI_PROP: invalid handle or flag")?;
                if (id == field::ON_CLICK && (value == 0 || value > 9_007_199_254_740_991))
                    || (id == field::ENABLED && value > 1)
                {
                    return Err("UI_PROP: invalid handle or flag".into());
                }
            } else if FIELDS[id].1 == "number" {
                let value = pair[1]
                    .as_f64()
                    .filter(|v| v.is_finite() && *v >= 0.0 && *v <= 16384.0)
                    .ok_or("UI_STYLE: invalid length")?;
                if matches!(id, field::FONT_SIZE | field::LINE_HEIGHT) && value == 0.0 {
                    return Err("UI_STYLE: font size and line height must be positive".into());
                }
                if (id == field::BACKDROP_BLUR && value > 64.0)
                    || (id == field::BACKDROP_SATURATION && value > 16.0)
                {
                    return Err("UI_STYLE: invalid backdrop filter".into());
                }
                if id == field::OPACITY && value > 1.0 {
                    return Err("UI_STYLE: opacity must be between 0 and 1".into());
                }
                if id == field::FONT_WEIGHT && !(1.0..=1000.0).contains(&value) {
                    return Err("UI_STYLE: font weight must be between 1 and 1000".into());
                }
            } else {
                let value = pair[1].as_str().ok_or("UI_STYLE: expected color")?;
                if id != field::BORDER_COLOR || value != "currentColor" {
                    color(value)?;
                }
            }
            fields.push((id, pair[1].clone()));
        }
        for (index, absolute) in [0, 1, 28, 29, 30, 31].into_iter().enumerate() {
            if fields.iter().any(|(id, _)| *id == absolute)
                && fields
                    .iter()
                    .any(|(id, _)| *id == field::WIDTH_PERCENT + index)
            {
                return Err("UI_STYLE: conflicting absolute and percentage dimensions".into());
            }
        }
        if fields.iter().any(|(id, _)| *id == field::HEIGHT_AUTO)
            && fields
                .iter()
                .any(|(id, _)| matches!(*id, field::HEIGHT | field::HEIGHT_PERCENT))
        {
            return Err("UI_STYLE: conflicting automatic height".into());
        }
        if (tag == tag::RADIO_OPTION) != parent.is_some_and(|p| records[p].tag == tag::RADIO) {
            return Err(
                "UI_RADIO: options require a radio parent, groups accept only options".into(),
            );
        }
        if tag == tag::RADIO_OPTION
            && (key.is_empty() || !fields.iter().any(|(id, _)| *id == field::CHECKED))
        {
            return Err("UI_RADIO: option requires a key and checked flag".into());
        }
        if tag == tag::GRID && !fields.iter().any(|(id, _)| *id == field::GRID_COLUMNS) {
            return Err("UI_GRID: missing columns".into());
        }
        if tag == tag::IMAGE && !fields.iter().any(|(id, _)| *id == field::IMAGE_SRC) {
            return Err("UI_IMAGE: missing source".into());
        }
        if tag == tag::LIST
            && ![11, 12, 13, 14]
                .iter()
                .all(|required| fields.iter().any(|(id, _)| id == required))
        {
            return Err("UI_LIST: missing geometry".into());
        }
        if parent.is_some_and(|p| records[p].tag == tag::LIST)
            && !fields.iter().any(|(id, _)| *id == field::LIST_ROW_INDEX)
        {
            return Err("UI_LIST: expected row wrapper".into());
        }
        if let Some(roster) = item_keys.get(&records.len()) {
            let count = fields
                .iter()
                .find(|(id, _)| *id == field::LIST_COUNT)
                .and_then(|(_, v)| v.as_u64());
            if count != Some(roster.len() as u64) {
                return Err("UI_LIST: itemKeys must match count".into());
            }
        }
        if let Some(roster) = parent.and_then(|p| item_keys.get(&p)) {
            let row = fields
                .iter()
                .find(|(id, _)| *id == field::LIST_ROW_INDEX)
                .and_then(|(_, v)| v.as_u64())
                .ok_or("UI_LIST: missing row index")? as usize;
            if roster
                .get(row)
                .is_none_or(|item| crate::layout::list::item_key(item) != key)
            {
                return Err("UI_LIST: row key does not match itemKeys".into());
            }
        }
        offsets.insert(start as i64, records.len());
        records.push(Record {
            tag,
            parent,
            key,
            test_id,
            text,
            fields,
        });
        cursor += count * 2;
    }
    if records.is_empty() {
        return Err("UI_ROOT: empty buffer".into());
    }
    let mut groups = std::collections::HashMap::new();
    for record in records.iter().filter(|r| r.tag == tag::SLIDER) {
        crate::SliderSnapshot::from_record(record)?;
    }
    let mut vector_work = 0;
    for record in records.iter().filter(|r| r.tag == tag::VECTOR) {
        let vector = crate::scene::vector::Vector::parse(
            record
                .string(field::VECTOR_SOURCE)
                .ok_or("UI_VECTOR: missing source")?,
        )?;
        vector_work += vector.work;
        if vector_work > 32768 {
            return Err("UI_LIMIT: vector geometry budget exceeded".into());
        }
    }
    let mut plot_work = 0;
    for record in records.iter().filter(|r| r.tag == tag::PLOT) {
        let plot = crate::scene::plot::Plot::from_record(record)?;
        plot_work += plot.work();
        if plot_work > 16384 {
            return Err("UI_LIMIT: plot geometry budget exceeded".into());
        }
    }
    for record in records.iter().filter(|r| r.tag == tag::RADIO_OPTION) {
        let group = groups
            .entry(record.parent.unwrap())
            .or_insert_with(|| (0usize, 0usize, std::collections::HashSet::new()));
        group.0 += 1;
        group.1 += usize::from(record.number(field::CHECKED) == Some(1.0));
        if !group.2.insert(record.key.as_str()) || group.0 > 64 || group.1 > 1 {
            return Err("UI_RADIO: duplicate, oversized or multiply selected options".into());
        }
    }
    if records
        .iter()
        .enumerate()
        .any(|(i, r)| r.tag == tag::RADIO && !groups.contains_key(&i))
    {
        return Err("UI_RADIO: group requires options".into());
    }
    Ok(records)
}

pub fn color(value: &str) -> Result<[u8; 4], String> {
    let hex = match value {
        "background" => "f6f7fb",
        "surface" => "ffffff",
        "text" => "172033",
        "textMuted" => "596579",
        "primary" => "365cdd",
        "primaryText" => "ffffff",
        "danger" => "c93434",
        "border" => "dce1ec",
        "accent" => "0b877d",
        value => value.strip_prefix('#').ok_or("UI_COLOR: unknown token")?,
    };
    if !matches!(hex.len(), 6 | 8) {
        return Err("UI_COLOR: expected #rrggbb or #rrggbbaa".into());
    }
    if !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("UI_COLOR: invalid hex".into());
    }
    let n = u32::from_str_radix(hex, 16).map_err(|_| "UI_COLOR: invalid hex")?;
    Ok(if hex.len() == 8 {
        [(n >> 24) as u8, (n >> 16) as u8, (n >> 8) as u8, n as u8]
    } else {
        [(n >> 16) as u8, (n >> 8) as u8, n as u8, 255]
    })
}

#[cfg(test)]
mod tests {
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
        for tag in [3, 5, 6, 8, 9, 10, 11, 12, 14, 15, 18] {
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
        for tag in [3, 5, 6, 8, 9, 10, 11, 12, 14, 15, 18] {
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
                assert!(
                    decode(json!([0, -1, "", "", "", 1, id, value]).as_array().unwrap()).is_err()
                );
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
                assert!(
                    decode(json!([0, -1, "", "", "", 1, id, value]).as_array().unwrap()).is_ok()
                );
            }
            for value in [
                json!(-1),
                json!(max + 1),
                json!("4"),
                json!(true),
                Value::Null,
            ] {
                assert!(
                    decode(json!([0, -1, "", "", "", 1, id, value]).as_array().unwrap()).is_err()
                );
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
        for value in [json!("#12345680"), json!("textMuted")] {
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
    fn current_color_is_only_accepted_for_borders() {
        for field in [5, 6] {
            assert!(
                decode(
                    json!([0, -1, "", "", "", 1, field, "currentColor"])
                        .as_array()
                        .unwrap()
                )
                .is_err()
            );
        }
        for value in ["currentColor", "#123456", "#12345680"] {
            assert!(decode(json!([0, -1, "", "", "", 1, 66, value]).as_array().unwrap()).is_ok());
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
}
