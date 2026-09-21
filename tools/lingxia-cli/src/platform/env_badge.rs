//! Cross-platform host-env icon badge drawing.
//!
//! Dev builds get a small `D` badge composited onto the launcher icon so they
//! can be distinguished from prod. Appearance matches the Android adaptive
//! overlay: a red circle, white ring, and the same even-odd vector letter.
//! This module only owns that shared badge; platform modules decide which
//! icon copy to badge and where to stage it.

use image::{RgbaImage, imageops};

use crate::config::AppEnv;

/// Letter + accent color for the env's badge, or `None` for prod.
pub fn env_badge(version: AppEnv) -> Option<(char, [u8; 4])> {
    match version {
        // Match the Android accent (#D32F2F) so a dev build is obvious.
        AppEnv::Dev => Some(('D', [0xD3, 0x2F, 0x2F, 0xFF])),
        AppEnv::Prod => None,
    }
}

/// Android adaptive overlay: 20dp circle, 18dp from the 108dp canvas edges.
/// Seats the badge on the typical 72dp launcher-mask edge.
pub const ANDROID_CANVAS_DP: f32 = 108.0;
pub const ANDROID_BADGE_DP: f32 = 20.0;
pub const ANDROID_BADGE_INSET_DP: f32 = 18.0;

/// Vector path data shared with the Android `<vector>` overlay (42×42 viewport).
pub fn badge_letter_path(letter: char) -> Option<&'static str> {
    match letter {
        'D' => Some(
            "M12,10 L22,10 C29,10 34,15 34,21 C34,27 29,32 22,32 L12,32 Z M18,16 L18,26 L22,26 C25.5,26 28,24 28,21 C28,18 25.5,16 22,16 Z",
        ),
        'P' => Some(
            "M13,10 L25,10 C30,10 34,14 34,19 C34,24 30,28 25,28 L19,28 L19,32 L13,32 Z M19,16 L19,22 L24,22 C26.5,22 28,20.8 28,19 C28,17.2 26.5,16 24,16 Z",
        ),
        _ => None,
    }
}

/// Badge a desktop (or iOS full-bleed) icon on the corner of its visible
/// plate, measured from alpha so it follows whatever margin the icon
/// pipeline left.
pub fn composite_corner_badge(img: &mut RgbaImage, letter: char, accent: [u8; 4]) {
    let plate = opaque_plate(img).unwrap_or((0, 0, img.width() as i32, img.height() as i32));
    composite_badge_in_plate(img, letter, accent, plate);
}

/// Badge a layered-icon canvas the way Android's overlay does: 20/108 of
/// the canvas, 18/108 in from the edges, so the circle sits on the launcher
/// mask rather than dominating the 108-style plate.
pub fn composite_android_canvas_badge(img: &mut RgbaImage, letter: char, accent: [u8; 4]) {
    let (w, h) = img.dimensions();
    let canvas = w.min(h) as f32;
    let diameter = (canvas * ANDROID_BADGE_DP / ANDROID_CANVAS_DP)
        .round()
        .max(1.0) as i32;
    let inset = (canvas * ANDROID_BADGE_INSET_DP / ANDROID_CANVAS_DP).round() as i32;
    let center_x = w as i32 - inset - diameter / 2;
    let center_y = h as i32 - inset - diameter / 2;
    blit_badge(img, letter, accent, diameter, center_x, center_y);
}

/// Bounding rect of the pixels that are clearly part of the plate.
fn opaque_plate(img: &RgbaImage) -> Option<(i32, i32, i32, i32)> {
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (u32::MAX, u32::MAX, 0u32, 0u32);
    for (x, y, pixel) in img.enumerate_pixels() {
        if pixel.0[3] <= 12 {
            continue;
        }
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }
    (min_x <= max_x).then(|| {
        (
            min_x as i32,
            min_y as i32,
            (max_x - min_x + 1) as i32,
            (max_y - min_y + 1) as i32,
        )
    })
}

