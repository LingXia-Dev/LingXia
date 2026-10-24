//! Record-to-Taffy style resolution and primitive control metrics.
use crate::protocol::Record;
use taffy::prelude::*;

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
        10 | 13 => 36.0,
        11 => 64.0,
        _ => 0.0,
    }
}

pub(crate) fn check_height(tag: usize) -> f32 {
    match tag {
        10 | 13 => 24.0,
        11 => 32.0,
        _ => 0.0,
    }
}
