//! Record-to-Taffy style resolution and primitive control metrics.
use crate::protocol::{Record, field, tag};
use taffy::prelude::*;
use taffy::{Overflow, Point};

pub(crate) fn dimension<T: taffy::style_helpers::FromLength + taffy::style_helpers::FromPercent>(
    record: &Record,
    field: usize,
) -> Option<T> {
    let (_, relative) = crate::protocol::DIMENSIONS
        .iter()
        .find(|(absolute, _)| *absolute == field)?;
    record
        .number(*relative)
        .map(|value| percent(value / 100.0))
        .or_else(|| record.number(field).map(length))
}

/// `safe` when the alignment container scrolls, since content overflowing a scroll's start could
/// never be scrolled to, or when the page asked for it (`items-center-safe`). Elsewhere an
/// overflowing item spills past both edges, as CSS does by default.
pub(crate) fn cross_alignment(value: &str, safe: bool) -> AlignItems {
    match (value, safe) {
        ("center", true) => AlignItems::SAFE_CENTER,
        ("center", false) => AlignItems::CENTER,
        ("end", true) => AlignItems::SAFE_FLEX_END,
        ("end", false) => AlignItems::FLEX_END,
        ("stretch", _) => AlignItems::STRETCH,
        ("baseline", _) => AlignItems::BASELINE,
        _ => AlignItems::FLEX_START,
    }
}

/// A scroll container: a `<scroll>`, a `<list>`, or a popover capped to the room on its side.
pub(crate) fn scrolls(record: &Record) -> bool {
    matches!(record.tag, tag::SCROLL | tag::LIST) || popover_cap(record).is_some()
}

/// Whether the record scrolls along `axis` (0 x, 1 y): a horizontal `<scroll>` only sideways,
/// every other scroller only down.
pub(crate) fn scrolls_along(record: &Record, axis: usize) -> bool {
    scrolls(record) && (record.string(field::SCROLL_AXIS) == Some("horizontal")) == (axis == 0)
}

/// The height a popover is capped to, set by `cap_popovers`.
pub(crate) fn popover_cap(record: &Record) -> Option<f32> {
    (record.tag == tag::COLUMN && record.string(field::SCROLL_AXIS) == Some(CAPPED))
        .then(|| record.number(field::MAX_HEIGHT))
        .flatten()
}

/// The scroll axis marking a capped popover; authors cannot set it on a column.
const CAPPED: &str = "vertical";

/// Caps each popover `caps` names (by identity) to its height there and makes it scroll.
pub(crate) fn cap_popovers(records: &mut [Record], caps: &std::collections::HashMap<String, f32>) {
    if caps.is_empty() {
        return;
    }
    let identities = crate::protocol::identities(records);
    for (record, identity) in records.iter_mut().zip(identities) {
        let Some(cap) = caps.get(&*identity) else {
            continue;
        };
        if record.tag != tag::COLUMN || record.string(field::ANCHOR_TO).is_none() {
            continue;
        }
        record
            .fields
            .retain(|(id, _)| !matches!(*id, field::MAX_HEIGHT | field::SCROLL_AXIS));
        record
            .fields
            .push((field::MAX_HEIGHT, serde_json::Value::from(f64::from(*cap))));
        record
            .fields
            .push((field::SCROLL_AXIS, serde_json::Value::from(CAPPED)));
    }
}

/// `alignSafe` bits: the alignment properties a page made `safe` (`justify-center-safe`).
pub(crate) mod safe {
    pub const JUSTIFY: u64 = 1;
    pub const ALIGN: u64 = 2;
    pub const ALIGN_SELF: u64 = 4;
    pub const ALIGN_CONTENT: u64 = 8;
    pub const JUSTIFY_ITEMS: u64 = 16;
    pub const JUSTIFY_SELF: u64 = 32;
}

/// The inline room a control's own mark takes beside its label; a select's chevron grows with
/// its text (`growth`, from `TextScale::growth`).
pub(crate) fn check_reserve(record: &Record, growth: f32) -> f32 {
    match record.tag {
        tag::CHECKBOX | tag::RADIO_OPTION => 36.0,
        tag::SWITCH => crate::scene::slider::switch_track(record)[0] + 12.0,
        tag::SELECT => 28.0 * growth,
        _ => 0.0,
    }
}

/// A switch with element children: they lay out beside its track, which they never cover.
pub(crate) fn composed_switch(records: &[Record], index: usize) -> bool {
    // Records come in tree order, so a first child follows its parent.
    records[index].tag == tag::SWITCH
        && records
            .get(index + 1)
            .is_some_and(|next| next.parent == Some(index))
}

pub(crate) fn check_height(record: &Record) -> f32 {
    match record.tag {
        tag::CHECKBOX | tag::RADIO_OPTION => 24.0,
        tag::SWITCH => crate::scene::slider::switch_track(record)[1],
        _ => 0.0,
    }
}

