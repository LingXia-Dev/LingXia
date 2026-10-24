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

pub(crate) fn cross_alignment(value: &str) -> AlignItems {
    match value {
        "center" => AlignItems::SAFE_CENTER,
        "end" => AlignItems::SAFE_FLEX_END,
        "stretch" => AlignItems::STRETCH,
        "baseline" => AlignItems::BASELINE,
        _ => AlignItems::FLEX_START,
    }
}

pub(crate) fn check_reserve(tag: usize) -> f32 {
    match tag {
        tag::CHECKBOX | tag::RADIO_OPTION => 36.0,
        tag::SWITCH => 64.0,
        _ => 0.0,
    }
}

pub(crate) fn check_height(tag: usize) -> f32 {
    match tag {
        tag::CHECKBOX | tag::RADIO_OPTION => 24.0,
        tag::SWITCH => 32.0,
        _ => 0.0,
    }
}

/// Taffy style for one record; the root fills the viewport.
pub(crate) fn node_style(
    records: &[Record],
    index: usize,
    width: u16,
    height: u16,
    scale: f32,
) -> Result<Style, String> {
    let record = &records[index];
    let padding = record.number(field::PADDING).unwrap_or(
        if matches!(
            record.tag,
            tag::BUTTON
                | tag::INPUT
                | tag::TEXTAREA
                | tag::CHECKBOX
                | tag::SWITCH
                | tag::RADIO_OPTION
                | tag::SEARCH
                | tag::SLIDER
        ) {
            12.0
        } else {
            0.0
        },
    );
    let [padding_top, padding_right, padding_bottom, padding_left] =
        crate::protocol::PADDING_EDGES.map(|field| record.number(field).unwrap_or(padding));
    let border = record.number(field::BORDER_WIDTH).unwrap_or(0.0);
    let [border_top, border_right, border_bottom, border_left] =
        crate::protocol::BORDER_EDGES.map(|id| record.number(id).unwrap_or(border));
    Ok(Style {
        display: if matches!(record.tag, tag::GRID | tag::STACK) {
            Display::Grid
        } else {
            Display::Flex
        },
        grid_column: if record.parent.is_some_and(|p| records[p].tag == tag::STACK) {
            Line {
                start: line(1),
                end: line(2),
            }
        } else {
            Default::default()
        },
        grid_row: if record.parent.is_some_and(|p| records[p].tag == tag::STACK) {
            Line {
                start: line(1),
                end: line(2),
            }
        } else {
            Default::default()
        },
        inset: Rect {
            left: record.number(field::OFFSET_X).map(length).unwrap_or(auto()),
            top: record.number(field::OFFSET_Y).map(length).unwrap_or(auto()),
            right: auto(),
            bottom: auto(),
        },
        grid_template_columns: if record.tag == tag::GRID {
            vec![flex(1.0); record.number(field::GRID_COLUMNS).unwrap_or(1.0) as usize]
        } else {
            Vec::new()
        },
        flex_direction: match record.string(field::FLEX_DIRECTION) {
            Some("row") => FlexDirection::Row,
            Some("row-reverse") => FlexDirection::RowReverse,
            Some("column") => FlexDirection::Column,
            Some("column-reverse") => FlexDirection::ColumnReverse,
            _ if record.tag == tag::ROW => FlexDirection::Row,
            _ => FlexDirection::Column,
        },
        flex_wrap: match record.string(field::FLEX_WRAP) {
            Some("wrap") => FlexWrap::Wrap,
            Some("wrap-reverse") => FlexWrap::WrapReverse,
            _ => FlexWrap::NoWrap,
        },
        flex_grow: record.number(field::GROW).unwrap_or(0.0),
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
        flex_shrink: record.number(field::SHRINK).unwrap_or(
            if (matches!(record.tag, tag::SCROLL | tag::LIST)
                || record.number(field::GROW).unwrap_or(0.0) > 0.0)
                && dimension::<Dimension>(record, field::HEIGHT).is_none()
            {
                1.0
            } else {
                0.0
            },
        ),
        align_items: record.string(field::ALIGN).map(cross_alignment),
        align_self: record.string(field::ALIGN_SELF).map(cross_alignment),
        justify_content: record.string(field::JUSTIFY).map(|value| match value {
            "center" => JustifyContent::SAFE_CENTER,
            "end" => JustifyContent::SAFE_FLEX_END,
            "between" => JustifyContent::SPACE_BETWEEN,
            _ => JustifyContent::FLEX_START,
        }),
        overflow: Point {
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
        },
        min_size: Size {
            width: dimension(record, field::MIN_WIDTH).unwrap_or(if record.tag == tag::SLIDER {
                length(120.0)
            } else if matches!(record.tag, tag::CHECKBOX | tag::SWITCH | tag::RADIO_OPTION) {
                length(
                    check_reserve(record.tag)
                        + padding_left
                        + padding_right
                        + border_left
                        + border_right,
                )
            } else if matches!(record.tag, tag::SCROLL | tag::LIST)
                || record.number(field::GROW).unwrap_or(0.0) > 0.0
            {
                length(0.0)
            } else {
                auto()
            }),
            height: dimension(record, field::MIN_HEIGHT).unwrap_or(
                if matches!(record.tag, tag::INPUT | tag::SEARCH)
                    && dimension::<taffy::Dimension>(record, field::HEIGHT).is_none()
                    && record.number(field::HEIGHT_AUTO) != Some(1.0)
                {
                    length(48.0)
                } else if matches!(
                    record.tag,
                    tag::CHECKBOX | tag::SWITCH | tag::RADIO_OPTION | tag::SEARCH | tag::SLIDER
                ) {
                    length(44.0)
                } else if matches!(record.tag, tag::SCROLL | tag::LIST)
                    || record.number(field::GROW).unwrap_or(0.0) > 0.0
                {
                    length(0.0)
                } else {
                    auto()
                },
            ),
        },
        max_size: Size {
            width: dimension(record, field::MAX_WIDTH).unwrap_or(auto()),
            height: dimension(record, field::MAX_HEIGHT).unwrap_or(auto()),
        },
        size: Size {
            width: dimension(record, field::WIDTH).unwrap_or(if index == 0 {
                length(width as f32 / scale)
            } else if record.tag == tag::VECTOR {
                length(24.0)
            } else if record.tag == tag::IMAGE {
                length(64.0)
            } else {
                auto()
            }),
            height: dimension(record, field::HEIGHT).unwrap_or(
                if record.number(field::HEIGHT_AUTO) == Some(1.0)
                    || matches!(record.tag, tag::INPUT | tag::SEARCH)
                {
                    auto()
                } else if record.tag == tag::SLIDER {
                    length(48.0)
                } else if record.tag == tag::TEXTAREA {
                    length(120.0)
                } else if index == 0 {
                    length(height as f32 / scale)
                } else if record.tag == tag::VECTOR {
                    length(24.0)
                } else if record.tag == tag::IMAGE {
                    length(64.0)
                } else {
                    auto()
                },
            ),
        },
        gap: Size {
            width: length(if record.tag == tag::LIST {
                0.0
            } else {
                record.number(field::GAP).unwrap_or(0.0)
            }),
            height: length(if record.tag == tag::LIST {
                0.0
            } else {
                record.number(field::GAP).unwrap_or(0.0)
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
            bottom: length(
                record
                    .parent
                    .filter(|p| records[*p].tag == tag::LIST)
                    .filter(|p| {
                        record.number(field::LIST_ROW_INDEX).unwrap_or(0.0) + 1.0
                            < records[*p].number(field::LIST_COUNT).unwrap_or(0.0)
                    })
                    .and_then(|p| records[p].number(field::GAP))
                    .unwrap_or(0.0),
            ),
            left: length(0.0),
            right: length(0.0),
            top: length(0.0),
        },
        ..Default::default()
    })
}
