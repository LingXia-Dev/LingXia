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

    /// Run `act` on a match; `force` skips the visibility check, never the enabled one.
    pub fn automate(
        &mut self,
        selector: &str,
        index: Option<usize>,
        act: AutomationAct,
        force: bool,
        page: PageTicket,
    ) -> Result<Vec<Effect>, String> {
        let (id, value, enabled, editable, visible) = {
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
            )
        };
        if !force && !visible {
            return Err(format!("Element not interactable: {selector} is hidden"));
        }
        if !enabled {
            return Err(format!(
                "Element not interactable: {selector} is not enabled"
            ));
        }
        let action = match act {
            AutomationAct::Click => AccessibilityAction::Click,
            AutomationAct::ScrollIntoView => AccessibilityAction::ShowOnScreen,
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
}
