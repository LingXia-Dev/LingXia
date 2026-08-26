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
    pub buffered_value: f64,
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
        buffered_value: number_field(props, "bufferedValue")
            .unwrap_or(value)
            .clamp(min, max),
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
                tappable_display_text(&content, props)
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
    let mut pixels = vec![0; (width * height) as usize];
    match kind {
        "view" => {
            paint_rounded_background(
                &mut pixels,
                width,
                height,
                base_fill_color(kind, props, &tappable_content_from_props(props)),
                props,
            );
            paint_scrim(&mut pixels, width, height, props);
        }
        "slider" => paint_slider(
            &mut pixels,
            width,
            height,
            &slider_paint_from_props(props),
            props,
        ),
        "tappable" => {
            paint_rounded_background(
                &mut pixels,
                width,
                height,
                base_fill_color(kind, props, &tappable_content_from_props(props)),
                props,
            );
            if let Some(text) = plan.text.as_deref() {
                blit_text(
                    &mut pixels,
                    width,
                    height,
                    text,
                    text_color(kind, props),
                    text_scale(props, kind),
                    TextAlign::Center,
                );
            }
        }
        "text" => {
            if let Some(text) = plan.text.as_deref() {
                blit_text(
                    &mut pixels,
                    width,
                    height,
                    text,
                    text_color(kind, props),
                    text_scale(props, kind),
                    text_align(props),
                );
            }
        }
        _ => {}
    }
    apply_raster_opacity(&mut pixels, style_number(props, "opacity").unwrap_or(1.0));
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
                    buffered_value: value,
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
    apply_color_opacity(
        base_fill_color(kind, props, content),
        style_number(props, "opacity").unwrap_or(1.0),
    )
}

fn base_fill_color(kind: &str, props: &Value, content: &TappableContent) -> u32 {
    if let Some(color) = style_color(props, "backgroundColor") {
        return color;
    }
    match kind {
        "video" => 0xff10_1010,
        "tappable" => {
            if content.disabled {
                return 0xff9c_a3af;
            }
            let intent = string_field(props, "intent").unwrap_or("neutral");
            let emphasis = string_field(props, "emphasis").unwrap_or("secondary");
            let base = match intent {
                "accent" => 0xff25_63eb,
                "destructive" => 0xffdc_2626,
                _ => 0xff37_4151,
            };
            match emphasis {
                "quiet" => 0,
                "secondary" => with_alpha(base, if content.pressed { 112 } else { 80 }),
                _ if content.pressed => darken(base, 0.82),
                _ => base,
            }
        }
        "slider" => 0x0000_0000,
        "text" => 0x0000_0000,
        "view" => match cover_scrim_from_props(props) {
            Some(scrim) if scrim.scrim != "none" => {
                let alpha = (scrim.opacity * 255.0).round().clamp(0.0, 255.0) as u32;
                alpha << 24
            }
            _ => 0x0000_0000,
        },
        _ => 0x0000_0000,
    }
}

fn paint_rounded_background(
    pixels: &mut [u32],
    width: i32,
    height: i32,
    color: u32,
    props: &Value,
) {
    if width <= 0 || height <= 0 {
        return;
    }
    let radius = style_number(props, "borderRadius")
        .unwrap_or(0.0)
        .round()
        .clamp(0.0, (width.min(height) / 2) as f64) as i32;
    let border_width = style_number(props, "borderWidth")
        .unwrap_or(0.0)
        .round()
        .clamp(0.0, width.min(height) as f64 / 2.0) as i32;
    let border = style_color(props, "borderColor").unwrap_or(color);
    if color >> 24 == 0 && (border_width == 0 || border >> 24 == 0) {
        return;
    }
    for y in 0..height {
        for x in 0..width {
            if !inside_rounded_rect(x, y, width, height, radius) {
                continue;
            }
            let is_border = border_width > 0
                && !inside_rounded_rect(
                    x - border_width,
                    y - border_width,
                    width - border_width * 2,
                    height - border_width * 2,
                    (radius - border_width).max(0),
                );
            pixels[(y * width + x) as usize] = premultiply(if is_border { border } else { color });
        }
    }
}

