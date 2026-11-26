use crate::protocol::Record;
use crate::protocol::field;
use crate::scene::paint::Paint;
use serde::Serialize;

/// The default thumb's diameter.
pub const THUMB: f32 = 24.0;

#[derive(Clone, Debug, Serialize)]
pub struct SliderSnapshot {
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub value: f64,
    /// Track ends inside the control box: left and right padding plus the thumb's half width.
    pub inset: [f32; 2],
    pub enabled: bool,
    pub on_change: Option<u64>,
    /// A second value drawn behind the range (buffered media); clamped, never snapped.
    pub secondary: Option<f64>,
    /// `aria-valuetext`: how the value reads aloud, in place of the number.
    pub text: Option<String>,
    /// The values this thumb may take: the whole range, or up to the thumb it shares a track with.
    #[serde(skip)]
    pub limit: [f64; 2],
    /// On a shared track: the other thumb's value, and whether this is the upper thumb.
    #[serde(skip)]
    pub pair: Option<(f64, bool)>,
}

impl SliderSnapshot {
    pub(crate) fn from_record(record: &Record) -> Result<Self, String> {
        let number = |field, default| {
            record
                .fields
                .iter()
                .find(|(id, _)| *id == field)
                .and_then(|(_, v)| v.as_f64())
                .unwrap_or(default)
        };
        let min = number(43, 0.0);
        let max = number(44, 100.0);
        let step = number(45, 1.0);
        let value = number(42, min);
        if ![min, max, step, value]
            .iter()
            .all(|v| v.is_finite() && v.abs() <= 1e9)
            || max <= min
            || step <= 0.0
            || step > max - min
            || (max - min) / step > 1e6
            || min + step == min
            || max - step == max
        {
            return Err("UI_SLIDER: invalid range or precision".into());
        }
        if record
            .string(field::ACCESSIBILITY_LABEL)
            .is_none_or(|label| label.trim().is_empty())
        {
            return Err("UI_SLIDER: accessibilityLabel is required".into());
        }
        let enabled = record.number(field::ENABLED) != Some(0.0);
        let other = record
            .fields
            .iter()
            .find(|(id, _)| *id == field::SLIDER_OTHER)
            .and_then(|(_, v)| v.as_f64());
        let pair = match (other, record.string(field::SLIDER_THUMB)) {
            (Some(other), Some(thumb)) => Some((other.clamp(min, max), thumb == "upper")),
            (None, None) => None,
            _ => return Err("UI_SLIDER: a thumb of a pair needs the other thumb's value".into()),
        };
        let limit = match pair {
            Some((other, true)) => [other, max],
            Some((other, false)) => [min, other],
            None => [min, max],
        };
        let mut result = Self {
            min,
            max,
            step,
            value,
            inset: [THUMB / 2.0; 2],
            enabled,
            on_change: enabled
                .then(|| {
                    record
                        .fields
                        .iter()
                        .find(|(id, _)| *id == field::ON_SLIDER_CHANGE)
                        .and_then(|(_, v)| v.as_u64())
                })
                .flatten(),
            secondary: record
                .fields
                .iter()
                .find(|(id, _)| *id == field::SLIDER_SECONDARY)
                .and_then(|(_, v)| v.as_f64())
                .map(|v| v.clamp(min, max)),
            text: record.string(field::ARIA_VALUE_TEXT).map(str::to_owned),
            limit,
            pair,
        };
        result.value = result.normalize(value);
        Ok(result)
    }

    pub(crate) fn normalize(&self, value: f64) -> f64 {
        let value = value.clamp(self.min, self.max);
        let snapped = (self.min + ((value - self.min) / self.step + 0.5).floor() * self.step)
            .clamp(self.min, self.max);
        let snapped = if (self.max - value).abs() < (snapped - value).abs() {
            self.max
        } else {
            snapped
        };
        snapped.clamp(self.limit[0], self.limit[1])
    }
}

