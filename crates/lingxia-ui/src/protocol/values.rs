//! Value grammars shared by decode and layout: lengths, colors, bases and paint deltas.
use super::{FIELDS, FOCUS_PAINT_FIELDS, field, limits, tag};
use serde_json::Value;

/// Largest callback handle a JS number represents exactly.
const MAX_HANDLE: u64 = 9_007_199_254_740_991;

/// A logical length, or a bounded percentage string such as `"50%"`.
pub(crate) fn length_or_percent(value: &Value, signed: bool) -> Option<(f32, bool)> {
    let percent = value.as_str().and_then(|v| v.strip_suffix('%'));
    let number = match percent {
        Some(text) => text.parse::<f64>().ok(),
        None => value.as_f64(),
    }?;
    let bound = if signed { -16384.0 } else { 0.0 };
    (number.is_finite() && (bound..=16384.0).contains(&number))
        .then_some((number as f32, percent.is_some()))
}

pub(crate) fn flex_basis(value: &Value) -> Result<Option<(f32, bool)>, String> {
    if value.as_str() == Some("auto") {
        return Ok(None);
    }
    let percent = value.as_str().and_then(|v| v.strip_suffix('%'));
    let number = if let Some(text) = percent {
        if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
            return Err("UI_STYLE: invalid flex basis".into());
        }
        text.parse::<f64>().ok()
    } else {
        value.as_f64()
    };
    let number = number
        .filter(|v| v.is_finite() && (0.0..=16384.0).contains(v))
        .ok_or("UI_STYLE: invalid flex basis")?;
    Ok(Some((number as f32, percent.is_some())))
}

pub(crate) fn focus_style(value: &Value) -> Result<Vec<(usize, Value)>, String> {
    let text = value
        .as_str()
        .filter(|s| s.len() <= 16384)
        .ok_or("UI_STYLE: invalid focus style")?;
    let pairs: Vec<Value> =
        serde_json::from_str(text).map_err(|_| "UI_STYLE: invalid focus style")?;
    if pairs.len() > 14
        || !pairs.len().is_multiple_of(2)
        || pairs.chunks_exact(2).any(|p| {
            p[0].as_u64()
                .is_none_or(|id| !FOCUS_PAINT_FIELDS.contains(&(id as usize)))
        })
    {
        return Err("UI_STYLE: focus supports paint properties only".into());
    }
    let mut record = vec![
        serde_json::json!(0),
        serde_json::json!(-1),
        serde_json::json!(""),
        serde_json::json!(""),
        serde_json::json!(""),
        serde_json::json!(pairs.len() / 2),
    ];
    record.extend(pairs);
    Ok(super::decode(&record)?.remove(0).fields)
}

pub fn color(value: &str) -> Result<[u8; 4], String> {
    let hex = match value {
        "background" => "f6f7fb",
        "surface" => "ffffff",
        "text" => "172033",
        "textMuted" => "596579",
        "primary" => "365cdd",
        "primaryText" => "ffffff",
        "danger" => "c93434",
        "border" => "dce1ec",
        "accent" => "0b877d",
        value => value.strip_prefix('#').ok_or("UI_COLOR: unknown token")?,
    };
    if !matches!(hex.len(), 6 | 8) {
        return Err("UI_COLOR: expected #rrggbb or #rrggbbaa".into());
    }
    if !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("UI_COLOR: invalid hex".into());
    }
    let n = u32::from_str_radix(hex, 16).map_err(|_| "UI_COLOR: invalid hex")?;
    Ok(if hex.len() == 8 {
        [(n >> 24) as u8, (n >> 16) as u8, (n >> 8) as u8, n as u8]
    } else {
        [(n >> 16) as u8, (n >> 8) as u8, n as u8, 255]
    })
}

fn handle(value: &Value) -> bool {
    value.as_u64().is_some_and(|v| v > 0 && v <= MAX_HANDLE)
}

fn ensure(valid: bool, error: &str) -> Result<(), String> {
    if valid { Ok(()) } else { Err(error.into()) }
}

fn finite_within(value: &Value, bound: f64) -> bool {
    value
        .as_f64()
        .is_some_and(|v| v.is_finite() && v.abs() <= bound)
}

fn flag(value: &Value) -> bool {
    value.as_u64().is_some_and(|v| v <= 1)
}

fn bounded_str(value: &Value, max: usize) -> bool {
    value.as_str().is_some_and(|s| s.len() <= max)
}