fn inside_rounded_rect(x: i32, y: i32, width: i32, height: i32, radius: i32) -> bool {
    if width <= 0 || height <= 0 || x < 0 || y < 0 || x >= width || y >= height {
        return false;
    }
    if radius <= 0 || (x >= radius && x < width - radius) || (y >= radius && y < height - radius) {
        return true;
    }
    let center_x = if x < radius {
        radius
    } else {
        width - radius - 1
    };
    let center_y = if y < radius {
        radius
    } else {
        height - radius - 1
    };
    let dx = x - center_x;
    let dy = y - center_y;
    dx * dx + dy * dy <= radius * radius
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

fn paint_slider(pixels: &mut [u32], width: i32, height: i32, slider: &SliderPaint, props: &Value) {
    let track_y = (height / 2).clamp(0, height.saturating_sub(1));
    let accent = style_color(props, "accentColor").unwrap_or(0xff3b_82f6);
    let track = premultiply(style_color(props, "backgroundColor").unwrap_or(0x665f_6368));
    let buffered = premultiply(with_alpha(accent, 112));
    let fill = premultiply(if slider.disabled { 0xff9c_a3af } else { accent });
    let thumb = premultiply(text_color("slider", props));
    let t = if slider.max > slider.min {
        ((slider.value - slider.min) / (slider.max - slider.min)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let buffered_t = if slider.max > slider.min {
        ((slider.buffered_value - slider.min) / (slider.max - slider.min)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let thumb_x = ((width - 1) as f64 * t).round() as i32;
    let buffered_x = ((width - 1) as f64 * buffered_t).round() as i32;
    let half_track = (height / 12).clamp(1, 3);
    for y in (track_y - half_track).max(0)..=(track_y + half_track).min(height - 1) {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            if idx < pixels.len() {
                pixels[idx] = if x <= thumb_x {
                    fill
                } else if x <= buffered_x {
                    buffered
                } else {
                    track
                };
            }
        }
    }
    let thumb_radius = (height / 4).clamp(3, 7);
    for dy in -thumb_radius..=thumb_radius {
        for dx in -thumb_radius..=thumb_radius {
            if dx * dx + dy * dy > thumb_radius * thumb_radius {
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
        blit_text(
            pixels,
            width,
            height,
            &label,
            text_color("slider", props),
            1,
            TextAlign::End,
        );
    }
}

#[derive(Debug, Clone, Copy)]
enum TextAlign {
    Start,
    Center,
    End,
}

fn blit_text(
    pixels: &mut [u32],
    width: i32,
    height: i32,
    text: &str,
    color: u32,
    scale: i32,
    align: TextAlign,
) {
    const GLYPH_W: i32 = 5;
    const GAP: i32 = 1;
    let scale = scale.clamp(1, 4);
    let glyph_advance = (GLYPH_W + GAP) * scale;
    let text_width = text.chars().count() as i32 * glyph_advance - GAP * scale;
    let mut cursor_x = match align {
        TextAlign::Start => 2 * scale,
        TextAlign::Center => ((width - text_width) / 2).max(0),
        TextAlign::End => (width - text_width - 2 * scale).max(0),
    };
    let cursor_y = ((height - 7 * scale) / 2).max(0);
    let color = premultiply(color);
    for ch in text.chars() {
        if cursor_x + GLYPH_W * scale > width {
            break;
        }
        if let Some(rows) = glyph_5x7(ch) {
            for (row, bits) in rows.iter().enumerate() {
                for col in 0..GLYPH_W {
                    if bits & (1 << (4 - col)) == 0 {
                        continue;
                    }
                    for sy in 0..scale {
                        let y = cursor_y + row as i32 * scale + sy;
                        if y < 0 || y >= height {
                            continue;
                        }
                        for sx in 0..scale {
                            let x = cursor_x + col * scale + sx;
                            if x >= 0 && x < width {
                                pixels[(y * width + x) as usize] = color;
                            }
                        }
                    }
                }
            }
        }
        cursor_x += glyph_advance;
    }
}

fn glyph_5x7(ch: char) -> Option<[u8; 7]> {
    Some(match ch.to_ascii_uppercase() {
        ' ' => [0, 0, 0, 0, 0, 0, 0],
        '.' => [0, 0, 0, 0, 0, 0, 0x04],
        ':' => [0, 0x04, 0, 0, 0x04, 0, 0],
        '-' => [0, 0, 0, 0x1f, 0, 0, 0],
        '+' => [0, 0x04, 0x04, 0x1f, 0x04, 0x04, 0],
        '/' => [0x01, 0x02, 0x02, 0x04, 0x08, 0x08, 0x10],
        '>' => [0x10, 0x08, 0x04, 0x02, 0x04, 0x08, 0x10],
        '<' => [0x01, 0x02, 0x04, 0x08, 0x04, 0x02, 0x01],
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
        'A' => [0x0e, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        'B' => [0x1e, 0x11, 0x11, 0x1e, 0x11, 0x11, 0x1e],
        'C' => [0x0e, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0e],
        'D' => [0x1e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1e],
        'E' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x1f],
        'F' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x10],
        'G' => [0x0e, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0f],
        'H' => [0x11, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        'I' => [0x0e, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0e],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x12, 0x12, 0x0c],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1f],
        'M' => [0x11, 0x1b, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        'O' => [0x0e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        'P' => [0x1e, 0x11, 0x11, 0x1e, 0x10, 0x10, 0x10],
        'Q' => [0x0e, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0d],
        'R' => [0x1e, 0x11, 0x11, 0x1e, 0x14, 0x12, 0x11],
        'S' => [0x0f, 0x10, 0x10, 0x0e, 0x01, 0x01, 0x1e],
        'T' => [0x1f, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0a, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0a],
        'X' => [0x11, 0x11, 0x0a, 0x04, 0x0a, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x0a, 0x04, 0x04, 0x04, 0x04],
        'Z' => [0x1f, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1f],
        _ => return None,
    })
}

fn tappable_display_text(content: &TappableContent, _props: &Value) -> Option<String> {
    let icon = content.icon_name.as_deref().and_then(semantic_icon);
    content.text.clone().or_else(|| icon.map(str::to_string))
}

fn semantic_icon(name: &str) -> Option<&'static str> {
    match name {
        "close" => Some("X"),
        "play" => Some(">"),
        "pause" => Some("II"),
        "mute" => Some("MUTE"),
        "unmute" => Some("SOUND"),
        "fullscreen" => Some("FULL"),
        "more" => Some("..."),
        _ => None,
    }
}

fn text_scale(props: &Value, kind: &str) -> i32 {
    let default = if kind == "tappable" && string_field(props, "size") == Some("compact") {
        12.0
    } else {
        14.0
    };
    (number_field(props, "fontSize")
        .or_else(|| style_number(props, "fontSize"))
        .unwrap_or(default)
        / 7.0)
        .round()
        .clamp(1.0, 4.0) as i32
}

fn text_align(props: &Value) -> TextAlign {
    match string_field(props, "textAlign").or_else(|| style_string(props, "textAlign")) {
        Some("center") => TextAlign::Center,
        Some("right" | "end") => TextAlign::End,
        _ => TextAlign::Start,
    }
}

fn text_color(kind: &str, props: &Value) -> u32 {
    if let Some(color) = color_field(props, "color").or_else(|| style_color(props, "color")) {
        return color;
    }
    if kind == "tappable" && string_field(props, "emphasis") == Some("quiet") {
        return match string_field(props, "intent").unwrap_or("neutral") {
            "accent" => 0xff25_63eb,
            "destructive" => 0xffdc_2626,
            _ => 0xff11_1827,
        };
    }
    0xffff_ffff
}

fn style_value<'a>(props: &'a Value, key: &str) -> Option<&'a Value> {
    props.get("nativeStyle")?.get(key)
}

fn style_string<'a>(props: &'a Value, key: &str) -> Option<&'a str> {
    style_value(props, key).and_then(Value::as_str)
}

fn style_number(props: &Value, key: &str) -> Option<f64> {
    let value = style_value(props, key)?;
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(parse_css_number))
}