/// One slider part as authored (`thumb:`, `track:` …); unset values take the part's default.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Part {
    pub background: Option<String>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub radius: Option<f32>,
    pub border: f32,
    pub border_color: Option<String>,
    pub shadow: Option<String>,
    pub ring: Option<String>,
    pub scale: [f32; 2],
}

impl Default for Part {
    fn default() -> Self {
        Self {
            background: None,
            width: None,
            height: None,
            radius: None,
            border: 0.0,
            border_color: None,
            shadow: None,
            ring: None,
            scale: [1.0; 2],
        }
    }
}

impl Part {
    /// The part in `id` of the record as its current state draws it.
    pub(crate) fn of(record: &Record, id: usize) -> Result<Self, String> {
        match record.fields.iter().find(|(field, _)| *field == id) {
            Some((_, value)) => Self::parse(value),
            None => Ok(Self::default()),
        }
    }

    fn parse(value: &serde_json::Value) -> Result<Self, String> {
        let mut part = Self::default();
        let mut corners: Option<f32> = None;
        for (id, value) in crate::protocol::part_style(value)? {
            let number = value.as_f64().map(|v| v as f32);
            match id {
                field::BACKGROUND => part.background = value.as_str().map(str::to_owned),
                field::WIDTH => part.width = number,
                field::HEIGHT => part.height = number,
                field::RADIUS => part.radius = number,
                field::RADIUS_TOP_LEFT
                | field::RADIUS_TOP_RIGHT
                | field::RADIUS_BOTTOM_RIGHT
                | field::RADIUS_BOTTOM_LEFT => {
                    corners = Some(corners.unwrap_or(f32::MAX).min(number.unwrap_or(0.0)))
                }
                field::BORDER_COLOR => part.border_color = value.as_str().map(str::to_owned),
                field::BOX_SHADOW => part.shadow = value.as_str().map(str::to_owned),
                field::RING => part.ring = value.as_str().map(str::to_owned),
                field::TRANSFORM => {
                    let source = value.as_str().ok_or("UI_TRANSFORM: invalid transform")?;
                    let scale = crate::scene::transform::StaticTransform::parse(source)?
                        .decomposed([0.0; 2])
                        .scale;
                    part.scale = scale.map(|v| v as f32);
                }
                _ => part.border = part.border.max(number.unwrap_or(0.0)),
            }
        }
        part.radius = corners.or(part.radius);
        Ok(part)
    }
}

/// The thumb's widest state: the track ends stay put while it grows on press or focus.
pub(crate) fn widest_thumb(record: &Record) -> Result<f32, String> {
    let mut widest = Part::of(record, field::THUMB_STYLE)?.width.unwrap_or(THUMB);
    for state in [
        field::PRESSED_STYLE,
        field::HOVER_STYLE,
        field::FOCUS_VISIBLE_STYLE,
    ] {
        let Some((_, delta)) = record.fields.iter().find(|(id, _)| *id == state) else {
            continue;
        };
        for (id, value) in crate::protocol::focus_style(delta)? {
            if id == field::THUMB_STYLE {
                widest = widest.max(Part::parse(&value)?.width.unwrap_or(THUMB));
            }
        }
    }
    Ok(widest)
}

/// Where a slider's parts sit in the scene's commands: track, secondary, range, thumb, border.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct SliderPaint([Option<usize>; 5]);

/// The eased values of a slider's parts: line thicknesses, thumb size and radius, and colours.
#[derive(Clone, Debug, PartialEq)]
pub struct PartValues {
    /// Track, secondary and range thickness; thumb width, height and radius.
    sizes: [f64; 6],
    /// Track, secondary, range, thumb and border colours.
    colors: [[u8; 4]; 5],
}

impl PartValues {
    pub(crate) fn mix(&self, to: &Self, t: f64) -> Self {
        Self {
            sizes: std::array::from_fn(|i| self.sizes[i] + (to.sizes[i] - self.sizes[i]) * t),
            colors: std::array::from_fn(|i| {
                crate::animation::transition::mix_oklab(self.colors[i], to.colors[i], t)
            }),
        }
    }
}

