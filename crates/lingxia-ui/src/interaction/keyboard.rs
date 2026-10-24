//! Hardware keyboard focus order. Radio groups take one tab stop: the
//! selected option, else the first enabled one.
use super::frame::{PresentedFrame, interactive};
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
pub fn candidate(
    frame: &PresentedFrame,
    index: usize,
    revealable: impl Fn(&NodeSnapshot) -> bool,
) -> bool {
    let Some(node) = frame.nodes.get(index) else {
        return false;
    };
    interactive(node)
        && node.bounds[2] > 0.0
        && node.bounds[3] > 0.0
        && (super::frame::visible(node) || revealable(node))
}

/// The next focus target from `current` in `direction`, skipping the current radio group.
pub fn next(
    frame: &PresentedFrame,
    current: Option<usize>,
    direction: i32,
    revealable: impl Fn(&NodeSnapshot) -> bool,
) -> Option<usize> {
    let count = frame.nodes.len() as i64;
    if count == 0 {
        return None;
    }
    let origin = current
        .map(|i| i as i64)
        .unwrap_or(if direction > 0 { -1 } else { count });
    let current_group = current
        .and_then(|i| frame.nodes.get(i))
        .filter(|node| option(node))
        .and_then(|node| node.radio_group.as_deref());
    (1..=count)
        .map(|step| (origin + i64::from(direction) * step).rem_euclid(count) as usize)
        .find(|index| {
            let node = &frame.nodes[*index];
            !current_group.is_some_and(|group| member(node, group))
                && candidate(frame, *index, &revealable)
                && tab_stop(frame, *index)
        })
}
