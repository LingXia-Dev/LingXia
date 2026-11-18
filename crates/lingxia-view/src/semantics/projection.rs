//! The accessibility tree an adapter publishes: roles, names, values, states,
//! actions, relations and geometry. Adapters map it to OS APIs and nothing else.
use super::ids::SemanticIds;
use crate::interaction::frame::{
    PresentedFrame, clickable, editable, plot_can_clear, plot_enabled, slider_enabled, steppable,
    visible,
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
    ComboBox,
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
    /// A two-dimensional set of cells (a date picker's month).
    Grid,
    /// One cell of a grid; actionable when it is a button.
    GridCell,
    /// A set of options to choose one from (a picker column).
    ListBox,
    /// One choice of a list box.
    Option,
    /// A popover of actions.
    Menu,
    /// One action of a menu; a button.
    MenuItem,
    /// A video player's picture; its media state rides on the node.
    Video,
    /// An amount done; its range is absent while indeterminate.
    ProgressBar,
    /// A dialog that interrupts with something to confirm or acknowledge.
    AlertDialog,
    /// Related content read as one named set.
    Group,
    /// A short description shown beside the control it describes.
    Tooltip,
    /// A row of tabs; one tab stop, arrows move between them.
    TabList,
    /// One tab of a tab list; a button.
    Tab,
    /// The content a tab shows.
    TabPanel,
    /// A row of controls sharing one tab stop.
    Toolbar,
    /// A hierarchy of items; `heading_level` carries each item's depth.
    Tree,
    /// One item of a tree; a button, expandable when it states `expanded`.
    TreeItem,
    /// A row of a grid.
    Row,
    /// A menu item with a checked state.
    MenuItemCheckBox,
    /// One of a set of menu items, only one checked.
    MenuItemRadio,
    /// An embedded web document (`<webview>`).
    WebView,
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
    /// Copy the selection: selected static text, or an editor's (not a secure one).
    Copy,
    /// Copy and delete an enabled, non-secure editor's selection.
    Cut,
    /// Replace an enabled, non-secure editor's selection with the clipboard.
    Paste,
    /// One of a swipe row's `aria-actions`, by position, named by its control.
    Custom {
        index: usize,
        label: String,
    },
    /// Close the dialog, as Escape or back would.
    Dismiss,
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
    /// Partly checked: assistive tech hears mixed.
    pub mixed: bool,
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
    /// Authored states; `None` when the element has no such state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pressed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expanded: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub busy: Option<bool>,
    /// A select's shown value: the chosen option's label, else its placeholder.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// A scroll region's position and extent along its axis, in logical pixels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scroll: Option<ScrollPosition>,
    /// The text of the elements `aria-describedby` names, read after the name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invalid: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct ScrollPosition {
    pub offset: f32,
    pub max_offset: f32,
    pub horizontal: bool,
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
            || matches!(node.tag, "image" | "video" | "webview")
            || node.drag.is_some()
            || steppable(node)
            || (node.tag == "plot"
                && node
                    .accessibility_label
                    .as_deref()
                    .is_some_and(|l| !l.is_empty())))
}

fn masked(input: &crate::scene::node::InputSnapshot) -> String {
    if let Some(display) = &input.display {
        return display.clone();
    }
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
    /// Live snap points of `drag` elements, ahead of the frame once input moved them.
    pub drags: Option<&'a crate::interaction::drag::DragModel>,
    /// Host chrome, system bars and the keyboard over the root's edges (top, right, bottom,
    /// left): what lies under them is outside the viewport.
    pub obscured: [f32; 4],
    /// The smallest box a finger press reaches, logical pixels; 0 for a fine pointer.
    pub touch_target: f32,
}