impl SliderPaint {
    pub(crate) fn values(&self, commands: &[Paint]) -> PartValues {
        let mut values = PartValues {
            sizes: [0.0; 6],
            colors: [[0; 4]; 5],
        };
        for (slot, command) in self.0.iter().enumerate() {
            match command.map(|index| &commands[index]) {
                Some(Paint::Rect {
                    color,
                    bounds,
                    radius,
                    ..
                }) => {
                    values.colors[slot] = *color;
                    if slot < 3 {
                        values.sizes[slot] = f64::from(bounds[3]);
                    } else {
                        values.sizes[3..]
                            .copy_from_slice(&[bounds[2], bounds[3], *radius].map(f64::from));
                    }
                }
                Some(Paint::Border { color, .. }) => values.colors[slot] = *color,
                _ => {}
            }
        }
        values
    }

    /// The node's commands (from `first`) with the parts resized and recoloured about their centres.
    pub(crate) fn apply(
        &self,
        commands: &[Paint],
        first: usize,
        values: &PartValues,
    ) -> Vec<Paint> {
        let mut out = commands.to_vec();
        let thumb = self.0[3].map(|index| &commands[index - first]);
        let Some(Paint::Rect { bounds: was, .. }) = thumb.cloned() else {
            return out;
        };
        let [w, h, r] = [values.sizes[3], values.sizes[4], values.sizes[5]].map(|v| v as f32);
        let centre = [was[0] + was[2] / 2.0, was[1] + was[3] / 2.0];
        let grown = [centre[0] - w / 2.0, centre[1] - h / 2.0, w, h];
        for (offset, paint) in out.iter_mut().enumerate() {
            let slot = self.0.iter().position(|slot| *slot == Some(first + offset));
            match (slot, paint) {
                (
                    Some(slot @ 0..3),
                    Paint::Rect {
                        color,
                        bounds,
                        radius,
                        ..
                    },
                ) => {
                    let height = values.sizes[slot] as f32;
                    let middle = bounds[1] + bounds[3] / 2.0;
                    // A fully rounded line stays fully rounded as it thickens.
                    *radius = if *radius >= bounds[3] / 2.0 - 0.01 {
                        height / 2.0
                    } else {
                        radius.min(height / 2.0)
                    };
                    bounds[1] = middle - height / 2.0;
                    bounds[3] = height;
                    *color = values.colors[slot];
                }
                (
                    Some(3),
                    Paint::Rect {
                        color,
                        bounds,
                        radius,
                        ..
                    },
                ) => {
                    *bounds = grown;
                    *radius = r;
                    *color = values.colors[3];
                }
                (
                    Some(4),
                    Paint::Border {
                        color,
                        bounds,
                        radius,
                        ..
                    },
                ) => {
                    *bounds = grown;
                    *radius = r;
                    *color = values.colors[4];
                }
                (None, Paint::Shadow { bounds, radius, .. }) if *bounds == was => {
                    *bounds = grown;
                    *radius = r;
                }
                _ => {}
            }
        }
        out
    }
}

