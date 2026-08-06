//! Box drawing, block elements and powerline separators, drawn to the cell.
//!
//! Fonts disagree about where these sit and how thick they are, so borders
//! built from them meet with gaps or overlaps at some sizes and in some faces.
//! Drawing them from the cell's own geometry makes them meet exactly, always —
//! the same reason the Apple host has `TerminalSprites.swift`.
//!
//! Output is coverage, so a sprite is placed in the atlas and tinted by the
//! run's color exactly like a glyph.

use super::text::Rasterized;

/// Whether this codepoint is drawn rather than looked up in the font.
pub(super) fn handles(scalar: u32) -> bool {
    matches!(scalar, 0x2500..=0x259F | 0xE0B0..=0xE0B3)
}

/// Line weight of one arm of a box-drawing character.
#[derive(Clone, Copy, PartialEq)]
enum Weight {
    None,
    Light,
    Heavy,
    Double,
}

impl Weight {
    fn thickness(self, light: f32) -> f32 {
        match self {
            Self::None => 0.0,
            Self::Light | Self::Double => light,
            Self::Heavy => (light * 2.0).round().max(light + 1.0),
        }
    }
}

/// The four arms leaving the cell's centre.
#[derive(Clone, Copy, Default)]
struct Arms {
    up: Option<Weight>,
    down: Option<Weight>,
    left: Option<Weight>,
    right: Option<Weight>,
}

/// A one-cell coverage bitmap under construction.
struct Canvas {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl Canvas {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0u8; (width * height) as usize],
        }
    }

    /// Fill a rectangle in float coordinates, antialiasing only the edges —
    /// the interior is solid, which is what keeps joins seamless.
    fn fill(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        let (x0, x1) = (x0.min(x1), x0.max(x1));
        let (y0, y1) = (y0.min(y1), y0.max(y1));
        for y in 0..self.height {
            let top = y as f32;
            let cover_y = (y1.min(top + 1.0) - y0.max(top)).clamp(0.0, 1.0);
            if cover_y <= 0.0 {
                continue;
            }
            for x in 0..self.width {
                let left = x as f32;
                let cover_x = (x1.min(left + 1.0) - x0.max(left)).clamp(0.0, 1.0);
                if cover_x <= 0.0 {
                    continue;
                }
                let value = (cover_x * cover_y * 255.0).round() as u32;
                let index = (y * self.width + x) as usize;
                self.pixels[index] = self.pixels[index].max(value.min(255) as u8);
            }
        }
    }

    /// Fill the half-plane on one side of the line through two points, which
    /// is all a powerline separator needs.
    fn fill_triangle(&mut self, points: [(f32, f32); 3]) {
        let edge = |a: (f32, f32), b: (f32, f32), p: (f32, f32)| {
            (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0)
        };
        // 2x2 supersampling: these are the only diagonals in the set, and a
        // hard edge on them is the one place aliasing shows.
        const OFFSETS: [(f32, f32); 4] = [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)];
        for y in 0..self.height {
            for x in 0..self.width {
                let mut inside = 0;
                for (dx, dy) in OFFSETS {
                    let p = (x as f32 + dx, y as f32 + dy);
                    let a = edge(points[0], points[1], p);
                    let b = edge(points[1], points[2], p);
                    let c = edge(points[2], points[0], p);
                    if (a >= 0.0 && b >= 0.0 && c >= 0.0) || (a <= 0.0 && b <= 0.0 && c <= 0.0) {
                        inside += 1;
                    }
                }
                if inside > 0 {
                    let index = (y * self.width + x) as usize;
                    let value = (inside * 255 / OFFSETS.len()) as u8;
                    self.pixels[index] = self.pixels[index].max(value);
                }
            }
        }
    }

    /// As a premultiplied-BGRA sprite: coverage in alpha, color at draw time.
    fn finish(self, left: i32, top: i32) -> Rasterized {
        let mut pixels = vec![0u8; (self.width * self.height * 4) as usize];
        for (texel, alpha) in pixels.chunks_exact_mut(4).zip(self.pixels) {
            texel[3] = alpha;
        }
        Rasterized {
            width: self.width,
            height: self.height,
            left,
            top,
            colored: false,
            pixels,
        }
    }
}

