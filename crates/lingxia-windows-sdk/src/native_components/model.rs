//! Native component message models and parsing helpers.

use serde_json::Value;

#[derive(Clone, Copy, PartialEq, Default)]
pub(super) struct DocRect {
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) width: f64,
    pub(super) height: f64,
}

/// Component props this host applies. Each field is `Some` only when the
/// view supplied it; updates merge field-wise into the stored state. Text
/// and video components read disjoint subsets of one props bag — the parse
/// is shared and unknown fields are simply never consulted.
#[derive(Clone, Default)]
pub(super) struct ComponentProps {
    pub(super) value: Option<String>,
    pub(super) placeholder: Option<String>,
    pub(super) text_color: Option<u32>,
    pub(super) font_size: Option<f64>,
    pub(super) disabled: Option<bool>,
    pub(super) password: Option<bool>,
    pub(super) maxlength: Option<u32>,
    pub(super) focus: Option<bool>,
    /// The element's measured CSS border-radius (CSS px); clips the
    /// container window. Geometry-only updates carry it at the payload top
    /// level; the message handlers lift it into the props.
    pub(super) corner_radius: Option<f64>,
    // — video —
    pub(super) src: Option<String>,
    pub(super) autoplay: Option<bool>,
    pub(super) looping: Option<bool>,
    pub(super) muted: Option<bool>,
    pub(super) volume: Option<f64>,
    pub(super) controls: Option<bool>,
    pub(super) progress_bar: Option<bool>,
    /// (label, url) quality presets for the controls-bar quality menu.
    pub(super) qualities: Option<Vec<(String, Option<String>)>>,
    pub(super) playback_rates: Option<Vec<f64>>,
    pub(super) bindings_json: Option<String>,
    pub(super) dataset_json: Option<String>,
}

impl ComponentProps {
    pub(super) fn merge_from(&mut self, other: &ComponentProps) {
        macro_rules! take {
            ($field:ident) => {
                if other.$field.is_some() {
                    self.$field = other.$field.clone();
                }
            };
        }
        take!(value);
        take!(placeholder);
        take!(text_color);
        take!(font_size);
        take!(disabled);
        take!(password);
        take!(maxlength);
        take!(focus);
        take!(corner_radius);
        take!(src);
        take!(autoplay);
        take!(looping);
        take!(muted);
        take!(volume);
        take!(controls);
        take!(progress_bar);
        take!(qualities);
        take!(playback_rates);
        take!(bindings_json);
        take!(dataset_json);
    }
}

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

pub(super) fn parse_rect(raw: Option<&Value>) -> Option<DocRect> {
    let rect = raw?;
    Some(DocRect {
        x: rect.get("x").and_then(Value::as_f64)?,
        y: rect.get("y").and_then(Value::as_f64)?,
        width: rect.get("width").and_then(Value::as_f64)?,
        height: rect.get("height").and_then(Value::as_f64)?,
    })
}

fn value_as_bool(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(flag) => Some(*flag),
        Value::String(text) => match text.trim().to_ascii_lowercase().as_str() {
            "true" | "1" => Some(true),
            "false" | "0" | "" => Some(false),
            _ => None,
        },
        Value::Number(number) => number.as_f64().map(|n| n != 0.0),
        _ => None,
    }
}

pub(super) fn corner_radius_value(raw: Option<&Value>) -> Option<f64> {
    raw.and_then(Value::as_f64)
        .filter(|radius| radius.is_finite() && *radius >= 0.0)
}

pub(super) fn parse_props(raw: Option<&Value>) -> ComponentProps {
    let mut props = ComponentProps::default();
    let Some(raw) = raw.and_then(Value::as_object) else {
        return props;
    };

    props.value = raw.get("value").and_then(Value::as_str).map(str::to_string);
    props.placeholder = raw
        .get("placeholder")
        .and_then(Value::as_str)
        .map(str::to_string);
    props.text_color = raw
        .get("textColor")
        .and_then(Value::as_str)
        .and_then(parse_css_color);
    props.font_size = raw
        .get("fontSize")
        .and_then(Value::as_f64)
        .filter(|size| *size > 1.0);
    props.disabled = raw.get("disabled").and_then(value_as_bool);
    props.password = raw.get("password").and_then(value_as_bool);
    props.maxlength = raw
        .get("maxlength")
        .and_then(Value::as_f64)
        .filter(|n| *n >= 0.0)
        .map(|n| n as u32);
    props.focus = raw.get("focus").and_then(value_as_bool);
    props.corner_radius = corner_radius_value(raw.get("cornerRadius"));
    props.src = raw.get("src").and_then(Value::as_str).map(str::to_string);
    props.autoplay = raw.get("autoplay").and_then(value_as_bool);
    props.looping = raw.get("loop").and_then(value_as_bool);
    props.muted = raw.get("muted").and_then(value_as_bool);
    props.volume = raw
        .get("volume")
        .and_then(Value::as_f64)
        .filter(|volume| volume.is_finite());
    props.controls = raw.get("controls").and_then(value_as_bool);
    props.progress_bar = raw.get("progressBar").and_then(value_as_bool);
    props.qualities = raw.get("qualities").and_then(Value::as_array).map(|items| {
        items
            .iter()
            .filter_map(|item| {
                let label = item.get("label").and_then(Value::as_str)?.to_string();
                let url = item.get("url").and_then(Value::as_str).map(str::to_string);
                Some((label, url))
            })
            .collect()
    });
    props.playback_rates = raw
        .get("playbackRates")
        .and_then(Value::as_array)
        .map(|rates| rates.iter().filter_map(Value::as_f64).collect());
    props.bindings_json = raw
        .get("pageFuncBindingsJson")
        .and_then(Value::as_str)
        .filter(|json| !json.is_empty() && *json != "{}")
        .map(str::to_string);
    props.dataset_json = raw
        .get("datasetJson")
        .and_then(Value::as_str)
        .filter(|json| !json.is_empty())
        .map(str::to_string);
    props
}

/// Parses a CSS color (`#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb()/rgba()`)
/// into a `COLORREF` value (0x00BBGGRR). Returns `None` for anything else.
fn parse_css_color(raw: &str) -> Option<u32> {
    let value = raw.trim();
    if let Some(hex) = value.strip_prefix('#') {
        let rgb = match hex.len() {
            3 => {
                let expanded: String = hex.chars().flat_map(|ch| [ch, ch]).collect();
                u32::from_str_radix(&expanded, 16).ok()?
            }
            6 => u32::from_str_radix(hex, 16).ok()?,
            8 => u32::from_str_radix(&hex[..6], 16).ok()?,
            _ => return None,
        };
        let (r, g, b) = ((rgb >> 16) & 0xff, (rgb >> 8) & 0xff, rgb & 0xff);
        return Some((b << 16) | (g << 8) | r);
    }

    let inner = value
        .strip_prefix("rgba(")
        .or_else(|| value.strip_prefix("rgb("))?
        .strip_suffix(')')?;
    let mut parts = inner.split(',').map(str::trim);
    let r: u32 = parts.next()?.parse().ok()?;
    let g: u32 = parts.next()?.parse().ok()?;
    let b: u32 = parts.next()?.parse().ok()?;
    Some(((b & 0xff) << 16) | ((g & 0xff) << 8) | (r & 0xff))
}

pub(super) fn to_wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// EDIT controls use CRLF line endings; the page view uses LF.
pub(super) fn to_edit_text(value: &str) -> String {
    value.replace("\r\n", "\n").replace('\n', "\r\n")
}

pub(super) fn from_edit_text(value: &str) -> String {
    value.replace("\r\n", "\n")
}
