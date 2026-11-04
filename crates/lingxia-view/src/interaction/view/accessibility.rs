use super::*;

#[derive(Clone, Debug, PartialEq)]
pub enum AccessibilityAction {
    Focus,
    ClearFocus,
    Click,
    ShowOnScreen,
    ScrollForward,
    ScrollBackward,
    SetProgress(f64),
    SetText(String),
    SetSelection(i32, i32),
    ClearSelection,
    SelectItem(usize),
    /// Copy the selection (static text or an editor's) to the clipboard.
    Copy,
    /// Copy an editor's selection, then delete it.
    Cut,
    /// Replace an editor's selection with this clipboard text.
    Paste(String),
}

impl ViewInteraction {
    pub(super) fn hover_to(&mut self, target: Option<u32>) -> Vec<Effect> {
        if target == self.hover {
            return Vec::new();
        }
        let old = std::mem::replace(&mut self.hover, target);
        let mut effects = Vec::new();
        if let Some(id) = target {
            effects.push(Effect::Accessibility(AccessibilityEffect::HoverEnter {
                id,
            }));
        }
        if let Some(id) = old {
            effects.push(Effect::Accessibility(AccessibilityEffect::HoverExit { id }));
        }
        effects
    }

    /// Touch exploration at a device-pixel point; `None` ends exploration.
    pub fn explore(&mut self, point: Option<[f32; 2]>) -> (bool, Vec<Effect>) {
        self.exploring = point.map(|[x, y]| self.logical(x, y));
        let target = self
            .exploring
            .and_then(|point| self.semantics().and_then(|semantics| semantics.hit(point)));
        let effects = self.hover_to(target);
        (self.hover.is_some(), effects)
    }

    /// Why an accessibility action on `id` would be refused, for diagnostics.
    pub fn accessibility_refusal(&self, id: u32) -> &'static str {
        let Some(frame) = &self.frame else {
            return "no frame";
        };
        let Some(index) = self.ids.index(id) else {
            return "unknown id";
        };
        if frame.nodes[index].accessibility_hidden.unwrap_or(false) {
            return "hidden";
        }
        if self.inert {
            return "inert";
        }
        if self.accessibility_focus == Some(id) {
            return "already focused";
        }
        "not applicable"
    }

    pub fn accessibility(
        &mut self,
        id: u32,
        action: AccessibilityAction,
        page: PageTicket,
    ) -> (bool, Vec<Effect>) {
        if self.inert
            && !matches!(
                action,
                AccessibilityAction::Focus | AccessibilityAction::ClearFocus
            )
        {
            return (false, Vec::new());
        }
        let Some(frame) = self.frame.clone() else {
            return (false, Vec::new());
        };
        let Some(index) = self.ids.index(id) else {
            return (false, Vec::new());
        };
        let node = &frame.nodes[index];
        if node.accessibility_hidden.unwrap_or(false) {
            return (false, Vec::new());
        }
        self.pending_focus = None;
        self.slider.end();
        let visible = frame::visible(node);
        match action {
            AccessibilityAction::Focus | AccessibilityAction::ClearFocus => {
                let focus = action == AccessibilityAction::Focus;
                if focus == (self.accessibility_focus == Some(id)) {
                    return (false, Vec::new());
                }
                let mut effects = Vec::new();
                if focus && let Some(old) = self.accessibility_focus {
                    effects.push(Effect::Accessibility(AccessibilityEffect::FocusCleared {
                        id: old,
                    }));
                }
                self.accessibility_focus = focus.then_some(id);
                effects.push(Effect::Accessibility(if focus {
                    AccessibilityEffect::Focused { id }
                } else {
                    AccessibilityEffect::FocusCleared { id }
                }));
                (true, effects)
            }
            AccessibilityAction::ShowOnScreen if projection::meaningful(node) => {
                let moved = self.scroll.reveal(&frame, node);
                let mut effects = vec![Effect::StopFling];
                if moved {
                    effects.push(Effect::Redraw);
                }
                (moved || visible, effects)
            }
            _ if !visible => (false, Vec::new()),
            AccessibilityAction::SetProgress(value) if node.range.is_some() => {
                let effects = self.propose_number(index, value, page);
                (!effects.is_empty(), effects)
            }
            AccessibilityAction::ScrollForward | AccessibilityAction::ScrollBackward
                if node.range.is_some() =>
            {
                let range = node.range.clone().unwrap();
                let value = self.slider.value(node).unwrap_or(range.value);
                let direction = if action == AccessibilityAction::ScrollForward {
                    1
                } else {
                    -1
                };
                let effects =
                    self.propose_number(index, slider::shift(&range, value, direction), page);
                (!effects.is_empty(), effects)
            }
            AccessibilityAction::ScrollForward | AccessibilityAction::ScrollBackward
                if frame::draggable(node) =>
            {
                let direction = if action == AccessibilityAction::ScrollForward {
                    1
                } else {
                    -1
                };
                let moved = self.drag.step(&node.identity, direction).is_some();
                (
                    moved,
                    if moved {
                        vec![Effect::Redraw]
                    } else {
                        Vec::new()
                    },
                )
            }
            AccessibilityAction::SetText(text) if frame::editable(node) => {
                if text.len() > crate::text::input::MAX_TEXT_BYTES {
                    return (false, Vec::new());
                }
                (
                    true,
                    vec![Effect::EditText {
                        node: index,
                        text: Some(text),
                        start: -1,
                        end: -1,
                    }],
                )
            }
            AccessibilityAction::SetSelection(start, end) if frame::editable(node) => (
                true,
                vec![Effect::EditText {
                    node: index,
                    text: None,
                    start,
                    end,
                }],
            ),
            AccessibilityAction::ScrollForward | AccessibilityAction::ScrollBackward
                if node.scroll.is_some() =>
            {
                let scroll = node.scroll.as_ref().unwrap();
                let length = match scroll.axis {
                    ScrollAxis::Horizontal => node.visible_bounds[2],
                    ScrollAxis::Vertical => node.visible_bounds[3],
                } * 0.8;
                let distance = if action == AccessibilityAction::ScrollForward {
                    length
                } else {
                    -length
                };
                let moved = self.scroll.scroll_by(&scroll.id, distance);
                let mut effects = vec![Effect::StopFling];
                if moved {
                    effects.push(Effect::Redraw);
                }
                (moved, effects)
            }
            AccessibilityAction::ClearSelection if frame::plot_can_clear(node) => {
                (true, self.activate(index, None, false, Some(-1)))
            }
            AccessibilityAction::SelectItem(item)
                if frame::plot_enabled(node)
                    && node.plot.as_ref().is_some_and(|p| p.selectable(item)) =>
            {
                (true, self.activate(index, None, false, Some(item as i64)))
            }
            AccessibilityAction::Click if frame::interactive(node) && node.range.is_none() => {
                (true, self.activate(index, None, false, None))
            }
            _ => (false, Vec::new()),
        }
    }
}