/// Draws the track, secondary, range and thumb at `[left, width, center]` of the control box.
pub(crate) fn paint(
    range: &SliderSnapshot,
    record: &Record,
    [left, width, center]: [f32; 3],
    accent: [u8; 4],
    palette: &crate::theme::Palette,
    commands: &mut Vec<crate::scene::paint::Paint>,
) -> Result<SliderPaint, String> {
    let mut slots = SliderPaint::default();
    use lingxia_theme::Role;
    let start = left + range.inset[0];
    let track_width = (width - range.inset[0] - range.inset[1]).max(0.0);
    let share = |value: f64| ((value - range.min) / (range.max - range.min)) as f32;
    let fill = |part: &Part, role: [u8; 4]| match part.background.as_deref() {
        Some("currentColor") => Ok(accent),
        Some(value) => palette.resolve(value),
        None => Ok(role),
    };
    // A disabled slider greys what the author left at its defaults.
    let lit = if range.enabled {
        accent
    } else {
        palette.role(Role::TextFaint)
    };
    let track = Part::of(record, field::TRACK_STYLE)?;
    let thickness = track.height.unwrap_or(4.0);
    let mut line =
        |slot: usize, part: &Part, default: [u8; 4], [from, to]: [f32; 2]| -> Result<(), String> {
            let height = part.height.unwrap_or(thickness);
            slots.0[slot] = Some(commands.len());
            commands.push(Paint::Rect {
                corners: None,
                color: fill(part, default)?,
                bounds: [
                    start + track_width * from,
                    center - height / 2.0,
                    track_width * (to - from).max(0.0),
                    height,
                ],
                radius: part.radius.unwrap_or(height / 2.0).min(height / 2.0),
            });
            Ok(())
        };
    // Two thumbs share one track: the lower draws it and the range between them, and the
    // upper, painted above, adds only its thumb.
    if !matches!(range.pair, Some((_, true))) {
        line(0, &track, palette.role(Role::Line), [0.0, 1.0])?;
        if let Some(secondary) = range.secondary {
            let part = Part::of(record, field::SECONDARY_STYLE)?;
            line(
                1,
                &part,
                palette.role(Role::TextFaint),
                [0.0, share(secondary)],
            )?;
        }
        let filled = match range.pair {
            Some((other, _)) => [share(range.value), share(other)],
            None => [0.0, share(range.value)],
        };
        line(2, &Part::of(record, field::RANGE_STYLE)?, lit, filled)?;
    }
    let thumb = Part::of(record, field::THUMB_STYLE)?;
    let [w, h] = [
        thumb.width.unwrap_or(THUMB) * thumb.scale[0],
        thumb.height.or(thumb.width).unwrap_or(THUMB) * thumb.scale[1],
    ];
    let bounds = [
        start + track_width * share(range.value) - w / 2.0,
        center - h / 2.0,
        w,
        h,
    ];
    let radius = thumb.radius.unwrap_or(f32::MAX).min(w.min(h) / 2.0);
    if let Some(source) = &thumb.shadow {
        for shadow in crate::scene::shadow::BoxShadow::parse(source)?
            .into_iter()
            .rev()
        {
            let color = if shadow.color == "currentColor" {
                accent
            } else {
                palette.resolve(&shadow.color)?
            };
            commands.push(Paint::Shadow {
                shadow,
                color,
                bounds,
                radius,
            });
        }
    }
    slots.0[3] = Some(commands.len());
    commands.push(Paint::Rect {
        corners: None,
        color: fill(&thumb, lit)?,
        bounds,
        radius,
    });
    if thumb.border > 0.0 {
        slots.0[4] = Some(commands.len());
        commands.push(Paint::Border {
            color: match thumb.border_color.as_deref() {
                Some("currentColor") | None => accent,
                Some(value) => palette.resolve(value)?,
            },
            bounds,
            widths: [thumb.border; 4],
            radius,
            corners: None,
            style: crate::scene::border::LineStyle::Solid,
        });
    }
    thumb_ring(&thumb, bounds, radius, accent, palette, commands)?;
    Ok(slots)
}

/// A thumb's `ring-*`: strokes around (or inside) the knob, `currentColor` being the accent.
fn thumb_ring(
    thumb: &Part,
    bounds: [f32; 4],
    radius: f32,
    accent: [u8; 4],
    palette: &crate::theme::Palette,
    commands: &mut Vec<Paint>,
) -> Result<(), String> {
    let Some(ring) = thumb
        .ring
        .as_deref()
        .map(crate::scene::ring::Ring::parse)
        .transpose()?
        .flatten()
    else {
        return Ok(());
    };
    ring.resolve(|tint| {
        if tint == "currentColor" {
            Ok(accent)
        } else {
            palette.resolve(tint)
        }
    })?
    .paint(
        &crate::scene::ring::RingBox {
            bounds,
            radius,
            corners: None,
            border: [thumb.border; 4],
        },
        commands,
    );
    Ok(())
}

