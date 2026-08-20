//! Shared island paint, hit-test, and slider math.
//!
//! Platforms and unit tests call these functions so Cover scrim, tappable
//! content, slider latch, and pointer routing are not reimplemented inside
//! a DComp / UIView callback.

use super::types::Rect;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerEventsMode {
    Auto,
    None,
    BoxOnly,
    BoxNone,
}

impl PointerEventsMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::None => "none",
            Self::BoxOnly => "box-only",
            Self::BoxNone => "box-none",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "auto" => Some(Self::Auto),
            "none" => Some(Self::None),
            "box-only" => Some(Self::BoxOnly),
            "box-none" => Some(Self::BoxNone),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScrimPaint {
    pub scrim: String,
    pub opacity: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TappableContent {
    pub text: Option<String>,
    pub icon_name: Option<String>,
    pub disabled: bool,
    pub loading: bool,
    pub pressed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SliderPaint {
    pub min: f64,
    pub max: f64,
    pub value: f64,
    pub step: f64,
    pub value_label: String,
    pub disabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IslandVisualPlan {
    pub kind: String,
    pub rect: Rect,
    pub texture_width: i32,
    pub texture_height: i32,
    pub dest_width: f32,
    pub dest_height: f32,
    pub color: u32,
    pub text: Option<String>,
    pub pointer_events: PointerEventsMode,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IslandHitTarget {
    pub id: String,
    pub kind: String,
    pub rect: Rect,
    pub pointer_events: PointerEventsMode,
    pub visible: bool,
    pub props: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IslandHit {
    Miss,
    Swallow { id: String, kind: String },
    Tappable { id: String },
    Slider { id: String, value: f64 },
    Video { id: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IslandPointerPhase {
    Down,
    Move,
    Up,
    Cancel,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IslandHostEvent {
    pub id: String,
    pub event: String,
    pub detail: Value,
}

#[derive(Debug, Default, Clone)]
pub struct IslandPointerTracker {
    down: Option<PointerDown>,
}

impl IslandPointerTracker {
    pub fn is_active(&self) -> bool {
        self.down.is_some()
    }

    pub fn cancel(&mut self) {
        self.down = None;
    }

    /// Slider thumb value latched for the current drag, if any.
    pub fn latched_slider(&self) -> Option<(String, f64)> {
        let down = self.down.as_ref()?;
        if down.kind != PointerKind::Slider {
            return None;
        }
        Some((down.id.clone(), down.latched?))
    }
}

/// Overlay a locally latched slider value onto committed props so paint does
/// not wait on a Logic `root.commit` to move the thumb.
pub fn props_with_slider_value(props: &Value, value: f64) -> Value {
    let mut next = props.clone();
    match &mut next {
        Value::Object(map) => {
            map.insert("value".to_string(), serde_json::json!(value));
        }
        _ => next = serde_json::json!({ "value": value }),
    }
    next
}

#[derive(Debug, Clone)]
struct PointerDown {
    id: String,
    kind: PointerKind,
    slider: Option<SliderPaint>,
    rect: Rect,
    latched: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PointerKind {
    Tappable,
    Slider,
    Video,
    Swallow,
}

pub fn cover_scrim_from_props(props: &Value) -> Option<ScrimPaint> {
    let scrim = props.get("scrimPaint")?;
    let name = scrim
        .get("scrim")
        .and_then(Value::as_str)
        .unwrap_or("none")
        .to_string();
    let opacity = number_field(scrim, "opacity").unwrap_or(0.6);
    Some(ScrimPaint {
        scrim: name,
        opacity: opacity.clamp(0.0, 1.0),
    })
}

pub fn tappable_content_from_props(props: &Value) -> TappableContent {
    let content = props.get("content");
    let text = content
        .and_then(|value| value.get("text"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            props
                .get("label")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .filter(|text| !text.is_empty());
    let icon_name = content
        .and_then(|value| value.get("icon"))
        .and_then(|icon| icon.get("name"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            props
                .get("icon")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .filter(|name| !name.is_empty());
    TappableContent {
        text,
        icon_name,
        disabled: bool_field(props, "disabled"),
        loading: bool_field(props, "loading"),
        pressed: bool_field(props, "pressed"),
    }
}

pub fn slider_paint_from_props(props: &Value) -> SliderPaint {
    let min = number_field(props, "min").unwrap_or(0.0);
    let mut max = number_field(props, "max").unwrap_or(100.0);
    if max < min {
        max = min;
    }
    let value = number_field(props, "value").unwrap_or(min).clamp(min, max);
    SliderPaint {
        min,
        max,
        value,
        step: number_field(props, "step").unwrap_or(0.0).max(0.0),
        value_label: props
            .get("valueLabel")
            .and_then(Value::as_str)
            .unwrap_or("none")
            .to_string(),
        disabled: bool_field(props, "disabled"),
    }
}

pub fn pointer_events_from_props(kind: &str, props: &Value) -> PointerEventsMode {
    if let Some(mode) = props
        .get("pointerEvents")
        .and_then(Value::as_str)
        .and_then(PointerEventsMode::parse)
    {
        return mode;
    }
    match kind {
        "text" => PointerEventsMode::None,
        "view" if cover_scrim_from_props(props).is_some() => PointerEventsMode::BoxNone,
        _ => PointerEventsMode::Auto,
    }
}

pub fn slider_value_from_x(slider: &SliderPaint, x: f64, width: f64) -> f64 {
    let span = (slider.max - slider.min).max(0.0);
    if width <= 0.0 || span == 0.0 {
        return slider.min;
    }
    let t = (x / width).clamp(0.0, 1.0);
    snap_slider_value(slider, slider.min + t * span)
}

pub fn format_value_label(slider: &SliderPaint) -> Option<String> {
    match slider.value_label.as_str() {
        "value" => Some(format_numeric_value(slider)),
        "time" => Some(format_time_value(slider.value)),
        _ => None,
    }
}

pub fn plan_island_visual(kind: &str, rect: &Rect, props: &Value) -> IslandVisualPlan {
    let dest_width = rect.width.max(1.0) as f32;
    let dest_height = rect.height.max(1.0) as f32;
    let texture_width = dest_width.round().clamp(1.0, 640.0) as i32;
    let texture_height = dest_height.round().clamp(1.0, 360.0) as i32;
    let content = tappable_content_from_props(props);
    let slider = slider_paint_from_props(props);
    let text = match kind {
        "text" => props
            .get("text")
            .and_then(Value::as_str)
            .map(str::to_string)
            .filter(|value| !value.is_empty()),
        "tappable" => {
            if content.loading {
                Some("...".into())
            } else {
                content.text.clone().or_else(|| content.icon_name.clone())
            }
        }
        "slider" => format_value_label(&slider),
        _ => None,
    };
    IslandVisualPlan {
        kind: kind.to_string(),
        rect: rect.clone(),
        texture_width,
        texture_height,
        dest_width,
        dest_height,
        color: fill_color(kind, props, &content),
        text,
        pointer_events: pointer_events_from_props(kind, props),
    }
}

pub fn rasterize_island_kind(kind: &str, width: i32, height: i32, props: &Value) -> Vec<u32> {
    let width = width.max(1);
    let height = height.max(1);
    let plan = plan_island_visual(
        kind,
        &Rect {
            x: 0.0,
            y: 0.0,
            width: width as f64,
            height: height as f64,
        },
        props,
    );
    let mut pixels = vec![premultiply(plan.color); (width * height) as usize];
    match kind {
        "view" => paint_scrim(&mut pixels, width, height, props),
        "slider" => paint_slider(&mut pixels, width, height, &slider_paint_from_props(props)),
        "tappable" => {
            if let Some(text) = plan.text.as_deref() {
                blit_text_5x7(&mut pixels, width, height, text, 0xffff_ffff);
            }
        }
        "text" => {
            if let Some(text) = plan.text.as_deref() {
                blit_text_5x7(&mut pixels, width, height, text, 0xffff_ffff);
            }
        }
        _ => {}
    }
    pixels
}

pub fn hit_test_island(targets: &[IslandHitTarget], x: f64, y: f64) -> IslandHit {
    for target in targets.iter().rev() {
        if !target.visible || !contains(&target.rect, x, y) {
            continue;
        }
        match target.pointer_events {
            PointerEventsMode::None | PointerEventsMode::BoxNone => continue,
            PointerEventsMode::Auto | PointerEventsMode::BoxOnly => {
                return hit_for_target(target, x);
            }
        }
    }
    IslandHit::Miss
}

pub fn dispatch_pointer(
    tracker: &mut IslandPointerTracker,
    targets: &[IslandHitTarget],
    phase: IslandPointerPhase,
    x: f64,
    y: f64,
) -> Vec<IslandHostEvent> {
    match phase {
        IslandPointerPhase::Down => pointer_down(tracker, targets, x, y),
        IslandPointerPhase::Move => pointer_move(tracker, x),
        IslandPointerPhase::Up => pointer_up(tracker, targets, x, y),
        IslandPointerPhase::Cancel => {
            tracker.cancel();
            Vec::new()
        }
    }
}

fn pointer_down(
    tracker: &mut IslandPointerTracker,
    targets: &[IslandHitTarget],
    x: f64,
    y: f64,
) -> Vec<IslandHostEvent> {
    tracker.cancel();
    match hit_test_island(targets, x, y) {
        IslandHit::Miss => Vec::new(),
        IslandHit::Swallow { id, .. } => {
            tracker.down = Some(PointerDown {
                id,
                kind: PointerKind::Swallow,
                slider: None,
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 0.0,
                    height: 0.0,
                },
                latched: None,
            });
            Vec::new()
        }
        IslandHit::Tappable { id } => {
            tracker.down = Some(PointerDown {
                id,
                kind: PointerKind::Tappable,
                slider: None,
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 0.0,
                    height: 0.0,
                },
                latched: None,
            });
            Vec::new()
        }
        IslandHit::Video { id } => {
            tracker.down = Some(PointerDown {
                id,
                kind: PointerKind::Video,
                slider: None,
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 0.0,
                    height: 0.0,
                },
                latched: None,
            });
            Vec::new()
        }
        IslandHit::Slider { id, value } => {
            let target = targets.iter().rev().find(|item| item.id == id);
            let slider = target
                .map(|item| slider_paint_from_props(&item.props))
                .unwrap_or(SliderPaint {
                    min: 0.0,
                    max: 100.0,
                    value,
                    step: 0.0,
                    value_label: "none".into(),
                    disabled: false,
                });
            let rect = target.map(|item| item.rect.clone()).unwrap_or(Rect {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            });
            tracker.down = Some(PointerDown {
                id: id.clone(),
                kind: PointerKind::Slider,
                slider: Some(slider),
                rect,
                latched: Some(value),
            });
            vec![value_event(&id, "valuechange", value)]
        }
    }
}

fn pointer_move(tracker: &mut IslandPointerTracker, x: f64) -> Vec<IslandHostEvent> {
    let Some(down) = tracker.down.as_mut() else {
        return Vec::new();
    };
    if down.kind != PointerKind::Slider {
        return Vec::new();
    }
    let Some(slider) = down.slider.clone() else {
        return Vec::new();
    };
    let value = slider_value_from_x(&slider, x - down.rect.x, down.rect.width);
    if down.latched == Some(value) {
        return Vec::new();
    }
    down.latched = Some(value);
    vec![value_event(&down.id, "valuechange", value)]
}

fn pointer_up(
    tracker: &mut IslandPointerTracker,
    targets: &[IslandHitTarget],
    x: f64,
    y: f64,
) -> Vec<IslandHostEvent> {
    let Some(down) = tracker.down.take() else {
        return Vec::new();
    };
    match down.kind {
        PointerKind::Slider => {
            let value = down.latched.unwrap_or_else(|| {
                down.slider
                    .as_ref()
                    .map(|slider| slider_value_from_x(slider, x - down.rect.x, down.rect.width))
                    .unwrap_or(0.0)
            });
            vec![value_event(&down.id, "valuecommit", value)]
        }
        PointerKind::Tappable | PointerKind::Video => match hit_test_island(targets, x, y) {
            IslandHit::Tappable { id } | IslandHit::Video { id } if id == down.id => {
                vec![press_event(&id)]
            }
            _ => Vec::new(),
        },
        PointerKind::Swallow => Vec::new(),
    }
}

fn hit_for_target(target: &IslandHitTarget, x: f64) -> IslandHit {
    match target.kind.as_str() {
        "tappable" if !tappable_content_from_props(&target.props).disabled => IslandHit::Tappable {
            id: target.id.clone(),
        },
        "slider" => {
            let slider = slider_paint_from_props(&target.props);
            if slider.disabled {
                IslandHit::Swallow {
                    id: target.id.clone(),
                    kind: target.kind.clone(),
                }
            } else {
                IslandHit::Slider {
                    id: target.id.clone(),
                    value: slider_value_from_x(&slider, x - target.rect.x, target.rect.width),
                }
            }
        }
        "video" => IslandHit::Video {
            id: target.id.clone(),
        },
        _ => IslandHit::Swallow {
            id: target.id.clone(),
            kind: target.kind.clone(),
        },
    }
}

fn fill_color(kind: &str, props: &Value, content: &TappableContent) -> u32 {
    match kind {
        "video" => 0xff10_1010,
        "tappable" => {
            if content.disabled {
                0x6633_3333
            } else if content.pressed {
                0xcc3a_3a3a
            } else {
                0xcc2a_2a2a
            }
        }
        "slider" => 0xff22_2222,
        "text" => 0x0000_0000,
        "view" => match cover_scrim_from_props(props) {
            Some(scrim) if scrim.scrim != "none" => {
                let alpha = (scrim.opacity * 255.0).round().clamp(0.0, 255.0) as u32;
                (alpha << 24) | 0x00_0000
            }
            _ => 0x0000_0000,
        },
        _ => 0x0000_0000,
    }
}

fn paint_scrim(pixels: &mut [u32], width: i32, height: i32, props: &Value) {
    let Some(scrim) = cover_scrim_from_props(props) else {
        return;
    };
    if scrim.scrim == "none" || height <= 0 {
        return;
    }
    for y in 0..height {
        let t = match scrim.scrim.as_str() {
            "top" => 1.0 - (y as f64 / (height - 1).max(1) as f64),
            "bottom" => y as f64 / (height - 1).max(1) as f64,
            "full" => 1.0,
            _ => 0.0,
        };
        let alpha = (scrim.opacity * t * 255.0).round().clamp(0.0, 255.0) as u32;
        let color = premultiply(alpha << 24);
        let start = (y * width) as usize;
        let end = start + width as usize;
        if let Some(row) = pixels.get_mut(start..end) {
            row.fill(color);
        }
    }
}

fn paint_slider(pixels: &mut [u32], width: i32, height: i32, slider: &SliderPaint) {
    let track_y = (height / 2).clamp(0, height.saturating_sub(1));
    let track = premultiply(0xff55_5555);
    let fill = premultiply(0xff3b_82f6);
    let thumb = premultiply(0xffff_ffff);
    let t = if slider.max > slider.min {
        ((slider.value - slider.min) / (slider.max - slider.min)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let thumb_x = ((width - 1) as f64 * t).round() as i32;
    for x in 0..width {
        let idx = (track_y * width + x) as usize;
        if idx < pixels.len() {
            pixels[idx] = if x <= thumb_x { fill } else { track };
        }
    }
    for dy in -3..=3 {
        for dx in -3..=3 {
            if dx * dx + dy * dy > 9 {
                continue;
            }
            let x = thumb_x + dx;
            let y = track_y + dy;
            if x < 0 || y < 0 || x >= width || y >= height {
                continue;
            }
            pixels[(y * width + x) as usize] = thumb;
        }
    }
    if let Some(label) = format_value_label(slider) {
        blit_text_5x7(pixels, width, height, &label, 0xffff_ffff);
    }
}

fn blit_text_5x7(pixels: &mut [u32], width: i32, height: i32, text: &str, color: u32) {
    const GLYPH_W: i32 = 5;
    const GAP: i32 = 1;
    let mut cursor_x = 2;
    let cursor_y = 2;
    for ch in text.chars() {
        if cursor_x + GLYPH_W > width {
            break;
        }
        if let Some(rows) = glyph_5x7(ch) {
            for (row, bits) in rows.iter().enumerate() {
                let y = cursor_y + row as i32;
                if y < 0 || y >= height {
                    continue;
                }
                for col in 0..GLYPH_W {
                    if bits & (1 << (4 - col)) == 0 {
                        continue;
                    }
                    let x = cursor_x + col;
                    if x < 0 || x >= width {
                        continue;
                    }
                    pixels[(y * width + x) as usize] = color;
                }
            }
        }
        cursor_x += GLYPH_W + GAP;
    }
}

fn glyph_5x7(ch: char) -> Option<[u8; 7]> {
    Some(match ch {
        ' ' => [0, 0, 0, 0, 0, 0, 0],
        '.' => [0, 0, 0, 0, 0, 0, 0x04],
        ':' => [0, 0x04, 0, 0, 0x04, 0, 0],
        '0' => [0x0e, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0e],
        '1' => [0x04, 0x0c, 0x04, 0x04, 0x04, 0x04, 0x0e],
        '2' => [0x0e, 0x11, 0x01, 0x06, 0x08, 0x10, 0x1f],
        '3' => [0x0e, 0x11, 0x01, 0x06, 0x01, 0x11, 0x0e],
        '4' => [0x02, 0x06, 0x0a, 0x12, 0x1f, 0x02, 0x02],
        '5' => [0x1f, 0x10, 0x1e, 0x01, 0x01, 0x11, 0x0e],
        '6' => [0x06, 0x08, 0x10, 0x1e, 0x11, 0x11, 0x0e],
        '7' => [0x1f, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0e, 0x11, 0x11, 0x0e, 0x11, 0x11, 0x0e],
        '9' => [0x0e, 0x11, 0x11, 0x0f, 0x01, 0x02, 0x0c],
        'I' => [0x0e, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0e],
        'P' => [0x1e, 0x11, 0x11, 0x1e, 0x10, 0x10, 0x10],
        'a' => [0x00, 0x00, 0x0e, 0x01, 0x0f, 0x11, 0x0f],
        'e' => [0x00, 0x00, 0x0e, 0x11, 0x1f, 0x10, 0x0e],
        'i' => [0x04, 0x00, 0x0c, 0x04, 0x04, 0x04, 0x0e],
        'l' => [0x0c, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0e],
        'n' => [0x00, 0x00, 0x1e, 0x11, 0x11, 0x11, 0x11],
        't' => [0x00, 0x04, 0x1f, 0x04, 0x04, 0x04, 0x03],
        'v' => [0x00, 0x00, 0x11, 0x11, 0x11, 0x0a, 0x04],
        'y' => [0x00, 0x00, 0x11, 0x11, 0x0f, 0x01, 0x0e],
        _ => return None,
    })
}

fn snap_slider_value(slider: &SliderPaint, raw: f64) -> f64 {
    let snapped = if slider.step > 0.0 {
        let steps = ((raw - slider.min) / slider.step).round();
        slider.min + steps * slider.step
    } else {
        raw
    };
    snapped.clamp(slider.min, slider.max)
}

fn format_numeric_value(slider: &SliderPaint) -> String {
    if slider.step >= 1.0 {
        format!("{}", slider.value.round() as i64)
    } else {
        format!("{:.1}", slider.value)
    }
}

fn format_time_value(seconds: f64) -> String {
    let total = seconds.max(0.0).round() as u64;
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let secs = total % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{secs:02}")
    } else {
        format!("{minutes}:{secs:02}")
    }
}

fn contains(rect: &Rect, x: f64, y: f64) -> bool {
    x >= rect.x && y >= rect.y && x < rect.x + rect.width && y < rect.y + rect.height
}

fn press_event(id: &str) -> IslandHostEvent {
    IslandHostEvent {
        id: id.to_string(),
        event: "press".into(),
        detail: serde_json::json!({ "source": "pointer" }),
    }
}

fn value_event(id: &str, event: &str, value: f64) -> IslandHostEvent {
    IslandHostEvent {
        id: id.to_string(),
        event: event.into(),
        detail: serde_json::json!({ "value": value }),
    }
}

fn premultiply(color: u32) -> u32 {
    let alpha = (color >> 24) & 0xff;
    let red = (color >> 16) & 0xff;
    let green = (color >> 8) & 0xff;
    let blue = color & 0xff;
    let mul = |channel: u32| (channel * alpha + 127) / 255;
    (alpha << 24) | (mul(red) << 16) | (mul(green) << 8) | mul(blue)
}

fn number_field(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(|raw| {
        raw.as_f64()
            .or_else(|| raw.as_i64().map(|n| n as f64))
            .or_else(|| raw.as_u64().map(|n| n as f64))
            .or_else(|| raw.as_str().and_then(|s| s.parse().ok()))
    })
}

fn bool_field(value: &Value, key: &str) -> bool {
    match value.get(key) {
        Some(Value::Bool(flag)) => *flag,
        Some(Value::String(text)) => text == "true" || text == "1",
        Some(Value::Number(number)) => number.as_f64().is_some_and(|n| n != 0.0),
        _ => false,
    }
}
