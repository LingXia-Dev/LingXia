//! The record walk: header, parent links, fields and page-wide budgets.
use super::{FIELDS, Record, TAGS, admission, field, limits, tag, values};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// Page-wide state the walk accumulates across records.
#[derive(Default)]
struct Page {
    animation_bytes: usize,
    item_key_bytes: usize,
    item_keys: HashMap<usize, Vec<String>>,
    lists: HashSet<String>,
}

/// A decoded buffer the next one may reference: its serial, records, and for each wire offset of
/// a node header the record it became, with each record's depth.
#[derive(Default)]
pub struct Wire {
    serial: u64,
    records: Vec<Record>,
    offsets: HashMap<i64, usize>,
    depths: Vec<usize>,
}

impl Wire {
    /// The buffer's serial; 0 when nothing may reference it.
    pub fn serial(&self) -> u64 {
        self.serial
    }
}

/// A buffer's trailing serial header, and a header standing for nodes of the previous buffer.
const WIRE_SERIAL: i64 = -3;
const WIRE_REUSE: i64 = -2;

pub fn decode(buffer: &[Value]) -> Result<Vec<Record>, String> {
    decode_wire(buffer, None).map(|(records, _)| records)
}

/// Decodes `buffer`, whose reused subtrees may stand as references into `previous`; also
/// returns what the next buffer may reference.
pub fn decode_wire(
    buffer: &[Value],
    previous: Option<&Wire>,
) -> Result<(Vec<Record>, Wire), String> {
    let mut page = Page::default();
    let mut serial = 0;
    let mut records: Vec<Record> = Vec::new();
    let mut offsets: HashMap<i64, usize> = HashMap::new();
    let mut keys = HashSet::new();
    let mut cursor = 0;
    let mut depths = Vec::new();
    if buffer.len() > limits::WIRE_VALUES {
        return Err("UI_LIMIT: buffer too large".into());
    }
    while cursor < buffer.len() {
        if records.len() >= limits::NODES {
            return Err("UI_LIMIT: too many nodes".into());
        }
        let start = cursor;
        let header = buffer
            .get(cursor..cursor + 6)
            .ok_or("UI_BUFFER: truncated header")?;
        match header[0].as_i64() {
            Some(WIRE_SERIAL) => {
                serial = header[1].as_u64().ok_or("UI_BUFFER: invalid serial")?;
                cursor += 6;
                continue;
            }
            Some(WIRE_REUSE) => {
                reuse(
                    header,
                    start,
                    previous,
                    &mut page,
                    &mut records,
                    &mut offsets,
                    &mut keys,
                    &mut depths,
                )?;
                cursor += 6;
                continue;
            }
            _ => {}
        }
        let tag = header[0]
            .as_u64()
            .filter(|v| *v < TAGS.len() as u64)
            .ok_or("UI_TAG: unknown tag id")? as usize;
        let parent_offset = header[1]
            .as_i64()
            .ok_or("UI_PARENT: expected a parent offset")?;
        let parent = if records.is_empty() && parent_offset == -1 {
            None
        } else {
            let index = *offsets
                .get(&parent_offset)
                .ok_or("UI_PARENT: must precede child")?;
            admission::child(&records, index, tag)?;
            Some(index)
        };
        let parent_tag = parent.map(|p| records[p].tag);
        let key = header[2]
            .as_str()
            .ok_or("UI_KEY: expected a string key")?
            .to_owned();
        let depth = parent.map(|p| depths[p] + 1).unwrap_or(0);
        if depth > limits::DEPTH {
            return Err("UI_LIMIT: tree too deep".into());
        }
        depths.push(depth);
        if !key.is_empty() && !keys.insert((parent, key.clone())) {
            return Err("UI_KEY: duplicate sibling".into());
        }
        let test_id = header[3]
            .as_str()
            .ok_or("UI_TEST_ID: expected a string test id")?
            .to_owned();
        let text = header[4]
            .as_str()
            .ok_or("UI_TEXT: expected string text")?
            .to_owned();
        if text.len() > limits::TEXT_BYTES
            || test_id.len() > limits::IDENTIFIER_BYTES
            || key.len() > limits::IDENTIFIER_BYTES
        {
            return Err("UI_LIMIT: string too large".into());
        }
        admission::text(tag, &text)?;
        let count = header[5]
            .as_u64()
            .filter(|v| *v <= FIELDS.len() as u64)
            .ok_or("UI_FIELDS: field count out of range")? as usize;
        cursor += 6;
        let pairs = buffer
            .get(cursor..cursor + count * 2)
            .ok_or("UI_BUFFER: truncated fields")?;
        let mut fields = Vec::new();
        for pair in pairs.chunks_exact(2) {
            let id = pair[0]
                .as_u64()
                .filter(|v| *v < FIELDS.len() as u64)
                .ok_or("UI_FIELD: unknown field id")? as usize;
            if fields.iter().any(|(old, _)| *old == id) {
                return Err("UI_FIELD: duplicate".into());
            }
            admission::field(tag, parent_tag, id)?;
            let value = &pair[1];
            match id {
                field::LIST_ITEM_KEYS => page.item_keys(records.len(), value)?,
                field::ANIMATION => page.animation(value)?,
                field::LIST_ID => page.list(value)?,
                field::LIST_ROW_INDEX => {
                    let count = parent
                        .and_then(|p| records[p].number(field::LIST_COUNT))
                        .unwrap_or(0.0) as u64;
                    value
                        .as_u64()
                        .filter(|n| *n < count)
                        .ok_or("UI_LIST: invalid row index")?;
                }
                _ => values::check(tag, id, value)?,
            }
            fields.push((id, value.clone()));
        }
        admission::record(tag, parent_tag, &key, &fields)?;
        page.rows(records.len(), parent, &key, &fields)?;
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
    budgets(&records)?;
    let wire = Wire {
        serial,
        records: if serial == 0 {
            Vec::new()
        } else {
            records.clone()
        },
        offsets,
        depths,
    };
    Ok((records, wire))
}

/// Copies the subtree a reuse header names from the previous buffer, checking what its new place
/// changes: the root's admission under its parent, sibling keys, depth and page budgets.
#[allow(clippy::too_many_arguments)]
fn reuse(
    header: &[Value],
    start: usize,
    previous: Option<&Wire>,
    page: &mut Page,
    records: &mut Vec<Record>,
    offsets: &mut HashMap<i64, usize>,
    keys: &mut HashSet<(Option<usize>, String)>,
    depths: &mut Vec<usize>,
) -> Result<(), String> {
    let stale = || String::from("UI_BUFFER: reference to a buffer the host does not hold");
    let previous = previous.ok_or_else(stale)?;
    if header[2].as_u64() != Some(previous.serial) || previous.serial == 0 {
        return Err(stale());
    }
    let from = header[3]
        .as_i64()
        .and_then(|offset| previous.offsets.get(&offset))
        .copied()
        .ok_or_else(stale)?;
    let count = header[4].as_u64().filter(|n| *n > 0).ok_or_else(stale)? as usize;
    let nodes = previous.records.get(from..from + count).ok_or_else(stale)?;
    if records.len() + count > limits::NODES {
        return Err("UI_LIMIT: too many nodes".into());
    }
    let parent_offset = header[1]
        .as_i64()
        .ok_or("UI_PARENT: expected a parent offset")?;
    let parent = *offsets
        .get(&parent_offset)
        .ok_or("UI_PARENT: must precede child")?;
    let root = &nodes[0];
    // Inside the subtree every record keeps its parent; only the root moves.
    if nodes[1..]
        .iter()
        .any(|r| r.parent.is_none_or(|p| p < from || p >= from + count))
    {
        return Err(stale());
    }
    admission::child(records, parent, root.tag)?;
    let parent_tag = Some(records[parent].tag);
    for (id, value) in &root.fields {
        admission::field(root.tag, parent_tag, *id)?;
        if *id == field::LIST_ROW_INDEX {
            let rows = records[parent].number(field::LIST_COUNT).unwrap_or(0.0) as u64;
            value
                .as_u64()
                .filter(|n| *n < rows)
                .ok_or("UI_LIST: invalid row index")?;
        }
    }
    admission::record(root.tag, parent_tag, &root.key, &root.fields)?;
    let base = records.len();
    page.rows(base, Some(parent), &root.key, &root.fields)?;
    if !root.key.is_empty() && !keys.insert((Some(parent), root.key.clone())) {
        return Err("UI_KEY: duplicate sibling".into());
    }
    let shift = depths[parent] + 1;
    let first = previous.depths[from];
    for (i, record) in nodes.iter().enumerate() {
        let depth = previous.depths[from + i] - first + shift;
        if depth > limits::DEPTH {
            return Err("UI_LIMIT: tree too deep".into());
        }
        depths.push(depth);
        for (id, value) in &record.fields {
            if *id == field::ANIMATION {
                page.animation(value)?;
            }
        }
        let mut copy = record.clone();
        copy.parent = Some(if i == 0 {
            parent
        } else {
            record.parent.unwrap() - from + base
        });
        if i > 0 && !copy.key.is_empty() {
            keys.insert((copy.parent, copy.key.clone()));
        }
        records.push(copy);
    }
    offsets.insert(start as i64, base);
    Ok(())
}

impl Page {
    fn item_keys(&mut self, record: usize, value: &Value) -> Result<(), String> {
        let source = value.as_str().ok_or("UI_LIST: invalid itemKeys")?;
        self.item_key_bytes = self.item_key_bytes.saturating_add(source.len());
        if self.item_key_bytes > limits::ITEM_KEY_BYTES {
            return Err("UI_LIST: itemKeys page budget exceeded".into());
        }
        self.item_keys
            .insert(record, crate::layout::list::decode_item_keys(source)?);
        Ok(())
    }

    fn animation(&mut self, value: &Value) -> Result<(), String> {
        let source = value
            .as_str()
            .ok_or("UI_ANIMATION: expected encoded definition")?;
        self.animation_bytes = self
            .animation_bytes
            .checked_add(source.len())
            .ok_or("UI_ANIMATION: page payload budget exceeded")?;
        if self.animation_bytes > limits::ANIMATION_BYTES {
            return Err("UI_ANIMATION: page payload budget exceeded".into());
        }
        crate::animation::decode_definition(source)?;
        Ok(())
    }

    fn list(&mut self, value: &Value) -> Result<(), String> {
        let value = value
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 8192)
            .ok_or("UI_LIST: invalid identity")?;
        if !self.lists.insert(value.to_owned()) {
            return Err("UI_LIST: duplicate identity".into());
        }
        Ok(())
    }

    /// A list's `itemKeys` roster names its count and every realized row key.
    fn rows(
        &self,
        record: usize,
        parent: Option<usize>,
        key: &str,
        fields: &[(usize, Value)],
    ) -> Result<(), String> {
        if let Some(roster) = self.item_keys.get(&record) {
            let count = fields
                .iter()
                .find(|(id, _)| *id == field::LIST_COUNT)
                .and_then(|(_, v)| v.as_u64());
            if count != Some(roster.len() as u64) {
                return Err("UI_LIST: itemKeys must match count".into());
            }
        }
        if let Some(roster) = parent.and_then(|p| self.item_keys.get(&p)) {
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
        Ok(())
    }
}

/// Checks that need the whole tree: control geometry, frame budgets and radio groups.
fn budgets(records: &[Record]) -> Result<(), String> {
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
        if vector_work > limits::VECTOR_WORK {
            return Err("UI_LIMIT: vector geometry budget exceeded".into());
        }
    }
    let mut plot_work = 0;
    for record in records.iter().filter(|r| r.tag == tag::PLOT) {
        let plot =
            crate::scene::plot::Plot::from_record(record, &crate::theme::Palette::builtin(false))?;
        plot_work += plot.work();
        if plot_work > limits::PLOT_WORK {
            return Err("UI_LIMIT: plot geometry budget exceeded".into());
        }
    }
    for record in records
        .iter()
        .filter(|r| r.string(field::POSITION) == Some("sticky"))
    {
        let mut ancestor = record.parent;
        loop {
            match ancestor.map(|a| records[a].tag) {
                // A list row's sticky box pins inside its row while the row is in view.
                Some(tag::SCROLL | tag::LIST) => break,
                Some(_) => ancestor = records[ancestor.unwrap()].parent,
                None => return Err("UI_STYLE: sticky needs an ancestor scroll".into()),
            }
        }
        let percent = [
            field::INSET_TOP,
            field::INSET_RIGHT,
            field::INSET_BOTTOM,
            field::INSET_LEFT,
            field::INSET_INLINE_START,
            field::INSET_INLINE_END,
        ]
        .iter()
        .any(|id| record.string(*id).is_some_and(|v| v.ends_with('%')));
        if percent || record.fields.iter().any(|(id, _)| *id == field::TRANSFORM) {
            return Err("UI_STYLE: sticky takes pixel insets and no transform".into());
        }
    }
    let mut runs: HashMap<usize, usize> = HashMap::new();
    for record in records
        .iter()
        .filter(|r| matches!(r.tag, tag::SPAN | tag::LINK))
    {
        let count = runs.entry(record.parent.unwrap()).or_default();
        *count += 1;
        if *count > limits::RUNS {
            return Err(format!(
                "UI_TEXT: more than {} runs in one text",
                limits::RUNS
            ));
        }
    }
    let mut groups = HashMap::new();
    for record in records.iter().filter(|r| r.tag == tag::RADIO_OPTION) {
        let group = groups
            .entry(record.parent.unwrap())
            .or_insert_with(|| (0usize, 0usize, HashSet::new()));
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
    Ok(())
}
