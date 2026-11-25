//! `ring-*`, `ring-offset-*` and `inset-ring-*` as strokes. A ring never takes layout space, and
//! its offset is a real gap: what is behind the box shows through unless a role fills it.
use crate::scene::border::LineStyle;
use crate::scene::paint::Paint;
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub(crate) struct Ring {
    #[serde(default)]
    outer: Option<Outer>,
    #[serde(default)]
    inner: Option<Inner>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct Outer {
    width: f32,
    #[serde(default)]
    offset: f32,
    #[serde(default = "current_color")]
    color: String,
    #[serde(default)]
    offset_color: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct Inner {
    width: f32,
    #[serde(default = "current_color")]
    color: String,
}

fn current_color() -> String {
    "currentColor".into()
}

/// The box a ring paints around: its bounds, uniform radius or per-corner radii, and border.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RingBox {
    pub bounds: [f32; 4],
    pub radius: f32,
    pub corners: Option<[[f32; 2]; 4]>,
    pub border: [f32; 4],
}

impl Ring {
    /// `none` is no ring.
    pub fn parse(source: &str) -> Result<Option<Self>, String> {
        if source == "none" {
            return Ok(None);
        }
        let invalid = || "UI_SHADOW: invalid ring".to_string();
        if source.len() > 1024 {
            return Err(invalid());
        }
        let ring: Self = serde_json::from_str(source).map_err(|_| invalid())?;
        let size = |v: f32| v.is_finite() && (0.0..=1024.0).contains(&v);
        let color = |c: &str| c == "currentColor" || crate::theme::parse(c).is_ok();
        let outer_ok = ring.outer.as_ref().is_none_or(|o| {
            size(o.width)
                && size(o.offset)
                && color(&o.color)
                && o.offset_color.as_deref().is_none_or(color)
        });
        let inner_ok = ring
            .inner
            .as_ref()
            .is_none_or(|i| size(i.width) && color(&i.color));
        if !outer_ok || !inner_ok {
            return Err(invalid());
        }
        Ok(Some(ring))
    }

    /// The ring with its colours resolved; `resolve` maps a colour (`currentColor` included) to
    /// RGBA.
    pub fn resolve(
        &self,
        resolve: impl Fn(&str) -> Result<[u8; 4], String>,
    ) -> Result<RingValue, String> {
        let mut value = RingValue::default();
        if let Some(outer) = &self.outer {
            value.width = outer.width;
            value.offset = outer.offset;
            value.color = resolve(&outer.color)?;
            if let Some(fill) = &outer.offset_color {
                value.offset_color = resolve(fill)?;
            }
        }
        if let Some(inner) = &self.inner {
            value.inner_width = inner.width;
            value.inner_color = resolve(&inner.color)?;
        }
        Ok(value)
    }
}

/// A ring as it paints and as a transition interpolates it; zero widths paint nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct RingValue {
    pub width: f32,
    pub offset: f32,
    pub color: [u8; 4],
    /// Transparent leaves the offset a gap.
    pub offset_color: [u8; 4],
    pub inner_width: f32,
    pub inner_color: [u8; 4],
}

impl RingValue {
    /// Pushes the offset band, the outer stroke and the inner stroke.
    pub fn paint(&self, target: &RingBox, commands: &mut Vec<Paint>) {
        if self.width > 0.0 {
            if self.offset_color[3] != 0 && self.offset > 0.0 {
                commands.push(band(target, 0.0, self.offset, self.offset_color));
            }
            commands.push(band(target, self.offset, self.width, self.color));
        }
        if self.inner_width > 0.0 {
            // Inside the border, as an inset shadow paints within the padding box.
            let [top, right, bottom, left] = target.border;
            let [x, y, w, h] = target.bounds;
            let bounds = [
                x + left,
                y + top,
                (w - left - right).max(0.0),
                (h - top - bottom).max(0.0),
            ];
            let shrink = top.max(right).max(bottom).max(left);
            commands.push(Paint::Border {
                color: self.inner_color,
                bounds,
                widths: [self.inner_width.min(bounds[2] / 2.0).min(bounds[3] / 2.0); 4],
                radius: (target.radius - shrink).max(0.0),
                corners: target
                    .corners
                    .map(|corners| corners.map(|pair| pair.map(|r| (r - shrink).max(0.0)))),
                style: LineStyle::Solid,
            });
        }
    }
}

