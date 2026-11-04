//! Test automation over the presented frame: queries answer like a WebView DOM
//! query; actions run through the same accessibility paths as assistive input.
//! Failures use the WebView driver's message forms so both map to one error code.
use super::{AccessibilityAction, Effect, ViewInteraction};
use crate::semantics::query;
use crate::session::lifecycle::PageTicket;

/// One locator action: `selector` + `index` (default the first match).
#[derive(Clone, Debug, PartialEq)]
pub enum AutomationAct {
    Click,
    /// Replace an editor's text.
    Fill(String),
    /// Append to an editor's text.
    Type(String),
    ScrollIntoView,
    /// A key by its DOM name (`Enter`, `Tab`, `Shift+Tab`, `ArrowUp`, `Home`, ` `, …),
    /// on the match or, with an empty selector, on what has focus.
    Press(String),
    /// Choose a `<select>` option by value, else by label.
    SelectOption(String),
    /// Select the word at the match's center (text and editors), as a long press or a
    /// desktop double click does; a click on anything else.
    DoubleClick,
    /// A semantic action by name: an `aria-actions` control's name, or `increment`,
    /// `decrement`, `dismiss`.
    Action(String),
}

fn key(name: &str) -> Result<(super::Key, bool), String> {
    use super::Key;
    let (shift, name) = match name.strip_prefix("Shift+") {
        Some(rest) => (true, rest),
        None => (false, name),
    };
    let key = match name {
        "Tab" => Key::Tab,
        "ArrowUp" => Key::Up,
        "ArrowDown" => Key::Down,
        "ArrowLeft" => Key::Left,
        "ArrowRight" => Key::Right,
        "Home" => Key::Home,
        "End" => Key::End,
        "Enter" | " " | "Space" => Key::Activate,
        "Escape" => Key::Escape,
        "PageUp" => Key::PageUp,
        "PageDown" => Key::PageDown,
        // A printable character: a menu's or select's type-ahead.
        other if other.chars().count() == 1 && !other.starts_with(char::is_control) => {
            Key::Char(other.chars().next().unwrap())
        }
        other => return Err(format!("E_UNSUPPORTED: key {other:?} on a GPU page")),
    };
    Ok((key, shift))
}

/// The nearest control enclosing `node`, enabled or not.
fn control_ancestor(frame: &super::PresentedFrame, node: usize) -> Option<usize> {
    let target = &frame.nodes[node].identity;
    frame
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, candidate)| {
            matches!(
                candidate.tag,
                "button" | "checkbox" | "switch" | "radio" | "link" | "select"
            ) && target.starts_with(&format!("{}/", candidate.identity))
        })
        .max_by_key(|(_, candidate)| candidate.identity.len())
        .map(|(index, _)| index)
}

impl ViewInteraction {
    /// The WebView query payload for `selector` over the presented frame.
    pub fn automation_query(
        &self,
        selector: &str,
        index: Option<usize>,
        all: bool,
        max_text: Option<usize>,
    ) -> Result<serde_json::Value, String> {
        let projection = self
            .semantics()
            .ok_or("E_PAGE_NOT_READY: no presented frame")?;
        query::query(&projection, selector, index, all, max_text)
    }

    /// The accessibility tree of the presented frame (`semantics::query::tree`).
    pub fn automation_tree(&self) -> Result<serde_json::Value, String> {
        let projection = self
            .semantics()
            .ok_or("E_PAGE_NOT_READY: no presented frame")?;
        Ok(query::tree(&projection))
    }