/// Surface overlap the page primitive keeps content out of, in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SafeArea {
    /// Top, right, bottom, left.
    pub insets: [f32; 4],
    pub keyboard: f32,
}

fn margin_with_bleed(record: &Record, field: usize, bleed_offset: f32) -> LengthPercentageAuto {
    match record
        .fields
        .iter()
        .find(|(id, _)| *id == field)
        .map(|(_, value)| value)
    {
        Some(value) if value.as_str() == Some("auto") => auto(),
        Some(value) => value
            .as_f64()
            .map(|v| length(v as f32 - bleed_offset))
            .unwrap_or(length(-bleed_offset)),
        None => length(-bleed_offset),
    }
}

/// `offset` moves a length inset in from the containing edge.
fn inset(record: &Record, field: usize, offset: f32) -> LengthPercentageAuto {
    // Composition moves an `anchor(center)` edge onto the anchor once the popover is placed.
    if record.string(field) == Some(crate::scene::anchor::ANCHOR_CENTER) {
        return length(0.0);
    }
    record
        .fields
        .iter()
        .find(|(id, _)| *id == field)
        .and_then(|(_, value)| crate::protocol::length_or_percent(value, true))
        .map(|(value, relative)| {
            if relative {
                percent(value / 100.0)
            } else {
                length(value + offset)
            }
        })
        .unwrap_or(auto())
}

fn keyword(record: &Record, field: usize) -> Option<Dimension> {
    match record.string(field)? {
        "fit-content" => Some(Dimension::fit_content()),
        "max-content" => Some(Dimension::max_content()),
        "min-content" => Some(Dimension::min_content()),
        _ => None,
    }
}

/// The box each record is laid out, clipped and painted in: the top layer escapes to the root,
/// a fixed box to the top-layer box it sits in (so it paints and takes presses above that
/// layer's backdrop), else to the root; every other record keeps its tree parent.
pub(crate) fn layout_parents(records: &[Record]) -> Vec<Option<usize>> {
    records
        .iter()
        .map(|record| {
            record.parent.map(|parent| {
                if record.number(field::TOP_LAYER).is_some() {
                    0
                } else if record.string(field::POSITION) == Some("fixed") {
                    top_layer_of(records, parent).unwrap_or(0)
                } else {
                    parent
                }
            })
        })
        .collect()
}

/// The top-layer box holding record `index`, itself included.
fn top_layer_of(records: &[Record], index: usize) -> Option<usize> {
    std::iter::successors(Some(index), |&at| records[at].parent)
        .find(|&at| records[at].number(field::TOP_LAYER).is_some())
}

/// Whether each record lays out right to left: its own `direction`, else its
/// parent's, else the environment's.
pub(crate) fn directions(records: &[Record], rtl: bool) -> Vec<bool> {
    let mut out: Vec<bool> = Vec::with_capacity(records.len());
    for record in records {
        // `auto` (editors only) leaves the box direction to its parent.
        out.push(match record.string(field::DIRECTION) {
            Some("rtl") => true,
            Some("ltr") => false,
            _ => record.parent.map(|parent| out[parent]).unwrap_or(rtl),
        });
    }
    out
}

/// A physical left/right field, or the flow-relative field that maps onto
/// that side under `rtl`; the flow-relative one wins when both are set.
pub(crate) fn side(
    record: &Record,
    physical: usize,
    start: usize,
    end: usize,
    left: bool,
    rtl: bool,
) -> usize {
    let logical = if left == rtl { end } else { start };
    if record.fields.iter().any(|(id, _)| *id == logical) {
        logical
    } else {
        physical
    }
}

/// Records under `display: none`, including their descendants.
pub(crate) fn hidden(records: &[Record]) -> Vec<bool> {
    let mut hidden = Vec::with_capacity(records.len());
    for record in records {
        let inherited = record.parent.is_some_and(|parent| hidden[parent]);
        hidden.push(inherited || record.string(field::DISPLAY) == Some("none"));
    }
    hidden
}

/// Top, right, bottom and left border width fields under `rtl`.
pub(crate) fn border_edges(record: &Record, rtl: bool) -> [usize; 4] {
    let [s, e] = [
        field::BORDER_INLINE_START_WIDTH,
        field::BORDER_INLINE_END_WIDTH,
    ];
    [
        field::BORDER_TOP_WIDTH,
        side(record, field::BORDER_RIGHT_WIDTH, s, e, false, rtl),
        field::BORDER_BOTTOM_WIDTH,
        side(record, field::BORDER_LEFT_WIDTH, s, e, true, rtl),
    ]
}