/// Badge anchored to an explicit plate rect `(left, top, width, height)`.
///
/// Sits just inside the plate's 22% corner arc (tangent from within, plus a
/// hairline gap so the white ring never meets the edge). iOS squircles this
/// same corner, so the badge lands on the mask the way Android's 20dp/18dp
/// overlay sits on the adaptive mask.
fn composite_badge_in_plate(
    img: &mut RgbaImage,
    letter: char,
    accent: [u8; 4],
    plate: (i32, i32, i32, i32),
) {
    let (left, top, plate_w, plate_h) = plate;
    let artwork = plate_w.min(plate_h).max(1);
    let badge_diameter = ((artwork as f32) * 0.30).round() as i32;
    let corner_r = artwork as f32 * 0.22;
    let badge_r = badge_diameter as f32 / 2.0;
    let gap = (artwork as f32 * 0.012).max(1.0);
    let edge = corner_r - (corner_r - badge_r - gap) / std::f32::consts::SQRT_2;
    let edge = edge.round() as i32;
    let center_x = left + plate_w - edge;
    let center_y = top + plate_h - edge;
    blit_badge(img, letter, accent, badge_diameter, center_x, center_y);
}

fn blit_badge(
    img: &mut RgbaImage,
    letter: char,
    accent: [u8; 4],
    diameter: i32,
    center_x: i32,
    center_y: i32,
) {
    let diameter = diameter.max(1) as u32;
    let sprite = render_badge_sprite(letter, accent, diameter);
    let left = center_x - sprite.width() as i32 / 2;
    let top = center_y - sprite.height() as i32 / 2;
    imageops::overlay(img, &sprite, left as i64, top as i64);
}

/// Rasterize the Android vector badge. Small diameters render at 2× and
/// downscale so the letter stays smooth on 60pt home-screen tiles.
fn render_badge_sprite(letter: char, accent: [u8; 4], diameter: u32) -> RgbaImage {
    let render_d = if diameter < 64 {
        diameter.saturating_mul(2).max(1)
    } else {
        diameter
    };
    let svg = badge_svg(letter, accent);
    let png = crate::r#gen::icons::svg_to_png_bytes(&svg, render_d)
        .expect("static env-badge SVG must render");
    let sprite = image::load_from_memory(&png)
        .expect("resvg PNG must decode")
        .to_rgba8();
    if render_d == diameter {
        sprite
    } else {
        imageops::resize(&sprite, diameter, diameter, imageops::FilterType::Lanczos3)
    }
}

