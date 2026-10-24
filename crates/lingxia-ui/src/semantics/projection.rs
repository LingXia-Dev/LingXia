//! The accessibility tree an adapter publishes: roles, names, values, states,
//! actions, relations and geometry. Adapters map it to OS APIs and nothing else.
use super::ids::SemanticIds;
use crate::interaction::frame::{
    PresentedFrame, editable, plot_can_clear, plot_enabled, slider_enabled, visible,
};
use crate::scene::node::{NodeSnapshot, ScrollAxis};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    Button,
    CheckBox,
    Switch,
    RadioGroup,
    RadioButton,
    TextField,
    ScrollView,
    HorizontalScrollView,
    Image,
    Slider,
    Text,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Action {
    Click,
    ScrollForward,
    ScrollBackward,
    SetText,
    SetSelection,
    SetProgress,
    ClearSelection,
    SelectItem { index: usize, label: String },
    ShowOnScreen,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Range {
    pub min: f64,
    pub max: f64,
    pub value: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TextField {
    /// Displayed text; secure values are masked.
    pub text: String,
    pub hint: String,
    pub secure: bool,
    pub multiline: bool,
    /// Logical UTF-16 selection while focused.
    pub selection: Option<[u32; 2]>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SemanticNode {
    pub id: u32,
    pub index: usize,
    pub role: Role,
    /// Spoken text: label, image alternative or content text.
    pub name: String,
    /// Visible text content, distinct from an explicit label.
    pub text: String,
    pub test_id: String,
    /// Logical bounds in surface coordinates; empty when clipped away.
    pub bounds: [f32; 4],
    pub visible: bool,
    pub enabled: bool,
    pub focusable: bool,
    pub clickable: bool,
    pub focused: bool,
    pub checked: Option<bool>,
    pub scrollable: bool,
    pub range: Option<Range>,
    pub text_field: Option<TextField>,
    /// Virtual id of the containing radio group.
    pub parent: Option<u32>,
    pub children: Vec<u32>,
    /// Row within its radio group and the group's option count.
    pub position: Option<(usize, usize)>,
    pub actions: Vec<Action>,
}

/// Nodes that convey information or accept input.
pub fn meaningful(node: &NodeSnapshot) -> bool {
    !node.accessibility_hidden.unwrap_or(false)
        && (!node.text.is_empty()
            || node.tag == "radio"
            || node.scroll.is_some()
            || node.checked.is_some()
            || node.range.is_some()
            || node.input.is_some()
            || node.tag == "image"
            || (node.tag == "plot"
                && node
                    .accessibility_label
                    .as_deref()
                    .is_some_and(|l| !l.is_empty())))
}

fn masked(input: &crate::scene::node::InputSnapshot) -> String {
    let value = input
        .edit
        .as_ref()
        .map(|edit| edit.text.as_str())
        .unwrap_or(&input.value);
    if input.secure {
        "\u{2022}".repeat(value.encode_utf16().count())
    } else {
        value.to_owned()
    }
}

pub struct Projection<'a> {
    pub frame: &'a PresentedFrame,
    pub ids: &'a SemanticIds,
    /// Keyboard focus ring, when the editor does not own focus.
    pub keyboard_focus: Option<usize>,
    /// An interrupted session leaves content readable but inert.
    pub inert: bool,
}

impl Projection<'_> {
    /// Top-level semantic children; radio options are reached through their group.
    pub fn roots(&self) -> Vec<u32> {
        self.frame
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.tag != "radio-option" && meaningful(node) && visible(node))
            .filter_map(|(index, _)| self.ids.id(index))
            .collect()
    }

    /// The topmost meaningful, visible node under a logical point.
    pub fn hit(&self, point: [f32; 2]) -> Option<u32> {
        self.frame
            .hit_order()
            .find(|(_, node)| {
                meaningful(node)
                    && visible(node)
                    && crate::interaction::frame::contains(node, point, false)
            })
            .and_then(|(index, _)| self.ids.id(index))
    }

    fn members(&self, group: &NodeSnapshot) -> impl Iterator<Item = (usize, &NodeSnapshot)> {
        self.frame
            .nodes
            .iter()
            .enumerate()
            .filter(move |(_, node)| {
                node.tag == "radio-option" && node.radio_group.as_deref() == Some(&group.identity)
            })
    }

    pub fn node(&self, id: u32) -> Option<SemanticNode> {
        let index = self.ids.index(id)?;
        let node = self.frame.node(index)?;
        if node.accessibility_hidden.unwrap_or(false) {
            return None;
        }
        let activatable = node.on_click.is_some_and(|handle| handle != 0);
        let button = node.tag == "button";
        let check = node.checked.is_some();
        let input = node.input.as_ref();
        let role = if input.is_some() {
            Role::TextField
        } else if let Some(scroll) = &node.scroll {
            if scroll.axis == ScrollAxis::Horizontal {
                Role::HorizontalScrollView
            } else {
                Role::ScrollView
            }
        } else if node.range.is_some() {
            Role::Slider
        } else if node.tag == "radio" {
            Role::RadioGroup
        } else if node.tag == "radio-option" {
            Role::RadioButton
        } else if check {
            if node.tag == "switch" {
                Role::Switch
            } else {
                Role::CheckBox
            }
        } else if button {
            Role::Button
        } else if matches!(node.tag, "image" | "plot") {
            Role::Image
        } else {
            Role::Text
        };
        let label = node.accessibility_label.clone().unwrap_or_default();
        let name = if matches!(role, Role::Image | Role::Slider | Role::RadioGroup)
            || (input.is_some() && !label.is_empty())
        {
            label
        } else {
            node.text.clone()
        };
        let mut enabled = match input {
            Some(input) => input.enabled,
            None => !(button || check) || activatable,
        };
        let mut actions = Vec::new();
        let mut focusable = button || check || input.is_some();
        if let Some(range) = &node.range {
            enabled = slider_enabled(node);
            focusable = true;
            if enabled {
                actions.push(Action::SetProgress);
                if range.value < range.max {
                    actions.push(Action::ScrollForward);
                }
                if range.value > range.min {
                    actions.push(Action::ScrollBackward);
                }
            }
        }
        let mut children = Vec::new();
        let mut parent = None;
        let mut position = None;
        if node.tag == "radio" {
            let members: Vec<_> = self.members(node).collect();
            enabled = members
                .iter()
                .any(|(_, member)| member.on_click.is_some_and(|h| h != 0));
            children = members
                .iter()
                .filter_map(|(index, _)| self.ids.id(*index))
                .collect();
            position = Some((0, members.len()));
        } else if node.tag == "radio-option" {
            let group = node.radio_group.as_deref();
            parent = self
                .frame
                .nodes
                .iter()
                .position(|candidate| {
                    candidate.tag == "radio" && Some(candidate.identity.as_str()) == group
                })
                .and_then(|index| self.ids.id(index));
            let row = self.frame.nodes[..index]
                .iter()
                .filter(|candidate| {
                    candidate.tag == "radio-option" && candidate.radio_group.as_deref() == group
                })
                .count();
            position = Some((row, 1));
        }
        let mut clickable = (button || check || input.is_some()) && enabled && node.range.is_none();
        if plot_enabled(node) {
            focusable = true;
            clickable = true;
            if plot_can_clear(node) {
                actions.push(Action::ClearSelection);
            }
            let plot = node.plot.as_ref().unwrap();
            for (item, value) in plot.values.iter().enumerate() {
                if plot.selectable(item) {
                    actions.push(Action::SelectItem {
                        index: item,
                        label: format!("Select item {}: {value}", item + 1),
                    });
                }
            }
        }
        if let Some(scroll) = &node.scroll {
            if scroll.offset < scroll.max_offset {
                actions.push(Action::ScrollForward);
            }
            if scroll.offset > 0.0 {
                actions.push(Action::ScrollBackward);
            }
        }
        let text_field = input.map(|input| {
            if input.enabled {
                actions.push(Action::SetText);
                actions.push(Action::SetSelection);
            }
            TextField {
                text: masked(input),
                hint: input.placeholder.clone(),
                secure: input.secure,
                multiline: node.tag == "textarea",
                selection: input
                    .edit_token
                    .filter(|token| *token > 0)
                    .and(input.edit.as_ref())
                    .map(|edit| edit.selection),
            }
        });
        if meaningful(node) {
            actions.push(Action::ShowOnScreen);
        }
        if self.inert {
            enabled = false;
            clickable = false;
        }
        if clickable {
            actions.push(Action::Click);
        }
        let focused = match input {
            Some(input) => input.edit_token.is_some_and(|token| token > 0),
            None => self.keyboard_focus == Some(index),
        };
        Some(SemanticNode {
            id,
            index,
            role,
            name,
            text: node.text.clone(),
            test_id: node.test_id.clone(),
            bounds: node.visible_bounds,
            visible: visible(node),
            enabled,
            focusable,
            clickable,
            focused,
            checked: node.checked,
            scrollable: node
                .scroll
                .as_ref()
                .is_some_and(|scroll| scroll.max_offset > 0.0),
            range: node.range.as_ref().map(|range| Range {
                min: range.min,
                max: range.max,
                value: range.value,
            }),
            text_field,
            parent,
            children,
            position,
            actions,
        })
    }

    /// The node holding input focus: the active editor, else the keyboard focus ring.
    pub fn input_focus(&self) -> Option<u32> {
        self.frame
            .nodes
            .iter()
            .position(|node| {
                editable(node)
                    && visible(node)
                    && node
                        .input
                        .as_ref()
                        .is_some_and(|input| input.edit_token.is_some_and(|token| token > 0))
            })
            .or(self.keyboard_focus)
            .and_then(|index| self.ids.id(index))
    }
}