/// Top-left, top-right, bottom-right and bottom-left radius fields under `rtl`.
pub(crate) fn radius_corners(record: &Record, rtl: bool) -> [usize; 4] {
    // `start-start` is block start and inline start: top-left in LTR, top-right in RTL.
    let pick = |physical, ltr, rtl_field| {
        let logical = if rtl { rtl_field } else { ltr };
        if record.fields.iter().any(|(id, _)| *id == logical) {
            logical
        } else {
            physical
        }
    };
    [
        pick(
            field::RADIUS_TOP_LEFT,
            field::RADIUS_START_START,
            field::RADIUS_START_END,
        ),
        pick(
            field::RADIUS_TOP_RIGHT,
            field::RADIUS_START_END,
            field::RADIUS_START_START,
        ),
        pick(
            field::RADIUS_BOTTOM_RIGHT,
            field::RADIUS_END_END,
            field::RADIUS_END_START,
        ),
        pick(
            field::RADIUS_BOTTOM_LEFT,
            field::RADIUS_END_START,
            field::RADIUS_END_END,
        ),
    ]
}

/// Whether `parent` places its children along a horizontal main axis.
fn lays_out_in_row(parent: &Record) -> bool {
    matches!(
        parent.string(field::FLEX_DIRECTION),
        Some("row" | "row-reverse")
    )
}

/// A replaced element's fixed length on `axis`: its automatic minimum, as in CSS.
fn replaced_size<T: taffy::style_helpers::FromLength>(record: &Record, axis: usize) -> Option<T> {
    if !matches!(
        record.tag,
        tag::IMAGE | tag::VECTOR | tag::VIDEO | tag::WEBVIEW
    ) {
        return None;
    }
    record.number(axis).map(length)
}

/// Whether a child keeps its parent's flow: the top layer, positioned and hidden boxes leave it.
fn in_flow(record: &Record) -> bool {
    record.number(field::TOP_LAYER).is_none()
        && !matches!(record.string(field::POSITION), Some("absolute" | "fixed"))
        && record.string(field::DISPLAY) != Some("none")
}

/// `parent`'s in-flow children, in order.
fn in_flow_children(records: &[Record], parent: usize) -> impl Iterator<Item = usize> + '_ {
    (parent + 1..records.len())
        .filter(move |&index| records[index].parent == Some(parent) && in_flow(&records[index]))
}

/// The first and last in-flow children of a page root: only they reach its top and bottom edges.
fn page_flow_ends(records: &[Record]) -> Option<(usize, usize)> {
    if records.first()?.tag != tag::PAGE {
        return None;
    }
    let first = in_flow_children(records, 0).next()?;
    let last = (first..records.len())
        .rev()
        .find(|&index| records[index].parent == Some(0) && in_flow(&records[index]))?;
    Some((first, last))
}

/// The scroll that ends a page under its bottom bars: the last in-flow page child, or the one
/// in-flow child of each wrapper down from it.
fn page_bottom_scroll(records: &[Record]) -> Option<usize> {
    let (_, mut at) = page_flow_ends(records)?;
    loop {
        if matches!(records[at].tag, tag::SCROLL | tag::LIST) {
            return Some(at);
        }
        let mut children = in_flow_children(records, at);
        let only = children.next()?;
        if children.next().is_some() {
            return None;
        }
        at = only;
    }
}

/// A dev hint when the page's bottom wrapper holds a scroll beside other in-flow content, so
/// the scroll stops above the bottom bars instead of running on under them.
pub(crate) fn page_bottom_wrapper_hint(records: &[Record]) -> Option<String> {
    if page_bottom_scroll(records).is_some() {
        return None;
    }
    let (_, mut at) = page_flow_ends(records)?;
    loop {
        let children: Vec<usize> = in_flow_children(records, at).collect();
        match children.as_slice() {
            [only] => at = *only,
            [] => return None,
            many => {
                let scroll = many
                    .iter()
                    .find(|&&child| matches!(records[child].tag, tag::SCROLL | tag::LIST))?;
                let record = &records[*scroll];
                let tag = crate::protocol::TAGS.get(record.tag).unwrap_or(&"scroll");
                let name = match record.key.as_str() {
                    "" => format!("<{tag}>"),
                    key => format!("<{tag} key=\"{key}\">"),
                };
                return Some(format!(
                    "UI_LAYOUT: {name} ends the page beside other in-flow content, so it stops \
                     above the bottom bars; make it the wrapper's only in-flow child (position the \
                     rest absolute) to let it run on under them"
                ));
            }
        }
    }
}

