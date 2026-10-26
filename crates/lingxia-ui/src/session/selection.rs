//! Static-text selection: long-press selects a word (or everything for
//! `select-all`), handles move either end, Copy reads the selected text.
use super::*;
use crate::interaction::input_state::TextSelection;
use parley::editing::{Cursor, Selection};

impl ViewSession {
    fn selectable(&self, identity: &str) -> Option<Arc<crate::scene::retained::SelectableText>> {
        let retained = self.retained.as_ref()?;
        let index = retained
            .nodes
            .iter()
            .position(|node| node.identity == identity)?;
        retained.selectables[index].clone()
    }

    /// The paragraph-local point for a logical point on the presented frame;
    /// composing again follows scrolls, sticky offsets and transforms.
    fn local(&self, identity: &str, point: [f32; 2]) -> Option<[f32; 2]> {
        let retained = self.retained.as_ref()?;
        let index = retained.nodes.iter().position(|n| n.identity == identity)?;
        let text = retained.selectables[index].as_ref()?;
        let composed = retained.compose(&self.scroll_offsets).ok()?;
        let [x, y] = [
            composed.nodes[index].bounds[0] - retained.nodes[index].bounds[0],
            composed.nodes[index].bounds[1] - retained.nodes[index].bounds[1],
        ];
        Some([point[0] - text.origin[0] - x, point[1] - text.origin[1] - y])
    }

    /// Long-press at a logical point of the presented frame: select the word
    /// there, or the whole text for `select-all`.
    pub fn select_text(
        &mut self,
        generation: lifecycle::CommitTicket,
        identity: &str,
        point: [f32; 2],
    ) -> Result<bool, String> {
        self.check_input_active()?;
        if generation != self.generation || point.iter().any(|v| !v.is_finite()) {
            return Ok(false);
        }
        let (Some(text), Some([x, y])) = (self.selectable(identity), self.local(identity, point))
        else {
            return Ok(false);
        };
        let all = self.retained.as_ref().is_some_and(|retained| {
            retained
                .nodes
                .iter()
                .any(|n| n.identity == identity && n.selectable == Some("all"))
        });
        let range = if all {
            0..text.text.len()
        } else {
            Selection::word_from_point(&text.layout, x, y).text_range()
        };
        self.set_selection(identity, &text.text, range.start, range.end);
        Ok(true)
    }

    /// Move the anchor (0) or focus (1) end to the text under `point`.
    pub fn move_text_selection(
        &mut self,
        identity: &str,
        which: usize,
        point: [f32; 2],
    ) -> Result<bool, String> {
        self.check_input_active()?;
        let Some(current) = self
            .inputs
            .text_selection
            .clone()
            .filter(|s| s.identity == identity)
        else {
            return Ok(false);
        };
        let (Some(text), Some([x, y])) = (self.selectable(identity), self.local(identity, point))
        else {
            return Ok(false);
        };
        let at = Cursor::from_point(&text.layout, x, y).index();
        let (anchor, focus) = if which == 0 {
            (at, current.focus)
        } else {
            (current.anchor, at)
        };
        self.set_selection(identity, &text.text, anchor, focus);
        Ok(true)
    }

    fn set_selection(&mut self, identity: &str, text: &str, anchor: usize, focus: usize) {
        if self.pending_inputs.is_none() {
            self.pending_inputs = Some(self.inputs.clone());
        }
        self.inputs.text_selection = Some(TextSelection {
            identity: identity.to_owned(),
            text: text.to_owned(),
            anchor,
            focus,
        });
        self.input_dirty = true;
    }

    /// Accessibility SetSelection: UTF-16 offsets into the node text.
    pub fn set_text_selection(
        &mut self,
        identity: &str,
        start: u32,
        end: u32,
    ) -> Result<bool, String> {
        self.check_input_active()?;
        let Some(text) = self.selectable(identity) else {
            return Ok(false);
        };
        let byte = |units: u32| {
            let mut seen = 0;
            text.text
                .char_indices()
                .find(|(_, c)| {
                    let found = seen >= units as usize;
                    seen += c.len_utf16();
                    found
                })
                .map(|(at, _)| at)
                .unwrap_or(text.text.len())
        };
        self.set_selection(identity, &text.text, byte(start), byte(end));
        Ok(true)
    }

    /// Drop the static-text selection (tap elsewhere, page hidden, Copy done).
    pub fn clear_text_selection(&mut self) -> bool {
        let had = self.inputs.text_selection.take().is_some();
        self.input_dirty |= had;
        had
    }

    /// The selected text for Copy, in logical order.
    pub fn selected_text(&self) -> Option<String> {
        let selection = self.inputs.text_selection.as_ref()?;
        let (start, end) = (
            selection.anchor.min(selection.focus),
            selection.anchor.max(selection.focus),
        );
        selection.text.get(start..end).map(str::to_owned)
    }
}
