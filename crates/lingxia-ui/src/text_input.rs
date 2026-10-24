//! Worker-owned text editing. Platform offsets are UTF-16, layout offsets are UTF-8.
use parley::{FontContext, LayoutContext, editing::PlainEditor};
use serde::Serialize;
use std::ops::Range;
use std::time::Instant;

mod history;
use history::{History, Point};

pub const MAX_TEXT_BYTES: usize = 64 * 1024;
pub const MAX_BATCH_EDITS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    TooLong,
    InvalidOffset,
    NotFocused,
    TooManyEdits,
}

#[derive(Clone, Copy, Debug)]
pub enum OffsetUnit {
    Utf16,
    CodePoint,
}

#[derive(Clone, Debug)]
pub enum Edit {
    Focus,
    Blur,
    Value(String),
    Selection {
        anchor: u32,
        focus: u32,
    },
    Composition {
        start: u32,
        end: u32,
    },
    Replace {
        text: String,
        cursor: i32,
        composing: bool,
    },
    FinishComposition,
    Undo,
    Redo,
    DeleteSurrounding {
        before: u32,
        after: u32,
        unit: OffsetUnit,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Snapshot {
    pub text: String,
    pub selection: [u32; 2],
    pub composition: Option<[u32; 2]>,
    pub focused: bool,
    pub can_undo: bool,
    pub can_redo: bool,
}

#[derive(Debug)]
pub struct Change {
    pub snapshot: Snapshot,
    /// Committed text for onInput. Preedit and controlled value updates never emit it.
    pub input: Option<String>,
}

#[derive(Clone)]
pub struct Editor {
    visual: PlainEditor<()>,
    // IME positions may be inside a grapheme; Parley cursors snap to clusters.
    selection: [usize; 2],
    composition: Option<Range<usize>>,
    controlled: String,
    published: String,
    focused: bool,
    history: History,
}

impl Editor {
    pub(crate) fn projected_value_bytes(&self, value: &str, enabled: bool) -> usize {
        if self.focused && enabled {
            self.visual.raw_text().len() + self.published.len() + value.len() + self.history.bytes()
        } else {
            value.len() * 3
                + if self.keeps_history(value) {
                    self.history.bytes()
                } else {
                    0
                }
        }
    }
    pub(crate) fn byte_len(&self) -> usize {
        self.visual.raw_text().len()
            + self.controlled.len()
            + self.published.len()
            + self.history.bytes()
    }
    pub fn new(value: &str) -> Result<Self, Error> {
        check_size(value)?;
        let mut visual = PlainEditor::new(16.0);
        visual.set_text(value);
        Ok(Self {
            visual,
            selection: [value.len(); 2],
            composition: None,
            controlled: value.into(),
            published: value.into(),
            focused: false,
            history: History::default(),
        })
    }

    pub fn snapshot(&self) -> Snapshot {
        let text = self.visual.raw_text();
        Snapshot {
            text: text.into(),
            selection: self.selection.map(|b| utf16_offset(text, b)),
            composition: self
                .composition
                .as_ref()
                .map(|r| [utf16_offset(text, r.start), utf16_offset(text, r.end)]),
            focused: self.focused,
            can_undo: self.history.can_undo() || self.preedit_changed(),
            can_redo: self.history.can_redo() && !self.preedit_changed(),
        }
    }

    /// Invalid edits leave buffer, selection, composition and controlled value unchanged.
    pub fn apply(
        &mut self,
        edit: Edit,
        fonts: &mut FontContext,
        layouts: &mut LayoutContext<()>,
    ) -> Result<Change, Error> {
        self.apply_batch(std::slice::from_ref(&edit), fonts, layouts)
    }

    /// One accepted snapshot and at most one committed-text notification per batch.
    pub fn apply_batch(
        &mut self,
        edits: &[Edit],
        fonts: &mut FontContext,
        layouts: &mut LayoutContext<()>,
    ) -> Result<Change, Error> {
        if edits.len() > MAX_BATCH_EDITS {
            return Err(Error::TooManyEdits);
        }
        let mut next = self.clone();
        let mut input = None;
        for edit in edits {
            let before = next.history_point();
            let composing = next.composition.is_some();
            let publish = next.edit(edit.clone())?;
            next.record_edit(edit, before, composing, Instant::now());
            let text = next.visual.raw_text();
            if publish && next.composition.is_none() && text != next.published {
                next.published = text.into();
                input = Some(next.published.clone());
            }
        }
        // Restore exact composition and its approximate visual selection together.
        let sync_compose = next.visual.raw_compose() != &next.composition;
        let mut driver = next.visual.driver(fonts, layouts);
        if sync_compose {
            driver.finish_compose();
            if let Some(r) = &next.composition {
                driver.set_compose_byte_range(r.start, r.end);
            }
        }
        driver.select_byte_range(next.selection[0], next.selection[1]);
        let snapshot = next.snapshot();
        *self = next;
        Ok(Change { snapshot, input })
    }

    /// Rendering uses Parley's layout, caret and selection geometry, not another text shaper.
    pub fn visual(&self) -> &PlainEditor<()> {
        &self.visual
    }

    fn history_point(&self) -> Point {
        Point {
            text: self.visual.raw_text().into(),
            selection: self.selection,
        }
    }

    fn preedit_changed(&self) -> bool {
        self.history
            .pending
            .as_ref()
            .is_some_and(|point| point.text.as_ref() != self.visual.raw_text())
    }

    fn record_edit(&mut self, edit: &Edit, before: Point, composing: bool, now: Instant) {
        if matches!(
            edit,
            Edit::Focus | Edit::Blur | Edit::Value(_) | Edit::Undo | Edit::Redo
        ) {
            return;
        }
        if matches!(edit, Edit::Selection { .. }) {
            if before.selection != self.selection {
                self.history.boundary();
            }
            return;
        }
        if !composing && self.composition.is_some() {
            self.history.boundary();
            self.history.pending = Some(before);
            self.history.trim();
        } else if self.composition.is_none() {
            let pending = self.history.pending.take();
            let typing = pending.is_none()
                && !composing
                && before.selection[0] == before.selection[1]
                && matches!(edit, Edit::Replace { text, cursor: 1, composing: false }
                    if text.chars().count() == 1 && !text.chars().next().unwrap().is_whitespace());
            self.history
                .record(pending.unwrap_or(before), self.history_point(), typing, now);
        }
    }

    fn edit(&mut self, edit: Edit) -> Result<bool, Error> {
        match edit {
            Edit::Focus => {
                self.history.boundary();
                self.focused = true;
                return Ok(false);
            }
            Edit::Blur => {
                self.focused = false;
                self.install_value(self.controlled.clone());
                return Ok(false);
            }
            Edit::Value(value) => {
                check_size(&value)?;
                if value == self.controlled && (self.focused || self.visual.raw_text() == value) {
                    return Ok(false);
                }
                self.controlled = value.clone();
                if !self.focused {
                    self.install_value(value);
                }
                return Ok(false);
            }
            _ if !self.focused => return Err(Error::NotFocused),
            _ => {}
        }
        match edit {
            Edit::Undo | Edit::Redo => {
                // IMEs can mark existing text for suggestions without editing it.
                if !self.preedit_changed() {
                    self.history.pending = None;
                    self.composition = None;
                }
                let point = if matches!(edit, Edit::Undo) {
                    self.history.undo()
                } else {
                    self.history.redo()
                };
                if let Some(point) = point {
                    self.visual.set_text(&point.text);
                    self.selection = point.selection;
                    self.composition = None;
                    return Ok(true);
                }
                Ok(false)
            }
            Edit::Selection { anchor, focus } => {
                self.selection = [
                    byte_offset(self.visual.raw_text(), anchor)?,
                    byte_offset(self.visual.raw_text(), focus)?,
                ];
                Ok(false)
            }
            Edit::Composition { start, end } => {
                let text = self.visual.raw_text();
                let (a, b) = (byte_offset(text, start)?, byte_offset(text, end)?);
                self.composition = (a != b).then_some(a.min(b)..a.max(b));
                Ok(self.composition.is_none())
            }
            Edit::FinishComposition => {
                self.composition = None;
                Ok(true)
            }
            Edit::Replace {
                text,
                cursor,
                composing,
            } => {
                check_size(&text)?;
                let range = self.composition.clone().unwrap_or(self.selected_range());
                let mut buffer = self.visual.raw_text().to_owned();
                if buffer.len() - range.len() + text.len() > MAX_TEXT_BYTES {
                    return Err(Error::TooLong);
                }
                let start = utf16_offset(&buffer, range.start) as i64;
                buffer.replace_range(range.clone(), &text);
                let end = start + text.encode_utf16().count() as i64;
                let position = if cursor > 0 {
                    end + cursor as i64 - 1
                } else {
                    start + cursor as i64
                };
                let position = position.clamp(0, buffer.encode_utf16().count() as i64) as u32;
                self.selection = [byte_offset(&buffer, position)?; 2];
                self.composition = (composing && !text.is_empty())
                    .then_some(range.start..range.start + text.len());
                self.visual.set_text(&buffer);
                Ok(!composing || text.is_empty())
            }
            Edit::DeleteSurrounding {
                before,
                after,
                unit,
            } => {
                let range = self.selected_range();
                let text = self.visual.raw_text();
                let (start, end) = match unit {
                    OffsetUnit::Utf16 => (
                        byte_offset(text, utf16_offset(text, range.start).saturating_sub(before))?,
                        byte_offset(
                            text,
                            utf16_offset(text, range.end)
                                .saturating_add(after)
                                .min(text.encode_utf16().count() as u32),
                        )?,
                    ),
                    OffsetUnit::CodePoint => (
                        text[..range.start]
                            .char_indices()
                            .rev()
                            .nth(before.saturating_sub(1) as usize)
                            .map(|(i, _)| i)
                            .unwrap_or(0),
                        text[range.end..]
                            .char_indices()
                            .nth(after as usize)
                            .map(|(i, _)| range.end + i)
                            .unwrap_or(text.len()),
                    ),
                };
                let start = if before == 0 { range.start } else { start };
                let end = if after == 0 { range.end } else { end };
                // Delete the right span first so left-side offsets remain valid.
                self.delete_range(range.end..end);
                self.delete_range(start..range.start);
                Ok(true)
            }
            _ => unreachable!(),
        }
    }

    fn selected_range(&self) -> Range<usize> {
        self.selection[0].min(self.selection[1])..self.selection[0].max(self.selection[1])
    }

    fn install_value(&mut self, value: String) {
        if !self.keeps_history(&value) {
            self.history = History::default();
        } else {
            self.history.pending = None;
            self.history.boundary();
        }
        self.visual.set_text(&value);
        for position in &mut self.selection {
            *position = (*position).min(value.len());
            while !value.is_char_boundary(*position) {
                *position -= 1;
            }
        }
        self.composition = None;
        self.published = value;
    }

    fn keeps_history(&self, value: &str) -> bool {
        value == self.visual.raw_text()
            || self
                .history
                .pending
                .as_ref()
                .is_some_and(|point| point.text.as_ref() == value)
    }

    fn delete_range(&mut self, range: Range<usize>) {
        if range.is_empty() {
            return;
        }
        let remap = |index: usize| {
            if index <= range.start {
                index
            } else {
                index - (index.min(range.end) - range.start)
            }
        };
        self.selection = self.selection.map(remap);
        self.composition = self.composition.as_ref().and_then(|r| {
            let start = remap(r.start);
            let end = remap(r.end);
            (start != end).then_some(start..end)
        });
        let mut text = self.visual.raw_text().to_owned();
        text.replace_range(range, "");
        self.visual.set_text(&text);
    }
}

fn check_size(text: &str) -> Result<(), Error> {
    if text.len() > MAX_TEXT_BYTES {
        Err(Error::TooLong)
    } else {
        Ok(())
    }
}

fn utf16_offset(text: &str, byte: usize) -> u32 {
    text[..byte].encode_utf16().count() as u32
}

fn byte_offset(text: &str, offset: u32) -> Result<usize, Error> {
    let mut units = 0;
    for (byte, ch) in text.char_indices() {
        if units == offset {
            return Ok(byte);
        }
        units += ch.len_utf16() as u32;
        if units > offset {
            return Err(Error::InvalidOffset);
        }
    }
    if units == offset {
        Ok(text.len())
    } else {
        Err(Error::InvalidOffset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unchanged_ime_composition_region_does_not_intercept_history_commands() {
        let mut c = Client::new("base");
        c.replace("hello", false);
        c.edit(Edit::Composition { start: 0, end: 9 }).unwrap();
        assert_eq!(c.edit(Edit::Undo).unwrap().snapshot.text, "base");
        c.edit(Edit::Composition { start: 0, end: 4 }).unwrap();
        assert!(c.editor.snapshot().can_redo);
        assert_eq!(c.edit(Edit::Redo).unwrap().snapshot.text, "basehello");
        c.edit(Edit::Undo).unwrap();
        c.replace("pending", true);
        assert!(!c.editor.snapshot().can_redo);
        assert_eq!(c.edit(Edit::Redo).unwrap().snapshot.text, "basepending");
        assert_eq!(c.edit(Edit::Undo).unwrap().snapshot.text, "base");
        assert!(c.editor.snapshot().can_redo);
    }

    #[test]
    fn undo_groups_typing_and_round_trips_exact_selection() {
        let mut c = Client::new("A\u{1f600}Z");
        c.edit(Edit::Selection {
            anchor: 3,
            focus: 1,
        })
        .unwrap();
        c.replace("word", false);
        let restored = c.edit(Edit::Undo).unwrap();
        assert_eq!(restored.snapshot.text, "A\u{1f600}Z");
        assert_eq!(restored.snapshot.selection, [3, 1]);
        assert_eq!(restored.input.as_deref(), Some("A\u{1f600}Z"));
        assert!(restored.snapshot.can_redo);
        assert_eq!(c.edit(Edit::Redo).unwrap().snapshot.text, "AwordZ");
        c.replace("a", false);
        c.replace("b", false);
        c.replace("c", false);
        assert_eq!(c.edit(Edit::Undo).unwrap().snapshot.text, "AwordZ");
        assert_eq!(c.edit(Edit::Undo).unwrap().snapshot.text, "A\u{1f600}Z");
    }

    #[test]
    fn composition_undo_cancels_preedit_then_undoes_committed_input() {
        let mut c = Client::new("");
        c.replace("a", false);
        c.replace("ni", true);
        c.replace("nih", true);
        let cancelled = c.edit(Edit::Undo).unwrap();
        assert_eq!(cancelled.snapshot.text, "a");
        assert_eq!(cancelled.snapshot.composition, None);
        assert_eq!(cancelled.input, None);
        assert_eq!(c.edit(Edit::Undo).unwrap().snapshot.text, "");
        c.edit(Edit::Redo).unwrap();
        c.replace("ni", true);
        c.replace("\u{4f60}", false);
        assert_eq!(c.edit(Edit::Undo).unwrap().snapshot.text, "a");
        assert_eq!(c.edit(Edit::Redo).unwrap().snapshot.text, "a\u{4f60}");
    }

    #[test]
    fn history_follows_commits_within_batches_and_rolls_back_with_invalid_edits() {
        let mut c = Client::new("");
        let batch = [
            Edit::Replace {
                text: "one".into(),
                cursor: 1,
                composing: false,
            },
            Edit::Replace {
                text: "two".into(),
                cursor: 1,
                composing: true,
            },
        ];
        c.editor
            .apply_batch(&batch, &mut c.fonts, &mut c.layouts)
            .unwrap();
        let before = c.editor.snapshot();
        assert!(
            c.editor
                .apply_batch(
                    &[
                        Edit::Undo,
                        Edit::Selection {
                            anchor: 999,
                            focus: 999
                        }
                    ],
                    &mut c.fonts,
                    &mut c.layouts
                )
                .is_err()
        );
        assert_eq!(c.editor.snapshot(), before);
        assert_eq!(c.edit(Edit::Undo).unwrap().snapshot.text, "one");
        assert_eq!(c.edit(Edit::Undo).unwrap().snapshot.text, "");
        assert_eq!(c.edit(Edit::Redo).unwrap().snapshot.text, "one");
        c.replace("different", false);
        assert!(!c.editor.snapshot().can_redo);
    }

    #[test]
    fn matching_echo_and_blur_keep_history_but_external_values_clear_it() {
        let mut c = Client::new("");
        c.replace("word", false);
        c.edit(Edit::Value("word".into())).unwrap();
        c.replace("preedit", true);
        c.edit(Edit::Blur).unwrap();
        c.edit(Edit::Focus).unwrap();
        assert!(c.editor.snapshot().can_undo);
        assert_eq!(c.edit(Edit::Undo).unwrap().snapshot.text, "");
        c.edit(Edit::Value("server".into())).unwrap();
        c.edit(Edit::Blur).unwrap();
        assert!(!c.editor.snapshot().can_undo);
        assert!(!c.editor.snapshot().can_redo);
    }

    struct Client {
        editor: Editor,
        fonts: FontContext,
        layouts: LayoutContext<()>,
    }
    impl Client {
        fn new(value: &str) -> Self {
            let mut client = Self {
                editor: Editor::new(value).unwrap(),
                fonts: FontContext::new(),
                layouts: LayoutContext::new(),
            };
            client.edit(Edit::Focus).unwrap();
            client
        }
        fn edit(&mut self, event: Edit) -> Result<Change, Error> {
            self.editor.apply(event, &mut self.fonts, &mut self.layouts)
        }
        fn replace(&mut self, text: &str, composing: bool) -> Change {
            self.edit(Edit::Replace {
                text: text.into(),
                cursor: 1,
                composing,
            })
            .unwrap()
        }
    }

    #[test]
    fn chinese_preedit_is_private_until_commit_and_echo_cannot_interrupt_it() {
        let mut client = Client::new("你好");
        assert_eq!(client.replace("sh", true).input, None);
        assert_eq!(
            client.replace("shi", true).snapshot.composition,
            Some([2, 5])
        );
        client.edit(Edit::Value("迟到的回显".into())).unwrap();
        assert_eq!(client.editor.snapshot().text, "你好shi");
        let committed = client.replace("世界", false);
        assert_eq!(committed.input.as_deref(), Some("你好世界"));
        assert_eq!(committed.snapshot.selection, [4, 4]);
        assert_eq!(committed.snapshot.composition, None);
        assert_eq!(client.edit(Edit::FinishComposition).unwrap().input, None);
        client.edit(Edit::Blur).unwrap();
        assert_eq!(client.editor.snapshot().text, "迟到的回显");
        assert_eq!(client.replace_unfocused(), Error::NotFocused);
    }
    impl Client {
        fn replace_unfocused(&mut self) -> Error {
            self.edit(Edit::Replace {
                text: "x".into(),
                cursor: 1,
                composing: false,
            })
            .unwrap_err()
        }
    }

    #[test]
    fn emoji_offsets_reverse_selection_and_invalid_edits_are_atomic() {
        let mut client = Client::new("A😀中B");
        client
            .edit(Edit::Selection {
                anchor: 4,
                focus: 1,
            })
            .unwrap();
        assert_eq!(client.replace("好", false).snapshot.text, "A好B");
        client.edit(Edit::Value("server".into())).unwrap();
        client.replace("😀", true);
        let before = client.editor.snapshot();
        assert_eq!(
            client
                .edit(Edit::Selection {
                    anchor: 3,
                    focus: 3
                })
                .unwrap_err(),
            Error::InvalidOffset
        );
        assert_eq!(client.editor.snapshot(), before);
        assert_eq!(
            client
                .edit(Edit::Replace {
                    text: "a".repeat(MAX_TEXT_BYTES + 1),
                    cursor: 1,
                    composing: false
                })
                .unwrap_err(),
            Error::TooLong
        );
        assert_eq!(client.editor.snapshot(), before);
        client.edit(Edit::Blur).unwrap();
        assert_eq!(client.editor.snapshot().text, "server");
    }

    #[test]
    fn ime_can_replace_inside_a_grapheme_without_expanding_to_visual_cluster() {
        let mut client = Client::new("ae\u{301}z");
        client
            .edit(Edit::Selection {
                anchor: 2,
                focus: 3,
            })
            .unwrap();
        assert_eq!(client.editor.snapshot().selection, [2, 3]);
        assert_eq!(client.replace("\u{300}", false).snapshot.text, "ae\u{300}z");
        client.edit(Edit::Composition { start: 3, end: 2 }).unwrap();
        assert_eq!(client.replace("\u{302}", true).snapshot.text, "ae\u{302}z");
        assert_eq!(
            client
                .edit(Edit::FinishComposition)
                .unwrap()
                .input
                .as_deref(),
            Some("ae\u{302}z")
        );
    }

    #[test]
    fn empty_preedit_and_relative_cursor_positions_follow_the_replacement_range() {
        let mut client = Client::new("abcd");
        client
            .edit(Edit::Selection {
                anchor: 1,
                focus: 3,
            })
            .unwrap();
        client.replace("你好", true);
        let empty = client.replace("", true);
        assert_eq!(empty.snapshot.text, "ad");
        assert_eq!(empty.input.as_deref(), Some("ad"));
        assert_eq!(
            client
                .edit(Edit::FinishComposition)
                .unwrap()
                .input
                .as_deref(),
            None
        );
        let change = client
            .edit(Edit::Replace {
                text: "XY".into(),
                cursor: 0,
                composing: false,
            })
            .unwrap();
        assert_eq!(change.snapshot.text, "aXYd");
        assert_eq!(change.snapshot.selection, [1, 1]);
        let change = client
            .edit(Edit::Replace {
                text: "!".into(),
                cursor: i32::MAX,
                composing: false,
            })
            .unwrap();
        assert_eq!(change.snapshot.selection, [5, 5]);
        let change = client
            .edit(Edit::Replace {
                text: "!".into(),
                cursor: i32::MIN,
                composing: false,
            })
            .unwrap();
        assert_eq!(change.snapshot.selection, [0, 0]);
    }

    #[test]
    fn surrounding_deletion_preserves_selection_and_remaps_composition() {
        let mut client = Client::new("a😀中文😀b");
        client
            .edit(Edit::Selection {
                anchor: 5,
                focus: 3,
            })
            .unwrap();
        let deleted = client
            .edit(Edit::DeleteSurrounding {
                before: 1,
                after: 1,
                unit: OffsetUnit::CodePoint,
            })
            .unwrap();
        assert_eq!(deleted.snapshot.text, "a中文b");
        assert_eq!(deleted.snapshot.selection, [3, 1]);
        client.edit(Edit::Composition { start: 0, end: 4 }).unwrap();
        let deleted = client
            .edit(Edit::DeleteSurrounding {
                before: 99,
                after: 99,
                unit: OffsetUnit::Utf16,
            })
            .unwrap();
        assert_eq!(deleted.snapshot.text, "中文");
        assert_eq!(deleted.snapshot.composition, Some([0, 2]));
        assert_eq!(deleted.input, None);
        assert_eq!(
            client
                .edit(Edit::FinishComposition)
                .unwrap()
                .input
                .as_deref(),
            Some("中文")
        );
    }

    #[test]
    fn utf16_deletion_rejects_split_surrogates_and_zero_lengths_are_noops() {
        let mut client = Client::new("😀");
        let before = client.editor.snapshot();
        assert_eq!(
            client
                .edit(Edit::DeleteSurrounding {
                    before: 1,
                    after: 0,
                    unit: OffsetUnit::Utf16
                })
                .unwrap_err(),
            Error::InvalidOffset
        );
        assert_eq!(client.editor.snapshot(), before);
        let noop = client
            .edit(Edit::DeleteSurrounding {
                before: 0,
                after: 0,
                unit: OffsetUnit::CodePoint,
            })
            .unwrap();
        assert_eq!(noop.snapshot, before);
        assert_eq!(noop.input, None);
        assert_eq!(
            client
                .edit(Edit::DeleteSurrounding {
                    before: 2,
                    after: 0,
                    unit: OffsetUnit::Utf16
                })
                .unwrap()
                .input
                .as_deref(),
            Some("")
        );
    }

    #[test]
    fn batches_publish_only_committed_text_and_roll_back_every_state_field_on_failure() {
        let mut client = Client::new("");
        let edits = [
            Edit::Replace {
                text: "ni".into(),
                cursor: 1,
                composing: true,
            },
            Edit::Replace {
                text: "你".into(),
                cursor: 1,
                composing: false,
            },
            Edit::Replace {
                text: "hao".into(),
                cursor: 1,
                composing: true,
            },
        ];
        let change = client
            .editor
            .apply_batch(&edits, &mut client.fonts, &mut client.layouts)
            .unwrap();
        assert_eq!(change.input.as_deref(), Some("你"));
        assert_eq!(change.snapshot.text, "你hao");
        assert_eq!(change.snapshot.composition, Some([1, 4]));
        let before = client.editor.snapshot();
        let bad = [
            Edit::Value("bad echo".into()),
            Edit::FinishComposition,
            Edit::Selection {
                anchor: 999,
                focus: 999,
            },
        ];
        assert_eq!(
            client
                .editor
                .apply_batch(&bad, &mut client.fonts, &mut client.layouts)
                .unwrap_err(),
            Error::InvalidOffset
        );
        assert_eq!(client.editor.snapshot(), before);
        assert_eq!(
            client
                .edit(Edit::FinishComposition)
                .unwrap()
                .input
                .as_deref(),
            Some("你hao")
        );
        assert_eq!(
            client
                .editor
                .apply_batch(
                    &vec![Edit::Focus; MAX_BATCH_EDITS + 1],
                    &mut client.fonts,
                    &mut client.layouts
                )
                .unwrap_err(),
            Error::TooManyEdits
        );
        client.edit(Edit::Blur).unwrap();
        assert_eq!(client.editor.snapshot().text, "");
    }

    #[test]
    fn clearing_a_composing_region_publishes_the_pending_text_once() {
        let mut client = Client::new("a");
        assert_eq!(client.replace("中", true).input, None);
        assert_eq!(
            client
                .edit(Edit::Composition { start: 1, end: 1 })
                .unwrap()
                .input
                .as_deref(),
            Some("a中")
        );
        assert_eq!(client.edit(Edit::FinishComposition).unwrap().input, None);
    }

    #[test]
    fn limits_apply_to_resulting_buffer_and_controlled_values() {
        assert!(matches!(
            Editor::new(&"x".repeat(MAX_TEXT_BYTES + 1)),
            Err(Error::TooLong)
        ));
        let mut client = Client::new("x");
        let before = client.editor.snapshot();
        assert_eq!(
            client
                .edit(Edit::Replace {
                    text: "y".repeat(MAX_TEXT_BYTES),
                    cursor: 1,
                    composing: true
                })
                .unwrap_err(),
            Error::TooLong
        );
        assert_eq!(client.editor.snapshot(), before);
        assert_eq!(
            client
                .edit(Edit::Value("z".repeat(MAX_TEXT_BYTES + 1)))
                .unwrap_err(),
            Error::TooLong
        );
        client.edit(Edit::Blur).unwrap();
        assert_eq!(client.editor.snapshot().text, "x");
    }
}