/// Draw one codepoint at the cell size, or `None` when it is not ours.
///
/// `baseline` is the pen's distance from the cell top, because sprites are
/// placed against the baseline like every other glyph.
pub(super) fn draw(
    scalar: u32,
    cell_width: f32,
    line_height: f32,
    baseline: f32,
) -> Option<Rasterized> {
    let width = cell_width.round().max(1.0) as u32;
    let height = line_height.round().max(1.0) as u32;
    let mut canvas = Canvas::new(width, height);
    let (w, h) = (width as f32, height as f32);
    // One "light" line, snapped to a whole pixel so runs of the same character
    // never alternate between one and two pixels.
    let light = (line_height / 12.0).round().max(1.0);

    match scalar {
        0x2500..=0x257F => draw_box(&mut canvas, arms(scalar)?, w, h, light),
        0x2580..=0x259F => draw_block(&mut canvas, scalar, w, h)?,
        0xE0B0..=0xE0B3 => draw_powerline(&mut canvas, scalar, w, h),
        _ => return None,
    }
    // The pen sits on the baseline; the sprite starts at the cell top.
    Some(canvas.finish(0, -(baseline.round() as i32)))
}

fn draw_box(canvas: &mut Canvas, arms: Arms, w: f32, h: f32, light: f32) {
    let (cx, cy) = (w / 2.0, h / 2.0);
    let mut arm = |weight: Option<Weight>, horizontal: bool, positive: bool| {
        let Some(weight) = weight else { return };
        let thickness = weight.thickness(light);
        if thickness <= 0.0 {
            return;
        }
        // A double line is two light lines with a light gap between them.
        let strokes: &[f32] = if weight == Weight::Double {
            &[-light, light]
        } else {
            &[0.0]
        };
        for offset in strokes {
            if horizontal {
                let (y0, y1) = (cy + offset - thickness / 2.0, cy + offset + thickness / 2.0);
                if positive {
                    canvas.fill(cx - thickness / 2.0, y0, w, y1);
                } else {
                    canvas.fill(0.0, y0, cx + thickness / 2.0, y1);
                }
            } else {
                let (x0, x1) = (cx + offset - thickness / 2.0, cx + offset + thickness / 2.0);
                if positive {
                    canvas.fill(x0, cy - thickness / 2.0, x1, h);
                } else {
                    canvas.fill(x0, 0.0, x1, cy + thickness / 2.0);
                }
            }
        }
    };
    arm(arms.right, true, true);
    arm(arms.left, true, false);
    arm(arms.down, false, true);
    arm(arms.up, false, false);
}

fn draw_block(canvas: &mut Canvas, scalar: u32, w: f32, h: f32) -> Option<()> {
    match scalar {
        // Upper/lower eighths and halves.
        0x2580 => canvas.fill(0.0, 0.0, w, h / 2.0),
        0x2581..=0x2588 => {
            let eighths = (scalar - 0x2580) as f32;
            canvas.fill(0.0, h - h * eighths / 8.0, w, h);
        }
        0x2589..=0x258F => {
            let eighths = 8.0 - (scalar - 0x2588) as f32;
            canvas.fill(0.0, 0.0, w * eighths / 8.0, h);
        }
        0x2590 => canvas.fill(w / 2.0, 0.0, w, h),
        // Shades, as a coverage the caller tints.
        0x2591..=0x2593 => {
            let level = match scalar {
                0x2591 => 0.25,
                0x2592 => 0.5,
                _ => 0.75,
            };
            canvas.fill(0.0, 0.0, w, h);
            for value in canvas.pixels.iter_mut() {
                *value = (f32::from(*value) * level).round() as u8;
            }
        }
        0x2594 => canvas.fill(0.0, 0.0, w, h / 8.0),
        0x2595 => canvas.fill(w * 7.0 / 8.0, 0.0, w, h),
        // Quadrants.
        0x2596..=0x259F => {
            let quadrant = |canvas: &mut Canvas, index: u32| match index {
                0 => canvas.fill(0.0, 0.0, w / 2.0, h / 2.0),
                1 => canvas.fill(w / 2.0, 0.0, w, h / 2.0),
                2 => canvas.fill(0.0, h / 2.0, w / 2.0, h),
                _ => canvas.fill(w / 2.0, h / 2.0, w, h),
            };
            // Bit per quadrant, in reading order.
            let mask = match scalar {
                0x2596 => 0b0100,
                0x2597 => 0b1000,
                0x2598 => 0b0001,
                0x2599 => 0b1101,
                0x259A => 0b1001,
                0x259B => 0b0111,
                0x259C => 0b1011,
                0x259D => 0b0010,
                0x259E => 0b0110,
                0x259F => 0b1110,
                _ => return None,
            };
            for index in 0..4 {
                if mask & (1 << index) != 0 {
                    quadrant(canvas, index);
                }
            }
        }
        _ => return None,
    }
    Some(())
}

