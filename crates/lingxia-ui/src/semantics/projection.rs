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
    Link,
    Heading,
    List,
    ListItem,
    Dialog,
    Alert,
    Status,
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
    SelectItem {
        index: usize,
        label: String,
    },
    ShowOnScreen,
    /// Copy the selected static text.
    Copy,
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
    /// 1..6 for headings.
    pub heading_level: Option<u8>,
    /// `polite` or `assertive`: announce text changes.
    pub live_region: Option<String>,
    /// Selected range of selectable static text, as UTF-16 offsets.
    pub text_selection: Option<[u32; 2]>,
}

/// Nodes that convey information or accept input.
pub fn meaningful(node: &NodeSnapshot) -> bool {
    !node.accessibility_hidden.unwrap_or(false)
        && (!node.text.is_empty()
            || node.role.is_some()
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
        // Links act like buttons: focusable, named by their text, clicked to activate.
        let button = matches!(node.tag, "button" | "link");
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
        } else if node.tag == "link" {
            Role::Link
        } else if let Some(role) = node.role.as_deref().filter(|_| !button) {
            match role {
                "heading" => Role::Heading,
                "list" => Role::List,
                "listitem" => Role::ListItem,
                "dialog" => Role::Dialog,
                "alert" => Role::Alert,
                _ => Role::Status,
            }
        } else if button {
            Role::Button
        } else if matches!(node.tag, "image" | "plot") {
            Role::Image
        } else {
            Role::Text
        };
        let label = node.accessibility_label.clone().unwrap_or_default();
        let name = if matches!(
            role,
            Role::Image | Role::Slider | Role::RadioGroup | Role::List | Role::Dialog
        ) || (matches!(
            role,
            Role::Heading | Role::ListItem | Role::Alert | Role::Status
        ) && !label.is_empty())
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
        } else if role == Role::List {
            position = Some((0, self.list_items(node).count()));
        } else if role == Role::ListItem {
            // Items belong to their nearest list ancestor, by identity prefix.
            if let Some(list) = self.owning_list(node) {
                let row = self
                    .list_items(list)
                    .take_while(|(_, item)| item.identity != node.identity)
                    .count();
                position = Some((row, 1));
            }
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
        if node.selectable.is_some() && !self.inert {
            actions.push(Action::SetSelection);
            if node.selection_range.is_some_and(|[a, b]| a != b) {
                actions.push(Action::Copy);
            }
        }
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
            heading_level: node.heading_level.or((role == Role::Heading).then_some(2)),
            live_region: node.live_region.clone(),
            text_selection: node.selection_range,
        })
    }

    fn owning_list(&self, item: &NodeSnapshot) -> Option<&NodeSnapshot> {
        self.frame
            .nodes
            .iter()
            .filter(|list| {
                list.role.as_deref() == Some("list")
                    && item.identity.starts_with(&format!("{}/", list.identity))
            })
            .max_by_key(|list| list.identity.len())
    }

    fn list_items<'a>(
        &'a self,
        list: &'a NodeSnapshot,
    ) -> impl Iterator<Item = (usize, &'a NodeSnapshot)> + 'a {
        self.frame
            .nodes
            .iter()
            .enumerate()
            .filter(move |(_, item)| {
                item.role.as_deref() == Some("listitem")
                    && !item.accessibility_hidden.unwrap_or(false)
                    && self
                        .owning_list(item)
                        .is_some_and(|owner| owner.identity == list.identity)
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
    /// A live region's spoken text changed; `assertive` interrupts speech.
    Announce {
        id: u32,
        text: String,
        assertive: bool,
    },
    ContentChanged,
}

/// What a live region says: its own text, else its visible descendants' text in order.
fn spoken(frame: &PresentedFrame, node: &NodeSnapshot) -> String {
    if !node.text.is_empty() {
        return node.text.clone();
    }
    let prefix = format!("{}/", node.identity);
    frame
        .nodes
        .iter()
        .filter(|other| {
            other.identity.starts_with(&prefix)
                && !other.text.is_empty()
                && !other.accessibility_hidden.unwrap_or(false)
        })
        .map(|other| other.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
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
        if let (Some(kind), Some(previous)) = (&node.live_region, previous)
            && !node.accessibility_hidden.unwrap_or(false)
        {
            let text = spoken(next, node);
            if !text.is_empty() && text != spoken(previous, prior) {
                events.push(Event::Announce {
                    id,
                    text,
                    assertive: kind == "assertive",
                });
            }
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
