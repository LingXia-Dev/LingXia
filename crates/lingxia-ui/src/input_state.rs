use crate::{
    DisplayList, Renderer,
    schema::{Record, identities},
    text_input::{Edit, Editor, Snapshot},
};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

const MAX_EDITORS: usize = 128;
const MAX_EDITOR_BYTES: usize = 1024 * 1024;

#[derive(Debug, Serialize)]
pub struct InputReply {
    pub token: u64,
    pub state: Snapshot,
    pub needs_view_render: bool,
}

#[derive(Clone)]
struct Entry {
    editor: Editor,
    enabled: bool,
    visible: bool,
    autofocus: bool,
    on_input: Option<u64>,
    on_submit: Option<u64>,
    scroll_offset: [f32; 2],
}

#[derive(Clone, Default)]
pub(crate) struct Inputs {
    entries: HashMap<String, Entry>,
    focused: Option<String>,
    token: u64,
}

impl Inputs {
    pub fn focused(&self) -> Option<&str> {
        self.focused.as_deref()
    }

    pub fn invalidate_after(&mut self, previous: &Self) -> Result<(), String> {
        self.token = self.token.max(previous.token);
        self.advance()
    }
    pub fn clear(&mut self) {
        self.entries.clear();
        self.focused = None;
        self.token = self.token.saturating_add(1);
    }

    fn advance(&mut self) -> Result<(), String> {
        self.token = self
            .token
            .checked_add(1)
            .ok_or("UI_INPUT: exhausted focus tokens")?;
        Ok(())
    }

    pub fn reconcile(&mut self, records: &[Record], renderer: &mut Renderer) -> Result<(), String> {
        let ids = identities(records);
        let wanted: HashSet<_> = records
            .iter()
            .zip(&ids)
            .filter(|(r, _)| matches!(r.tag, 8 | 9 | 14))
            .map(|(_, id)| id.clone())
            .collect();
        if wanted.len() > MAX_EDITORS {
            return Err("UI_INPUT: too many editors".into());
        }
        let projected: usize = records
            .iter()
            .zip(&ids)
            .filter(|(r, _)| matches!(r.tag, 8 | 9 | 14))
            .map(|(r, id)| {
                let value = r.string(21).unwrap_or("");
                self.entries
                    .get(id)
                    .map(|e| {
                        e.editor
                            .projected_value_bytes(value, r.number(9) != Some(0.0))
                    })
                    .unwrap_or(value.len() * 3)
            })
            .sum();
        if projected > MAX_EDITOR_BYTES {
            return Err("UI_INPUT: editor text budget exceeded".into());
        }
        self.entries.retain(|id, _| wanted.contains(id));
        if self.focused.as_ref().is_some_and(|id| !wanted.contains(id)) {
            self.focused = None;
            self.advance()?;
        }
        for (record, id) in records
            .iter()
            .zip(ids)
            .filter(|(r, _)| matches!(r.tag, 8 | 9 | 14))
        {
            let value = record.string(21).unwrap_or("");
            if !self.entries.contains_key(&id) {
                self.entries.insert(
                    id.clone(),
                    Entry {
                        editor: Editor::new(value).map_err(|e| format!("UI_INPUT: {e:?}"))?,
                        enabled: true,
                        visible: false,
                        autofocus: record.number(25) == Some(1.0),
                        on_input: None,
                        on_submit: None,
                        scroll_offset: [0.0; 2],
                    },
                );
            }
            let entry = self.entries.get_mut(&id).unwrap();
            entry.enabled = record.number(9) != Some(0.0);
            entry.on_input = record
                .fields
                .iter()
                .find(|(id, _)| *id == 26)
                .and_then(|(_, v)| v.as_u64());
            entry.on_submit = record
                .fields
                .iter()
                .find(|(id, _)| *id == 27)
                .and_then(|(_, v)| v.as_u64());
            renderer.edit_text(&mut entry.editor, &[Edit::Value(value.into())])?;
            if !entry.enabled && self.focused.as_deref() == Some(&id) {
                self.blur(renderer)?;
            }
        }
        self.check_budget()
    }

    pub fn apply_values(&self, records: &mut [Record]) {
        let ids = identities(records);
        for (record, id) in records.iter_mut().zip(ids) {
            if let Some(entry) = self.entries.get(&id) {
                let value = serde_json::Value::String(entry.editor.snapshot().text);
                if let Some((_, old)) = record.fields.iter_mut().find(|(field, _)| *field == 21) {
                    *old = value;
                } else {
                    record.fields.push((21, value));
                }
            }
        }
    }