fn draw_powerline(canvas: &mut Canvas, scalar: u32, w: f32, h: f32) {
    match scalar {
        // Filled right-pointing triangle, and its left-pointing mirror.
        0xE0B0 => canvas.fill_triangle([(0.0, 0.0), (w, h / 2.0), (0.0, h)]),
        0xE0B2 => canvas.fill_triangle([(w, 0.0), (0.0, h / 2.0), (w, h)]),
        // The outlined forms: the same wedge with its interior removed.
        0xE0B1 | 0xE0B3 => {
            let thickness = (w / 8.0).max(1.0);
            let (tip, back) = if scalar == 0xE0B1 {
                ((w, h / 2.0), 0.0)
            } else {
                ((0.0, h / 2.0), w)
            };
            canvas.fill_triangle([(back, 0.0), tip, (back, h)]);
            let inset = thickness * 1.8;
            let inner_tip = if scalar == 0xE0B1 {
                (tip.0 - inset, tip.1)
            } else {
                (tip.0 + inset, tip.1)
            };
            let inner_back = if scalar == 0xE0B1 {
                back + inset
            } else {
                back - inset
            };
            let mut hole = Canvas::new(canvas.width, canvas.height);
            hole.fill_triangle([(inner_back, inset), inner_tip, (inner_back, h - inset)]);
            for (value, cut) in canvas.pixels.iter_mut().zip(hole.pixels) {
                *value = value.saturating_sub(cut);
            }
        }
        _ => {}
    }
}

/// The arms of a box-drawing character, or `None` for one not covered.
fn arms(scalar: u32) -> Option<Arms> {
    use Weight::{Double, Heavy, Light};

    let both = |weight: Weight, horizontal: bool| {
        if horizontal {
            Arms {
                left: Some(weight),
                right: Some(weight),
                ..Default::default()
            }
        } else {
            Arms {
                up: Some(weight),
                down: Some(weight),
                ..Default::default()
            }
        }
    };
    // The four weights a corner or tee cycles through, in Unicode's order.
    let pair = |index: u32| -> (Weight, Weight) {
        match index {
            0 => (Light, Light),
            1 => (Heavy, Light),
            2 => (Light, Heavy),
            _ => (Heavy, Heavy),
        }
    };

    Some(match scalar {
        0x2500 => both(Light, true),
        0x2501 => both(Heavy, true),
        0x2502 => both(Light, false),
        0x2503 => both(Heavy, false),
        0x250C..=0x250F => {
            let (a, b) = pair(scalar - 0x250C);
            Arms {
                right: Some(a),
                down: Some(b),
                ..Default::default()
            }
        }
        0x2510..=0x2513 => {
            let (a, b) = pair(scalar - 0x2510);
            Arms {
                left: Some(a),
                down: Some(b),
                ..Default::default()
            }
        }
        0x2514..=0x2517 => {
            let (a, b) = pair(scalar - 0x2514);
            Arms {
                right: Some(a),
                up: Some(b),
                ..Default::default()
            }
        }
        0x2518..=0x251B => {
            let (a, b) = pair(scalar - 0x2518);
            Arms {
                left: Some(a),
                up: Some(b),
                ..Default::default()
            }
        }
        0x251C..=0x2523 => tee(scalar - 0x251C, Side::Right),
        0x2524..=0x252B => tee(scalar - 0x2524, Side::Left),
        0x252C..=0x2533 => tee(scalar - 0x252C, Side::Down),
        0x2534..=0x253B => tee(scalar - 0x2534, Side::Up),
        0x253C..=0x254B => cross(scalar - 0x253C),
        // Doubles: one weight, drawn as two lines.
        0x2550 => both(Double, true),
        0x2551 => both(Double, false),
        0x2552..=0x256C => double_joint(scalar)?,
        // Dashes fall back to their solid form rather than to the font, so a
        // dashed border still meets a solid one.
        0x2504..=0x250B => both(if scalar % 2 == 0 { Light } else { Heavy }, scalar < 0x2508),
        0x254C..=0x254F => both(if scalar % 2 == 0 { Light } else { Heavy }, scalar < 0x254E),
        // Half lines.
        0x2574 => Arms {
            left: Some(Light),
            ..Default::default()
        },
        0x2575 => Arms {
            up: Some(Light),
            ..Default::default()
        },
        0x2576 => Arms {
            right: Some(Light),
            ..Default::default()
        },
        0x2577 => Arms {
            down: Some(Light),
            ..Default::default()
        },
        0x2578 => Arms {
            left: Some(Heavy),
            ..Default::default()
        },
        0x2579 => Arms {
            up: Some(Heavy),
            ..Default::default()
        },
        0x257A => Arms {
            right: Some(Heavy),
            ..Default::default()
        },
        0x257B => Arms {
            down: Some(Heavy),
            ..Default::default()
        },
        _ => return None,
    })
}

enum Side {
    Up,
    Down,
    Left,
    Right,
}

