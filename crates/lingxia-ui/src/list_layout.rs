use crate::{
    DisplayList,
    list::{Align, Anchor, ListMetrics},
    schema::Record,
};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Request {
    start: usize,
    end: usize,
    leading: f64,
    trailing: f64,
}

impl Request {
    fn matches(&self, other: &Self) -> bool {
        self.start == other.start
            && self.end == other.end
            && (self.leading as f32 - other.leading as f32).abs() < 0.01
            && (self.trailing as f32 - other.trailing as f32).abs() < 0.01
    }
}

#[derive(Clone)]
pub(crate) struct ListState {
    metrics: ListMetrics,
    width: f32,
    measurement_dirty: bool,
    padding: f32,
    window: Request,
    anchor: Option<String>,
    next_anchor: Option<String>,
}

pub(crate) type Lists = HashMap<String, ListState>;
pub(crate) type Windows = HashMap<String, Request>;

pub(crate) fn invalidate_measurements(lists: &mut Lists) {
    for state in lists.values_mut() {
        state.measurement_dirty = true;
    }
}

pub(crate) fn current(lists: &Lists) -> Windows {
    lists
        .iter()
        .map(|(id, state)| (id.clone(), state.window.clone()))
        .collect()
}

fn request(
    state: &ListState,
    node: &crate::render::NodeSnapshot,
    offset: f32,
) -> Result<Request, String> {
    let visible = node.visible_bounds;
    let clipped_top = (visible[1] - node.bounds[1]).max(0.0);
    let window = state.metrics.window(
        (offset - state.padding + clipped_top).max(0.0) as f64,
        visible[3] as f64,
        node.bounds[3] as f64,
    )?;
    Ok(Request {
        start: window.rows.start,
        end: window.rows.end,
        leading: window.leading,
        trailing: window.trailing,
    })
}

pub(crate) fn scroll_windows(
    display: &DisplayList,
    lists: &Lists,
) -> Result<Option<Windows>, String> {
    let mut windows = current(lists);
    let mut changed = false;
    for node in &display.nodes {
        let Some(scroll) = &node.scroll else {
            continue;
        };
        let Some(state) = lists.get(&scroll.id) else {
            continue;
        };
        let desired = request(state, node, scroll.offset)?;
        changed |= !desired.matches(&state.window);
        windows.insert(scroll.id.clone(), desired);
    }
    Ok(changed.then_some(windows))
}

/// Reconcile measured row heights before publishing a candidate. All mutations are
/// confined to a cloned candidate cache until ViewSession commits the frame.
pub(crate) fn reconcile(
    records: &[Record],
    display: &DisplayList,
    lists: &mut Lists,
    offsets: &mut HashMap<String, f32>,
) -> Result<Option<Windows>, String> {
    let mut alive = HashSet::new();
    let mut windows = Windows::new();
    let mut changed = false;
    for (index, record) in records.iter().enumerate().filter(|(_, r)| r.tag == 6) {
        let id = record
            .string(11)
            .ok_or("UI_LIST: missing identity")?
            .to_owned();
        alive.insert(id.clone());
        let node = &display.nodes[index];
        let scroll = node
            .scroll
            .as_ref()
            .ok_or("UI_LIST: missing scroll geometry")?;
        let count = record.number(12).unwrap_or(0.0) as usize;
        let [padding, _, bottom, _] = scroll.content_insets;
        let gap = record.number(2).unwrap_or(0.0);
        let width = scroll.content_size[0];
        // Measurement restoration must use the same content box as row anchors;
        // ignoring the bottom inset clamps an already anchored list on rerender.
        let viewport = scroll.content_size[1] as f64;
        let mut position = (scroll.offset - padding).max(0.0) as f64;
        let original_position = position;
        if !lists.contains_key(&id) {
            lists.insert(
                id.clone(),
                ListState {
                    metrics: ListMetrics::new(count, 64.0)?,
                    width,
                    measurement_dirty: false,
                    padding,
                    window: Request {
                        start: 0,
                        end: 0,
                        leading: 0.0,
                        trailing: 0.0,
                    },
                    anchor: None,
                    next_anchor: None,
                },
            );
        }
        let state = lists.get_mut(&id).unwrap();
        if state.width != width || state.measurement_dirty {
            position = state.metrics.invalidate(64.0, position, viewport)?;
            state.width = width;
            state.measurement_dirty = false;
        }
        state.padding = padding;
        position = state.metrics.resize(count, position, viewport)?;
        let rows: Vec<(usize, f64)> = records
            .iter()
            .enumerate()
            .filter(|(_, r)| r.parent == Some(index))
            .map(|(i, r)| {
                let row = r.number(15).unwrap_or(0.0) as usize;
                (
                    row,
                    (display.nodes[i].bounds[3] + if row + 1 < count { gap } else { 0.0 }) as f64,
                )
            })
            .collect();
        let measured = state.metrics.measure(&rows, position, viewport)?;
        let mut target = scroll.offset + (measured - original_position) as f32;
        let next_anchor = record.string(16).map(str::to_owned);
        if next_anchor != state.anchor {
            if let Some(encoded) = &next_anchor {
                let value: serde_json::Value =
                    serde_json::from_str(encoded).map_err(|_| "UI_LIST: invalid anchor")?;
                if value == "end" {
                    target = (state.metrics.total() + (padding + bottom) as f64
                        - node.bounds[3] as f64)
                        .max(0.0) as f32;
                } else {
                    let row = value["index"]
                        .as_u64()
                        .ok_or("UI_LIST: invalid anchor index")?
                        as usize;
                    let align = if value["align"] == "end" {
                        Align::End
                    } else {
                        Align::Start
                    };
                    target = state.metrics.anchor_offset(
                        Anchor::Row { index: row, align },
                        scroll.content_size[1] as f64,
                    )? as f32;
                }
            }
        }
        state.next_anchor = next_anchor;
        if (target - scroll.offset).abs() > 0.01 {
            offsets.insert(id.clone(), target);
            changed = true;
        }
        let desired = request(state, node, target)?;
        let start = rows.first().map(|r| r.0).unwrap_or(desired.start);
        let end = rows.last().map(|r| r.0 + 1).unwrap_or(start);
        let actual = Request {
            start,
            end,
            leading: record.number(13).unwrap_or(0.0) as f64,
            trailing: record.number(14).unwrap_or(0.0) as f64,
        };
        changed |= !desired.matches(&actual);
        state.window = desired.clone();
        windows.insert(id, desired);
    }
    lists.retain(|id, _| alive.contains(id));
    Ok(changed.then_some(windows))
}

pub(crate) fn commit(lists: &mut Lists) {
    for state in lists.values_mut() {
        state.anchor.clone_from(&state.next_anchor);
    }
}
