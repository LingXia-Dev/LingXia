use serde_json::Value;

pub const TAGS: &[&str] = &[
    "column",
    "row",
    "text",
    "button",
    "spacer",
    "scroll",
    "list",
    "image",
    "input",
    "textarea",
    "checkbox",
    "switch",
    "radio",
    "radio-option",
    "search",
    "slider",
    "grid",
    "stack",
    "plot",
    "vector",
];
pub const FIELDS: &[(&str, &str)] = &[
    ("width", "number"),
    ("height", "number"),
    ("gap", "number"),
    ("padding", "number"),
    ("fontSize", "number"),
    ("color", "string"),
    ("background", "string"),
    ("grow", "number"),
    ("onClick", "handle"),
    ("enabled", "flag"),
    ("radius", "number"),
    ("listId", "list-id"),
    ("listCount", "list-count"),
    ("leading", "list-extent"),
    ("trailing", "list-extent"),
    ("rowIndex", "list-index"),
    ("anchor", "list-anchor"),
    ("onReachStart", "reach-handle"),
    ("onReachEnd", "reach-handle"),
    ("src", "image-source"),
    ("fit", "image-fit"),
    ("value", "input-text"),
    ("placeholder", "input-text"),
    ("secure", "input-flag"),
    ("keyboard", "input-keyboard"),
    ("autoFocus", "input-flag"),
    ("onInput", "input-handle"),
    ("onSubmit", "input-handle"),
    ("minWidth", "number"),
    ("minHeight", "number"),
    ("maxWidth", "number"),
    ("maxHeight", "number"),
    ("shrink", "number"),
    ("align", "alignment"),
    ("justify", "justification"),
    ("alignSelf", "alignment"),
    ("fontWeight", "number"),
    ("lineHeight", "number"),
    ("fontFamily", "font-family"),
    ("checked", "check-flag"),
    ("onChange", "check-handle"),
    ("accessibilityLabel", "radio-label"),
    ("value", "slider-number"),
    ("min", "slider-number"),
    ("max", "slider-number"),
    ("step", "slider-number"),
    ("onChange", "slider-handle"),
    ("columns", "grid-columns"),
    ("offsetX", "stack-offset"),
    ("offsetY", "stack-offset"),
    ("lines", "text-lines"),
    ("values", "plot-values"),
    ("kind", "plot-kind"),
    ("colors", "plot-colors"),
    ("smooth", "plot-flag"),
    ("area", "plot-flag"),
    ("onSelect", "plot-handle"),
    ("selectedIndex", "plot-index"),
    ("opacity", "number"),
    ("paddingTop", "number"),
    ("paddingRight", "number"),
    ("paddingBottom", "number"),
    ("paddingLeft", "number"),
    ("wordBreak", "word-break"),
    ("overflowWrap", "overflow-wrap"),
    ("borderWidth", "number"),
    ("borderColor", "string"),
    ("borderTopWidth", "number"),
    ("borderRightWidth", "number"),
    ("borderBottomWidth", "number"),
    ("borderLeftWidth", "number"),
    ("textAlign", "text-alignment"),
    ("backgroundGradient", "gradient"),
    ("boxShadow", "shadow"),
    ("widthPercent", "dimension-percent"),
    ("heightPercent", "dimension-percent"),
    ("minWidthPercent", "dimension-percent"),
    ("minHeightPercent", "dimension-percent"),
    ("maxWidthPercent", "dimension-percent"),
    ("maxHeightPercent", "dimension-percent"),
    ("vector", "vector-source"),
    ("textDecorationLine", "text-decoration-line"),
    ("heightAuto", "dimension-auto"),
    ("submitOnEnter", "input-flag"),
    ("fontStyle", "font-style"),
    ("domainMin", "plot-domain"),
    ("domainMax", "plot-domain"),
    ("flexWrap", "flex-wrap"),
    ("pieLayout", "plot-layout"),
    ("letterSpacing", "number"),
    ("radiusTopLeft", "number"),
    ("radiusTopRight", "number"),
    ("radiusBottomRight", "number"),
    ("radiusBottomLeft", "number"),
    ("markerEmphasisScale", "plot-marker-scale"),
    ("backdropBlur", "number"),
    ("backdropSaturation", "number"),
    ("placeholderColor", "string"),
    ("focusStyle", "focus-style"),
    ("flexBasis", "flex-basis"),
    ("scrollAxis", "scroll-axis"),
    ("animation", "animation"),
    ("itemKeys", "list-keys"),
    ("flexDirection", "flex-direction"),
    ("display", "flex-display"),
    ("alt", "image-alt"),
    ("retryKey", "image-retry"),
    ("onLoad", "image-handle"),
    ("onError", "image-handle"),
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
        assert_eq!(records[1].number(15), Some(1.0));
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
            "../../../packages/lingxia-ui/tests/fixtures/animations.json"
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
        assert_eq!(records[0].string(101), Some(source.as_str()));
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
        || pairs.len() % 2 != 0
        || pairs
            .chunks_exact(2)
            .any(|p| !matches!(p[0].as_u64(), Some(5 | 6 | 58 | 66 | 72 | 73 | 97)))
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
            if matches!(records[index].tag, 2 | 4 | 7..=9 | 11 | 13..=15 | 18 | 19) {
                return Err("UI_CHILD: leaf node".into());
            }
            if matches!(records[index].tag, 3 | 10)
                && (!records[index].text.is_empty()
                    || records[index]
                        .string(41)
                        .is_none_or(|label| label.trim().is_empty()))
            {
                return Err(
                    "UI_BUTTON: composed button requires an explicit label and no inline text"
                        .into(),
                );
            }
            let mut ancestor = Some(index);
            while let Some(owner) = ancestor {
                if matches!(records[owner].tag, 3 | 10)
                    && !matches!(tag, 0 | 1 | 2 | 4 | 7 | 16 | 17 | 19)
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
        if !matches!(tag, 2 | 3 | 10 | 11 | 13) && !text.is_empty() {
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
            if let Some(options) = style_options(FIELDS[id].1) {
                if matches!(id, 103 | 104) && !matches!(tag, 0 | 1) {
                    return Err(
                        "UI_LAYOUT: flex direction/display requires a flex container".into(),
                    );
                }
                if id == 100 && tag != 5 {
                    return Err("UI_SCROLL: scrollAxis requires a scroll node".into());
                }
                let value = pair[1]
                    .as_str()
                    .filter(|v| options.contains(v))
                    .ok_or("UI_STYLE: invalid style option")?;
                if tag == 6 && ((id == 33 && value != "stretch") || (id == 34 && value != "start"))
                {
                    return Err(
                        "UI_LIST: rows require stretch alignment and start justification".into(),
                    );
                }
            } else if id == 102 {
                if tag != 6 {
                    return Err("UI_LIST: itemKeys requires a list".into());
                }
                let source = pair[1].as_str().ok_or("UI_LIST: invalid itemKeys")?;
                item_key_bytes = item_key_bytes.saturating_add(source.len());
                if item_key_bytes > 262_144 {
                    return Err("UI_LIST: itemKeys page budget exceeded".into());
                }
                item_keys.insert(records.len(), crate::list::decode_item_keys(source)?);
            } else if id == 101 {
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
            } else if id == 99 {
                flex_basis(&pair[1])?;
            } else if id == 98 {
                if !matches!(tag, 8 | 9 | 14) {
                    return Err("UI_STYLE: focus styles require an editor".into());
                }
                focus_style(&pair[1])?;
            } else if id == 80 {
                if tag != 19 || !pair[1].is_string() {
                    return Err("UI_VECTOR: source requires vector node".into());
                }
            } else if id == 83 {
                if tag != 9 || pair[1].as_u64().is_none_or(|v| v > 1) {
                    return Err("UI_INPUT: invalid submitOnEnter".into());
                }
            } else if id == 82 {
                if pair[1].as_u64() != Some(1) {
                    return Err("UI_STYLE: invalid automatic height".into());
                }
            } else if (74..=79).contains(&id) {
                pair[1]
                    .as_f64()
                    .filter(|v| v.is_finite() && (0.0..=16384.0).contains(v))
                    .ok_or("UI_STYLE: invalid percentage")?;
            } else if id == 73 {
                crate::shadow::BoxShadow::parse(
                    pair[1]
                        .as_str()
                        .ok_or("UI_SHADOW: expected encoded shadows")?,
                )?;
            } else if id == 72 {
                crate::gradient::LinearGradient::parse(
                    pair[1]
                        .as_str()
                        .ok_or("UI_GRADIENT: expected encoded gradient")?,
                )?;
            } else if id == 89 {
                if pair[1]
                    .as_f64()
                    .is_none_or(|v| !v.is_finite() || v.abs() > 1024.0)
                {
                    return Err("UI_STYLE: invalid letter spacing".into());
                }
            } else if id == 94 {
                if tag != 18
                    || pair[1]
                        .as_f64()
                        .is_none_or(|v| !v.is_finite() || !(1.0..=4.0).contains(&v))
                {
                    return Err("UI_PLOT: invalid marker emphasis scale".into());
                }
            } else if id == 88 {
                if tag != 18 || pair[1].as_str().is_none_or(|s| s.len() > 1024) {
                    return Err("UI_PLOT: invalid pie layout".into());
                }
            } else if matches!(id, 85 | 86) {
                if tag != 18
                    || pair[1]
                        .as_f64()
                        .is_none_or(|v| !v.is_finite() || v.abs() > 1e9)
                {
                    return Err("UI_PLOT: invalid domain bound".into());
                }
            } else if id == 56 {
                if tag != 18
                    || pair[1]
                        .as_u64()
                        .is_none_or(|v| v == 0 || v > 9_007_199_254_740_991)
                {
                    return Err("UI_PLOT: invalid selection callback".into());
                }
            } else if id == 57 {
                if tag != 18 || pair[1].as_i64().is_none_or(|v| !(-1..128).contains(&v)) {
                    return Err("UI_PLOT: invalid selected index".into());
                }
            } else if matches!(id, 54 | 55) {
                if tag != 18 || !matches!(pair[1].as_u64(), Some(0 | 1)) {
                    return Err("UI_PLOT: invalid boolean property".into());
                }
            } else if (51..=53).contains(&id) {
                if tag != 18 || pair[1].as_str().is_none_or(|s| s.len() > 8192) {
                    return Err("UI_PLOT: invalid encoded property".into());
                }
            } else if id == 50 {
                if tag != 2 || pair[1].as_u64().is_none_or(|v| !(1..=1024).contains(&v)) {
                    return Err(
                        "UI_TEXT: lines must be an integer between 1 and 1024 on text".into(),
                    );
                }
            } else if matches!(id, 48 | 49) {
                if parent.is_none_or(|p| records[p].tag != 17)
                    || pair[1]
                        .as_f64()
                        .is_none_or(|v| !v.is_finite() || v.abs() > 16384.0)
                {
                    return Err(
                        "UI_STACK: offset requires a stack parent and bounded number".into(),
                    );
                }
            } else if id == 47 {
                if tag != 16 || pair[1].as_u64().is_none_or(|v| !(1..=64).contains(&v)) {
                    return Err(
                        "UI_GRID: columns must be an integer between 1 and 64 on a grid".into(),
                    );
                }
            } else if (42..=46).contains(&id) {
                if tag != 15 {
                    return Err("UI_SLIDER: property on another tag".into());
                }
                if id == 46 {
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
            } else if id == 106 {
                if tag != 7
                    || pair[1]
                        .as_str()
                        .is_none_or(|s| s.len() > 256 || s.contains('\0'))
                {
                    return Err("UI_IMAGE: invalid retry key".into());
                }
            } else if matches!(id, 107 | 108) {
                if tag != 7
                    || pair[1]
                        .as_u64()
                        .is_none_or(|v| v == 0 || v > 9_007_199_254_740_991)
                {
                    return Err("UI_IMAGE: invalid callback".into());
                }
            } else if id == 105 {
                if tag != 7
                    || pair[1].as_str().is_none_or(|s| {
                        s.len() > 1024 || s.contains('\0') || (!s.is_empty() && s.trim().is_empty())
                    })
                {
                    return Err("UI_IMAGE: invalid alternative text".into());
                }
            } else if id == 41 {
                if !matches!(tag, 3 | 8 | 9 | 10 | 12 | 14 | 15 | 18)
                    || pair[1].as_str().is_none_or(|s| {
                        s.len() > 1024
                            || s.contains('\0')
                            || (matches!(tag, 3 | 10) && s.trim().is_empty())
                    })
                {
                    return Err("UI_RADIO: invalid accessibility label".into());
                }
            } else if matches!(id, 39 | 40) {
                if !matches!(tag, 10 | 11 | 13) {
                    return Err("UI_CHECK: property on another tag".into());
                }
                let value = pair[1].as_u64().ok_or("UI_CHECK: invalid flag or handle")?;
                if (id == 39 && value > 1)
                    || (id == 40 && (value == 0 || value > 9_007_199_254_740_991))
                {
                    return Err("UI_CHECK: invalid flag or handle".into());
                }
            } else if (11..=27).contains(&id) {
                match id {
                    21 | 22 if matches!(tag, 8 | 9 | 14) => {
                        pair[1]
                            .as_str()
                            .filter(|s| s.len() <= crate::text_input::MAX_TEXT_BYTES)
                            .ok_or("UI_INPUT: invalid text")?;
                    }
                    23 | 25 if matches!(tag, 8 | 9 | 14) && !(tag == 14 && id == 23) => {
                        pair[1]
                            .as_u64()
                            .filter(|v| *v <= 1)
                            .ok_or("UI_INPUT: invalid flag")?;
                    }
                    24 if matches!(tag, 8 | 9 | 14) => {
                        if tag == 14 && pair[1].as_str() != Some("search") {
                            return Err("UI_INPUT: search requires search keyboard".into());
                        }
                        if !matches!(
                            pair[1].as_str(),
                            Some("text" | "number" | "email" | "phone" | "search")
                        ) {
                            return Err("UI_INPUT: invalid keyboard".into());
                        }
                    }
                    26 | 27 if matches!(tag, 8 | 9 | 14) => {
                        pair[1]
                            .as_u64()
                            .filter(|v| *v > 0 && *v <= 9_007_199_254_740_991)
                            .ok_or("UI_INPUT: invalid callback")?;
                    }
                    19 if tag == 7 => {
                        pair[1]
                            .as_str()
                            .filter(|s| !s.is_empty() && s.len() <= 8192 && !s.contains('\0'))
                            .ok_or("UI_IMAGE: invalid source")?;
                    }
                    20 if tag == 7 => {
                        if !matches!(pair[1].as_str(), Some("cover" | "contain" | "fill")) {
                            return Err("UI_IMAGE: invalid fit".into());
                        }
                    }
                    17 | 18 if matches!(tag, 5 | 6) => {
                        pair[1]
                            .as_u64()
                            .filter(|v| *v > 0 && *v <= 9_007_199_254_740_991)
                            .ok_or("UI_SCROLL: invalid endpoint handle")?;
                    }
                    11 if tag == 6 => {
                        let value = pair[1]
                            .as_str()
                            .filter(|s| !s.is_empty() && s.len() <= 8192)
                            .ok_or("UI_LIST: invalid identity")?;
                        if !lists.insert(value.to_owned()) {
                            return Err("UI_LIST: duplicate identity".into());
                        }
                    }
                    12 if tag == 6 => {
                        pair[1]
                            .as_u64()
                            .filter(|n| *n <= crate::list::MAX_ROWS as u64)
                            .ok_or("UI_LIST: invalid count")?;
                    }
                    13 | 14 if tag == 6 => {
                        pair[1]
                            .as_f64()
                            .filter(|v| v.is_finite() && *v >= 0.0 && *v <= 1_638_400_000.0)
                            .ok_or("UI_LIST: invalid extent")?;
                    }
                    15 if tag == 0 && parent.is_some_and(|p| records[p].tag == 6) => {
                        let count = records[parent.unwrap()].number(12).unwrap_or(0.0) as u64;
                        pair[1]
                            .as_u64()
                            .filter(|n| *n < count)
                            .ok_or("UI_LIST: invalid row index")?;
                    }
                    16 if tag == 6 => {
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
            } else if id == 8 || id == 9 {
                if tag != 3 && !(id == 9 && matches!(tag, 8..=15)) {
                    return Err("UI_PROP: button property on another tag".into());
                }
                let value = pair[1].as_u64().ok_or("UI_PROP: invalid handle or flag")?;
                if (id == 8 && (value == 0 || value > 9_007_199_254_740_991))
                    || (id == 9 && value > 1)
                {
                    return Err("UI_PROP: invalid handle or flag".into());
                }
            } else if FIELDS[id].1 == "number" {
                let value = pair[1]
                    .as_f64()
                    .filter(|v| v.is_finite() && *v >= 0.0 && *v <= 16384.0)
                    .ok_or("UI_STYLE: invalid length")?;
                if matches!(id, 4 | 37) && value == 0.0 {
                    return Err("UI_STYLE: font size and line height must be positive".into());
                }
                if (id == 95 && value > 64.0) || (id == 96 && value > 16.0) {
                    return Err("UI_STYLE: invalid backdrop filter".into());
                }
                if id == 58 && value > 1.0 {
                    return Err("UI_STYLE: opacity must be between 0 and 1".into());
                }
                if id == 36 && !(1.0..=1000.0).contains(&value) {
                    return Err("UI_STYLE: font weight must be between 1 and 1000".into());
                }
            } else {
                let value = pair[1].as_str().ok_or("UI_STYLE: expected color")?;
                if id != 66 || value != "currentColor" {
                    color(value)?;
                }
            }
            fields.push((id, pair[1].clone()));
        }
        for (index, absolute) in [0, 1, 28, 29, 30, 31].into_iter().enumerate() {
            if fields.iter().any(|(id, _)| *id == absolute)
                && fields.iter().any(|(id, _)| *id == 74 + index)
            {
                return Err("UI_STYLE: conflicting absolute and percentage dimensions".into());
            }
        }
        if fields.iter().any(|(id, _)| *id == 82)
            && fields.iter().any(|(id, _)| matches!(*id, 1 | 75))
        {
            return Err("UI_STYLE: conflicting automatic height".into());
        }
        if (tag == 13) != parent.is_some_and(|p| records[p].tag == 12) {
            return Err(
                "UI_RADIO: options require a radio parent, groups accept only options".into(),
            );
        }
        if tag == 13 && (key.is_empty() || !fields.iter().any(|(id, _)| *id == 39)) {
            return Err("UI_RADIO: option requires a key and checked flag".into());
        }
        if tag == 16 && !fields.iter().any(|(id, _)| *id == 47) {
            return Err("UI_GRID: missing columns".into());
        }
        if tag == 7 && !fields.iter().any(|(id, _)| *id == 19) {
            return Err("UI_IMAGE: missing source".into());
        }
        if tag == 6
            && ![11, 12, 13, 14]
                .iter()
                .all(|required| fields.iter().any(|(id, _)| id == required))
        {
            return Err("UI_LIST: missing geometry".into());
        }
        if parent.is_some_and(|p| records[p].tag == 6) && !fields.iter().any(|(id, _)| *id == 15) {
            return Err("UI_LIST: expected row wrapper".into());
        }
        if let Some(roster) = item_keys.get(&records.len()) {
            let count = fields
                .iter()
                .find(|(id, _)| *id == 12)
                .and_then(|(_, v)| v.as_u64());
            if count != Some(roster.len() as u64) {
                return Err("UI_LIST: itemKeys must match count".into());
            }
        }
        if let Some(roster) = parent.and_then(|p| item_keys.get(&p)) {
            let row = fields
                .iter()
                .find(|(id, _)| *id == 15)
                .and_then(|(_, v)| v.as_u64())
                .ok_or("UI_LIST: missing row index")? as usize;
            if roster
                .get(row)
                .is_none_or(|item| crate::list::item_key(item) != key)
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
    for record in records.iter().filter(|r| r.tag == 15) {
        crate::SliderSnapshot::from_record(record)?;
    }
    let mut vector_work = 0;
    for record in records.iter().filter(|r| r.tag == 19) {
        let vector =
            crate::vector::Vector::parse(record.string(80).ok_or("UI_VECTOR: missing source")?)?;
        vector_work += vector.work;
        if vector_work > 32768 {
            return Err("UI_LIMIT: vector geometry budget exceeded".into());
        }
    }
    let mut plot_work = 0;
    for record in records.iter().filter(|r| r.tag == 18) {
        let plot = crate::plot::Plot::from_record(record)?;
        plot_work += plot.work();
        if plot_work > 16384 {
            return Err("UI_LIMIT: plot geometry budget exceeded".into());
        }
    }
    for record in records.iter().filter(|r| r.tag == 13) {
        let group = groups
            .entry(record.parent.unwrap())
            .or_insert_with(|| (0usize, 0usize, std::collections::HashSet::new()));
        group.0 += 1;
        group.1 += usize::from(record.number(39) == Some(1.0));
        if !group.2.insert(record.key.as_str()) || group.0 > 64 || group.1 > 1 {
            return Err("UI_RADIO: duplicate, oversized or multiply selected options".into());
        }
    }
    if records
        .iter()
        .enumerate()
        .any(|(i, r)| r.tag == 12 && !groups.contains_key(&i))
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

fn style_options(kind: &str) -> Option<&'static [&'static str]> {
    match kind {
        "font-family" => Some(&["sans", "mono"]),
        "font-style" => Some(&["normal", "italic"]),
        "flex-wrap" => Some(&["nowrap", "wrap", "wrap-reverse"]),
        "flex-direction" => Some(&["row", "column", "row-reverse", "column-reverse"]),
        "flex-display" => Some(&["flex"]),
        "scroll-axis" => Some(&["horizontal", "vertical"]),
        "word-break" => Some(&["normal", "break-all", "keep-all"]),
        "overflow-wrap" => Some(&["normal", "break-word", "anywhere"]),
        "text-decoration-line" => Some(&["none", "line-through"]),
        "text-alignment" => Some(&["start", "end", "left", "center", "right", "justify"]),
        "alignment" => Some(&["start", "center", "end", "stretch", "baseline"]),
        "justification" => Some(&["start", "center", "end", "between"]),
        _ => None,
    }
}

pub fn javascript_schema() -> String {
    let tags: serde_json::Map<String, Value> = TAGS
        .iter()
        .enumerate()
        .map(|(i, n)| (n.to_string(), i.into()))
        .collect();
    let fields: serde_json::Map<String, Value> = FIELDS
        .iter()
        .enumerate()
        .filter(|(_, (_, kind))| {
            matches!(
                *kind,
                "number"
                    | "string"
                    | "alignment"
                    | "justification"
                    | "font-family"
                    | "font-style"
                    | "flex-wrap"
                    | "flex-direction"
                    | "flex-display"
                    | "flex-basis"
                    | "scroll-axis"
                    | "animation"
                    | "word-break"
                    | "overflow-wrap"
                    | "text-alignment"
                    | "text-decoration-line"
                    | "gradient"
                    | "shadow"
                    | "stack-offset"
            )
        })
        .map(|(i, (n, _))| (n.to_string(), i.into()))
        .collect();
    let options: serde_json::Map<String, Value> = FIELDS
        .iter()
        .filter_map(|(name, kind)| {
            style_options(kind).map(|options| (name.to_string(), serde_json::json!(options)))
        })
        .collect();
    let props: serde_json::Map<String, Value> = FIELDS
        .iter()
        .enumerate()
        .filter(|(_, (_, kind))| matches!(*kind, "handle" | "flag" | "radio-label"))
        .map(|(i, (n, _))| (n.to_string(), i.into()))
        .collect();
    let internal: serde_json::Map<String, Value> = FIELDS
        .iter()
        .enumerate()
        .filter(|(_, (_, kind))| kind.starts_with("list-"))
        .map(|(i, (name, _))| (name.to_string(), i.into()))
        .collect();
    let reaches: serde_json::Map<String, Value> = FIELDS
        .iter()
        .enumerate()
        .filter(|(_, (_, kind))| *kind == "reach-handle")
        .map(|(i, (name, _))| (name.to_string(), i.into()))
        .collect();
    let images: serde_json::Map<String, Value> = FIELDS
        .iter()
        .enumerate()
        .filter(|(_, (_, kind))| kind.starts_with("image-"))
        .map(|(i, (name, _))| (name.to_string(), i.into()))
        .collect();
    let inputs: serde_json::Map<String, Value> = FIELDS
        .iter()
        .enumerate()
        .filter(|(_, (_, kind))| kind.starts_with("input-"))
        .map(|(i, (name, _))| (name.to_string(), i.into()))
        .collect();
    let radio: serde_json::Map<String, Value> = FIELDS
        .iter()
        .enumerate()
        .filter(|(_, (_, kind))| kind.starts_with("radio-"))
        .map(|(i, (name, _))| (name.to_string(), i.into()))
        .collect();
    let sliders: serde_json::Map<String, Value> = FIELDS
        .iter()
        .enumerate()
        .filter(|(_, (_, kind))| kind.starts_with("slider-") || *kind == "radio-label")
        .map(|(i, (name, _))| (name.to_string(), i.into()))
        .collect();
    let grid: serde_json::Map<String, Value> = FIELDS
        .iter()
        .enumerate()
        .filter(|(_, (_, kind))| kind.starts_with("grid-"))
        .map(|(i, (name, _))| (name.to_string(), i.into()))
        .collect();
    let text: serde_json::Map<String, Value> = FIELDS
        .iter()
        .enumerate()
        .filter(|(_, (_, kind))| kind.starts_with("text-"))
        .map(|(i, (name, _))| (name.to_string(), i.into()))
        .collect();
    let checks: serde_json::Map<String, Value> = FIELDS
        .iter()
        .enumerate()
        .filter(|(_, (_, kind))| kind.starts_with("check-"))
        .map(|(i, (name, _))| {
            (
                (if *name == "checked" { "value" } else { name }).to_string(),
                i.into(),
            )
        })
        .collect();
    format!(
        "// Generated by lingxia-ui; do not edit.\nexport const tags = {};\nexport const fields = {};\nexport const vectorField = 80;\nexport const dimensionPercentFields = {{\"width\":74,\"height\":75,\"minWidth\":76,\"minHeight\":77,\"maxWidth\":78,\"maxHeight\":79}};\nexport const styleOptions = {};\nexport const buttonProps = {};\nexport const listFields = {};\nexport const reachProps = {};\nexport const imageProps = {};\nexport const inputProps = {};\nexport const checkProps = {};\nexport const radioProps = {};\nexport const sliderProps = {};\nexport const gridProps = {};\nexport const textProps = {};\nexport const plotProps = {{\"values\":51,\"kind\":52,\"colors\":53,\"accessibilityLabel\":41,\"smooth\":54,\"area\":55,\"onSelect\":56,\"selectedIndex\":57,\"domainMin\":85,\"domainMax\":86,\"pieLayout\":88,\"markerEmphasisScale\":94}};\n",
        Value::Object(tags),
        Value::Object(fields),
        Value::Object(options),
        Value::Object(props),
        Value::Object(internal),
        Value::Object(reaches),
        Value::Object(images),
        Value::Object(inputs),
        Value::Object(checks),
        Value::Object(radio),
        Value::Object(sliders),
        Value::Object(grid),
        Value::Object(text)
    )
}

pub fn typescript_schema() -> String {
    let fields = FIELDS
        .iter()
        .filter(|(_, kind)| {
            matches!(
                *kind,
                "number"
                    | "string"
                    | "alignment"
                    | "justification"
                    | "font-family"
                    | "font-style"
                    | "flex-wrap"
                    | "flex-direction"
                    | "flex-display"
                    | "flex-basis"
                    | "scroll-axis"
                    | "word-break"
                    | "overflow-wrap"
                    | "text-alignment"
                    | "text-decoration-line"
                    | "gradient"
                    | "shadow"
            )
        })
        .map(|(name, ty)| {
            let ty = style_options(ty)
                .map(|values| {
                    values
                        .iter()
                        .map(|v| format!("\"{v}\""))
                        .collect::<Vec<_>>()
                        .join(" | ")
                })
                .unwrap_or_else(|| {
                    if *name == "height" || *name == "flexBasis" {
                        "number | `${number}%` | \"auto\"".to_string()
                    } else if [
                        "width",
                        "height",
                        "minWidth",
                        "minHeight",
                        "maxWidth",
                        "maxHeight",
                    ]
                    .contains(name)
                    {
                        "number | `${number}%`".to_string()
                    } else if *ty == "shadow" {
                        "ReadonlyArray<BoxShadow> | \"none\"".to_string()
                    } else if *ty == "gradient" {
                        "LinearGradient | \"none\"".to_string()
                    } else {
                        ty.to_string()
                    }
                });
            format!("  {name}?: {ty};\n")
        })
        .collect::<String>();
    format!(
        "// Generated by lingxia-ui; do not edit.\nexport interface BoxShadow {{ offsetX?: number; offsetY?: number; blur?: number; spread?: number; color?: string }}\nexport interface LinearGradient {{ direction?: \"top\" | \"right\" | \"bottom\" | \"left\" | \"top-right\" | \"bottom-right\" | \"bottom-left\" | \"top-left\"; angle?: number; colorSpace?: \"srgb\" | \"oklab\"; stops: ReadonlyArray<{{ offset: number; color: string }}> }}\nexport interface Style {{\n  offset?: {{ x?: number; y?: number }};\n{fields}}}\n"
    )
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
    fn generated_schema_has_not_drifted() {
        assert_eq!(
            include_str!("../../../packages/lingxia-ui/src/schema.js").replace("\r\n", "\n"),
            javascript_schema()
        );
        assert_eq!(
            include_str!("../../../packages/lingxia-ui/src/generated/style.d.ts")
                .replace("\r\n", "\n"),
            typescript_schema()
        );
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