fn parse_css_number(value: &str) -> Option<f64> {
    let numeric: String = value
        .trim()
        .chars()
        .take_while(|ch| ch.is_ascii_digit() || matches!(ch, '-' | '+' | '.'))
        .collect();
    numeric.parse().ok()
}

fn color_field(props: &Value, key: &str) -> Option<u32> {
    props.get(key).and_then(parse_css_color)
}

fn style_color(props: &Value, key: &str) -> Option<u32> {
    style_value(props, key).and_then(parse_css_color)
}

fn parse_css_color(value: &Value) -> Option<u32> {
    let raw = value.as_str()?.trim().to_ascii_lowercase();
    match raw.as_str() {
        "transparent" => return Some(0),
        "black" => return Some(0xff00_0000),
        "white" => return Some(0xffff_ffff),
        "red" => return Some(0xffff_0000),
        "green" => return Some(0xff00_8000),
        "blue" => return Some(0xff00_00ff),
        _ => {}
    }
    if let Some(hex) = raw.strip_prefix('#') {
        return match hex.len() {
            3 => {
                let r = u32::from_str_radix(&hex[0..1], 16).ok()? * 17;
                let g = u32::from_str_radix(&hex[1..2], 16).ok()? * 17;
                let b = u32::from_str_radix(&hex[2..3], 16).ok()? * 17;
                Some(0xff00_0000 | r << 16 | g << 8 | b)
            }
            4 => {
                let r = u32::from_str_radix(&hex[0..1], 16).ok()? * 17;
                let g = u32::from_str_radix(&hex[1..2], 16).ok()? * 17;
                let b = u32::from_str_radix(&hex[2..3], 16).ok()? * 17;
                let a = u32::from_str_radix(&hex[3..4], 16).ok()? * 17;
                Some(a << 24 | r << 16 | g << 8 | b)
            }
            6 => Some(0xff00_0000 | u32::from_str_radix(hex, 16).ok()?),
            8 => {
                let rgb = u32::from_str_radix(&hex[..6], 16).ok()?;
                let alpha = u32::from_str_radix(&hex[6..], 16).ok()?;
                Some(alpha << 24 | rgb)
            }
            _ => None,
        };
    }
    let fields = raw
        .strip_prefix("rgb(")
        .or_else(|| raw.strip_prefix("rgba("))?
        .strip_suffix(')')?
        .split(',')
        .map(str::trim)
        .collect::<Vec<_>>();
    if fields.len() < 3 {
        return None;
    }
    let channel = |value: &str| -> Option<u32> {
        if let Some(percent) = value.strip_suffix('%') {
            return Some(
                (percent.parse::<f64>().ok()? * 2.55)
                    .round()
                    .clamp(0.0, 255.0) as u32,
            );
        }
        Some(value.parse::<f64>().ok()?.round().clamp(0.0, 255.0) as u32)
    };
    let alpha = fields
        .get(3)
        .and_then(|value| value.parse::<f64>().ok())
        .map(|value| if value <= 1.0 { value * 255.0 } else { value })
        .unwrap_or(255.0)
        .round()
        .clamp(0.0, 255.0) as u32;
    Some(alpha << 24 | channel(fields[0])? << 16 | channel(fields[1])? << 8 | channel(fields[2])?)
}

