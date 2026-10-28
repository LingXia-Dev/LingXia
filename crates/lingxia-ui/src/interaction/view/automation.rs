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
        other => return Err(format!("E_UNSUPPORTED: key {other:?} on a GPU page")),
    };
    Ok((key, shift))
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
            let node = *matches
                .get(position)
                .ok_or_else(|| format!("Element not found: {selector} (index {position})"))?;
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
        let action = match act {
            AutomationAct::Click | AutomationAct::DoubleClick => AccessibilityAction::Click,
            AutomationAct::ScrollIntoView
            | AutomationAct::Press(_)
            | AutomationAct::SelectOption(_) => {
                unreachable!("handled above")
            }
            AutomationAct::Fill(_) | AutomationAct::Type(_) if !editable => {
                return Err(format!(
                    "Element not interactable: {selector} is not editable"
                ));
            }
            AutomationAct::Fill(text) => AccessibilityAction::SetText(text),
            AutomationAct::Type(text) => AccessibilityAction::SetText(value + &text),
        };
        let (handled, effects) = self.accessibility(id, action, page);
        if !handled {
            return Err(format!(
                "Element not interactable: {selector} refused the action"
            ));
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
        });
        Ok(vec![Effect::Dispatch])
    }
}