    /// Run `act` on a match; `force` skips the visibility check, never the enabled one.
    pub fn automate(
        &mut self,
        selector: &str,
        index: Option<usize>,
        act: AutomationAct,
        force: bool,
        page: PageTicket,
    ) -> Result<Vec<Effect>, String> {
        if self.parked {
            return Err("E_PAGE_NOT_READY: the page is parked (it left the stack); queries read its last frame".into());
        }
        if let AutomationAct::Press(name) = &act {
            return self.press(selector, index, name, page);
        }
        if let AutomationAct::SelectOption(choice) = &act {
            return self.select_option(selector, index, choice);
        }
        let (id, value, enabled, editable, visible, in_viewport) = {
            let projection = self
                .semantics()
                .ok_or("E_PAGE_NOT_READY: no presented frame")?;
            let matches = query::select(&projection, selector)?;
            let position = index.unwrap_or(0);
            let mut node = *matches
                .get(position)
                .ok_or_else(|| format!("Element not found: {selector} (index {position})"))?;
            // A click on content inside a control lands on that control, as a finger's does.
            if matches!(act, AutomationAct::Click | AutomationAct::DoubleClick)
                && let Some(frame) = &self.frame
                && !crate::interaction::frame::interactive(&frame.nodes[node])
                && !(act == AutomationAct::DoubleClick && frame.nodes[node].selectable.is_some())
                && let Some(control) = control_ancestor(frame, node)
            {
                node = control;
            }
            let id = projection
                .ids
                .id(node)
                .ok_or_else(|| format!("Element not interactable: {selector} is hidden"))?;
            let described = query::describe(&projection, node, position, matches.len(), None);
            let semantic = projection.node(id);
            (
                id,
                described["value"].as_str().unwrap_or_default().to_owned(),
                semantic.as_ref().is_some_and(|s| s.enabled),
                described["editable"].as_bool().unwrap_or(false),
                described["visible"].as_bool().unwrap_or(false),
                described["inViewport"].as_bool().unwrap_or(false),
            )
        };
        if !force && !visible {
            return Err(format!("Element not interactable: {selector} is hidden"));
        }
        // Input reaches only what the presented frame shows; bring the match on screen first.
        if !in_viewport && act != AutomationAct::ScrollIntoView {
            return Err(format!(
                "Element not interactable: {selector} is outside the viewport"
            ));
        }
        if !enabled {
            return Err(format!(
                "Element not interactable: {selector} is not enabled"
            ));
        }
        if act == AutomationAct::ScrollIntoView {
            // Any match can be revealed, published to assistive input or not.
            let frame = self
                .frame
                .clone()
                .ok_or("E_PAGE_NOT_READY: no presented frame")?;
            let node = self.ids.index(id).map(|index| &frame.nodes[index]);
            let moved = node.is_some_and(|node| self.scroll.reveal(&frame, node));
            if !moved && !in_viewport {
                return Err(format!(
                    "Element not interactable: no scroll brings {selector} on screen"
                ));
            }
            return Ok(if moved {
                vec![Effect::StopFling, Effect::Redraw]
            } else {
                Vec::new()
            });
        }
        if act == AutomationAct::DoubleClick
            && let Some(effects) = self.select_word(id)
        {
            return Ok(effects);
        }
        let click = matches!(act, AutomationAct::Click | AutomationAct::DoubleClick);
        let action = match act {
            AutomationAct::Click | AutomationAct::DoubleClick => AccessibilityAction::Click,
            AutomationAct::ScrollIntoView
            | AutomationAct::Press(_)
            | AutomationAct::SelectOption(_) => {
                unreachable!("handled above")
            }
            AutomationAct::Action(name) => {
                let projection = self
                    .semantics()
                    .ok_or("E_PAGE_NOT_READY: no presented frame")?;
                let actions = projection
                    .node(id)
                    .map(|node| node.actions)
                    .unwrap_or_default();
                let wanted = name.to_lowercase();
                let action = actions
                    .iter()
                    .find_map(|action| match (action, wanted.as_str()) {
                        (crate::semantics::projection::Action::Custom { index, label }, _)
                            if label.to_lowercase() == wanted =>
                        {
                            Some(AccessibilityAction::Custom(*index))
                        }
                        (crate::semantics::projection::Action::ScrollForward, "increment") => {
                            Some(AccessibilityAction::ScrollForward)
                        }
                        (crate::semantics::projection::Action::ScrollBackward, "decrement") => {
                            Some(AccessibilityAction::ScrollBackward)
                        }
                        (crate::semantics::projection::Action::Dismiss, "dismiss") => {
                            Some(AccessibilityAction::Dismiss)
                        }
                        _ => None,
                    });
                match action {
                    Some(action) => action,
                    None => {
                        return Err(format!(
                            "Element not interactable: {selector} has no action {name:?}"
                        ));
                    }
                }
            }
            AutomationAct::Fill(_) | AutomationAct::Type(_) if !editable => {
                return Err(format!(
                    "Element not interactable: {selector} is not editable"
                ));
            }
            AutomationAct::Fill(text) => AccessibilityAction::SetText(text),
            AutomationAct::Type(text) => AccessibilityAction::SetText(value + &text),
        };
        let inert = self.inert;
        let (handled, effects) = self.accessibility(id, action, page);
        if !handled {
            let reason = if inert {
                "the page is behind an open dialog; act inside the dialog or close it"
            } else if click {
                "it is not a control and no control holds it; click a button, link or other control"
            } else {
                "it did not take the input"
            };
            return Err(format!("Element not interactable: {selector}: {reason}"));
        }
        Ok(effects)
    }