/// Checks one field value whose grammar needs no page state. The field is
/// already admitted on `tag`; page-budgeted and cross-record fields are
/// checked by the record walk.
pub(super) fn check(tag: usize, id: usize, value: &Value) -> Result<(), String> {
    if let Some(options) = super::options(FIELDS[id].1) {
        let value = value
            .as_str()
            .filter(|v| options.contains(v))
            .ok_or("UI_STYLE: invalid style option")?;
        if id == field::KEYBOARD {
            if (tag == tag::SEARCH) != (value == "search") {
                return Err("UI_INPUT: search requires search keyboard".into());
            }
            if matches!(value, "date" | "time") && tag != tag::INPUT {
                return Err("UI_INPUT: date and time pickers need input".into());
            }
        }
        return ensure(
            tag != tag::LIST
                || !((id == field::ALIGN && value != "stretch")
                    || (id == field::JUSTIFY && value != "start")),
            "UI_LIST: rows require stretch alignment and start justification",
        );
    }
    match id {
        field::FLEX_BASIS => flex_basis(value).map(drop),
        field::FONT_VARIANT_NUMERIC => ensure(
            value.as_str().is_some_and(|text| {
                text == "normal"
                    || (text.len() <= 128
                        && text.split(' ').all(|keyword| {
                            matches!(
                                keyword,
                                "ordinal"
                                    | "slashed-zero"
                                    | "lining-nums"
                                    | "oldstyle-nums"
                                    | "proportional-nums"
                                    | "tabular-nums"
                                    | "diagonal-fractions"
                                    | "stacked-fractions"
                            )
                        }))
            }),
            "UI_STYLE: invalid font-variant-numeric",
        ),
        field::GRID_TEMPLATE_COLUMNS | field::GRID_TEMPLATE_ROWS => {
            crate::layout::grid::template(value.as_str().ok_or("UI_GRID: expected track list")?)
                .map(drop)
        }
        field::GRID_AUTO_COLUMNS | field::GRID_AUTO_ROWS => {
            crate::layout::grid::auto_tracks(value.as_str().ok_or("UI_GRID: expected track list")?)
                .map(drop)
        }
        field::GRID_COLUMN | field::GRID_ROW => {
            crate::layout::grid::placement(value.as_str().ok_or("UI_GRID: expected line")?)
                .map(drop)
        }
        field::ORDER => ensure(
            value.as_i64().is_some_and(|v| v.abs() <= 10_000),
            "UI_STYLE: invalid order",
        ),
        field::HEADING_LEVEL => ensure(
            value.as_u64().is_some_and(|v| (1..=6).contains(&v)),
            "UI_PROP: headingLevel must be 1..6",
        ),
        field::ACCESSIBILITY_HIDDEN => ensure(
            value.as_u64() == Some(1),
            "UI_PROP: invalid accessibilityHidden",
        ),
        field::ASPECT_RATIO => ensure(
            value
                .as_f64()
                .is_some_and(|v| v.is_finite() && v > 0.0 && v <= 10000.0),
            "UI_STYLE: invalid aspect ratio",
        ),
        field::MAX_WIDTH_CH => ensure(
            value
                .as_f64()
                .is_some_and(|v| v.is_finite() && (0.0..=4096.0).contains(&v)),
            "UI_STYLE: invalid ch length",
        ),
        field::TRANSFORM => crate::scene::transform::StaticTransform::parse(
            value
                .as_str()
                .ok_or("UI_TRANSFORM: expected encoded transform")?,
        )
        .map(drop),
        field::FOCUS_STYLE => focus_style(value).map(drop),
        field::VECTOR_SOURCE => ensure(value.is_string(), "UI_VECTOR: source requires vector node"),
        field::SUBMIT_ON_ENTER => ensure(flag(value), "UI_INPUT: invalid submitOnEnter"),
        field::HEIGHT_AUTO => ensure(
            value.as_u64() == Some(1),
            "UI_STYLE: invalid automatic height",
        ),
        field::BOX_SHADOW => crate::scene::shadow::BoxShadow::parse(
            value
                .as_str()
                .ok_or("UI_SHADOW: expected encoded shadows")?,
        )
        .map(drop),
        field::BACKGROUND_GRADIENT => crate::scene::gradient::BackgroundGradient::parse(
            value
                .as_str()
                .ok_or("UI_GRADIENT: expected encoded gradient")?,
        )
        .map(drop),
        field::LETTER_SPACING => ensure(
            finite_within(value, 1024.0),
            "UI_STYLE: invalid letter spacing",
        ),
        field::PLOT_MARKER_EMPHASIS_SCALE => ensure(
            value
                .as_f64()
                .is_some_and(|v| v.is_finite() && (1.0..=4.0).contains(&v)),
            "UI_PLOT: invalid marker emphasis scale",
        ),
        field::PLOT_PIE_LAYOUT => ensure(bounded_str(value, 1024), "UI_PLOT: invalid pie layout"),
        field::PLOT_DOMAIN_MIN | field::PLOT_DOMAIN_MAX => {
            ensure(finite_within(value, 1e9), "UI_PLOT: invalid domain bound")
        }
        field::ON_PLOT_SELECT => ensure(handle(value), "UI_PLOT: invalid selection callback"),
        field::PLOT_SELECTED_INDEX => ensure(
            value.as_i64().is_some_and(|v| (-1..128).contains(&v)),
            "UI_PLOT: invalid selected index",
        ),
        field::PLOT_SMOOTH | field::PLOT_AREA => ensure(
            matches!(value.as_u64(), Some(0 | 1)),
            "UI_PLOT: invalid boolean property",
        ),
        field::PLOT_VALUES | field::PLOT_KIND | field::PLOT_COLORS => ensure(
            bounded_str(value, 8192),
            "UI_PLOT: invalid encoded property",
        ),
        field::TEXT_LINES => ensure(
            value.as_u64().is_some_and(|v| (1..=1024).contains(&v)),
            "UI_TEXT: lines must be an integer between 1 and 1024 on text",
        ),
        field::GRID_COLUMNS => ensure(
            value.as_u64().is_some_and(|v| (1..=64).contains(&v)),
            "UI_GRID: columns must be an integer between 1 and 64 on a grid",
        ),
        field::ON_SLIDER_CHANGE => ensure(handle(value), "UI_SLIDER: invalid callback"),
        field::SLIDER_VALUE | field::SLIDER_MIN | field::SLIDER_MAX | field::SLIDER_STEP => {
            ensure(finite_within(value, 1e9), "UI_SLIDER: invalid number")
        }
        field::IMAGE_RETRY_KEY => ensure(
            value
                .as_str()
                .is_some_and(|s| s.len() <= 256 && !s.contains('\0')),
            "UI_IMAGE: invalid retry key",
        ),
        field::ON_IMAGE_LOAD | field::ON_IMAGE_ERROR => {
            ensure(handle(value), "UI_IMAGE: invalid callback")
        }
        field::IMAGE_ALT => ensure(
            value.as_str().is_some_and(|s| {
                s.len() <= 1024 && !s.contains('\0') && (s.is_empty() || !s.trim().is_empty())
            }),
            "UI_IMAGE: invalid alternative text",
        ),
        field::ACCESSIBILITY_LABEL => ensure(
            value.as_str().is_some_and(|s| {
                s.len() <= 1024
                    && !s.contains('\0')
                    && !(matches!(tag, tag::BUTTON | tag::CHECKBOX | tag::SWITCH)
                        && s.trim().is_empty())
            }),
            "UI_PROP: invalid accessibility label",
        ),
        field::CHECKED => ensure(flag(value), "UI_CHECK: invalid flag or handle"),
        field::ON_CHECKED_CHANGE => ensure(handle(value), "UI_CHECK: invalid flag or handle"),
        field::INPUT_VALUE | field::PLACEHOLDER => ensure(
            bounded_str(value, crate::text::input::MAX_TEXT_BYTES),
            "UI_INPUT: invalid text",
        ),
        field::SECURE | field::AUTO_FOCUS | field::SPELL_CHECK => {
            ensure(flag(value), "UI_INPUT: invalid flag")
        }
        field::MAX_LENGTH => ensure(
            value.as_u64().is_some_and(|v| (1..=65536).contains(&v)),
            "UI_INPUT: maxLength must be 1..65536",
        ),
        field::INPUT_STEP => ensure(
            value
                .as_f64()
                .is_some_and(|v| v.is_finite() && v > 0.0 && v <= 1e15),
            "UI_INPUT: invalid step",
        ),
        field::INPUT_MIN | field::INPUT_MAX => ensure(
            finite_within(value, 1e15)
                || value.as_str().is_some_and(|text| {
                    crate::interaction::input_rules::iso(text, "date")
                        || crate::interaction::input_rules::iso(text, "time")
                }),
            "UI_INPUT: invalid min or max",
        ),
        field::ON_INPUT | field::ON_SUBMIT => ensure(handle(value), "UI_INPUT: invalid callback"),
        field::IMAGE_SRC => ensure(
            value.as_str().is_some_and(|s| {
                !s.is_empty() && s.len() <= limits::SOURCE_BYTES && !s.contains('\0')
            }),
            "UI_IMAGE: invalid source",
        ),
        field::IMAGE_FIT => ensure(
            matches!(value.as_str(), Some("cover" | "contain" | "fill")),
            "UI_IMAGE: invalid fit",
        ),
        field::ON_REACH_START | field::ON_REACH_END => {
            ensure(handle(value), "UI_SCROLL: invalid endpoint handle")
        }
        field::LIST_COUNT => ensure(
            value
                .as_u64()
                .is_some_and(|n| n <= crate::layout::list::MAX_ROWS as u64),
            "UI_LIST: invalid count",
        ),
        field::LIST_LEADING | field::LIST_TRAILING => ensure(
            value
                .as_f64()
                .is_some_and(|v| v.is_finite() && (0.0..=1_638_400_000.0).contains(&v)),
            "UI_LIST: invalid extent",
        ),
        field::LIST_ANCHOR => list_anchor(value),
        field::ON_CLICK => ensure(handle(value), "UI_PROP: invalid handle or flag"),
        field::ENABLED => ensure(flag(value), "UI_PROP: invalid handle or flag"),
        field::Z_INDEX => ensure(
            value.as_i64().is_some_and(|v| v.abs() <= 10_000),
            "UI_STYLE: invalid z-index",
        ),
        _ => match FIELDS[id].1 {
            "dimension-percent" => ensure(
                value
                    .as_f64()
                    .is_some_and(|v| v.is_finite() && (0.0..=16384.0).contains(&v)),
                "UI_STYLE: invalid percentage",
            ),
            "margin" => ensure(
                value.as_str() == Some("auto") || finite_within(value, 16384.0),
                "UI_STYLE: invalid margin",
            ),
            "inset" => ensure(
                length_or_percent(value, true).is_some(),
                "UI_STYLE: invalid inset",
            ),
            "state-style" => focus_style(value).map(drop),
            "number" => number(id, value),
            _ => {
                let value = value.as_str().ok_or("UI_STYLE: expected color")?;
                if !matches!(id, field::BORDER_COLOR | field::BACKGROUND) || value != "currentColor"
                {
                    color(value)?;
                }
                Ok(())
            }
        },
    }
}

