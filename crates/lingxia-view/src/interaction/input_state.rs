use crate::protocol::{field, tag};
use crate::{
    DisplayList, Renderer,
    protocol::{Record, identities},
    text::input::{Edit, Editor, Snapshot},
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
    /// `enterKeyHint` next/previous moved focus: token and state belong to this editor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub moved_to: Option<String>,
}

#[derive(Clone)]
struct Entry {
    editor: Editor,
    enabled: bool,
    visible: bool,
    autofocus: bool,
    /// A `focus()` request moves focus here even from another editor.
    refocus: bool,
    focus_request: u64,
    /// Record order, which is reading order.
    order: usize,
    enter_key_hint: Option<String>,
    on_input: Option<u64>,
    on_submit: Option<u64>,
    scroll_offset: [f32; 2],
    rules: crate::interaction::input_rules::Rules,
}

#[derive(Clone, Default)]
pub(crate) struct Inputs {
    entries: HashMap<String, Entry>,
    focused: Option<String>,
    token: u64,
    /// Values a blur or submit committed (bounds, step), for the session to report as input.
    committed: Vec<(u64, String)>,
    /// The one static-text selection a page may hold; editors never share it.
    pub(crate) text_selection: Option<TextSelection>,
    /// The platform ended editing: a focus the page makes itself (autoFocus,
    /// `focus()`) shows no keyboard until the user focuses an editor again.
    keyboard_held: bool,
}