    /// The word at a text or editor node's center, as the pointer's long press selects it;
    /// `None` for nodes without text to select.
    fn select_word(&mut self, id: u32) -> Option<Vec<Effect>> {
        let frame = self.frame.clone()?;
        let index = self.ids.index(id)?;
        let node = &frame.nodes[index];
        let [x, y, w, h] = node.visible_bounds;
        let center = [x + w / 2.0, y + h / 2.0];
        if node.input.as_ref().is_some_and(|input| input.enabled) {
            return Some(self.activate(index, Some(center), true, None));
        }
        node.selectable?;
        self.dispatches.push(super::Dispatch::SelectText {
            commit: frame.commit,
            identity: node.identity.clone(),
            point: center,
        });
        Some(vec![Effect::Dispatch])
    }

    /// A key on the match (focused first) or on whatever holds focus.
    fn press(
        &mut self,
        selector: &str,
        index: Option<usize>,
        name: &str,
        page: PageTicket,
    ) -> Result<Vec<Effect>, String> {
        let (key, shift) = key(name)?;
        let frame = self
            .frame
            .clone()
            .ok_or("E_PAGE_NOT_READY: no presented frame")?;
        if !selector.is_empty() {
            let node = {
                let projection = self
                    .semantics()
                    .ok_or("E_PAGE_NOT_READY: no presented frame")?;
                let matches = query::select(&projection, selector)?;
                let position = index.unwrap_or(0);
                *matches
                    .get(position)
                    .ok_or_else(|| format!("Element not found: {selector} (index {position})"))?
            };
            let target = &frame.nodes[node];
            if target.input.is_some() {
                if key != super::Key::Activate {
                    return Err(format!(
                        "E_UNSUPPORTED: {name} in an editor; use type or fill"
                    ));
                }
                // Enter in an editor is its input method action: submit, or move on for next/previous.
                self.dispatches.push(super::Dispatch::SubmitEditor {
                    commit: frame.commit,
                    identity: target.identity.clone(),
                });
                return Ok(vec![Effect::Dispatch]);
            }
            self.keyboard_focus = Some(node);
        }
        let (handled, effects) = self.key(key, shift, false, page);
        if !handled {
            return Err(format!("Element not interactable: {name} had no effect"));
        }
        Ok(effects)
    }

    fn select_option(
        &mut self,
        selector: &str,
        index: Option<usize>,
        choice: &str,
    ) -> Result<Vec<Effect>, String> {
        let frame = self
            .frame
            .clone()
            .ok_or("E_PAGE_NOT_READY: no presented frame")?;
        let node = {
            let projection = self
                .semantics()
                .ok_or("E_PAGE_NOT_READY: no presented frame")?;
            let matches = query::select(&projection, selector)?;
            let position = index.unwrap_or(0);
            *matches
                .get(position)
                .ok_or_else(|| format!("Element not found: {selector} (index {position})"))?
        };
        let target = &frame.nodes[node];
        let select = target
            .select
            .as_ref()
            .ok_or_else(|| format!("Element not interactable: {selector} is not a select"))?;
        if select.on_select.is_none() {
            return Err(format!(
                "Element not interactable: {selector} is not enabled"
            ));
        }
        let option = select
            .options
            .iter()
            .position(|option| option.value == choice)
            .or_else(|| {
                select
                    .options
                    .iter()
                    .position(|option| option.label == choice)
            })
            .ok_or_else(|| {
                format!("Element not interactable: {selector} has no option {choice:?}")
            })?;
        // Waiting cannot enable an option, so this fails at once instead of being retried.
        if !select.options[option].enabled {
            return Err(format!("option {choice:?} of {selector} is disabled"));
        }
        self.dispatches.push(super::Dispatch::Select {
            commit: frame.commit,
            identity: target.identity.clone(),
            index: option,
            value: select.options[option].value.clone(),
        });
        Ok(vec![Effect::Dispatch])
    }
}