/// A switch track's width and height: its `track:` part, else 52 × 32.
pub(crate) fn switch_track(record: &Record) -> [f32; 2] {
    let track = Part::of(record, field::TRACK_STYLE).unwrap_or_default();
    [track.width.unwrap_or(52.0), track.height.unwrap_or(32.0)]
}

/// A switch at the inline end `right` of its content box, centred on `center`.
pub(crate) struct Switch {
    pub right: f32,
    pub center: f32,
    pub checked: bool,
    pub pressed: bool,
    /// The track when on, when on and pressed, and when off.
    pub on: [u8; 4],
    pub on_pressed: [u8; 4],
    pub off: [u8; 4],
    pub press_layer: [u8; 4],
}

/// A filled mark's colour while pressed: a solid role's own pressed shade, else the colour mixed
/// 14 % toward black, as the theme derives those shades.
pub(crate) fn pressed_fill(
    value: &str,
    palette: &crate::theme::Palette,
) -> Result<[u8; 4], String> {
    use lingxia_theme::Role;
    if let crate::theme::WireColor::Role(role, 100) = crate::theme::parse(value)? {
        match role {
            Role::AccentSolid => return Ok(palette.role(Role::AccentSolidPressed)),
            Role::DangerSolid => return Ok(palette.role(Role::DangerSolidPressed)),
            _ => {}
        }
    }
    let [r, g, b, a] = palette.resolve(value)?;
    let deep = |c: u8| (f32::from(c) * 0.86).round() as u8;
    Ok([deep(r), deep(g), deep(b), a])
}