impl Projection<'_> {
    /// Top-level semantic nodes; the rest are reached through their container.
    pub fn roots(&self) -> Vec<u32> {
        (0..self.frame.nodes.len())
            .filter(|index| self.published(*index) && self.container_of(*index).is_none())
            .filter_map(|index| self.ids.id(index))
            .collect()
    }

    /// Every node, a container before its children: the order a screen reader's swipe walks,
    /// as Android's virtual tree yields it.
    pub fn reading_order(&self) -> Vec<u32> {
        let mut order = Vec::new();
        let mut pending: Vec<u32> = self.roots();
        pending.reverse();
        while let Some(id) = pending.pop() {
            order.push(id);
            if let Some(node) = self.node(id) {
                pending.extend(node.children.iter().rev());
            }
        }
        order
    }

    fn published(&self, index: usize) -> bool {
        let node = &self.frame.nodes[index];
        meaningful(node) && visible(node) && !self.covered(node)
    }

    /// A control a swipe row names in `aria-actions` stays out of linear navigation while the
    /// row rests closed over it: the row offers it as an action instead.
    fn covered(&self, node: &NodeSnapshot) -> bool {
        let Some(id) = &node.element_id else {
            return false;
        };
        self.frame.nodes.iter().any(|row| {
            row.aria_actions.contains(id)
                && row.drag.as_ref().is_some_and(|drag| {
                    let at = self
                        .drags
                        .and_then(|drags| drags.target(&row.identity))
                        .unwrap_or(drag.index);
                    drag.points.get(at).is_some_and(|point| point.abs() < 0.5)
                })
        })
    }

    /// The controls a node's `aria-actions` names, in order, that the frame holds.
    pub fn action_targets(&self, node: &NodeSnapshot) -> Vec<usize> {
        node.aria_actions
            .iter()
            .filter_map(|id| {
                self.frame
                    .nodes
                    .iter()
                    .position(|other| other.element_id.as_ref() == Some(id))
            })
            .collect()
    }

    /// A published layout node with a role (or a scroll) groups the published nodes
    /// below it; unrolled wrappers collapse, so their content joins the next container up.
    fn is_container(&self, index: usize) -> bool {
        matches!(
            self.frame.nodes[index].tag,
            "column" | "scroll" | "list" | "radio"
        ) && self.published(index)
    }

    /// The nearest container ancestor. Nodes are in preorder and identities extend
    /// their parent's, so ancestors precede `index` with a prefix identity.
    fn container_of(&self, index: usize) -> Option<usize> {
        let node = &self.frame.nodes[index];
        if node.tag == "radio-option" {
            let group = node.radio_group.as_deref();
            return self.frame.nodes.iter().position(|candidate| {
                candidate.tag == "radio" && Some(candidate.identity.as_ref()) == group
            });
        }
        (0..index).rev().find(|&candidate| {
            let ancestor = &self.frame.nodes[candidate].identity;
            node.identity.len() > ancestor.len()
                && node.identity.starts_with(&**ancestor)
                && node.identity.as_bytes()[ancestor.len()] == b'/'
                && self.is_container(candidate)
        })
    }

    fn contained(&self, index: usize) -> Vec<u32> {
        let identity = &self.frame.nodes[index].identity;
        let subtree = |other: &NodeSnapshot| {
            other.identity.len() > identity.len()
                && other.identity.starts_with(&**identity)
                && other.identity.as_bytes()[identity.len()] == b'/'
        };
        (index + 1..self.frame.nodes.len())
            .take_while(|other| subtree(&self.frame.nodes[*other]))
            .filter(|other| self.published(*other) && self.container_of(*other) == Some(index))
            .filter_map(|other| self.ids.id(other))
            .collect()
    }

    /// The topmost meaningful, visible node under a logical point, or the small control a
    /// finger there presses (`minimum` as in `press_hit`) when it lies inside it.
    pub fn hit(&self, point: [f32; 2], minimum: f32) -> Option<u32> {
        let exact = self.frame.hit_order().find(|(_, node)| {
            meaningful(node)
                && visible(node)
                && crate::interaction::frame::contains(node, point, false)
        });
        let reached = self
            .frame
            .press_hit(point, minimum)
            .filter(|index| Some(*index) != self.frame.hit(point))
            .filter(|index| {
                let node = &self.frame.nodes[*index];
                meaningful(node)
                    && exact
                        .is_none_or(|(_, exact)| crate::interaction::frame::descends(node, exact))
            });
        reached
            .or(exact.map(|(index, _)| index))
            .and_then(|index| self.ids.id(index))
    }

    fn members(&self, group: &NodeSnapshot) -> impl Iterator<Item = (usize, &NodeSnapshot)> {
        self.frame
            .nodes
            .iter()
            .enumerate()
            .filter(move |(_, node)| {
                node.tag == "radio-option" && node.radio_group.as_deref() == Some(&*group.identity)
            })
    }

    pub fn node(&self, id: u32) -> Option<SemanticNode> {
        let index = self.ids.index(id)?;
        if self.covered(self.frame.node(index)?) {
            return None;
        }
        self.reachable_node(id)
    }

    /// [`Self::node`] as input sees it: `aria-hidden` keeps a control from assistive tech, not
    /// from a tap, so a test id that targets one acts on it.
    pub fn acting_node(&self, id: u32) -> Option<SemanticNode> {
        let index = self.ids.index(id)?;
        if self.covered(self.frame.node(index)?) {
            return None;
        }
        self.semantic_node(id, true)
    }

    /// [`Self::node`] for a control a closed swipe row covers too, to name the row's actions.
    fn reachable_node(&self, id: u32) -> Option<SemanticNode> {
        self.semantic_node(id, false)
    }

    fn semantic_node(&self, id: u32, hidden: bool) -> Option<SemanticNode> {
        let index = self.ids.index(id)?;
        let node = self.frame.node(index)?;
        if !hidden && node.accessibility_hidden.unwrap_or(false) {
            return None;
        }
        // Links act like buttons: focusable, named by their text, clicked to activate.
        let button = matches!(node.tag, "button" | "link");
        let check = node.checked.is_some();
        let input = node.input.as_ref();
        let select = node.select.as_ref();
        let spin = input.is_some() && node.role.as_deref() == Some("spinbutton");
        let role = if select.is_some() {
            Role::ComboBox
        } else if input.is_some() {
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
        } else if let Some(role) = node.role.as_deref().filter(|role| {
            !button
                || matches!(
                    *role,
                    "gridcell"
                        | "option"
                        | "menuitem"
                        | "tab"
                        | "radio"
                        | "treeitem"
                        | "menuitemcheckbox"
                        | "menuitemradio"
                        | "combobox"
                )
        }) {
            match role {
                "heading" => Role::Heading,
                "list" => Role::List,
                "listitem" => Role::ListItem,
                "dialog" => Role::Dialog,
                "alert" => Role::Alert,
                "grid" => Role::Grid,
                "gridcell" => Role::GridCell,
                "listbox" => Role::ListBox,
                "option" => Role::Option,
                "menu" => Role::Menu,
                "menuitem" => Role::MenuItem,
                "progressbar" => Role::ProgressBar,
                "combobox" => Role::ComboBox,
                "alertdialog" => Role::AlertDialog,
                "group" => Role::Group,
                "tooltip" => Role::Tooltip,
                "img" => Role::Image,
                "tablist" => Role::TabList,
                "tab" => Role::Tab,
                "tabpanel" => Role::TabPanel,
                "radiogroup" => Role::RadioGroup,
                "radio" => Role::RadioButton,
                "toolbar" => Role::Toolbar,
                "tree" => Role::Tree,
                "treeitem" => Role::TreeItem,
                "row" => Role::Row,
                "menuitemcheckbox" => Role::MenuItemCheckBox,
                "menuitemradio" => Role::MenuItemRadio,
                _ => Role::Status,
            }
        } else if button {
            Role::Button
        } else if node.drag.is_some() || steppable(node) {
            // Moved between its snap points, or stepped through its values, it adjusts like a slider.
            Role::Slider
        } else if node.tag == "video" {
            Role::Video
        } else if node.tag == "webview" {
            Role::WebView
        } else if matches!(node.tag, "image" | "plot") {
            Role::Image
        } else {
            Role::Text
        };
        let label = node.accessibility_label.clone().unwrap_or_default();
        let name = if matches!(
            role,
            Role::Image
                | Role::Video
                | Role::WebView
                | Role::ProgressBar
                | Role::Slider
                | Role::RadioGroup
                | Role::List
                // A named `<List>` keeps its scrolling role and says its name.
                | Role::ScrollView
                | Role::HorizontalScrollView
                | Role::Dialog
                | Role::AlertDialog
                | Role::Group
                | Role::ComboBox
                | Role::Grid
                | Role::ListBox
                | Role::Menu
                | Role::TabList
                | Role::TabPanel
                | Role::Toolbar
                | Role::Tree
                | Role::Row
        ) || (matches!(
            role,
            Role::Heading
                | Role::ListItem
                | Role::Alert
                | Role::Status
                | Role::Tooltip
                | Role::GridCell
                | Role::Option
                | Role::MenuItem
                | Role::Tab
                | Role::TreeItem
                | Role::MenuItemCheckBox
                | Role::MenuItemRadio
        ) && !label.is_empty())
            || (input.is_some() && !label.is_empty())
        {
            label
        } else {
            node.text.clone()
        };
        // Enabled is what the author wrote: a control without a handler is enabled but does nothing.
        let mut enabled = !node.disabled && input.is_none_or(|input| input.enabled);
        let mut actions = Vec::new();
        let mut focusable = button
            || check
            || input.is_some()
            || select.is_some()
            || (node.tag == "webview" && !node.disabled);
        if let Some(range) = &node.range {
            focusable = true;
            if slider_enabled(node) {
                actions.push(Action::SetProgress);
                if range.value < range.limit[1] {
                    actions.push(Action::ScrollForward);
                }
                if range.value > range.limit[0] {
                    actions.push(Action::ScrollBackward);
                }
            }
        }
        let drag = node.drag.as_ref().map(|drag| {
            let at = self
                .drags
                .and_then(|drags| drags.target(&node.identity))
                .unwrap_or(drag.index);
            (at, drag.points.len())
        });
        if let Some((at, count)) = drag {
            focusable = true;
            if at + 1 < count {
                actions.push(Action::ScrollForward);
            }
            if at > 0 {
                actions.push(Action::ScrollBackward);
            }
        }
        // A spinbutton editor steps through the Up and Down handlers its page gave `onKeyDown`.
        if spin && let Some((_, keys)) = &node.key_map {
            use crate::interaction::view::Key;
            if keys.contains(&Key::Up) {
                actions.push(Action::ScrollForward);
            }
            if keys.contains(&Key::Down) {
                actions.push(Action::ScrollBackward);
            }
        }
        // An `onPointer` element with a value is adjusted in place.
        if steppable(node)
            && let Some(value) = &node.progress
        {
            focusable = true;
            let now = value.now.unwrap_or(value.min);
            if now < value.max {
                actions.push(Action::ScrollForward);
            }
            if now > value.min {
                actions.push(Action::ScrollBackward);
            }
        }
        let mut children = if self.is_container(index) {
            self.contained(index)
        } else {
            Vec::new()
        };
        let parent = self
            .container_of(index)
            .and_then(|container| self.ids.id(container));
        let mut position = None;
        if node.tag == "radio" {
            let members: Vec<_> = self.members(node).collect();
            children = members
                .iter()
                .filter_map(|(index, _)| self.ids.id(*index))
                .collect();
            position = Some((0, members.len()));
        } else if node.tag == "radio-option" {
            let group = node.radio_group.as_deref();
            let row = self.frame.nodes[..index]
                .iter()
                .filter(|candidate| {
                    candidate.tag == "radio-option" && candidate.radio_group.as_deref() == group
                })
                .count();
            position = Some((row, 1));
        } else if let Some((row, count)) = self.composite_position(index) {
            position = Some((row, count));
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
        let mut clickable = (clickable(node) || editable(node)) && node.range.is_none();
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
            let selected = input
                .edit
                .as_ref()
                .is_some_and(|edit| edit.selection[0] != edit.selection[1]);
            if !input.secure && !self.inert {
                if selected {
                    actions.push(Action::Copy);
                }
                if input.enabled {
                    if selected {
                        actions.push(Action::Cut);
                    }
                    actions.push(Action::Paste);
                }
            }
            TextField {
                text: masked(input),
                hint: input.placeholder.clone(),
                secure: input.secure,
                multiline: node.tag == "textarea",
                selection: input
                    .edit_token
                    .filter(|_| input.display.is_none())
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
        for (position, target) in self.action_targets(node).into_iter().enumerate() {
            let label = self
                .ids
                .id(target)
                .and_then(|id| self.reachable_node(id))
                .map(|target| target.name)
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| self.frame.nodes[target].text.clone());
            actions.push(Action::Custom {
                index: position,
                label,
            });
        }
        if (matches!(role, Role::Dialog | Role::AlertDialog)
            && self.frame.nodes.iter().any(|node| node.close_request))
            || crate::interaction::frame::dismissable(&self.frame.nodes, node)
        {
            actions.push(Action::Dismiss);
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
        // A determinate progressbar reads its `aria-valuetext`, else its share as a percentage.
        let progress = node
            .progress
            .as_ref()
            .filter(|_| role == Role::ProgressBar || steppable(node) || spin)
            .and_then(|progress| {
                let now = progress.now?.clamp(progress.min, progress.max);
                let share = (now - progress.min) / (progress.max - progress.min);
                let text = progress
                    .text
                    .clone()
                    .unwrap_or_else(|| format!("{}%", (share * 100.0).round()));
                Some((
                    Range {
                        min: progress.min,
                        max: progress.max,
                        value: now,
                    },
                    text,
                ))
            });
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
            checked: node.checked.or(node.states[6]),
            mixed: node.mixed,
            scrollable: node
                .scroll
                .as_ref()
                .is_some_and(|scroll| scroll.max_offset > 0.0),
            range: node
                .range
                .as_ref()
                // A thumb that shares its track reports the values it can reach.
                .map(|range| Range {
                    min: range.limit[0],
                    max: range.limit[1],
                    value: range.value,
                })
                .or_else(|| progress.as_ref().map(|(range, _)| range.clone()))
                // Any drag element reports its point, whatever role it keeps (a sheet, a swipe row).
                .or_else(|| {
                    drag.map(|(at, count)| Range {
                        min: 1.0,
                        max: count as f64,
                        value: (at + 1) as f64,
                    })
                }),
            text_field,
            parent,
            children,
            position,
            actions,
            heading_level: node.heading_level.or(match role {
                Role::Heading => Some(2),
                Role::TreeItem => Some(1),
                _ => None,
            }),
            live_region: node.live_region.clone(),
            text_selection: node.selection_range,
            pressed: node.states[0],
            selected: node.states[1],
            expanded: node.states[2],
            busy: node.states[3],
            value: select
                .map(|_| node.text.clone())
                .filter(|text| !text.is_empty())
                .or_else(|| progress.map(|(_, text)| text))
                // A combobox button says what is chosen.
                .or_else(|| {
                    node.progress
                        .as_ref()
                        .filter(|_| node.role.as_deref() == Some("combobox"))
                        .and_then(|value| value.text.clone())
                })
                .or_else(|| node.range.as_ref().and_then(|range| range.text.clone())),
            scroll: node.scroll.as_ref().map(|scroll| ScrollPosition {
                offset: scroll.offset,
                max_offset: scroll.max_offset,
                horizontal: scroll.axis == ScrollAxis::Horizontal,
            }),
            description: self.description(node),
            invalid: node.states[4],
            required: node.states[5],
        })
    }

    /// A composite's item count as `(0, n)`, or an item's place in it as `(row, 1)`, the way radio
    /// groups and lists carry theirs. Tree items count within their own level run.
    fn composite_position(&self, index: usize) -> Option<(usize, usize)> {
        use crate::interaction::composite::{Composite, Composites};
        let node = &self.frame.nodes[index];
        let item = matches!(
            node.role.as_deref(),
            Some(
                "tab"
                    | "radio"
                    | "option"
                    | "menuitem"
                    | "menuitemcheckbox"
                    | "menuitemradio"
                    | "treeitem"
            )
        );
        if !item && Composite::of(node).is_none_or(|kind| kind == Composite::Toolbar) {
            return None;
        }
        let composites = Composites::new(self.frame);
        if Composite::of(node).is_some() {
            let count = (0..self.frame.nodes.len())
                .filter(|item| {
                    composites
                        .container_of(*item)
                        .is_some_and(|(c, _)| c == index)
                })
                .count();
            return Some((0, count));
        }
        let (container, kind) = composites
            .container_of(index)
            .filter(|(_, kind)| *kind != Composite::Toolbar)?;
        let level = |item: usize| self.frame.nodes[item].heading_level.unwrap_or(1);
        let earlier = (0..index).rev().filter(|item| {
            composites
                .container_of(*item)
                .is_some_and(|(c, _)| c == container)
        });
        let row = if kind == Composite::Tree {
            // Siblings back to the parent: items one level deeper belong to an earlier sibling.
            earlier
                .take_while(|item| level(*item) >= level(index))
                .filter(|item| level(*item) == level(index))
                .count()
        } else {
            earlier.count()
        };
        Some((row, 1))
    }

    /// What `aria-describedby` names, in order: each element's label, else its text and its
    /// subtree's; ids the frame lacks are skipped.
    fn description(&self, node: &NodeSnapshot) -> Option<String> {
        let nodes = &self.frame.nodes;
        let parts: Vec<String> = node
            .described_by
            .iter()
            .filter_map(|id| {
                let at = nodes
                    .iter()
                    .position(|other| other.element_id.as_ref() == Some(id))?;
                let target = &nodes[at];
                if let Some(label) = target
                    .accessibility_label
                    .as_deref()
                    .filter(|l| !l.is_empty())
                {
                    return Some(label.to_owned());
                }
                let prefix = format!("{}/", target.identity);
                let text: Vec<&str> = std::iter::once(target)
                    .chain(
                        nodes[at + 1..]
                            .iter()
                            .take_while(|other| other.identity.starts_with(&prefix)),
                    )
                    .map(|other| other.text.trim())
                    .filter(|text| !text.is_empty())
                    .collect();
                (!text.is_empty()).then(|| text.join(" "))
            })
            .collect();
        (!parts.is_empty()).then(|| parts.join(" "))
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
    let mut events = semantic_changes(previous, next, ids);
    let changed = previous.is_none_or(|previous| !previous.same_content(next));
    if changed {
        events.push(Event::ContentChanged);
    }
    events
}