fn badge_svg(letter: char, accent: [u8; 4]) -> String {
    let path = badge_letter_path(letter).unwrap_or("");
    // 20×20 matches the Android overlay item. The circle's 2-unit centered
    // stroke is the same 2dp white ring; the letter lives in the 42-unit
    // vector viewport and is scaled onto that 20×20 plate.
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20">
  <circle cx="10" cy="10" r="9" fill="#{r:02X}{g:02X}{b:02X}" stroke="#FFFFFF" stroke-width="2"/>
  <g transform="scale({scale})">
    <path fill="#FFFFFF" fill-rule="evenodd" d="{path}"/>
  </g>
</svg>"##,
        r = accent[0],
        g = accent[1],
        b = accent[2],
        scale = 20.0 / 42.0,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        ANDROID_BADGE_DP, ANDROID_BADGE_INSET_DP, ANDROID_CANVAS_DP, badge_letter_path,
        composite_android_canvas_badge, composite_badge_in_plate, composite_corner_badge,
        env_badge, render_badge_sprite,
    };
    use crate::config::AppEnv;
    use image::{Rgba, RgbaImage};

    #[test]
    fn prod_has_no_badge() {
        assert!(env_badge(AppEnv::Prod).is_none());
        assert!(env_badge(AppEnv::Dev).is_some());
    }

    #[test]
    fn path_table_covers_required_letters() {
        assert!(badge_letter_path('D').is_some());
        assert!(badge_letter_path('P').is_some());
        assert!(badge_letter_path('X').is_none());
    }

    #[test]
    fn sprite_is_a_smooth_circle_with_the_android_letter() {
        let sprite = render_badge_sprite('D', [0xD3, 0x2F, 0x2F, 0xFF], 80);
        assert_eq!(sprite.dimensions(), (80, 80));
        // Corners of the sprite square stay empty — it's a circle, not a disc tile.
        assert_eq!(sprite.get_pixel(0, 0).0[3], 0);
        assert_eq!(sprite.get_pixel(79, 0).0[3], 0);
        // Center is the accent fill (inside the D's bowl / the circle).
        let center = sprite.get_pixel(40, 40).0;
        assert!(
            center[0] > 0xA0,
            "center should be the red accent, got {center:?}"
        );
        assert!(center[3] > 0xF0);
        // White ink exists (ring and/or the D) and is not a 5×7 block grid:
        // a curved letter produces more than a handful of near-white pixels.
        let white = sprite
            .pixels()
            .filter(|p| p.0[0] > 0xE0 && p.0[1] > 0xE0 && p.0[2] > 0xE0 && p.0[3] > 0xE0)
            .count();
        assert!(
            white > 80,
            "expected a filled vector D + ring, got {white} white pixels"
        );
    }

    #[test]
    fn composite_badge_modifies_bottom_right_pixels() {
        let mut img = RgbaImage::from_pixel(120, 120, Rgba([0, 0, 0, 0xFF]));
        composite_badge_in_plate(&mut img, 'D', [0xD3, 0x2F, 0x2F, 0xFF], (0, 0, 120, 120));
        // Pixel at the badge center should now be the accent color rather than
        // the original black.
        let center = *img.get_pixel(100, 100);
        assert_ne!(center, Rgba([0, 0, 0, 0xFF]));
        // Upper-left should be untouched.
        assert_eq!(*img.get_pixel(10, 10), Rgba([0, 0, 0, 0xFF]));
    }

    #[test]
    fn corner_badge_stays_inside_the_plate_corner() {
        // 100px opaque plate inside a 20px transparent margin. A 30px badge
        // sits in the rounded corner, near both edges, never past them.
        let mut img = RgbaImage::from_pixel(140, 140, Rgba([0, 0, 0, 0]));
        for y in 20..120 {
            for x in 20..120 {
                img.put_pixel(x, y, Rgba([40, 40, 40, 0xFF]));
            }
        }
        composite_corner_badge(&mut img, 'D', [0xD3, 0x2F, 0x2F, 0xFF]);
        let plate = Rgba([40, 40, 40, 0xFF]);
        // Nothing lands in the transparent margin.
        for i in 0..140 {
            for j in 120..140 {
                assert_eq!(img.get_pixel(i, j).0[3], 0, "below the plate at {i},{j}");
                assert_eq!(img.get_pixel(j, i).0[3], 0, "right of the plate at {j},{i}");
            }
        }
        // The badge reaches close to both edges.
        assert_ne!(
            *img.get_pixel(103, 116),
            plate,
            "badge near the bottom edge"
        );
        assert_ne!(*img.get_pixel(116, 103), plate, "badge near the right edge");
        // The rounded corner itself stays plate.
        assert_eq!(*img.get_pixel(118, 118), plate);
        assert_eq!(*img.get_pixel(40, 40), plate);
    }

    #[test]
    fn ios_full_bleed_corner_sits_on_the_squircle_not_inward() {
        // 180px = iPhone home-screen @3x. Inside placement used to park the
        // badge ~12% in from the edge; Corner should put the outer ring
        // within a few percent of the 22% squircle.
        let mut img = RgbaImage::from_pixel(180, 180, Rgba([20, 40, 80, 0xFF]));
        composite_corner_badge(&mut img, 'D', [0xD3, 0x2F, 0x2F, 0xFF]);
        let plate = Rgba([20, 40, 80, 0xFF]);
        // Extreme corner stays plate (the squircle cuts this away anyway).
        assert_eq!(*img.get_pixel(178, 178), plate);
        // Ring reaches the lower-right quadrant near the edge.
        let near_edge = *img.get_pixel(168, 150);
        assert_ne!(
            near_edge, plate,
            "badge should sit on the iOS corner, got {near_edge:?}"
        );
    }

    #[test]
    fn android_canvas_badge_uses_the_20_over_108_overlay() {
        let mut img = RgbaImage::from_pixel(108, 108, Rgba([10, 10, 10, 0xFF]));
        composite_android_canvas_badge(&mut img, 'D', [0xD3, 0x2F, 0x2F, 0xFF]);
        let diameter = (108.0 * ANDROID_BADGE_DP / ANDROID_CANVAS_DP).round() as i32;
        let inset = (108.0 * ANDROID_BADGE_INSET_DP / ANDROID_CANVAS_DP).round() as i32;
        assert_eq!(diameter, 20);
        assert_eq!(inset, 18);
        let cx = (108 - inset - diameter / 2) as u32;
        let cy = cx;
        let center = img.get_pixel(cx, cy).0;
        assert!(
            center[0] > 0xA0,
            "badge center should be accent red, got {center:?}"
        );
        assert_eq!(*img.get_pixel(4, 4), Rgba([10, 10, 10, 0xFF]));
    }
}