/// A tee: a stem plus the two arms across from it, weights cycling in
/// Unicode's order (stem light/heavy × arms light/heavy).
fn tee(index: u32, stem: Side) -> Arms {
    use Weight::{Heavy, Light};
    let stem_weight = if index >= 4 { Heavy } else { Light };
    let first = if index % 4 == 1 || index % 4 == 3 {
        Heavy
    } else {
        Light
    };
    let second = if index % 4 >= 2 { Heavy } else { Light };
    let mut arms = Arms::default();
    match stem {
        Side::Right => {
            arms.right = Some(stem_weight);
            arms.up = Some(first);
            arms.down = Some(second);
        }
        Side::Left => {
            arms.left = Some(stem_weight);
            arms.up = Some(first);
            arms.down = Some(second);
        }
        Side::Down => {
            arms.down = Some(stem_weight);
            arms.left = Some(first);
            arms.right = Some(second);
        }
        Side::Up => {
            arms.up = Some(stem_weight);
            arms.left = Some(first);
            arms.right = Some(second);
        }
    }
    arms
}

fn cross(index: u32) -> Arms {
    use Weight::{Heavy, Light};
    let horizontal = if index % 2 == 1 { Heavy } else { Light };
    let vertical = if index >= 8 { Heavy } else { Light };
    Arms {
        up: Some(vertical),
        down: Some(vertical),
        left: Some(horizontal),
        right: Some(horizontal),
    }
}

/// The double-line joints, where one axis is doubled and the other single.
fn double_joint(scalar: u32) -> Option<Arms> {
    use Weight::{Double, Light};
    let (h, v) = match (scalar - 0x2552) % 3 {
        0 => (Light, Double),
        1 => (Double, Light),
        _ => (Double, Double),
    };
    let mut arms = Arms::default();
    match scalar {
        0x2552..=0x2554 => {
            arms.right = Some(h);
            arms.down = Some(v);
        }
        0x2555..=0x2557 => {
            arms.left = Some(h);
            arms.down = Some(v);
        }
        0x2558..=0x255A => {
            arms.right = Some(h);
            arms.up = Some(v);
        }
        0x255B..=0x255D => {
            arms.left = Some(h);
            arms.up = Some(v);
        }
        0x255E..=0x2560 => {
            arms.right = Some(h);
            arms.up = Some(v);
            arms.down = Some(v);
        }
        0x2561..=0x2563 => {
            arms.left = Some(h);
            arms.up = Some(v);
            arms.down = Some(v);
        }
        0x2564..=0x2566 => {
            arms.left = Some(h);
            arms.right = Some(h);
            arms.down = Some(v);
        }
        0x2567..=0x2569 => {
            arms.left = Some(h);
            arms.right = Some(h);
            arms.up = Some(v);
        }
        0x256A..=0x256C => {
            arms.left = Some(h);
            arms.right = Some(h);
            arms.up = Some(v);
            arms.down = Some(v);
        }
        _ => return None,
    }
    Some(arms)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole point: a horizontal line and a vertical one must meet, so
    /// each has to reach the cell edge it is drawn toward.
    #[test]
    fn lines_reach_the_cell_edges_they_point_at() {
        let horizontal = draw(0x2500, 8.0, 16.0, 12.0).expect("horizontal");
        let row = (horizontal.height / 2) as usize * horizontal.width as usize;
        assert!(horizontal.pixels[row * 4 + 3] > 0, "left edge is covered");
        let last = row + horizontal.width as usize - 1;
        assert!(horizontal.pixels[last * 4 + 3] > 0, "right edge is covered");

        let vertical = draw(0x2502, 8.0, 16.0, 12.0).expect("vertical");
        let column = (vertical.width / 2) as usize;
        assert!(vertical.pixels[column * 4 + 3] > 0, "top edge is covered");
        let bottom = ((vertical.height - 1) * vertical.width) as usize + column;
        assert!(
            vertical.pixels[bottom * 4 + 3] > 0,
            "bottom edge is covered"
        );
    }

    /// A full block covers its cell completely; a half block covers half.
    #[test]
    fn blocks_cover_what_they_name() {
        let full = draw(0x2588, 8.0, 16.0, 12.0).expect("full block");
        assert!(
            full.pixels.chunks_exact(4).all(|texel| texel[3] == 0xff),
            "the full block leaves no gap"
        );
        let lower = draw(0x2584, 8.0, 16.0, 12.0).expect("lower half");
        let covered = lower
            .pixels
            .chunks_exact(4)
            .filter(|texel| texel[3] > 0)
            .count();
        let total = (lower.width * lower.height) as usize;
        assert_eq!(covered, total / 2, "the lower half is exactly half");
    }

    #[test]
    fn only_the_drawn_ranges_are_claimed() {
        assert!(handles(0x2500) && handles(0x259F) && handles(0xE0B0));
        assert!(!handles('A' as u32) && !handles(0x2600));
        assert!(draw('A' as u32, 8.0, 16.0, 12.0).is_none());
    }
}
