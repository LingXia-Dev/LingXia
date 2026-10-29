use super::*;
use crate::interaction::keyboard as navigation;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Tab,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    Activate,
    /// Dismisses what the View raised over the page (Logic's modal, action sheet or toast).
    Escape,
    PageUp,
    PageDown,
    /// A printable character: digits into the time picker, letters to a select's type-ahead.
    Char(char),
}

/// Codes 0..=10 name the keys in declaration order, Tab to PageDown; a printable character
/// travels as `CHAR_BASE + its code point`.
pub const CHAR_BASE: i64 = 0x1_0000;

impl Key {
    /// The key a platform adapter sends as `code`, or `None` for one the View does not use.
    pub fn from_code(code: i64) -> Option<Key> {
        Some(match code {
            0 => Key::Tab,
            1 => Key::Up,
            2 => Key::Down,
            3 => Key::Left,
            4 => Key::Right,
            5 => Key::Home,
            6 => Key::End,
            7 => Key::Activate,
            8 => Key::Escape,
            9 => Key::PageUp,
            10 => Key::PageDown,
            _ => Key::Char(
                char::from_u32(u32::try_from(code.checked_sub(CHAR_BASE)?).ok()?)
                    .filter(|c| !c.is_control())?,
            ),
        })
    }
}

impl ViewInteraction {
    pub(super) fn revealable<'a>(
        &'a self,
        frame: &'a PresentedFrame,
    ) -> impl Fn(&NodeSnapshot) -> bool + 'a {
        move |node| self.scroll.can_reveal(frame, node)
    }

    pub(super) fn keyboard_origin(&self) -> Option<usize> {
        let frame = self.frame.as_ref()?;
        match &self.pending_focus {
            Some(pending) => frame.index_of(&pending.identity),
            None => self.keyboard_focus,
        }
    }

    /// Navigate to `index`; reveal it first if needed, then optionally activate it.
    pub(super) fn navigate(
        &mut self,
        index: usize,
        activate: bool,
        page: PageTicket,
    ) -> Vec<Effect> {
        let Some(frame) = self.frame.clone() else {
            return Vec::new();
        };
        if !navigation::candidate(&frame, index, self.revealable(&frame)) {
            return Vec::new();
        }
        self.pending_focus = Some(PendingFocus {
            identity: frame.nodes[index].identity.clone(),
            activate,
            page,
        });
        let mut effects = vec![Effect::StopFling];
        if self.scroll.reveal(&frame, &frame.nodes[index]) {
            effects.push(Effect::Redraw);
            return effects;
        }
        effects.extend(self.finish_keyboard(page));
        effects
    }

    /// A dialog that opened takes keyboard focus; one that closed returns it to
    /// what had it before. Only while the keyboard is in use, so pointers see no ring.
    fn track_dialogs(&mut self, page: PageTicket) -> Vec<Effect> {
        let Some(frame) = self.frame.clone() else {
            return Vec::new();
        };
        let open =
            navigation::active_dialog(&frame).map(|index| frame.nodes[index].identity.clone());
        let keyboard = self.keyboard_used;
        let mut effects = Vec::new();
        // Dialogs no longer shown hand focus back, innermost first.
        while let Some((dialog, _)) = self.dialogs.last() {
            if frame
                .index_of(dialog)
                .is_some_and(|index| frame::visible(&frame.nodes[index]))
            {
                break;
            }
            let (_, restore) = self.dialogs.pop().unwrap();
            if keyboard && let Some(index) = restore.and_then(|identity| frame.index_of(&identity))
            {
                effects.extend(self.navigate(index, false, page));
            }
        }
        if let Some(dialog) = open
            && self.dialogs.last().is_none_or(|(top, _)| *top != dialog)
        {
            let before = self
                .keyboard_origin()
                .map(|index| frame.nodes[index].identity.clone());
            self.dialogs.push((dialog, before));
            if keyboard
                && let Some(first) = navigation::next(&frame, None, 1, self.revealable(&frame))
            {
                effects.extend(self.navigate(first, false, page));
            }
        }
        effects
    }

    pub(super) fn finish_keyboard(&mut self, page: PageTicket) -> Vec<Effect> {
        let mut effects = self.track_dialogs(page);
        effects.extend(self.finish_pending(page));
        effects
    }

    fn finish_pending(&mut self, page: PageTicket) -> Vec<Effect> {
        let Some(pending) = self.pending_focus.take() else {
            return Vec::new();
        };
        let Some(frame) = self.frame.clone() else {
            return Vec::new();
        };
        if pending.page != page {
            return Vec::new();
        }
        let Some(index) = frame.index_of(&pending.identity) else {
            return Vec::new();
        };
        if !navigation::candidate(&frame, index, self.revealable(&frame)) {
            return Vec::new();
        }
        if self.scroll.reveal(&frame, &frame.nodes[index]) {
            self.pending_focus = Some(pending);
            return vec![Effect::Redraw];
        }
        if !frame::visible(&frame.nodes[index]) {
            return Vec::new();
        }
        let changed = self.keyboard_focus != Some(index);
        self.keyboard_focus = Some(index);
        let mut effects = vec![Effect::FocusRing { node: index }];
        if changed && let Some(id) = self.ids.id(index) {
            effects.push(Effect::Accessibility(AccessibilityEffect::InputFocused {
                id,
            }));
        }
        if pending.activate {
            effects.extend(self.activate(index, None, false, None));
        }
        effects
    }

    /// Hardware keys while no editor owns focus. Returns whether the key was consumed.
    pub fn key(
        &mut self,
        key: Key,
        shift: bool,
        repeat: bool,
        page: PageTicket,
    ) -> (bool, Vec<Effect>) {
        if self.inert {
            return (false, Vec::new());
        }
        let Some(frame) = self.frame.clone() else {
            return (false, Vec::new());
        };
        self.keyboard_used = true;
        // The View's picker takes the keys while it is open: arrows, pages, Return, Escape, digits.
        if frame.nodes.iter().any(|node| node.test_id == "lx-picker") {
            let name = match key {
                Key::Up => Some("ArrowUp".to_owned()),
                Key::Down => Some("ArrowDown".to_owned()),
                Key::Left => Some("ArrowLeft".to_owned()),
                Key::Right => Some("ArrowRight".to_owned()),
                Key::PageUp => Some("PageUp".to_owned()),
                Key::PageDown => Some("PageDown".to_owned()),
                Key::Activate => Some("Enter".to_owned()),
                Key::Escape => Some("Escape".to_owned()),
                Key::Char(character) if !character.is_control() => Some(character.to_string()),
                _ => None,
            };
            if let Some(key) = name {
                self.dispatches.push(super::Dispatch::PickerKey { key });
                return (true, vec![Effect::Dispatch]);
            }
        }
        if key == Key::Escape {
            // Consumed only while a dialog is open; otherwise the platform may use it too.
            self.dispatches.push(super::Dispatch::Escape);
            return (!self.dialogs.is_empty(), vec![Effect::Dispatch]);
        }
        let origin = self.keyboard_origin();
        if let Some(index) = origin
            && let Some(node) = frame.node(index).filter(|node| frame::slider_enabled(node))
        {
            let range = node.range.clone().unwrap();
            let value = self.slider.value(node).unwrap_or(range.value);
            let next = match key {
                Key::Left | Key::Down => Some(slider::shift(&range, value, -1)),
                Key::Right | Key::Up => Some(slider::shift(&range, value, 1)),
                Key::Home => Some(range.min),
                Key::End => Some(range.max),
                _ => None,
            };
            if let Some(next) = next {
                return (true, self.propose_number(index, next, page));
            }
            if key == Key::Activate {
                return (true, Vec::new());
            }
        }
        // Arrows change a select's choice without opening its picker, as on the Web.
        if matches!(key, Key::Left | Key::Right | Key::Up | Key::Down)
            && let Some(index) = origin
            && let Some(select) = frame.nodes[index]
                .select
                .as_ref()
                .filter(|s| s.on_select.is_some())
        {
            let forward = matches!(key, Key::Right | Key::Down);
            let count = select.options.len();
            let start = select
                .selected
                .unwrap_or(if forward { count - 1 } else { 0 });
            let next = (1..count)
                .map(|step| {
                    if forward {
                        (start + step) % count
                    } else {
                        (start + count - step) % count
                    }
                })
                .find(|candidate| select.options[*candidate].enabled);
            if let Some(next) = next {
                self.dispatches.push(Dispatch::Select {
                    commit: frame.commit,
                    identity: frame.nodes[index].identity.clone(),
                    index: next,
                    value: select.options[next].value.clone(),
                });
                return (true, vec![Effect::Dispatch]);
            }
            return (true, Vec::new());
        }
        if matches!(key, Key::Left | Key::Right | Key::Up | Key::Down)
            && let Some(index) = origin.filter(|index| navigation::is_option(&frame, *index))
        {
            let direction = if matches!(key, Key::Left | Key::Up) {
                -1
            } else {
                1
            };
            let effects = navigation::adjacent_option(&frame, index, direction)
                .map(|next| self.navigate(next, true, page))
                .unwrap_or_default();
            return (true, effects);
        }
        if matches!(key, Key::Tab | Key::Up | Key::Down) {
            let direction = if shift || key == Key::Up { -1 } else { 1 };
            if let Some(next) = navigation::next(&frame, origin, direction, self.revealable(&frame))
            {
                return (true, self.navigate(next, false, page));
            }
        }
        if key == Key::Activate {
            if let Some(pending) = &mut self.pending_focus {
                if !repeat {
                    pending.activate = true;
                }
                return (true, Vec::new());
            }
            if let Some(index) = self.keyboard_focus {
                return (
                    true,
                    if repeat {
                        Vec::new()
                    } else {
                        self.activate(index, None, false, None)
                    },
                );
            }
        }
        (false, Vec::new())
    }

    /// The host view gained keyboard focus from the system focus search.
    pub fn focus_entered(&mut self, backward: bool, page: PageTicket) -> Vec<Effect> {
        let Some(frame) = self.frame.clone() else {
            return Vec::new();
        };
        self.keyboard_used = true;
        match navigation::next(
            &frame,
            self.keyboard_origin(),
            if backward { -1 } else { 1 },
            self.revealable(&frame),
        ) {
            Some(next) => self.navigate(next, false, page),
            None => Vec::new(),
        }
    }

    pub fn focus_left(&mut self) -> Vec<Effect> {
        self.slider.cancel();
        self.pending_focus = None;
        self.keyboard_focus = None;
        vec![Effect::ClearFocusRing]
    }

    // ---- accessibility -------------------------------------------------
}
