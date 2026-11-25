//! `onVisibleChange`: the children a scroll shows, reported on a coarse gate while it moves
//! (the set, a quarter step of a ratio, the current child) and once exactly when it rests.
use crate::scene::display::DisplayList;
use crate::scene::node::ScrollAxis;
use std::collections::HashMap;
use std::fmt::Write;

/// Past this many children on screen the rest go unreported.
const MAX_ITEMS: usize = 256;

#[derive(Clone, PartialEq)]
pub(crate) enum Child {
    Id(String),
    Row(u32),
}

#[derive(Clone, PartialEq)]
pub(crate) struct Item {
    child: Child,
    ratio: f32,
    top: f32,
}

/// The children each `onVisibleChange` scroll shows in one frame, by scroll id.
pub(crate) type Measured = Vec<(String, u64, Vec<Item>)>;

#[derive(Default)]
struct Seen {
    handle: u64,
    reported: Vec<Item>,
    latest: Vec<Item>,
}

#[derive(Default)]
pub(crate) struct Visibility {
    seen: HashMap<String, Seen>,
    due: Vec<(u64, String)>,
}

/// The children of each scroll with a handle: an element with an `id` whose nearest scroll it
/// is, or a row of a `List`, in order along the axis.
pub(crate) fn measure(display: &DisplayList) -> Measured {
    let scrolls = display
        .nodes
        .iter()
        .filter_map(|node| {
            let scroll = node.scroll.as_ref()?;
            Some((scroll, scroll.on_visible?, node.bounds, node.tag == "list"))
        })
        .collect::<Vec<_>>();
    if scrolls.is_empty() {
        return Vec::new();
    }
    scrolls
        .into_iter()
        .map(|(scroll, handle, view, list)| {
            let axis = scroll.axis.index();
            let mut items = display
                .nodes
                .iter()
                .filter(|node| node.scroll_parent.as_deref() == Some(&*scroll.id))
                .filter_map(|node| {
                    let child = if list {
                        Child::Row(node.list_row?)
                    } else {
                        Child::Id(node.element_id.clone()?)
                    };
                    let (start, len) = (node.bounds[axis], node.bounds[axis + 2]);
                    let (view_start, view_len) = (view[axis], view[axis + 2]);
                    let shown = (start + len).min(view_start + view_len) - start.max(view_start);
                    let inside = shown > 0.0 || (len == 0.0 && shown == 0.0);
                    if !inside {
                        return None;
                    }
                    let rtl = scroll.rtl && scroll.axis == ScrollAxis::Horizontal;
                    let top = if rtl {
                        view_start + view_len - (start + len)
                    } else {
                        start - view_start
                    };
                    let ratio = if len > 0.0 { shown / len } else { 1.0 };
                    Some(Item {
                        child,
                        ratio: (ratio.clamp(0.0, 1.0) * 1000.0).round() / 1000.0,
                        top: (top * 8.0).round() / 8.0,
                    })
                })
                .collect::<Vec<_>>();
            items.sort_by(|a, b| a.top.total_cmp(&b.top));
            items.truncate(MAX_ITEMS);
            (scroll.id.clone(), handle, items)
        })
        .collect()
}

/// The last child at or past the top, else the first.
fn current(items: &[Item]) -> Option<&Child> {
    items
        .iter()
        .rev()
        .find(|item| item.top <= 0.0)
        .or(items.first())
        .map(|item| &item.child)
}

fn quarter(ratio: f32) -> u8 {
    (ratio * 4.0).floor() as u8
}

/// The gate: the set or its order, a ratio crossing a quarter step, or the current child.
fn crossed(old: &[Item], new: &[Item]) -> bool {
    old.len() != new.len()
        || old
            .iter()
            .zip(new)
            .any(|(a, b)| a.child != b.child || quarter(a.ratio) != quarter(b.ratio))
        || current(old) != current(new)
}

fn payload(items: &[Item], time: u64) -> String {
    let mut out = String::from(r#"{"items":["#);
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        match &item.child {
            Child::Id(id) => {
                let id = serde_json::to_string(id).unwrap_or_default();
                let _ = write!(out, r#"{{"id":{id}"#);
            }
            Child::Row(row) => {
                let _ = write!(out, r#"{{"index":{row}"#);
            }
        }
        let _ = write!(out, r#","ratio":{},"top":{}}}"#, item.ratio, item.top);
    }
    let _ = write!(out, r#"],"timeStamp":{time}}}"#);
    out
}

impl Visibility {
    /// Takes a committed frame's measure: a crossing is due at once; a smaller move waits for
    /// [`settle`](Self::settle).
    pub(crate) fn note(&mut self, measured: Measured, time: u64) {
        if measured.is_empty() && self.seen.is_empty() {
            return;
        }
        let live = measured
            .iter()
            .map(|(id, ..)| id.clone())
            .collect::<std::collections::HashSet<_>>();
        self.seen.retain(|id, _| live.contains(id));
        for (id, handle, items) in measured {
            let seen = self.seen.entry(id).or_default();
            seen.handle = handle;
            if crossed(&seen.reported, &items) {
                self.due.push((handle, payload(&items, time)));
                seen.reported = items.clone();
            }
            seen.latest = items;
        }
    }

    /// A move since the last report that only a settle would tell.
    pub(crate) fn unsettled(&self) -> bool {
        self.seen.values().any(|seen| seen.latest != seen.reported)
    }

    /// The scroll came to rest: each one's exact children, where they moved since its report.
    pub(crate) fn settle(&mut self, time: u64) {
        for seen in self.seen.values_mut() {
            if seen.latest != seen.reported {
                self.due.push((seen.handle, payload(&seen.latest, time)));
                seen.reported = seen.latest.clone();
            }
        }
    }

    pub(crate) fn due(&self) -> bool {
        !self.due.is_empty()
    }

    pub(crate) fn take_due(&mut self) -> Vec<(u64, String)> {
        std::mem::take(&mut self.due)
    }
}