fn number(id: usize, value: &Value) -> Result<(), String> {
    let value = value
        .as_f64()
        .filter(|v| v.is_finite() && *v >= 0.0 && *v <= 16384.0)
        .ok_or("UI_STYLE: invalid length")?;
    if matches!(id, field::FONT_SIZE | field::LINE_HEIGHT) && value == 0.0 {
        return Err("UI_STYLE: font size and line height must be positive".into());
    }
    if (id == field::BACKDROP_BLUR && value > 64.0)
        || (id == field::BACKDROP_SATURATION && value > 16.0)
    {
        return Err("UI_STYLE: invalid backdrop filter".into());
    }
    if id == field::OPACITY && value > 1.0 {
        return Err("UI_STYLE: opacity must be between 0 and 1".into());
    }
    if id == field::FONT_WEIGHT && !(1.0..=1000.0).contains(&value) {
        return Err("UI_STYLE: font weight must be between 1 and 1000".into());
    }
    Ok(())
}

fn list_anchor(value: &Value) -> Result<(), String> {
    let text = value
        .as_str()
        .filter(|s| s.len() <= 256)
        .ok_or("UI_LIST: invalid anchor")?;
    let value: Value = serde_json::from_str(text).map_err(|_| "UI_LIST: invalid anchor")?;
    if value != "end" {
        let object = value.as_object().ok_or("UI_LIST: invalid anchor")?;
        if object
            .keys()
            .any(|k| !matches!(k.as_str(), "index" | "align" | "seq"))
            || object.get("index").and_then(Value::as_u64).is_none()
            || !matches!(
                object.get("align").and_then(Value::as_str),
                Some("start" | "end")
            )
            || object.get("seq").is_some_and(|v| v.as_u64().is_none())
        {
            return Err("UI_LIST: invalid anchor".into());
        }
    }
    Ok(())
}