/// Taffy style for one record; the root fills the viewport and a page root keeps out of the safe area.
pub(crate) fn node_style(
    records: &[Record],
    index: usize,
    (width, height, scale): (u16, u16, f32),
    safe: SafeArea,
    rtl: bool,
) -> Result<Style, String> {
    let record = &records[index];
    let left = |physical, start, end| side(record, physical, start, end, true, rtl);
    let right = |physical, start, end| side(record, physical, start, end, false, rtl);
    let page = index == 0 && record.tag == tag::PAGE;
    let keyboard_above = safe.keyboard > safe.insets[2];
    let bleed_mode = (record.parent == Some(0) && records[0].tag == tag::PAGE)
        .then(|| record.string(field::BLEED))
        .flatten();
    let out_of_flow = matches!(record.string(field::POSITION), Some("absolute" | "fixed"));
    // Fixed boxes measure their length insets from the page's safe content box:
    // inside the system insets and above the keyboard, unless `bleed` extends that edge.
    let [safe_top, safe_right, safe_bottom, safe_left] =
        if records[0].tag == tag::PAGE && record.string(field::POSITION) == Some("fixed") {
            // A top-layer box already ends above the keyboard by its own bottom inset.
            let lifted = record
                .parent
                .and_then(|parent| top_layer_of(records, parent))
                .and_then(|layer| records[layer].number(field::INSET_BOTTOM))
                .unwrap_or(0.0);
            [
                if matches!(bleed_mode, Some("all" | "top")) {
                    0.0
                } else {
                    safe.insets[0]
                },
                if matches!(bleed_mode, Some("all" | "horizontal")) {
                    0.0
                } else {
                    safe.insets[1]
                },
                if matches!(bleed_mode, Some("all" | "bottom")) && !keyboard_above {
                    0.0
                } else {
                    (safe.insets[2].max(safe.keyboard) - lifted).max(0.0)
                },
                if matches!(bleed_mode, Some("all" | "horizontal")) {
                    0.0
                } else {
                    safe.insets[3]
                },
            ]
        } else {
            [0.0; 4]
        };
    // A fixed box in a top layer at a system edge paints out to it; its content stays inside.
    let bleed = if safe_top + safe_right + safe_bottom + safe_left > 0.0
        && record.string(field::POSITION) == Some("fixed")
        && record
            .parent
            .and_then(|parent| top_layer_of(records, parent))
            .is_some()
    {
        let touches = |field: usize, safe: f32| {
            let at_edge = record
                .fields
                .iter()
                .find(|(id, _)| *id == field)
                .and_then(|(_, value)| crate::protocol::length_or_percent(value, true))
                .is_some_and(|(value, _)| value == 0.0);
            if at_edge { safe } else { 0.0 }
        };
        [
            touches(field::INSET_TOP, safe_top),
            touches(
                right(
                    field::INSET_RIGHT,
                    field::INSET_INLINE_START,
                    field::INSET_INLINE_END,
                ),
                safe_right,
            ),
            // Over an open keyboard the edge is the keyboard's, not the system's.
            if safe.keyboard > safe.insets[2] {
                0.0
            } else {
                touches(field::INSET_BOTTOM, safe_bottom)
            },
            touches(
                left(
                    field::INSET_LEFT,
                    field::INSET_INLINE_START,
                    field::INSET_INLINE_END,
                ),
                safe_left,
            ),
        ]
    } else {
        [0.0; 4]
    };
    let [safe_top, safe_right, safe_bottom, safe_left] = [
        safe_top - bleed[0],
        safe_right - bleed[1],
        safe_bottom - bleed[2],
        safe_left - bleed[3],
    ];
    // Opposing insets size an absolute box, so fixed media defaults must not override them.
    let spans = |a: usize, b: usize| {
        let (a, b) = (
            side(
                record,
                a,
                field::INSET_INLINE_START,
                field::INSET_INLINE_END,
                a == field::INSET_LEFT,
                rtl,
            ),
            side(
                record,
                b,
                field::INSET_INLINE_START,
                field::INSET_INLINE_END,
                b == field::INSET_LEFT,
                rtl,
            ),
        );
        matches!(record.string(field::POSITION), Some("absolute" | "fixed"))
            && record.fields.iter().any(|(id, _)| *id == a)
            && record.fields.iter().any(|(id, _)| *id == b)
    };
    // Scrolling regions fill the space a page leaves them unless sized explicitly.
    let fills = record.parent.is_some_and(|p| records[p].tag == tag::PAGE)
        && matches!(record.tag, tag::SCROLL | tag::LIST)
        && [
            field::GROW,
            field::HEIGHT,
            field::HEIGHT_PERCENT,
            field::FLEX_BASIS,
        ]
        .iter()
        .all(|id| record.fields.iter().all(|(field, _)| field != id));
    // A single-line control the author sizes vertically (a height, a min-height, or stretched
    // across a row) keeps that size: no default vertical padding or touch height adds to it.
    let sized_control = matches!(record.tag, tag::INPUT | tag::SEARCH | tag::SELECT)
        && ([
            field::HEIGHT,
            field::HEIGHT_PERCENT,
            field::HEIGHT_KEYWORD,
            field::MIN_HEIGHT,
            field::MIN_HEIGHT_PERCENT,
        ]
        .iter()
        .any(|id| record.fields.iter().any(|(field, _)| field == id))
            || (record.string(field::ALIGN_SELF) == Some("stretch")
                && record.parent.is_some_and(|p| lays_out_in_row(&records[p]))));
    let padding = record.number(field::PADDING).unwrap_or(
        if matches!(
            record.tag,
            tag::INPUT
                | tag::TEXTAREA
                | tag::CHECKBOX
                | tag::SWITCH
                | tag::RADIO_OPTION
                | tag::SELECT
                | tag::SEARCH
                | tag::SLIDER
        ) {
            12.0
        } else {
            0.0
        },
    );
    let [
        mut padding_top,
        mut padding_right,
        mut padding_bottom,
        mut padding_left,
    ] = [
        field::PADDING_TOP,
        right(
            field::PADDING_RIGHT,
            field::PADDING_INLINE_START,
            field::PADDING_INLINE_END,
        ),
        field::PADDING_BOTTOM,
        left(
            field::PADDING_LEFT,
            field::PADDING_INLINE_START,
            field::PADDING_INLINE_END,
        ),
    ]
    .map(|field| {
        let vertical = field == field::PADDING_TOP || field == field::PADDING_BOTTOM;
        record.number(field).unwrap_or(
            if vertical && sized_control && record.number(field::PADDING).is_none() {
                0.0
            } else {
                padding
            },
        )
    });
    // Only a bleeding page child asks: the scan walks the last child's subtree.
    let flow_ends = (!out_of_flow && bleed_mode.is_some())
        .then(|| page_flow_ends(records))
        .flatten();
    let bleed_top = if !out_of_flow
        && matches!(bleed_mode, Some("all" | "top"))
        && flow_ends.is_some_and(|(first, _)| first == index)
    {
        safe.insets[0]
    } else {
        0.0
    };
    let bleed_right = if !out_of_flow && matches!(bleed_mode, Some("all" | "horizontal")) {
        safe.insets[1]
    } else {
        0.0
    };
    let bleed_bottom = if !out_of_flow
        && matches!(bleed_mode, Some("all" | "bottom"))
        && flow_ends.is_some_and(|(_, last)| last == index)
        && !keyboard_above
        && page_bottom_scroll(records).is_none()
    {
        safe.insets[2]
    } else {
        0.0
    };
    let bleed_left = if !out_of_flow && matches!(bleed_mode, Some("all" | "horizontal")) {
        safe.insets[3]
    } else {
        0.0
    };
    if page {
        padding_top += safe.insets[0];
        padding_right += safe.insets[1];
        // The keyboard overlaps bottom chrome; avoid whichever is taller.
        padding_bottom += if keyboard_above || page_bottom_scroll(records).is_none() {
            safe.insets[2].max(safe.keyboard)
        } else {
            0.0
        };
        padding_left += safe.insets[3];
    } else if !keyboard_above
        && matches!(record.tag, tag::SCROLL | tag::LIST)
        && page_bottom_scroll(records) == Some(index)
        && !page_flow_ends(records).is_some_and(|(_, last)| {
            matches!(records[last].string(field::BLEED), Some("all" | "bottom"))
        })
    {
        // It runs on under the bottom bars, which show it through; its end still clears them.
        padding_bottom += safe.insets[2];
    }
    padding_top += bleed[0];
    padding_right += bleed[1];
    padding_bottom += bleed[2];
    padding_left += bleed[3];
    let composed_switch = composed_switch(records, index);
    if composed_switch {
        padding_right += check_reserve(record, 1.0);
    }
    let display = record.string(field::DISPLAY);
    // `block` children stretch down a column; inline boxes shrink to content.
    let block = matches!(display, Some("block" | "inline-block"));
    let in_row = record.parent.is_some_and(|p| lays_out_in_row(&records[p]));
    let inline = matches!(display, Some("inline-flex" | "inline-block"));
    let scroll = scrolls(record);
    let in_scroll = record.parent.is_some_and(|p| scrolls(&records[p]));
    let asked = record.number(field::ALIGN_SAFE).unwrap_or(0.0) as u64;
    let safe_for = |bit: u64, scrolling: bool| scrolling || asked & bit != 0;
    // A replaced element keeps its own size unless it asks to stretch across its line
    // (`self-stretch`), as an `<img>` does in CSS; a stretching parent alone leaves icons whole.
    // A direct `<page>` child with `bleed` (`"all"` or `"horizontal"`) also stretches across the page.
    let bleed_horizontal = !out_of_flow && matches!(bleed_mode, Some("all" | "horizontal"));
    let stretched = (matches!(record.tag, tag::VECTOR | tag::IMAGE)
        && record.string(field::ALIGN_SELF) == Some("stretch"))
        || bleed_horizontal;
    // Its line runs across a flex column; in a row or a grid cell, `align-self` is vertical.
    let stretched_across = stretched
        && !in_row
        && record
            .parent
            .is_some_and(|p| records[p].string(field::DISPLAY) != Some("grid"));
    let stretched_down = stretched && !bleed_horizontal && !stretched_across;
    let border = record.number(field::BORDER_WIDTH).unwrap_or(0.0);
    let [border_top, border_right, border_bottom, border_left] =
        border_edges(record, rtl).map(|id| record.number(id).unwrap_or(border));
    let mut style = Style {
        display: if display == Some("none") {
            Display::None
        } else if display == Some("grid") {
            Display::Grid
        } else {
            Display::Flex
        },
        grid_column: record
            .string(field::GRID_COLUMN)
            .map(crate::layout::grid::placement)
            .transpose()?
            .unwrap_or_default(),
        grid_row: record
            .string(field::GRID_ROW)
            .map(crate::layout::grid::placement)
            .transpose()?
            .unwrap_or_default(),
        position: if matches!(record.string(field::POSITION), Some("absolute" | "fixed")) {
            Position::Absolute
        } else {
            Position::Relative
        },
        // Sticky insets offset the box at composition, never in layout.
        inset: if record.string(field::POSITION) == Some("sticky") {
            Rect {
                left: auto(),
                right: auto(),
                top: auto(),
                bottom: auto(),
            }
        } else {
            Rect {
                left: inset(
                    record,
                    left(
                        field::INSET_LEFT,
                        field::INSET_INLINE_START,
                        field::INSET_INLINE_END,
                    ),
                    safe_left,
                ),
                top: inset(record, field::INSET_TOP, safe_top),
                right: inset(
                    record,
                    right(
                        field::INSET_RIGHT,
                        field::INSET_INLINE_START,
                        field::INSET_INLINE_END,
                    ),
                    safe_right,
                ),
                bottom: inset(record, field::INSET_BOTTOM, safe_bottom),
            }
        },
        direction: if rtl {
            taffy::Direction::Rtl
        } else {
            taffy::Direction::Ltr
        },
        grid_template_columns: record
            .string(field::GRID_TEMPLATE_COLUMNS)
            .map(crate::layout::grid::template)
            .transpose()?
            .unwrap_or_default(),
        grid_template_rows: record
            .string(field::GRID_TEMPLATE_ROWS)
            .map(crate::layout::grid::template)
            .transpose()?
            .unwrap_or_default(),
        grid_auto_columns: record
            .string(field::GRID_AUTO_COLUMNS)
            .map(crate::layout::grid::auto_tracks)
            .transpose()?
            .unwrap_or_default(),
        grid_auto_rows: record
            .string(field::GRID_AUTO_ROWS)
            .map(crate::layout::grid::auto_tracks)
            .transpose()?
            .unwrap_or_default(),
        grid_auto_flow: match record.string(field::GRID_AUTO_FLOW) {
            Some("column") => GridAutoFlow::Column,
            Some("row-dense") => GridAutoFlow::RowDense,
            Some("column-dense") => GridAutoFlow::ColumnDense,
            _ => GridAutoFlow::Row,
        },
        justify_items: record
            .string(field::JUSTIFY_ITEMS)
            .map(|value| cross_alignment(value, safe_for(safe::JUSTIFY_ITEMS, scroll))),
        justify_self: record
            .string(field::JUSTIFY_SELF)
            .map(|value| cross_alignment(value, safe_for(safe::JUSTIFY_SELF, in_scroll))),
        align_content: record.string(field::ALIGN_CONTENT).map(|value| {
            match (value, safe_for(safe::ALIGN_CONTENT, scroll)) {
                ("center", true) => AlignContent::SAFE_CENTER,
                ("center", false) => AlignContent::CENTER,
                ("end", true) => AlignContent::SAFE_END,
                ("end", false) => AlignContent::END,
                ("between", _) => AlignContent::SPACE_BETWEEN,
                ("around", _) => AlignContent::SPACE_AROUND,
                ("evenly", _) => AlignContent::SPACE_EVENLY,
                ("stretch", _) => AlignContent::STRETCH,
                _ => AlignContent::START,
            }
        }),
        flex_direction: match record.string(field::FLEX_DIRECTION) {
            Some("row") => FlexDirection::Row,
            Some("row-reverse") => FlexDirection::RowReverse,
            Some("column") => FlexDirection::Column,
            Some("column-reverse") => FlexDirection::ColumnReverse,
            _ => FlexDirection::Column,
        },
        flex_wrap: match record.string(field::FLEX_WRAP) {
            Some("wrap") => FlexWrap::Wrap,
            Some("wrap-reverse") => FlexWrap::WrapReverse,
            _ => FlexWrap::NoWrap,
        },
        flex_grow: record
            .number(field::GROW)
            .unwrap_or(if fills { 1.0 } else { 0.0 }),
        flex_basis: match record
            .fields
            .iter()
            .find(|(id, _)| *id == field::FLEX_BASIS)
            .map(|(_, value)| crate::protocol::flex_basis(value))
            .transpose()?
            .flatten()
        {
            Some((value, true)) => percent(value / 100.0),
            Some((value, false)) => length(value),
            None => auto(),
        },
        // In a row an item shrinks as in CSS, down to its automatic minimum size. A column stands
        // for block flow, where stacked boxes never shrink: only scrolls and growing boxes do.
        flex_shrink: record.number(field::SHRINK).unwrap_or(
            if in_row
                || ((scrolls(record) || record.number(field::GROW).unwrap_or(0.0) > 0.0)
                    && dimension::<Dimension>(record, field::HEIGHT).is_none())
            {
                1.0
            } else {
                0.0
            },
        ),
        align_items: record
            .string(field::ALIGN)
            .map(|value| cross_alignment(value, safe_for(safe::ALIGN, scroll)))
            .or(block.then_some(AlignItems::STRETCH)),
        align_self: record
            .string(field::ALIGN_SELF)
            .map(|value| cross_alignment(value, safe_for(safe::ALIGN_SELF, in_scroll)))
            .or(inline.then_some(AlignItems::FLEX_START))
            .or(bleed_horizontal.then_some(AlignItems::STRETCH)),
        justify_content: record.string(field::JUSTIFY).map(|value| {
            match (value, safe_for(safe::JUSTIFY, scroll)) {
                ("center", true) => JustifyContent::SAFE_CENTER,
                ("center", false) => JustifyContent::CENTER,
                ("end", true) => JustifyContent::SAFE_FLEX_END,
                ("end", false) => JustifyContent::FLEX_END,
                ("between", _) => JustifyContent::SPACE_BETWEEN,
                ("around", _) => JustifyContent::SPACE_AROUND,
                ("evenly", _) => JustifyContent::SPACE_EVENLY,
                _ => JustifyContent::FLEX_START,
            }
        }),
        overflow: if record.string(field::OVERFLOW) == Some("hidden") {
            Point {
                x: Overflow::Hidden,
                y: Overflow::Hidden,
            }
        } else {
            Point {
                x: if record.string(field::SCROLL_AXIS) == Some("horizontal") {
                    Overflow::Scroll
                } else if scrolls(record) {
                    Overflow::Hidden
                } else {
                    Overflow::Visible
                },
                y: if record.string(field::SCROLL_AXIS) == Some("horizontal") {
                    Overflow::Hidden
                } else if scrolls(record) {
                    Overflow::Scroll
                } else {
                    Overflow::Visible
                },
            }
        },
        min_size: Size {
            width: dimension(record, field::MIN_WIDTH)
                .or_else(|| keyword(record, field::MIN_WIDTH_KEYWORD))
                .unwrap_or(if record.tag == tag::SLIDER {
                    length(120.0)
                } else if matches!(record.tag, tag::CHECKBOX | tag::SWITCH | tag::RADIO_OPTION) {
                    length(
                        if composed_switch {
                            0.0
                        } else {
                            check_reserve(record, 1.0)
                        } + padding_left
                            + padding_right
                            + border_left
                            + border_right,
                    )
                } else if scrolls(record)
                    || (!in_row && record.number(field::GROW).unwrap_or(0.0) > 0.0)
                {
                    // A scroll container's automatic minimum is zero; an item in a row keeps its
                    // min-content width unless the author sets `min-w-0`.
                    length(0.0)
                } else if let Some(width) = replaced_size(record, field::WIDTH) {
                    // A replaced element (image, vector) keeps its specified size, as CSS's `<img>`.
                    width
                } else {
                    auto()
                }),
            height: dimension(record, field::MIN_HEIGHT)
                .or_else(|| keyword(record, field::MIN_HEIGHT_KEYWORD))
                .unwrap_or(
                    if matches!(record.tag, tag::INPUT | tag::SEARCH)
                        && !sized_control
                        && record.number(field::HEIGHT_AUTO) != Some(1.0)
                    {
                        length(48.0)
                    } else if matches!(
                        record.tag,
                        tag::CHECKBOX | tag::SWITCH | tag::RADIO_OPTION | tag::SLIDER
                    ) || (record.tag == tag::SEARCH && !sized_control)
                    {
                        length(44.0)
                    } else if scrolls(record) || record.number(field::GROW).unwrap_or(0.0) > 0.0 {
                        length(0.0)
                    } else if let Some(height) = replaced_size(record, field::HEIGHT) {
                        height
                    } else {
                        auto()
                    },
                ),
        },
        max_size: Size {
            width: dimension(record, field::MAX_WIDTH)
                .or_else(|| keyword(record, field::MAX_WIDTH_KEYWORD))
                .unwrap_or(auto()),
            height: dimension(record, field::MAX_HEIGHT)
                .or_else(|| keyword(record, field::MAX_HEIGHT_KEYWORD))
                .unwrap_or(auto()),
        },
        size: Size {
            width: if bleed_horizontal
                && record.number(field::WIDTH).is_none()
                && keyword(record, field::WIDTH_KEYWORD).is_none()
                && record
                    .number(field::WIDTH_PERCENT)
                    .is_none_or(|pct| (pct - 100.0).abs() < f32::EPSILON)
            {
                auto()
            } else {
                dimension(record, field::WIDTH)
                    .or_else(|| keyword(record, field::WIDTH_KEYWORD))
                    .unwrap_or(if page || index == 0 {
                        length(width as f32 / scale)
                    } else if spans(field::INSET_LEFT, field::INSET_RIGHT) || stretched_across {
                        auto()
                    } else if record.tag == tag::VECTOR {
                        length(24.0)
                    } else if record.tag == tag::IMAGE {
                        length(64.0)
                    } else if record.tag == tag::VIDEO
                        && dimension::<Dimension>(record, field::HEIGHT).is_none()
                        && keyword(record, field::HEIGHT_KEYWORD).is_none()
                    {
                        // Unsized, a video is a full-width 16:9 box; given a height, the ratio sets its width.
                        percent(1.0)
                    } else if record.tag == tag::WEBVIEW {
                        percent(1.0)
                    } else {
                        auto()
                    })
            },
            height: dimension(record, field::HEIGHT)
                .or_else(|| keyword(record, field::HEIGHT_KEYWORD))
                .unwrap_or(if page {
                    length(height as f32 / scale)
                } else if record.number(field::HEIGHT_AUTO) == Some(1.0)
                    || matches!(record.tag, tag::INPUT | tag::SEARCH)
                {
                    auto()
                } else if record.tag == tag::SLIDER {
                    length(48.0)
                } else if record.tag == tag::TEXTAREA && record.number(field::ROWS).is_none() {
                    length(120.0)
                } else if index == 0 {
                    length(height as f32 / scale)
                } else if spans(field::INSET_TOP, field::INSET_BOTTOM) || stretched_down {
                    auto()
                } else if record.tag == tag::VECTOR {
                    length(24.0)
                } else if record.tag == tag::IMAGE {
                    length(64.0)
                } else if record.tag == tag::WEBVIEW && record.number(field::ASPECT_RATIO).is_none()
                {
                    length(150.0)
                } else {
                    auto()
                }),
        },
        gap: Size {
            width: length(if record.tag == tag::LIST {
                0.0
            } else {
                record
                    .number(field::COLUMN_GAP)
                    .or(record.number(field::GAP))
                    .unwrap_or(0.0)
            }),
            height: length(if record.tag == tag::LIST {
                0.0
            } else {
                record
                    .number(field::ROW_GAP)
                    .or(record.number(field::GAP))
                    .unwrap_or(0.0)
            }),
        },
        border: Rect {
            left: length(border_left),
            right: length(border_right),
            top: length(border_top),
            bottom: length(border_bottom),
        },
        padding: Rect {
            left: length(padding_left + if record.tag == tag::SEARCH { 32.0 } else { 0.0 }),
            right: length(padding_right),
            top: length(padding_top),
            bottom: length(padding_bottom),
        },
        margin: Rect {
            // List rows are realized windows; the list gap separates them as row margins.
            bottom: match record
                .parent
                .filter(|p| records[*p].tag == tag::LIST)
                .filter(|p| {
                    record.number(field::LIST_ROW_INDEX).unwrap_or(0.0) + 1.0
                        < records[*p].number(field::LIST_COUNT).unwrap_or(0.0)
                })
                .and_then(|p| records[p].number(field::GAP))
            {
                Some(gap) => length(gap),
                None => margin_with_bleed(record, field::MARGIN_BOTTOM, bleed_bottom),
            },
            left: margin_with_bleed(
                record,
                left(
                    field::MARGIN_LEFT,
                    field::MARGIN_INLINE_START,
                    field::MARGIN_INLINE_END,
                ),
                bleed_left,
            ),
            right: margin_with_bleed(
                record,
                right(
                    field::MARGIN_RIGHT,
                    field::MARGIN_INLINE_START,
                    field::MARGIN_INLINE_END,
                ),
                bleed_right,
            ),
            top: margin_with_bleed(record, field::MARGIN_TOP, bleed_top),
        },
        aspect_ratio: record
            .number(field::ASPECT_RATIO)
            .or((record.tag == tag::VIDEO).then_some(16.0 / 9.0)),
        ..Default::default()
    };
    // An authored size grows by what it paints past its edges, so its content keeps that size.
    let grow = |size: &mut Dimension, by: f32| {
        if by > 0.0
            && let taffy::style::ExpandedDimension::Length(authored) = size.expand()
        {
            *size = length(authored + by);
        }
    };
    grow(&mut style.size.width, bleed[1] + bleed[3]);
    grow(&mut style.size.height, bleed[0] + bleed[2]);
    Ok(style)
}