/// Draws a switch's track and thumb; the thumb rests `gap` inside the track at either end.
pub(crate) fn paint_switch(
    switch: &Switch,
    record: &Record,
    palette: &crate::theme::Palette,
    commands: &mut Vec<Paint>,
) -> Result<(SliderPaint, [f32; 4], f32), String> {
    let mut slots = SliderPaint::default();
    let track = Part::of(record, field::TRACK_STYLE)?;
    let thumb = Part::of(record, field::THUMB_STYLE)?;
    let [width, height] = [track.width.unwrap_or(52.0), track.height.unwrap_or(32.0)];
    let left = switch.right - width;
    let bounds = [left, switch.center - height / 2.0, width, height];
    let radius = track.radius.unwrap_or(f32::MAX).min(height / 2.0);
    let fill = |part: &Part, default: [u8; 4]| match part.background.as_deref() {
        Some("currentColor") => Ok(switch.on),
        Some(value) => palette.resolve(value),
        None => Ok(default),
    };
    // A pressed track deepens, on or off, authored or not: the text-colour layer would lighten
    // it in dark. Only a translucent off track takes the layer.
    let resting = fill(
        &track,
        if switch.checked {
            switch.on
        } else {
            switch.off
        },
    )?;
    let deepened = switch.pressed && (switch.checked || resting[3] == 255);
    let color = if !deepened {
        resting
    } else if switch.checked {
        match track.background.as_deref() {
            None | Some("currentColor") => switch.on_pressed,
            Some(value) => pressed_fill(value, palette)?,
        }
    } else {
        let deep = |c: u8| (f32::from(c) * 0.86).round() as u8;
        [deep(resting[0]), deep(resting[1]), deep(resting[2]), 255]
    };
    slots.0[0] = Some(commands.len());
    commands.push(Paint::Rect {
        corners: None,
        color,
        bounds,
        radius,
    });
    if switch.pressed && !deepened {
        commands.push(Paint::Rect {
            corners: None,
            color: switch.press_layer,
            bounds,
            radius,
        });
    }
    let size = [
        thumb.width.unwrap_or(24.0),
        thumb.height.or(thumb.width).unwrap_or(24.0),
    ];
    let gap = ((height - size[1]) / 2.0).max(0.0);
    // A held default thumb stretches toward the side it will move to.
    let stretch = if switch.pressed && thumb.width.is_none() {
        6.0
    } else {
        0.0
    };
    let [w, h] = [
        (size[0] + stretch) * thumb.scale[0],
        size[1] * thumb.scale[1],
    ];
    let middle = if switch.checked {
        left + width - gap - (size[0] + stretch) / 2.0
    } else {
        left + gap + (size[0] + stretch) / 2.0
    };
    let knob = [middle - w / 2.0, switch.center - h / 2.0, w, h];
    let round = thumb.radius.unwrap_or(f32::MAX).min(w.min(h) / 2.0);
    if let Some(source) = &thumb.shadow {
        for shadow in crate::scene::shadow::BoxShadow::parse(source)?
            .into_iter()
            .rev()
        {
            let color = if shadow.color == "currentColor" {
                switch.on
            } else {
                palette.resolve(&shadow.color)?
            };
            commands.push(Paint::Shadow {
                shadow,
                color,
                bounds: knob,
                radius: round,
            });
        }
    }
    slots.0[3] = Some(commands.len());
    commands.push(Paint::Rect {
        corners: None,
        color: fill(&thumb, palette.role(lingxia_theme::Role::Knob))?,
        bounds: knob,
        radius: round,
    });
    if thumb.border > 0.0 {
        slots.0[4] = Some(commands.len());
        commands.push(Paint::Border {
            color: match thumb.border_color.as_deref() {
                Some("currentColor") | None => switch.on,
                Some(value) => palette.resolve(value)?,
            },
            bounds: knob,
            widths: [thumb.border; 4],
            radius: round,
            corners: None,
            style: crate::scene::border::LineStyle::Solid,
        });
    }
    thumb_ring(&thumb, knob, round, switch.on, palette, commands)?;
    Ok((slots, bounds, radius))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(color: [u8; 4], bounds: [f32; 4], radius: f32) -> Paint {
        Paint::Rect {
            corners: None,
            color,
            bounds,
            radius,
        }
    }

    #[test]
    fn eased_parts_resize_about_their_centres_and_never_move_the_thumb() {
        let slots = SliderPaint([Some(0), None, Some(1), Some(2), None]);
        let rest = vec![
            rect([0; 4], [10.0, 23.0, 100.0, 2.0], 1.0),
            rect([0, 0, 255, 255], [10.0, 23.0, 40.0, 2.0], 1.0),
            rect([255, 0, 0, 255], [50.0, 24.0, 0.0, 0.0], 0.0),
        ];
        let held = vec![
            rect([0; 4], [10.0, 21.0, 100.0, 6.0], 3.0),
            rect([0, 0, 255, 255], [10.0, 21.0, 40.0, 6.0], 3.0),
            rect([255, 0, 0, 255], [42.0, 16.0, 16.0, 16.0], 8.0),
        ];
        let half = slots.values(&rest).mix(&slots.values(&held), 0.5);
        let drawn = slots.apply(&rest, 0, &half);
        let Paint::Rect { bounds, radius, .. } = &drawn[0] else {
            panic!()
        };
        assert_eq!((*bounds, *radius), ([10.0, 22.0, 100.0, 4.0], 2.0));
        let Paint::Rect { bounds, radius, .. } = &drawn[2] else {
            panic!()
        };
        assert_eq!((*bounds, *radius), ([46.0, 20.0, 8.0, 8.0], 4.0));
        let parts = crate::animation::transition::TransitionDef::parts();
        assert!(parts.covers(crate::animation::transition::Property::Parts));
        assert!(!parts.covers(crate::animation::transition::Property::Background));
    }

    #[test]
    fn a_pressed_on_track_deepens_in_dark_whether_or_not_it_is_authored() {
        use lingxia_theme::Role;
        let palette = crate::theme::Palette::builtin(true);
        let record = |background: Option<&str>| {
            let fields: Vec<_> = background
                .map(|bg| {
                    let pairs = serde_json::json!([field::BACKGROUND, bg]).to_string();
                    (field::TRACK_STYLE, serde_json::Value::String(pairs))
                })
                .into_iter()
                .collect();
            crate::scene::test_prelude::node(crate::protocol::tag::SWITCH, None, "s", &fields)
        };
        let track = |background: Option<&str>, checked: bool| {
            let switch = Switch {
                right: 100.0,
                center: 20.0,
                checked,
                pressed: true,
                on: palette.role(Role::AccentSolid),
                on_pressed: palette.role(Role::AccentSolidPressed),
                off: palette.role(Role::Line),
                press_layer: [255, 255, 255, 51],
            };
            let mut commands = Vec::new();
            paint_switch(&switch, &record(background), &palette, &mut commands).unwrap();
            let fills: Vec<[u8; 4]> = commands
                .iter()
                .filter_map(|paint| match paint {
                    Paint::Rect { color, .. } => Some(*color),
                    _ => None,
                })
                .collect();
            fills
        };
        let deep = palette.role(Role::AccentSolidPressed);
        assert_eq!(track(None, true)[0], deep);
        // Liuli's `checked:track:bg-accent-solid`.
        let authored = track(Some("accent-solid"), true);
        assert_eq!(authored[0], deep);
        assert!(
            !authored.contains(&[255, 255, 255, 51]),
            "no lightening layer"
        );
        // Another colour mixes toward black.
        assert_eq!(track(Some("#00b050"), true)[0], [0, 151, 69, 255]);
        // Off, an opaque track deepens the same way in dark (the layer would lighten it)...
        let off = track(Some("#00b050"), false);
        assert_eq!(off[0], [0, 151, 69, 255]);
        assert!(!off.contains(&[255, 255, 255, 51]), "no lightening layer");
        // ...and only a translucent one takes the layer.
        assert!(track(Some("#00b05080"), false).contains(&[255, 255, 255, 51]));
    }

    #[test]
    fn a_pressed_on_switch_in_the_showcase_green_deepens_in_light_and_dark() {
        use lingxia_theme::{Appearance, Prefs, Role};
        let pack = lingxia_theme::complete(
            serde_json::from_str(include_str!(
                "../../../../examples/lingxia-showcase/lxapp/theme.json"
            ))
            .unwrap(),
        );
        // A track has no label: whatever its solid's label needs, pressed is darker.
        let luminance = |[r, g, b, _]: [u8; 4]| {
            0.2126 * f32::from(r) + 0.7152 * f32::from(g) + 0.0722 * f32::from(b)
        };
        for appearance in [Appearance::Light, Appearance::Dark] {
            let prefs = Prefs {
                appearance,
                ..Prefs::default()
            };
            let theme = lingxia_theme::resolve(&pack, &prefs).unwrap();
            let palette = crate::theme::Palette::from_theme(&theme);
            let pairs = serde_json::json!([field::BACKGROUND, "accent-solid"]).to_string();
            let record = crate::scene::test_prelude::node(
                crate::protocol::tag::SWITCH,
                None,
                "s",
                &[(field::TRACK_STYLE, serde_json::Value::String(pairs))],
            );
            let switch = Switch {
                right: 100.0,
                center: 20.0,
                checked: true,
                pressed: true,
                on: palette.role(Role::AccentSolid),
                on_pressed: palette.role(Role::AccentSolidPressed),
                off: palette.role(Role::Line),
                press_layer: [255, 255, 255, 51],
            };
            let mut commands = Vec::new();
            paint_switch(&switch, &record, &palette, &mut commands).unwrap();
            let pressed = commands
                .iter()
                .find_map(|paint| match paint {
                    Paint::Rect { color, .. } => Some(*color),
                    _ => None,
                })
                .unwrap();
            let on = palette.role(Role::AccentSolid);
            assert!(
                luminance(pressed) < luminance(on),
                "{appearance:?}: pressed {pressed:?} is not darker than {on:?}"
            );
        }
    }
}
