//! The record walk: header, parent links, fields and page-wide budgets.
use super::{FIELDS, Record, TAGS, admission, field, limits, tag, values};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

pub(crate) const MAX_BUFFER_VALUES: usize = 100_000;

/// Page-wide state the walk accumulates across records.
#[derive(Default)]
struct Page {
    animation_bytes: usize,
    item_key_bytes: usize,
    item_keys: HashMap<usize, Vec<String>>,
    lists: HashSet<String>,
}

pub fn decode(buffer: &[Value]) -> Result<Vec<Record>, String> {
    let mut page = Page::default();
    let mut records: Vec<Record> = Vec::new();
    let mut offsets: HashMap<i64, usize> = HashMap::new();
    let mut keys = HashSet::new();
    let mut cursor = 0;
    let mut depths = Vec::new();
    if buffer.len() > MAX_BUFFER_VALUES {
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
            admission::child(&records, index, tag)?;
            Some(index)
        };
        let parent_tag = parent.map(|p| records[p].tag);
        let key = header[2].as_str().ok_or("UI_KEY")?.to_owned();
        let depth = parent.map(|p| depths[p] + 1).unwrap_or(0);
        if depth > limits::DEPTH {
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
        admission::text(tag, &text)?;
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
    Ok(records)
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
