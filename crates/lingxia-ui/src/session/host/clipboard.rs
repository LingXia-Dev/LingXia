//! Accessibility Copy, Cut and Paste on an editor: the selection goes to the platform
//! clipboard and the text change runs through the same edit path as SetText.
use super::ViewHost;
use crate::interaction::view::{AccessibilityAction, Effect};
use crate::platform::PlatformAdapter;

/// `text` split at UTF-16 offsets `start..end`, clamped to whole characters.
fn split(text: &str, start: u32, end: u32) -> (&str, &str, &str) {
    let byte = |offset: u32| {
        let mut units = 0;
        for (index, ch) in text.char_indices() {
            if units >= offset {
                return index;
            }
            units += ch.len_utf16() as u32;
        }
        text.len()
    };
    let (a, b) = (byte(start.min(end)), byte(start.max(end)));
    (&text[..a], &text[a..b], &text[b..])
}

impl<A: PlatformAdapter> ViewHost<A> {
    /// `None` when `action` is not a clipboard action on an editor.
    pub(super) fn editor_clipboard_action(
        &self,
        id: u32,
        action: &AccessibilityAction,
    ) -> Option<bool> {
        if !matches!(
            action,
            AccessibilityAction::Copy | AccessibilityAction::Cut | AccessibilityAction::Paste(_)
        ) {
            return None;
        }
        let index = self.semantics(|projection, _| {
            projection
                .and_then(|projection| projection.node(id))
                .filter(|node| node.text_field.is_some())
                .map(|node| node.index)
        })?;
        let snapshot = self.snapshot()?;
        let input = snapshot.frame.nodes.get(index)?.input.as_ref()?;
        let Some(edit) = input.edit.as_ref().filter(|_| !input.secure) else {
            return Some(false);
        };
        let [start, end] = edit.selection;
        let (before, selected, after) = split(&edit.text, start, end);
        let replace = |inserted: &str| {
            let caret = (before.encode_utf16().count() + inserted.encode_utf16().count()) as i32;
            vec![
                Effect::EditText {
                    node: index,
                    text: Some(format!("{before}{inserted}{after}")),
                    start: -1,
                    end: -1,
                },
                Effect::EditText {
                    node: index,
                    text: None,
                    start: caret,
                    end: caret,
                },
            ]
        };
        let effects = match action {
            AccessibilityAction::Copy if !selected.is_empty() => {
                self.adapter().copy_text(selected);
                return Some(true);
            }
            AccessibilityAction::Cut if input.enabled && !selected.is_empty() => {
                self.adapter().copy_text(selected);
                replace("")
            }
            AccessibilityAction::Paste(text)
                if input.enabled && text.len() <= crate::text::input::MAX_TEXT_BYTES =>
            {
                replace(text)
            }
            _ => return Some(false),
        };
        self.inner.deliver(effects);
        Some(!self.inner.take_edit_failure())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn splits_at_utf16_offsets_on_whole_characters() {
        assert_eq!(super::split("a😀bc", 1, 3), ("a", "😀", "bc"));
        assert_eq!(super::split("a😀bc", 4, 1), ("a", "😀b", "c"));
        assert_eq!(super::split("ab", 9, 9), ("ab", "", ""));
    }
}
