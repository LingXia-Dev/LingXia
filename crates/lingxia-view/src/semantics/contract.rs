//! The authoring contract of roles: what a page must state so its controls and composites work
//! for the keyboard, assistive tech and automation. The engine supplies the behaviour of each
//! role; this reports what the page left out, all at once.
use super::projection::{Projection, Role, SemanticNode};
use crate::interaction::composite::{Composite, Composites};
use crate::interaction::frame::PresentedFrame;
use crate::interaction::keyboard;
use serde::Serialize;
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Rule {
    /// A control or named container without an accessible name.
    Name,
    /// A composite holding none of its items.
    Items,
    /// An item outside the composite its role belongs to.
    Owner,
    /// A role whose required state is missing.
    State,
    /// A tab list without exactly one selected tab, or a radio group with several checked.
    Chosen,
    /// Two sibling controls with the same role and name: neither can be told apart.
    Distinct,
    /// A composite that takes more than one Tab stop.
    TabStops,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Violation {
    pub rule: Rule,
    pub role: Role,
    pub name: String,
    pub test_id: String,
    /// The element's stable path, for nodes without a test id.
    pub identity: String,
    pub message: String,
}

/// Every contract violation on the page.
pub fn check(projection: &Projection) -> Vec<Violation> {
    let frame = projection.frame;
    let composites = Composites::new(frame);
    let nodes: Vec<SemanticNode> = projection
        .reading_order()
        .into_iter()
        .filter_map(|id| projection.node(id))
        .collect();
    let mut found = Vec::new();
    // A control with nothing to say is left out of the tree entirely; Tab still reaches it.
    let published: HashSet<usize> = nodes.iter().map(|node| node.index).collect();
    for (index, snapshot) in frame.nodes.iter().enumerate() {
        if published.contains(&index)
            || snapshot.accessibility_hidden == Some(true)
            || !crate::interaction::frame::focusable(snapshot)
            || !crate::interaction::frame::visible(snapshot)
        {
            continue;
        }
        if let Some(node) = projection
            .ids
            .id(index)
            .and_then(|id| projection.acting_node(id))
            .filter(|node| node.name.trim().is_empty())
        {
            found.push(violation(
                frame,
                &node,
                Rule::Name,
                "a focusable control needs an accessible name".into(),
            ));
        }
    }
    for node in &nodes {
        names(frame, node, &mut found);
        states(frame, node, &mut found);
        owners(frame, &composites, node, &mut found);
    }
    for node in &nodes {
        if let Some(kind) = Composite::of(&frame.nodes[node.index]) {
            items(frame, &composites, &nodes, node, kind, &mut found);
        }
    }
    distinct(frame, &nodes, &mut found);
    tab_stops(frame, &composites_of(frame, &nodes), &mut found);
    found
}

fn violation(
    frame: &PresentedFrame,
    node: &SemanticNode,
    rule: Rule,
    message: String,
) -> Violation {
    Violation {
        rule,
        role: node.role,
        name: node.name.clone(),
        test_id: node.test_id.clone(),
        identity: frame.nodes[node.index].identity.to_string(),
        message,
    }
}

/// The role as the page wrote it, else as the projection gave it.
fn label(frame: &PresentedFrame, node: &SemanticNode) -> String {
    authored(frame, node).map(str::to_owned).unwrap_or_else(|| {
        serde_json::to_value(node.role)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_else(|| format!("{:?}", node.role))
    })
}

fn authored<'a>(frame: &'a PresentedFrame, node: &SemanticNode) -> Option<&'a str> {
    frame.nodes[node.index].role.as_deref()
}

/// Controls a person or agent acts on by name.
fn control(role: Role) -> bool {
    matches!(
        role,
        Role::Button
            | Role::CheckBox
            | Role::Switch
            | Role::RadioButton
            | Role::ComboBox
            | Role::TextField
            | Role::Slider
            | Role::Link
            | Role::Tab
            | Role::MenuItem
            | Role::MenuItemCheckBox
            | Role::MenuItemRadio
            | Role::Option
            | Role::TreeItem
    )
}

/// Containers whose name is how a person tells which set they are in. A menu and a tab list
/// usually take theirs from the control that opens or heads them, so they are not required.
fn named_container(role: Role) -> bool {
    matches!(
        role,
        Role::Dialog
            | Role::AlertDialog
            | Role::ListBox
            | Role::RadioGroup
            | Role::Tree
            | Role::Grid
    )
}

fn names(frame: &PresentedFrame, node: &SemanticNode, found: &mut Vec<Violation>) {
    if node.name.trim().is_empty() && (control(node.role) || named_container(node.role)) {
        found.push(violation(
            frame,
            node,
            Rule::Name,
            format!("a {} needs an accessible name", label(frame, node)),
        ));
    }
}

/// Required states of authored roles; native elements always carry theirs.
fn states(frame: &PresentedFrame, node: &SemanticNode, found: &mut Vec<Violation>) {
    if authored(frame, node).is_none() {
        return;
    }
    let missing = match node.role {
        Role::CheckBox
        | Role::Switch
        | Role::RadioButton
        | Role::MenuItemCheckBox
        | Role::MenuItemRadio => node.checked.is_none().then_some("aria-checked"),
        Role::Tab => node.selected.is_none().then_some("aria-selected"),
        Role::ComboBox => node.expanded.is_none().then_some("aria-expanded"),
        _ => None,
    };
    if let Some(state) = missing {
        found.push(violation(
            frame,
            node,
            Rule::State,
            format!("a {} needs {state}", label(frame, node)),
        ));
    }
}

