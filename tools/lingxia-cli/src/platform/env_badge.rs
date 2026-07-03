//! Cross-platform env-version icon badge drawing.
//!
//! Developer/preview builds get a small accent badge composited onto the
//! launcher icon so they can be visually distinguished from release builds.
//! This module only owns the shared badge appearance; platform modules decide
//! which icon copy to badge and where to stage it.

use anyhow::{Context, Result};
use image::{ImageFormat, Rgba, RgbaImage};
use std::path::Path;

use crate::config::EnvVersion;

/// Letter + accent color for the env's badge, or `None` for release.
pub fn env_badge(version: EnvVersion) -> Option<(char, [u8; 4])> {
    match version {
        // Match the Android accent (#D32F2F) so dev/preview look the same
        // across platforms.
        EnvVersion::Developer => Some(('D', [0xD3, 0x2F, 0x2F, 0xFF])),
        EnvVersion::Preview => Some(('P', [0xD3, 0x2F, 0x2F, 0xFF])),
        EnvVersion::Release => None,
    }
}

/// Badge a single PNG file in place. No-op (returns `false`) when the env
/// needs no badge, the file is missing, or the icon is too small to carry the
/// badge legibly (< 60 px wide, e.g. a tiny notification glyph).
pub fn badge_png_file(path: &Path, version: EnvVersion) -> Result<bool> {
    let Some((letter, accent)) = env_badge(version) else {
        return Ok(false);
    };
    if !path.is_file() {
        return Ok(false);
    }
    let img = image::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
    let mut rgba = img.to_rgba8();
    if rgba.width() < 60 {
        return Ok(false);
    }
    composite_badge(&mut rgba, letter, accent);
    rgba.save_with_format(path, ImageFormat::Png)
        .with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(true)
}

/// Composite a circular badge with a hand-rolled bitmap letter at the
/// bottom-right of `img`. Sized relative to the icon so it stays readable
/// from 60x60 home-screen icons up to the 1024x1024 marketing icon.
pub fn composite_badge(img: &mut RgbaImage, letter: char, accent: [u8; 4]) {
    composite_badge_inset(img, letter, accent, 0.0);
}

/// Like [`composite_badge`], but anchored to an artwork rect inset from the
/// canvas by `margin_frac` per side. macOS launcher icons keep ~10% of the
/// canvas transparent around the rounded square, so a canvas-anchored badge
/// would float outside the visible icon.
pub fn composite_badge_inset(img: &mut RgbaImage, letter: char, accent: [u8; 4], margin_frac: f32) {
    let (w, h) = img.dimensions();
    let margin = ((w.min(h) as f32) * margin_frac).round() as i32;
    let artwork = (w.min(h) as i32 - 2 * margin).max(1);
    let badge_diameter = ((artwork as f32) * 0.30).round() as i32;
    // Pull the badge in from the artwork corner so the whole circle clears the
    // icon's rounded corner (a corner-anchored badge pokes into the transparent
    // zone past the squircle). ~0.4·diameter of corner clearance seats it inside.
    let inset = ((badge_diameter as f32 * 0.4).round() as i32).max(2) + margin;
    let center_x = w as i32 - badge_diameter / 2 - inset;
    let center_y = h as i32 - badge_diameter / 2 - inset;
    let outer_r = badge_diameter / 2;
    let border_w = (badge_diameter / 16).max(2);
    let inner_r = (outer_r - border_w).max(1);

    let accent_color = Rgba(accent);
    let white = Rgba([0xFF, 0xFF, 0xFF, 0xFF]);

    for dy in -outer_r..=outer_r {
        for dx in -outer_r..=outer_r {
            let dist_sq = dx * dx + dy * dy;
            if dist_sq > outer_r * outer_r {
                continue;
            }
            let x = center_x + dx;
            let y = center_y + dy;
            if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 {
                continue;
            }
            let pixel = if dist_sq <= inner_r * inner_r {
                accent_color
            } else {
                white
            };
            img.put_pixel(x as u32, y as u32, pixel);
        }
    }

    draw_letter(img, letter, center_x, center_y, inner_r, white);
}

/// Render a 5x7 bitmap letter centered at `(cx, cy)`, scaled to fit inside
/// `inner_r` (the inner accent circle's radius). Pixels are drawn directly.
fn draw_letter(img: &mut RgbaImage, letter: char, cx: i32, cy: i32, inner_r: i32, fg: Rgba<u8>) {
    let glyph = letter_glyph(letter);
    if glyph.is_empty() {
        return;
    }
    let (w, h) = img.dimensions();
    let glyph_w = 5_i32;
    let glyph_h = glyph.len() as i32;
    // Fit inside ~70% of the inner circle so the letter doesn't bleed onto
    // the white border ring.
    let max_height = (inner_r * 2 * 7 / 10).max(7);
    let scale = (max_height / glyph_h).max(1);
    let actual_w = glyph_w * scale;
    let actual_h = glyph_h * scale;
    let origin_x = cx - actual_w / 2;
    let origin_y = cy - actual_h / 2;

    for (gy, row) in glyph.iter().enumerate() {
        for gx in 0..glyph_w {
            let bit = (row >> (glyph_w - 1 - gx)) & 1;
            if bit == 0 {
                continue;
            }
            for sy in 0..scale {
                for sx in 0..scale {
                    let x = origin_x + gx * scale + sx;
                    let y = origin_y + gy as i32 * scale + sy;
                    if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 {
                        continue;
                    }
                    img.put_pixel(x as u32, y as u32, fg);
                }
            }
        }
    }
}

/// 5x7 bitmap glyph rows (MSB-first within each 5-bit row).
fn letter_glyph(letter: char) -> &'static [u8] {
    match letter {
        'D' => &[
            0b11110, // ####.
            0b10001, // #...#
            0b10001, // #...#
            0b10001, // #...#
            0b10001, // #...#
            0b10001, // #...#
            0b11110, // ####.
        ],
        'P' => &[
            0b11110, // ####.
            0b10001, // #...#
            0b10001, // #...#
            0b11110, // ####.
            0b10000, // #....
            0b10000, // #....
            0b10000, // #....
        ],
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::{composite_badge, env_badge, letter_glyph};
    use crate::config::EnvVersion;
    use image::{Rgba, RgbaImage};

    #[test]
    fn release_has_no_badge() {
        assert!(env_badge(EnvVersion::Release).is_none());
        assert!(env_badge(EnvVersion::Developer).is_some());
        assert!(env_badge(EnvVersion::Preview).is_some());
    }

    #[test]
    fn glyph_table_covers_required_letters() {
        assert!(!letter_glyph('D').is_empty());
        assert!(!letter_glyph('P').is_empty());
        assert!(letter_glyph('X').is_empty());
    }

    #[test]
    fn composite_badge_modifies_bottom_right_pixels() {
        let mut img = RgbaImage::from_pixel(120, 120, Rgba([0, 0, 0, 0xFF]));
        composite_badge(&mut img, 'D', [0xD3, 0x2F, 0x2F, 0xFF]);
        // Pixel at the badge center should now be the accent color rather than
        // the original black.
        let center = *img.get_pixel(95, 95);
        assert_ne!(center, Rgba([0, 0, 0, 0xFF]));
        // Upper-left should be untouched.
        assert_eq!(*img.get_pixel(10, 10), Rgba([0, 0, 0, 0xFF]));
    }
}