fn with_alpha(color: u32, alpha: u32) -> u32 {
    alpha.min(255) << 24 | color & 0x00ff_ffff
}

fn darken(color: u32, factor: f64) -> u32 {
    let alpha = color >> 24;
    let scale = |shift: u32| (((color >> shift) & 0xff_u32) as f64 * factor).round() as u32;
    alpha << 24 | scale(16) << 16 | scale(8) << 8 | scale(0)
}

fn apply_color_opacity(color: u32, opacity: f64) -> u32 {
    let alpha = ((color >> 24) as f64 * opacity.clamp(0.0, 1.0)).round() as u32;
    with_alpha(color, alpha)
}

fn apply_raster_opacity(pixels: &mut [u32], opacity: f64) {
    let opacity = opacity.clamp(0.0, 1.0);
    if opacity >= 1.0 {
        return;
    }
    for pixel in pixels {
        let alpha = ((*pixel >> 24) as f64 * opacity).round() as u32;
        let red = ((*pixel >> 16) & 0xff) as f64 * opacity;
        let green = ((*pixel >> 8) & 0xff) as f64 * opacity;
        let blue = (*pixel & 0xff) as f64 * opacity;
        *pixel = alpha << 24
            | (red.round() as u32) << 16
            | (green.round() as u32) << 8
            | blue.round() as u32;
    }
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

fn string_field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn bool_field(value: &Value, key: &str) -> bool {
    match value.get(key) {
        Some(Value::Bool(flag)) => *flag,
        Some(Value::String(text)) => text == "true" || text == "1",
        Some(Value::Number(number)) => number.as_f64().is_some_and(|n| n != 0.0),
        _ => false,
    }
}

#[cfg(test)]
mod paint_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn computed_native_style_controls_color_opacity_and_radius() {
        let props = json!({
            "label": "Play",
            "intent": "accent",
            "emphasis": "primary",
            "nativeStyle": {
                "backgroundColor": "rgba(16, 32, 48, 0.8)",
                "color": "#fefefe",
                "opacity": "0.5",
                "borderRadius": "8px"
            }
        });
        let plan = plan_island_visual(
            "tappable",
            &Rect {
                x: 0.0,
                y: 0.0,
                width: 80.0,
                height: 32.0,
            },
            &props,
        );
        assert_eq!(plan.color, 0x6610_2030);
        let pixels = rasterize_island_kind("tappable", 80, 32, &props);
        assert_eq!(pixels[0], 0, "rounded corners stay transparent");
        assert!(pixels.iter().any(|pixel| *pixel != 0));
        assert!(pixels.iter().all(|pixel| pixel >> 24 <= 128));
    }

    #[test]
    fn slider_paints_committed_buffer_before_remaining_track() {
        let props = json!({
            "min": 0,
            "max": 100,
            "value": 20,
            "bufferedValue": 70,
            "valueLabel": "none",
            "nativeStyle": { "accentColor": "#ff0000" }
        });
        let slider = slider_paint_from_props(&props);
        assert_eq!(slider.buffered_value, 70.0);
        let pixels = rasterize_island_kind("slider", 101, 24, &props);
        let y = 12;
        assert_eq!(pixels[y * 101 + 10], premultiply(0xffff_0000));
        assert_eq!(pixels[y * 101 + 50], premultiply(0x70ff_0000));
        assert_eq!(pixels[y * 101 + 90], premultiply(0x665f_6368));
    }

    #[test]
    fn button_semantics_change_visual_treatment_and_icon_content() {
        let rect = Rect {
            x: 0.0,
            y: 0.0,
            width: 80.0,
            height: 32.0,
        };
        let primary = plan_island_visual(
            "tappable",
            &rect,
            &json!({
                "content": { "text": "Play", "icon": { "name": "play" } },
                "intent": "accent",
                "emphasis": "primary"
            }),
        );
        let quiet = plan_island_visual(
            "tappable",
            &rect,
            &json!({ "label": "Play", "intent": "accent", "emphasis": "quiet" }),
        );
        assert_eq!(primary.text.as_deref(), Some("Play"));
        assert_eq!(primary.color, 0xff25_63eb);
        assert_eq!(quiet.color, 0);
    }
}