/// Announcements caused by moving from one presented frame to the next.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Event {
    TextChanged {
        id: u32,
        before: String,
        after: String,
        secure: bool,
    },
    SelectionChanged {
        id: u32,
        text: String,
        start: u32,
        end: u32,
        secure: bool,
    },
    ValueChanged {
        id: u32,
    },
    ContentChanged,
}

pub fn changes(
    previous: Option<&PresentedFrame>,
    next: &PresentedFrame,
    ids: &SemanticIds,
) -> Vec<Event> {
    let before: std::collections::HashMap<&str, &NodeSnapshot> = previous
        .map(|frame| {
            frame
                .nodes
                .iter()
                .map(|node| (node.identity.as_str(), node))
                .collect()
        })
        .unwrap_or_default();
    let mut events = Vec::new();
    for (index, node) in next.nodes.iter().enumerate() {
        let (Some(id), Some(prior)) = (ids.id(index), before.get(node.identity.as_str())) else {
            continue;
        };
        if let (Some(input), Some(old)) = (&node.input, &prior.input) {
            let (value, was) = (masked(input), masked(old));
            if value != was {
                events.push(Event::TextChanged {
                    id,
                    before: was.clone(),
                    after: value.clone(),
                    secure: input.secure,
                });
            }
            let selection = |input: &crate::scene::node::InputSnapshot| {
                input
                    .edit_token
                    .filter(|t| *t > 0)
                    .and(input.edit.as_ref())
                    .map(|edit| edit.selection)
            };
            if let Some([start, end]) = selection(input)
                && (selection(old) != Some([start, end]) || value != was)
            {
                events.push(Event::SelectionChanged {
                    id,
                    text: value,
                    start,
                    end,
                    secure: input.secure,
                });
            }
        }
        if let (Some(range), Some(old)) = (&node.range, &prior.range)
            && range.value != old.value
        {
            events.push(Event::ValueChanged { id });
        }
    }
    let changed = previous.is_none_or(|previous| {
        serde_json::to_string(&previous.nodes).ok() != serde_json::to_string(&next.nodes).ok()
    });
    if changed {
        events.push(Event::ContentChanged);
    }
    events
}
