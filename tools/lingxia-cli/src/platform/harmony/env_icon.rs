//! HarmonyOS host-env launcher-icon overlay.
//!
//! Harmony's launcher uses the full layered-icon canvas as its tile. Android's
//! 108dp/72dp crop inset does not apply. Badge the staged foreground and start
//! icon against the full canvas, independent of the foreground artwork's
//! transparent margins. Source media is never mutated.

use anyhow::{Context, Result};
use std::path::Path;

use crate::config::AppEnv;
use crate::platform::env_badge::{composite_canvas_corner_badge, env_badge};

/// Badge staged Harmony media icons for `env`. Returns whether any file
/// was rewritten.
pub fn badge_staged_icons(staging: &Path, env: AppEnv) -> Result<bool> {
    let Some((letter, accent)) = env_badge(env) else {
        return Ok(false);
    };
    let mut any = false;
    for media in [
        staging.join("AppScope/resources/base/media"),
        staging.join("entry/src/main/resources/base/media"),
    ] {
        any |= badge_layered_media(&media, letter, accent)?;
    }
    Ok(any)
}

fn badge_layered_media(dir: &Path, letter: char, accent: [u8; 4]) -> Result<bool> {
    let foreground = dir.join("foreground.png");
    if !foreground.is_file() {
        return Ok(false);
    }
    badge_canvas_corner(&foreground, letter, accent)?;
    let start_icon = dir.join("startIcon.png");
    if start_icon.is_file() {
        badge_canvas_corner(&start_icon, letter, accent)?;
    }
    Ok(true)
}

fn badge_canvas_corner(path: &Path, letter: char, accent: [u8; 4]) -> Result<()> {
    let img = image::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
    let mut rgba = img.to_rgba8();
    if rgba.width() < 60 {
        return Ok(());
    }
    composite_canvas_corner_badge(&mut rgba, letter, accent);
    rgba.save_with_format(path, image::ImageFormat::Png)
        .with_context(|| format!("Failed to write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::badge_staged_icons;
    use crate::config::AppEnv;
    use image::{ImageFormat, Rgba, RgbaImage};
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    fn write_png(path: &Path, size: u32, color: [u8; 4]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let img = RgbaImage::from_pixel(size, size, Rgba(color));
        img.save_with_format(path, ImageFormat::Png).unwrap();
    }

    #[test]
    fn prod_leaves_media_intact() {
        let tmp = TempDir::new().unwrap();
        let fg = tmp
            .path()
            .join("AppScope/resources/base/media/foreground.png");
        write_png(&fg, 128, [0x20, 0x40, 0x80, 0xFF]);
        let before = fs::read(&fg).unwrap();
        assert!(!badge_staged_icons(tmp.path(), AppEnv::Prod).unwrap());
        assert_eq!(fs::read(&fg).unwrap(), before);
    }

    #[test]
    fn dev_badge_uses_the_tile_corner_for_transparent_and_opaque_layers() {
        let tmp = TempDir::new().unwrap();
        for scope in [
            "AppScope/resources/base/media",
            "entry/src/main/resources/base/media",
        ] {
            let media = tmp.path().join(scope);
            let fg = media.join("foreground.png");
            let start = media.join("startIcon.png");
            write_png(&fg, 1024, [0, 0, 0, 0]);
            // Small, offset artwork must not pull the badge away from the tile corner.
            let mut artwork = image::open(&fg).unwrap().to_rgba8();
            for y in 300..600 {
                for x in 200..500 {
                    artwork.put_pixel(x, y, Rgba([0x20, 0x40, 0x80, 0xFF]));
                }
            }
            artwork.save(&fg).unwrap();
            write_png(&start, 256, [0x20, 0x40, 0x80, 0xFF]);
        }
        assert!(badge_staged_icons(tmp.path(), AppEnv::Dev).unwrap());
        for scope in [
            "AppScope/resources/base/media",
            "entry/src/main/resources/base/media",
        ] {
            let media = tmp.path().join(scope);
            for name in ["foreground.png", "startIcon.png"] {
                let img = image::open(media.join(name)).unwrap().to_rgba8();
                // The old Android inset stopped at 83% of the tile. The
                // corrected badge must reach its lower/right corner instead.
                let n = img.width();
                for (x, y) in [(0.93, 0.82), (0.82, 0.93)] {
                    let pixel = img
                        .get_pixel((n as f32 * x) as u32, (n as f32 * y) as u32)
                        .0;
                    assert!(
                        pixel[0] > 0xA0 && pixel[1] < 0x60 && pixel[3] > 0xF0,
                        "{scope}/{name}: expected red near tile edge, got {pixel:?}"
                    );
                }
                assert_eq!(
                    img.get_pixel(n - 1, n - 1).0,
                    if name == "foreground.png" {
                        [0, 0, 0, 0]
                    } else {
                        [0x20, 0x40, 0x80, 0xFF]
                    }
                );
            }
            let foreground = image::open(media.join("foreground.png"))
                .unwrap()
                .to_rgba8();
            assert_eq!(foreground.get_pixel(300, 400).0, [0x20, 0x40, 0x80, 0xFF]);
            assert_eq!(foreground.get_pixel(8, 8).0, [0, 0, 0, 0]);
        }
    }
}
