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

    pub(super) fn finish_keyboard(&mut self, page: PageTicket) -> Vec<Effect> {
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
