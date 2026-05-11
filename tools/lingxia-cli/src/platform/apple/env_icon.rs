//! Apple env-version launcher-icon overlay.
//!
//! Mirrors the Android `prepare_launcher_icon_overlay` flow: when the active
//! env is developer/preview, build a parallel `Assets.xcassets` under
//! `<platform_dir>/.lingxia/overlay/<env>/Resources/` whose `AppIcon.appiconset`
//! has each PNG composited with a small accent badge (filled circle + bitmap
//! "D" / "P"). The build then points `actool` at the staging resources dir so
//! the source asset catalog is never mutated and dev/release can be installed
//! side by side and visually distinguished on the home screen.

use anyhow::{Context, Result};
use image::{ImageFormat, Rgba, RgbaImage};
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::EnvVersion;

/// Letter + accent color for the env's badge, or `None` for release.
fn env_badge(version: EnvVersion) -> Option<(char, [u8; 4])> {
    match version {
        // Match the Android accent (#D32F2F) so dev/preview look the same
        // across platforms.
        EnvVersion::Developer => Some(('D', [0xD3, 0x2F, 0x2F, 0xFF])),
        EnvVersion::Preview => Some(('P', [0xD3, 0x2F, 0x2F, 0xFF])),
        EnvVersion::Release => None,
    }
}

/// If the active env needs a badge, stage a copy of `Assets.xcassets` with a
/// badged `AppIcon.appiconset` and return the staging *resources_dir*; the
/// caller should pass that to `compile_asset_catalog` instead of the source
/// dir. Returns `None` when no badge applies (release, or no source catalog).
pub fn prepare_overlay_resources_dir(
    platform_dir: &Path,
    resources_dir: &Path,
    env: EnvVersion,
) -> Result<Option<PathBuf>> {
    let Some((letter, accent)) = env_badge(env) else {
        return Ok(None);
    };
    let original_xcassets = resources_dir.join("Assets.xcassets");
    let original_appicon = original_xcassets.join("AppIcon.appiconset");
    if !original_appicon.exists() {
        return Ok(None);
    }

    let staging_root = platform_dir
        .join(".lingxia")
        .join("overlay")
        .join(env.as_str());
    let staging_resources = staging_root.join("Resources");
    let staging_xcassets = staging_resources.join("Assets.xcassets");
    if staging_root.exists() {
        fs::remove_dir_all(&staging_root)
            .with_context(|| format!("Failed to clean {}", staging_root.display()))?;
    }
    copy_dir_recursive(&original_xcassets, &staging_xcassets)?;

    let staging_appicon = staging_xcassets.join("AppIcon.appiconset");
    badge_appiconset(&staging_appicon, letter, accent)?;

    Ok(Some(staging_resources))
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst).with_context(|| format!("Failed to create {}", dst.display()))?;
    for entry in fs::read_dir(src).with_context(|| format!("Failed to read {}", src.display()))? {
        let entry = entry?;
        let path = entry.path();
        let dest = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(&path, &dest)?;
        } else {
            fs::copy(&path, &dest).with_context(|| {
                format!("Failed to copy {} -> {}", path.display(), dest.display())
            })?;
        }
    }
    Ok(())
}

fn badge_appiconset(dir: &Path, letter: char, accent: [u8; 4]) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("Failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("png") {
            continue;
        }
        // Skip very small icons (e.g. notification badges). Anything below
        // 60px wide can't legibly carry the badge, so leave them clean.
        let img =
            image::open(&path).with_context(|| format!("Failed to open {}", path.display()))?;
        let mut rgba = img.to_rgba8();
        if rgba.width() < 60 {
            continue;
        }
        composite_badge(&mut rgba, letter, accent);
        rgba.save_with_format(&path, ImageFormat::Png)
            .with_context(|| format!("Failed to write {}", path.display()))?;
    }
    Ok(())
}

/// Composite a circular badge with a hand-rolled bitmap letter at the
/// bottom-right of `img`. Sized relative to the icon so it stays readable
/// from 60×60 home-screen icons up to the 1024×1024 marketing icon.
fn composite_badge(img: &mut RgbaImage, letter: char, accent: [u8; 4]) {
    let (w, h) = img.dimensions();
    let badge_diameter = ((w.min(h) as f32) * 0.35).round() as i32;
    let inset = (badge_diameter / 8).max(2);
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

/// Render a 5×7 bitmap letter centered at `(cx, cy)`, scaled to fit inside
/// `inner_r` (the inner accent circle's radius). Pixels are drawn directly,
/// no anti-aliasing — at 24 px and above the letter still reads clearly.
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

/// 5×7 bitmap glyph rows (MSB-first within each 5-bit row). Adding more
/// letters here extends what envs we can label — but we currently only need
/// "D" and "P", matching the Android badge contract.
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
        // Pixel at the badge center (offset from bottom-right by ~inset+r)
        // should now be the accent color rather than the original black.
        let center = *img.get_pixel(95, 95);
        assert_ne!(center, Rgba([0, 0, 0, 0xFF]));
        // Upper-left should be untouched.
        assert_eq!(*img.get_pixel(10, 10), Rgba([0, 0, 0, 0xFF]));
    }
}