/// The composite an authored item role must sit in. A native radio option belongs to its
/// `radio` element instead.
fn owner_kind(frame: &PresentedFrame, node: &SemanticNode) -> Option<Composite> {
    Some(match authored(frame, node)? {
        "tab" => Composite::TabList,
        "menuitem" | "menuitemcheckbox" | "menuitemradio" => Composite::Menu,
        "option" => Composite::ListBox,
        "radio" => Composite::RadioGroup,
        "treeitem" => Composite::Tree,
        "gridcell" => Composite::Grid,
        _ => return None,
    })
}

fn owners(
    frame: &PresentedFrame,
    composites: &Composites,
    node: &SemanticNode,
    found: &mut Vec<Violation>,
) {
    let Some(kind) = owner_kind(frame, node) else {
        return;
    };
    if composites.container_of(node.index).map(|(_, k)| k) != Some(kind) {
        found.push(violation(
            frame,
            node,
            Rule::Owner,
            format!(
                "a {} must sit inside a {}",
                label(frame, node),
                composite_name(kind)
            ),
        ));
    }
}

fn composite_name(kind: Composite) -> &'static str {
    match kind {
        Composite::Menu => "menu",
        Composite::TabList => "tablist",
        Composite::RadioGroup => "radiogroup",
        Composite::Toolbar => "toolbar",
        Composite::ListBox => "listbox",
        Composite::Tree => "tree",
        Composite::Grid => "grid",
    }
}

fn items(
    frame: &PresentedFrame,
    composites: &Composites,
    nodes: &[SemanticNode],
    container: &SemanticNode,
    kind: Composite,
    found: &mut Vec<Violation>,
) {
    let members: Vec<&SemanticNode> = nodes
        .iter()
        .filter(|node| {
            composites
                .container_of(node.index)
                .is_some_and(|(owner, _)| owner == container.index)
        })
        .collect();
    // A list box, tree or grid may be empty for now: no results, an empty folder.
    let never_empty = matches!(
        kind,
        Composite::Menu | Composite::TabList | Composite::RadioGroup | Composite::Toolbar
    );
    if members.is_empty() {
        if !never_empty {
            return;
        }
        found.push(violation(
            frame,
            container,
            Rule::Items,
            format!("a {} holds none of its items", composite_name(kind)),
        ));
        return;
    }
    match kind {
        Composite::TabList => {
            let selected = members
                .iter()
                .filter(|tab| tab.selected == Some(true))
                .count();
            if selected != 1 {
                found.push(violation(
                    frame,
                    container,
                    Rule::Chosen,
                    format!("a tablist needs exactly one selected tab; it has {selected}"),
                ));
            }
        }
        Composite::RadioGroup => {
            let checked = members
                .iter()
                .filter(|radio| radio.checked == Some(true))
                .count();
            if checked > 1 {
                found.push(violation(
                    frame,
                    container,
                    Rule::Chosen,
                    format!("a radiogroup checks at most one radio; it checks {checked}"),
                ));
            }
        }
        _ => {}
    }
}

/// Identity of a node's direct parent element.
fn parent(identity: &str) -> &str {
    identity.rsplit_once('/').map_or("", |(parent, _)| parent)
}

fn distinct(frame: &PresentedFrame, nodes: &[SemanticNode], found: &mut Vec<Violation>) {
    let mut seen = HashSet::new();
    for node in nodes {
        if !control(node.role) || !node.enabled || node.name.trim().is_empty() {
            continue;
        }
        let key = (
            parent(&frame.nodes[node.index].identity),
            label(frame, node),
            node.name.as_str(),
        );
        if !seen.insert(key) {
            found.push(violation(
                frame,
                node,
                Rule::Distinct,
                format!(
                    "two {}s beside each other are both named \"{}\"",
                    label(frame, node),
                    node.name
                ),
            ));
        }
    }
}

fn descends(frame: &PresentedFrame, index: usize, ancestor: usize) -> bool {
    let node: &str = &frame.nodes[index].identity;
    let ancestor: &str = &frame.nodes[ancestor].identity;
    node.len() > ancestor.len()
        && node.starts_with(ancestor)
        && node.as_bytes()[ancestor.len()] == b'/'
}

/// The published composites, with their kind. Menus keep every item a stop, so they are left out.
fn composites_of<'a>(
    frame: &PresentedFrame,
    nodes: &'a [SemanticNode],
) -> Vec<(&'a SemanticNode, Composite)> {
    nodes
        .iter()
        .filter_map(|node| Composite::of(&frame.nodes[node.index]).map(|kind| (node, kind)))
        .filter(|(_, kind)| *kind != Composite::Menu)
        .collect()
}

/// Walks Tab once around the page and charges each stop to its nearest composite, so a radio
/// group inside a toolbar counts toward the group alone.
fn tab_stops(
    frame: &PresentedFrame,
    composites: &[(&SemanticNode, Composite)],
    found: &mut Vec<Violation>,
) {
    let mut stops = Vec::new();
    let mut seen = HashSet::new();
    let mut current = None;
    while let Some(next) = keyboard::next(frame, current, 1, |_| true, |_| None) {
        if !seen.insert(next) {
            break;
        }
        stops.push(next);
        current = Some(next);
    }
    let mut counts = vec![0usize; composites.len()];
    for stop in stops {
        let nearest = composites
            .iter()
            .enumerate()
            .filter(|(_, (container, _))| descends(frame, stop, container.index))
            .max_by_key(|(_, (container, _))| frame.nodes[container.index].identity.len());
        if let Some((slot, _)) = nearest {
            counts[slot] += 1;
        }
    }
    for ((container, kind), count) in composites.iter().zip(counts) {
        if count > 1 {
            found.push(violation(
                frame,
                container,
                Rule::TabStops,
                format!(
                    "a {} takes one Tab stop; it takes {count}",
                    composite_name(*kind)
                ),
            ));
        }
    }
}