    pub fn apply_styles(&self, records: &mut [Record]) -> Result<(), String> {
        let ids = identities(records);
        for (record, id) in records.iter_mut().zip(ids) {
            if self.focused.as_deref() != Some(&id) {
                continue;
            }
            if let Some((_, value)) = record.fields.iter().find(|(field, _)| *field == 98) {
                let overrides = crate::schema::focus_style(value)?;
                for (field, value) in overrides {
                    if let Some((_, old)) = record.fields.iter_mut().find(|(id, _)| *id == field) {
                        *old = value;
                    } else {
                        record.fields.push((field, value));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn decorate(
        &mut self,
        display: &mut DisplayList,
        renderer: &mut Renderer,
    ) -> Result<(), String> {
        for entry in self.entries.values_mut() {
            entry.visible = false;
        }
        for node in &display.nodes {
            if let Some(entry) = self.entries.get_mut(&node.identity) {
                entry.visible = node.visible_bounds[2] > 0.0 && node.visible_bounds[3] > 0.0;
            }
        }
        if self
            .focused
            .as_ref()
            .is_some_and(|id| !self.entries[id].visible)
        {
            self.blur(renderer)?;
        }
        for node in &display.nodes {
            if let Some(entry) = self.entries.get_mut(&node.identity)
                && entry.autofocus
                && entry.visible
            {
                entry.autofocus = false;
                if self.focused.is_none() {
                    self.focus(&node.identity, renderer)?;
                }
            }
        }
        for node in &mut display.nodes {
            if let (Some(input), Some(entry)) = (&mut node.input, self.entries.get(&node.identity))
            {
                let snapshot = entry.editor.snapshot();
                input.scroll_offset = if snapshot.focused {
                    entry.scroll_offset
                } else {
                    [0.0; 2]
                };
                input.edit = Some(snapshot);
                input.edit_token =
                    (self.focused.as_deref() == Some(&node.identity)).then_some(self.token);
            }
        }
        display.decorate_inputs();
        for node in &display.nodes {
            if let (Some(input), Some(entry)) = (&node.input, self.entries.get_mut(&node.identity))
            {
                entry.scroll_offset = if input.geometry.is_some() {
                    input.scroll_offset
                } else {
                    [0.0; 2]
                };
            }
        }
        Ok(())
    }

    pub fn needs_layout(&self, display: &DisplayList) -> bool {
        display.nodes.iter().any(|node| {
            node.input.as_ref().is_some_and(|input| {
                self.entries
                    .get(&node.identity)
                    .is_some_and(|entry| entry.editor.snapshot().text != input.value)
            })
        })
    }

    pub fn focus(&mut self, id: &str, renderer: &mut Renderer) -> Result<bool, String> {
        if !self.entries.get(id).is_some_and(|e| e.enabled && e.visible) {
            return Ok(false);
        }
        if self.focused.as_deref() == Some(id) {
            return Ok(true);
        }
        self.blur(renderer)?;
        self.advance()?;
        renderer.edit_text(
            &mut self.entries.get_mut(id).unwrap().editor,
            &[Edit::Focus],
        )?;
        self.focused = Some(id.into());
        Ok(true)
    }

    pub fn blur(&mut self, renderer: &mut Renderer) -> Result<(), String> {
        if let Some(id) = self.focused.take() {
            if let Some(entry) = self.entries.get_mut(&id) {
                renderer.edit_text(&mut entry.editor, &[Edit::Blur])?;
            }
            self.advance()?;
        }
        Ok(())
    }

    pub fn valid(&self, id: &str, token: u64) -> bool {
        token != 0 && token == self.token && self.focused.as_deref() == Some(id)
    }
    pub fn reply(&self, id: &str, needs_view_render: bool) -> InputReply {
        InputReply {
            token: self.token,
            state: self.entries[id].editor.snapshot(),
            needs_view_render,
        }
    }
    pub fn handler(&self, id: &str, submit: bool) -> Option<u64> {
        self.entries
            .get(id)
            .and_then(|e| if submit { e.on_submit } else { e.on_input })
    }
    pub fn edit(
        &mut self,
        id: &str,
        edits: &[Edit],
        renderer: &mut Renderer,
    ) -> Result<Option<String>, String> {
        if edits
            .iter()
            .any(|e| matches!(e, Edit::Focus | Edit::Blur | Edit::Value(_)))
        {
            return Err("UI_INPUT: host edit cannot change control state".into());
        }
        let change = renderer.edit_text(
            &mut self
                .entries
                .get_mut(id)
                .ok_or("UI_INPUT: missing editor")?
                .editor,
            edits,
        )?;
        self.check_budget()?;
        Ok(change.input)
    }
    fn check_budget(&self) -> Result<(), String> {
        if self
            .entries
            .values()
            .map(|e| e.editor.byte_len())
            .sum::<usize>()
            > MAX_EDITOR_BYTES
        {
            Err("UI_INPUT: editor text budget exceeded".into())
        } else {
            Ok(())
        }
    }
}
