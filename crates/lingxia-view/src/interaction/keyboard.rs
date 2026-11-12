//! Hardware keyboard focus order. Radio groups and composites take one tab stop each.
use super::composite::Composites;
use super::frame::PresentedFrame;
use crate::scene::node::NodeSnapshot;

fn option(node: &NodeSnapshot) -> bool {
    node.tag == "radio-option"
}

fn member(node: &NodeSnapshot, group: &str) -> bool {
    option(node) && node.radio_group.as_deref() == Some(group)
}

fn enabled_option(node: &NodeSnapshot) -> bool {
    node.on_click.is_some_and(|handle| handle != 0)
}

pub fn tab_stop(frame: &PresentedFrame, index: usize) -> bool {
    let node = &frame.nodes[index];
    let Some(group) = node.radio_group.as_deref().filter(|_| option(node)) else {
        return true;
    };
    let mut first = None;
    for (candidate_index, candidate) in frame.nodes.iter().enumerate() {
        if !member(candidate, group) || !enabled_option(candidate) {
            continue;
        }
        first.get_or_insert(candidate_index);
        if candidate.checked == Some(true) {
            return candidate_index == index;
        }
    }
    first == Some(index)
}

/// The next enabled option in the same group, wrapping, for arrow keys.
pub fn adjacent_option(frame: &PresentedFrame, index: usize, direction: i32) -> Option<usize> {
    let node = frame.nodes.get(index)?;
    let group = node.radio_group.as_deref().filter(|_| option(node))?;
    let count = frame.nodes.len() as i64;
    (1..=count)
        .map(|step| (index as i64 + i64::from(direction) * step).rem_euclid(count) as usize)
        .find(|next| member(&frame.nodes[*next], group) && enabled_option(&frame.nodes[*next]))
}

pub fn is_option(frame: &PresentedFrame, index: usize) -> bool {
    frame.nodes.get(index).is_some_and(option)
}

/// Can receive keyboard focus now or after revealing it through scroll ancestors.
/// A control hidden from assistive technology, such as a popover's dismiss backdrop, is no stop.
pub fn candidate(
    frame: &PresentedFrame,
    index: usize,
    revealable: impl Fn(&NodeSnapshot) -> bool,
) -> bool {
    let Some(node) = frame.nodes.get(index) else {
        return false;
    };
    super::frame::focusable(node)
        && node.accessibility_hidden != Some(true)
        && node.bounds[2] > 0.0
        && node.bounds[3] > 0.0
        && (super::frame::visible(node) || revealable(node))
}

/// The open dialog keyboard focus stays inside: the last visible `role="dialog"`.
pub fn active_dialog(frame: &PresentedFrame) -> Option<usize> {
    frame.nodes.iter().rposition(|node| {
        matches!(node.role.as_deref(), Some("dialog" | "alertdialog"))
            && super::frame::visible(node)
    })
}

/// The topmost open dialog or menu: opening one from the keyboard moves focus into it.
pub fn active_overlay(frame: &PresentedFrame) -> Option<usize> {
    frame.nodes.iter().rposition(|node| {
        matches!(node.role.as_deref(), Some("dialog" | "menu")) && super::frame::visible(node)
    })
}

/// The first focus target inside `container`, in focus order. `remembered` names the item a
/// composite last had focused.
pub fn first_inside(
    frame: &PresentedFrame,
    container: usize,
    revealable: impl Fn(&NodeSnapshot) -> bool,
    remembered: impl Fn(usize) -> Option<usize>,
) -> Option<usize> {
    let composites = Composites::new(frame);
    (container + 1..frame.nodes.len()).find(|index| {
        inside(frame, Some(container), *index)
            && candidate(frame, *index, &revealable)
            && tab_stop(frame, *index)
            && composites.tab_stop(frame, *index, &remembered, &revealable)
    })
}

fn inside(frame: &PresentedFrame, scope: Option<usize>, index: usize) -> bool {
    scope.is_none_or(|scope| {
        let container = &frame.nodes[scope].identity;
        let node = &frame.nodes[index].identity;
        node.len() > container.len()
            && node.starts_with(&**container)
            && node.as_bytes()[container.len()] == b'/'
    })
}

/// The next focus target from `current` in `direction`, skipping the current radio group or
/// composite. With a dialog open the order wraps inside it.
pub fn next(
    frame: &PresentedFrame,
    current: Option<usize>,
    direction: i32,
    revealable: impl Fn(&NodeSnapshot) -> bool,
    remembered: impl Fn(usize) -> Option<usize>,
) -> Option<usize> {
    let count = frame.nodes.len() as i64;
    if count == 0 {
        return None;
    }
    let origin = current
        .map(|i| i as i64)
        .unwrap_or(if direction > 0 { -1 } else { count });
    let scope = active_dialog(frame);
    let current_group = current
        .and_then(|i| frame.nodes.get(i))
        .filter(|node| option(node))
        .and_then(|node| node.radio_group.as_deref());
    let composites = Composites::new(frame);
    let current_composite = current
        .and_then(|index| composites.container_of(index))
        .filter(|(_, kind)| *kind != super::composite::Composite::Menu)
        .map(|(container, _)| container);
    (1..=count)
        .map(|step| (origin + i64::from(direction) * step).rem_euclid(count) as usize)
        .find(|index| {
            let node = &frame.nodes[*index];
            !current_group.is_some_and(|group| member(node, group))
                && (current_composite.is_none()
                    || composites.container_of(*index).map(|(c, _)| c) != current_composite)
                && inside(frame, scope, *index)
                && candidate(frame, *index, &revealable)
                && tab_stop(frame, *index)
                && composites.tab_stop(frame, *index, &remembered, &revealable)
        })
}