/// A `width` stroke whose inner edge sits `gap` outside the box, its corners following the box's.
fn band(target: &RingBox, gap: f32, width: f32, color: [u8; 4]) -> Paint {
    let grow = gap + width;
    let [x, y, w, h] = target.bounds;
    let round = |r: f32| if r > 0.0 { r + grow } else { 0.0 };
    Paint::Border {
        color,
        bounds: [x - grow, y - grow, w + 2.0 * grow, h + 2.0 * grow],
        widths: [width; 4],
        radius: round(target.radius),
        corners: target
            .corners
            .map(|corners| corners.map(|pair| pair.map(round))),
        style: LineStyle::Solid,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target() -> RingBox {
        RingBox {
            bounds: [10.0, 10.0, 100.0, 40.0],
            radius: 8.0,
            corners: None,
            border: [1.0; 4],
        }
    }

    #[test]
    fn an_offset_ring_leaves_a_gap_unless_a_role_fills_it() {
        let ring = Ring::parse(r#"{"outer":{"width":2,"offset":3,"color":"accent"}}"#)
            .unwrap()
            .unwrap();
        let mut commands = Vec::new();
        ring.resolve(|_| Ok([1, 2, 3, 255]))
            .unwrap()
            .paint(&target(), &mut commands);
        assert_eq!(commands.len(), 1, "a gap paints nothing");
        let Paint::Border {
            bounds,
            widths,
            radius,
            ..
        } = &commands[0]
        else {
            panic!("a stroke");
        };
        assert_eq!(*bounds, [5.0, 5.0, 110.0, 50.0]);
        assert_eq!(*widths, [2.0; 4]);
        assert_eq!(*radius, 13.0);
        let filled = Ring::parse(
            r#"{"outer":{"width":2,"offset":3,"color":"accent","offsetColor":"surface"}}"#,
        )
        .unwrap()
        .unwrap();
        commands.clear();
        filled
            .resolve(|_| Ok([1, 2, 3, 255]))
            .unwrap()
            .paint(&target(), &mut commands);
        assert_eq!(commands.len(), 2, "the band, then the ring");
    }

    #[test]
    fn an_inset_ring_strokes_inside_the_border_and_a_sharp_box_stays_sharp() {
        let ring = Ring::parse(r#"{"inner":{"width":2,"color":"line"}}"#)
            .unwrap()
            .unwrap();
        let mut commands = Vec::new();
        let sharp = RingBox {
            radius: 0.0,
            ..target()
        };
        ring.resolve(|_| Ok([0, 0, 0, 255]))
            .unwrap()
            .paint(&sharp, &mut commands);
        let Paint::Border { bounds, radius, .. } = &commands[0] else {
            panic!("a stroke");
        };
        assert_eq!(*bounds, [11.0, 11.0, 98.0, 38.0]);
        assert_eq!(*radius, 0.0);
    }

    #[test]
    fn invalid_rings_are_refused() {
        for source in [
            r#"{"outer":{"width":-1}}"#,
            r#"{"outer":{"width":2,"offset":2000}}"#,
            r#"{"inner":{"width":2,"color":"nope-role"}}"#,
        ] {
            assert!(Ring::parse(source).is_err(), "{source}");
        }
        assert_eq!(Ring::parse("none").unwrap(), None);
    }

    #[test]
    fn a_key_this_host_does_not_know_is_ignored() {
        assert!(
            Ring::parse(
                r#"{"outer":{"width":2,"later":1},"inner":{"width":1,"later":1},"later":1}"#
            )
            .is_ok_and(|ring| ring.is_some())
        );
    }
}
