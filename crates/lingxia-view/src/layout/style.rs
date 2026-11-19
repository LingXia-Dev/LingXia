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

fn scrolls(record: &Record) -> bool {
    matches!(record.tag, tag::SCROLL | tag::LIST)
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

pub(crate) fn check_reserve(record: &Record) -> f32 {
    match record.tag {
        tag::CHECKBOX | tag::RADIO_OPTION => 36.0,
        tag::SWITCH => crate::scene::slider::switch_track(record)[0] + 12.0,
        // The chevron at the end of a select.
        tag::SELECT => 28.0,
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

fn margin(record: &Record, field: usize) -> LengthPercentageAuto {
    match record
        .fields
        .iter()
        .find(|(id, _)| *id == field)
        .map(|(_, value)| value)
    {
        Some(value) if value.as_str() == Some("auto") => auto(),
        Some(value) => value
            .as_f64()
            .map(|v| length(v as f32))
            .unwrap_or(length(0.0)),
        None => length(0.0),
    }
}

/// `offset` moves a length inset in from the containing edge.
fn inset(record: &Record, field: usize, offset: f32) -> LengthPercentageAuto {
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

/// A page's last in-flow child when it scrolls: it reaches the page's bottom edge under the bars.
fn page_bottom_scroll(records: &[Record]) -> Option<usize> {
    if records.first()?.tag != tag::PAGE {
        return None;
    }
    let last = (1..records.len()).rev().find(|&index| {
        let record = &records[index];
        record.parent == Some(0)
            && !matches!(record.string(field::POSITION), Some("absolute" | "fixed"))
            && record.string(field::DISPLAY) != Some("none")
    })?;
    matches!(records[last].tag, tag::SCROLL | tag::LIST).then_some(last)
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
    // Fixed boxes measure their length insets from the page's safe content box:
    // inside the system insets and above the keyboard.
    let [safe_top, safe_right, safe_bottom, safe_left] =
        if records[0].tag == tag::PAGE && record.string(field::POSITION) == Some("fixed") {
            // A top-layer box already ends above the keyboard by its own bottom inset.
            let lifted = record
                .parent
                .and_then(|parent| top_layer_of(records, parent))
                .and_then(|layer| records[layer].number(field::INSET_BOTTOM))
                .unwrap_or(0.0);
            [
                safe.insets[0],
                safe.insets[1],
                (safe.insets[2].max(safe.keyboard) - lifted).max(0.0),
                safe.insets[3],
            ]
        } else {
            [0.0; 4]
        };
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
    let keyboard_above = safe.keyboard > safe.insets[2];
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
        && record.parent == Some(0)
        && page_bottom_scroll(records) == Some(index)
    {
        // It runs on under the bottom bars, which show it through; its end still clears them.
        padding_bottom += safe.insets[2];
    }
    let composed_switch = composed_switch(records, index);
    if composed_switch {
        padding_right += check_reserve(record);
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
    let stretched = matches!(record.tag, tag::VECTOR | tag::IMAGE)
        && record.string(field::ALIGN_SELF) == Some("stretch");
    // Its line runs across a flex column; in a row or a grid cell, `align-self` is vertical.
    let stretched_across = stretched
        && !in_row
        && record
            .parent
            .is_some_and(|p| records[p].string(field::DISPLAY) != Some("grid"));
    let stretched_down = stretched && !stretched_across;
    let border = record.number(field::BORDER_WIDTH).unwrap_or(0.0);
    let [border_top, border_right, border_bottom, border_left] =
        border_edges(record, rtl).map(|id| record.number(id).unwrap_or(border));
    Ok(Style {
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
                || ((matches!(record.tag, tag::SCROLL | tag::LIST)
                    || record.number(field::GROW).unwrap_or(0.0) > 0.0)
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
            .or(inline.then_some(AlignItems::FLEX_START)),
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
                } else if matches!(record.tag, tag::SCROLL | tag::LIST) {
                    Overflow::Hidden
                } else {
                    Overflow::Visible
                },
                y: if record.string(field::SCROLL_AXIS) == Some("horizontal") {
                    Overflow::Hidden
                } else if matches!(record.tag, tag::SCROLL | tag::LIST) {
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
                            check_reserve(record)
                        } + padding_left
                            + padding_right
                            + border_left
                            + border_right,
                    )
                } else if matches!(record.tag, tag::SCROLL | tag::LIST)
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
                    } else if matches!(record.tag, tag::SCROLL | tag::LIST)
                        || record.number(field::GROW).unwrap_or(0.0) > 0.0
                    {
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
            width: dimension(record, field::WIDTH)
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
                }),
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
                None => margin(record, field::MARGIN_BOTTOM),
            },
            left: margin(
                record,
                left(
                    field::MARGIN_LEFT,
                    field::MARGIN_INLINE_START,
                    field::MARGIN_INLINE_END,
                ),
            ),
            right: margin(
                record,
                right(
                    field::MARGIN_RIGHT,
                    field::MARGIN_INLINE_START,
                    field::MARGIN_INLINE_END,
                ),
            ),
            top: margin(record, field::MARGIN_TOP),
        },
        aspect_ratio: record
            .number(field::ASPECT_RATIO)
            .or((record.tag == tag::VIDEO).then_some(16.0 / 9.0)),
        ..Default::default()
    })
}