/// A static-text selection: byte offsets into the node's display text.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TextSelection {
    pub identity: String,
    pub text: String,
    pub anchor: usize,
    pub focus: usize,
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
        self.keyboard_held = false;
    }
    /// The platform blurred (`true`) or the user focused (`false`) an editor.
    pub fn hold_keyboard(&mut self, held: bool) {
        self.keyboard_held = held;
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
            .filter(|(r, _)| matches!(r.tag, tag::INPUT | tag::TEXTAREA | tag::SEARCH))
            .map(|(_, id)| id.clone())
            .collect();
        if wanted.len() > MAX_EDITORS {
            return Err("UI_INPUT: too many editors".into());
        }
        let projected: usize = records
            .iter()
            .zip(&ids)
            .filter(|(r, _)| matches!(r.tag, tag::INPUT | tag::TEXTAREA | tag::SEARCH))
            .map(|(r, id)| {
                let value = r.string(field::INPUT_VALUE).unwrap_or("");
                self.entries
                    .get(id)
                    .map(|e| {
                        e.editor
                            .projected_value_bytes(value, r.number(field::ENABLED) != Some(0.0))
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
        for (order, (record, id)) in records
            .iter()
            .zip(ids)
            .enumerate()
            .filter(|(_, (r, _))| matches!(r.tag, tag::INPUT | tag::TEXTAREA | tag::SEARCH))
        {
            let value = record.string(field::INPUT_VALUE).unwrap_or("");
            let focus_request = record.number(field::FOCUS_REQUEST).unwrap_or(0.0) as u64;
            if !self.entries.contains_key(&id) {
                self.entries.insert(
                    id.clone(),
                    Entry {
                        editor: Editor::new(value).map_err(|e| format!("UI_INPUT: {e:?}"))?,
                        enabled: true,
                        visible: false,
                        autofocus: record.number(field::AUTO_FOCUS) == Some(1.0),
                        refocus: false,
                        focus_request,
                        order,
                        enter_key_hint: None,
                        on_input: None,
                        on_submit: None,
                        scroll_offset: [0.0; 2],
                        rules: Default::default(),
                    },
                );
            }
            let entry = self.entries.get_mut(&id).unwrap();
            entry.enabled = record.number(field::ENABLED) != Some(0.0);
            entry.order = order;
            entry.enter_key_hint = record.string(field::ENTER_KEY_HINT).map(str::to_owned);
            if focus_request > entry.focus_request {
                entry.focus_request = focus_request;
                entry.refocus = true;
            }
            entry.rules = crate::interaction::input_rules::Rules::from_record(record);
            entry.on_input = record
                .fields
                .iter()
                .find(|(id, _)| *id == field::ON_INPUT)
                .and_then(|(_, v)| v.as_u64());
            entry.on_submit = record
                .fields
                .iter()
                .find(|(id, _)| *id == field::ON_SUBMIT)
                .and_then(|(_, v)| v.as_u64());
            renderer.edit_text(&mut entry.editor, &[Edit::Value(value.into())])?;
            if !entry.enabled && self.focused.as_deref() == Some(&id) {
                self.blur(renderer)?;
            }
        }
        self.check_budget()
    }

    pub fn apply_values(&self, records: &mut [Record]) {
        if self.entries.is_empty() {
            return;
        }
        let ids = identities(records);
        for (record, id) in records.iter_mut().zip(ids) {
            if let Some(entry) = self.entries.get(&id) {
                let value = serde_json::Value::String(entry.editor.snapshot().text);
                if let Some((_, old)) = record
                    .fields
                    .iter_mut()
                    .find(|(field, _)| *field == field::INPUT_VALUE)
                {
                    *old = value;
                } else {
                    record.fields.push((21, value));
                }
            }
        }
    }

    pub fn apply_styles(&self, records: &mut [Record]) -> Result<(), String> {
        match self.focused.as_deref() {
            Some(identity) => {
                apply_state_style(records, identity, field::FOCUS_STYLE)?;
                apply_focus_within(records, identity)
            }
            None => Ok(()),
        }
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
        let requested = display.nodes.iter().find(|node| {
            self.entries
                .get(&node.identity)
                .is_some_and(|e| e.refocus && e.visible)
        });
        if let Some(node) = requested {
            let identity = node.identity.clone();
            self.focus(&identity, renderer)?;
        }
        for entry in self.entries.values_mut() {
            entry.refocus &= !entry.visible;
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
                input.raise_keyboard = input.edit_token.is_some() && !self.keyboard_held;
            }
        }
        display.decorate_inputs();
        if let Some(selection) = &self.text_selection
            && !display.decorate_selection(selection)
        {
            self.text_selection = None;
        }
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
        self.text_selection = None;
        renderer.edit_text(
            &mut self.entries.get_mut(id).unwrap().editor,
            &[Edit::Focus],
        )?;
        self.focused = Some(id.into());
        Ok(true)
    }

    /// Apply bounds and step to the editor's value, queueing the change as input.
    pub fn commit(&mut self, id: &str, renderer: &mut Renderer) -> Result<(), String> {
        let Some(entry) = self.entries.get_mut(id) else {
            return Ok(());
        };
        let snapshot = entry.editor.snapshot();
        let Some(value) = entry.rules.commit(&snapshot.text) else {
            return Ok(());
        };
        let end = snapshot.text.encode_utf16().count() as u32;
        renderer.edit_text(
            &mut entry.editor,
            &[
                Edit::FinishComposition,
                Edit::Selection {
                    anchor: 0,
                    focus: end,
                },
                Edit::Replace {
                    text: value.clone(),
                    cursor: 1,
                    composing: false,
                },
            ],
        )?;
        if let Some(handle) = entry.on_input {
            self.committed.push((handle, value));
        }
        Ok(())
    }

    pub fn take_committed(&mut self) -> Vec<(u64, String)> {
        std::mem::take(&mut self.committed)
    }

    pub fn blur(&mut self, renderer: &mut Renderer) -> Result<(), String> {
        if let Some(id) = self.focused.take() {
            if self.entries.get(&id).is_some_and(|entry| entry.enabled) {
                self.commit(&id, renderer)?;
            }
            if let Some(entry) = self.entries.get_mut(&id) {
                // A composition still open is finished and reaches Logic before the value settles.
                let change = renderer.edit_text(&mut entry.editor, &[Edit::Blur])?;
                if let (Some(value), Some(handle)) = (change.input, entry.on_input) {
                    self.committed.push((handle, value));
                }
            }
            self.advance()?;
        }
        Ok(())
    }

    /// The enabled, visible editor before or after `id` in reading order, when its
    /// `enterKeyHint` is `previous` or `next`.
    pub fn adjacent(&self, id: &str) -> Option<String> {
        let entry = self.entries.get(id)?;
        let forward = match entry.enter_key_hint.as_deref() {
            Some("next") => true,
            Some("previous") => false,
            _ => return None,
        };
        let candidates = self
            .entries
            .iter()
            .filter(|(other, e)| *other != id && e.enabled && e.visible);
        if forward {
            candidates
                .filter(|(_, e)| e.order > entry.order)
                .min_by_key(|(_, e)| e.order)
        } else {
            candidates
                .filter(|(_, e)| e.order < entry.order)
                .max_by_key(|(_, e)| e.order)
        }
        .map(|(other, _)| other.clone())
    }

    /// The picker request for editor `id`, with its current value, when it is a date or time editor.
    pub fn picker_request(&self, id: &str) -> Option<serde_json::Value> {
        let entry = self.entries.get(id)?;
        let mut spec = entry.rules.picker_spec()?;
        spec["identity"] = id.into();
        spec["value"] = entry.editor.snapshot().text.into();
        Some(spec)
    }

    pub fn valid(&self, id: &str, token: u64) -> bool {
        token != 0 && token == self.token && self.focused.as_deref() == Some(id)
    }
    pub fn reply(&self, id: &str, needs_view_render: bool) -> InputReply {
        InputReply {
            token: self.token,
            state: self.entries[id].editor.snapshot(),
            needs_view_render,
            moved_to: None,
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
        let entry = self.entries.get_mut(id).ok_or("UI_INPUT: missing editor")?;
        let before = entry.editor.clone();
        let mut input = None;
        if entry.rules.max_length.is_none() {
            input = renderer.edit_text(&mut entry.editor, edits)?.input;
        } else {
            for edit in edits {
                // maxLength trims what an edit inserts; it never removes existing text.
                let edit = match edit {
                    Edit::Replace {
                        text,
                        cursor,
                        composing,
                    } => Edit::Replace {
                        text: entry.rules.fit(&entry.editor.snapshot(), text),
                        cursor: *cursor,
                        composing: *composing,
                    },
                    edit => edit.clone(),
                };
                let change = renderer.edit_text(&mut entry.editor, std::slice::from_ref(&edit))?;
                input = change.input.or(input);
            }
        }
        let text = entry.editor.snapshot().text;
        if !entry.rules.accepts(&text) {
            entry.editor = before;
            return Err(format!(
                "UI_INPUT: {} inputs take an ISO value",
                entry.rules.keyboard
            ));
        }
        self.check_budget()?;
        Ok(input)
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

/// Merge a node's paint-only state delta (focus, pressed or hover) into its record.
/// `focus-within:`/`has-focus:`: the focused node and each of its ancestors take their style.
pub(crate) fn apply_focus_within(records: &mut [Record], identity: &str) -> Result<(), String> {
    if !records
        .iter()
        .any(|record| record.string(field::FOCUS_WITHIN_STYLE).is_some())
    {
        return Ok(());
    }
    let ids = identities(records);
    let mut at = ids.iter().position(|id| id == identity);
    while let Some(index) = at {
        let record = &mut records[index];
        if let Some((_, value)) = record
            .fields
            .iter()
            .find(|(field, _)| *field == field::FOCUS_WITHIN_STYLE)
        {
            for (field, value) in crate::protocol::focus_style(value)? {
                if let Some((_, old)) = record.fields.iter_mut().find(|(id, _)| *id == field) {
                    *old = value;
                } else {
                    record.fields.push((field, value));
                }
            }
        }
        at = record.parent;
    }
    Ok(())
}

/// `group-active:`: the pressed control's descendants take their group-pressed paint.
pub(crate) fn apply_group_pressed(records: &mut [Record], identity: &str) -> Result<(), String> {
    if !records
        .iter()
        .any(|record| record.string(field::GROUP_PRESSED_STYLE).is_some())
    {
        return Ok(());
    }
    let ids = identities(records);
    let Some(target) = ids.iter().position(|id| id == identity) else {
        return Ok(());
    };
    let mut inside = vec![false; records.len()];
    for index in target + 1..records.len() {
        inside[index] = records[index]
            .parent
            .is_some_and(|parent| parent == target || inside[parent]);
        if !inside[index] {
            continue;
        }
        let record = &mut records[index];
        if let Some((_, value)) = record
            .fields
            .iter()
            .find(|(field, _)| *field == field::GROUP_PRESSED_STYLE)
        {
            for (field, value) in crate::protocol::focus_style(value)? {
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

pub(crate) fn apply_state_style(
    records: &mut [Record],
    identity: &str,
    state: usize,
) -> Result<(), String> {
    let ids = identities(records);
    let Some(target) = ids.iter().position(|id| id == identity) else {
        return Ok(());
    };
    // Pressing one piece of a styled link presses all of it.
    let leader = crate::scene::build::link_leader(records, target);
    let group: Vec<usize> = (leader..records.len())
        .take_while(|i| *i == target || crate::scene::build::link_leader(records, *i) == leader)
        .collect();
    for (index, record) in records.iter_mut().enumerate() {
        if !group.contains(&index) {
            continue;
        }
        if let Some((_, value)) = record.fields.iter().find(|(field, _)| *field == state) {
            for (field, value) in crate::protocol::focus_style(value)? {
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