/// Text, value, selection and live-region changes, without the whole-frame content compare.
pub fn semantic_changes(
    previous: Option<&PresentedFrame>,
    next: &PresentedFrame,
    ids: &SemanticIds,
) -> Vec<Event> {
    let before: std::collections::HashMap<&str, &NodeSnapshot> = previous
        .map(|frame| {
            frame
                .nodes
                .iter()
                .map(|node| (node.identity.as_ref(), node))
                .collect()
        })
        .unwrap_or_default();
    let mut events = Vec::new();
    for (index, node) in next.nodes.iter().enumerate() {
        // An alert speaks when it appears, not only when it changes; a status waits for a change.
        if previous.is_some()
            && !before.contains_key(node.identity.as_ref())
            && node.role.as_deref() == Some("alert")
            && let Some(kind) = &node.live_region
            && !node.accessibility_hidden.unwrap_or(false)
            && let Some(id) = ids.id(index)
        {
            let text = spoken(next, node);
            if !text.is_empty() {
                events.push(Event::Announce {
                    id,
                    text,
                    assertive: kind == "assertive",
                });
            }
        }
        let (Some(id), Some(prior)) = (ids.id(index), before.get(node.identity.as_ref())) else {
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
        // A stepped element or spinbutton moved to another value.
        if let (Some(value), Some(old)) = (&node.progress, &prior.progress)
            && node.role.as_deref() != Some("progressbar")
            && value.now != old.now
        {
            events.push(Event::ValueChanged { id });
        }
        // The option an editor's keys point at changed: it is spoken while focus stays in the text.
        if node.active_descendant != prior.active_descendant
            && let Some(active) = &node.active_descendant
            && let Some(option) = next
                .nodes
                .iter()
                .find(|other| other.element_id.as_deref() == Some(active))
        {
            let text = option
                .accessibility_label
                .clone()
                .filter(|label| !label.is_empty())
                .unwrap_or_else(|| spoken(next, option));
            if !text.is_empty() {
                events.push(Event::Announce {
                    id,
                    text,
                    assertive: false,
                });
            }
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
    events
}
