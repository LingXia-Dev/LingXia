//! HarmonyOS host-env launcher-icon overlay.
//!
//! Harmony layered icons are a 1024-style canvas (foreground + background)
//! that the launcher masks, same idea as Android's 108dp adaptive icon.
//! Stage the D badge onto the mirrored `foreground.png` / `startIcon.png`
//! with Android's 20dp / 18dp fractions so the circle sits on the mask
//! edge. Source media is never mutated — this runs inside the per-env
//! staging tree.

use anyhow::{Context, Result};
use std::path::Path;

use crate::config::AppEnv;
use crate::platform::env_badge::{composite_android_canvas_badge, env_badge};

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
    badge_android_canvas(&foreground, letter, accent)?;
    let start_icon = dir.join("startIcon.png");
    if start_icon.is_file() {
        badge_android_canvas(&start_icon, letter, accent)?;
    }
    Ok(true)
}

fn badge_android_canvas(path: &Path, letter: char, accent: [u8; 4]) -> Result<()> {
    let img = image::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
    let mut rgba = img.to_rgba8();
    if rgba.width() < 60 {
        return Ok(());
    }
    composite_android_canvas_badge(&mut rgba, letter, accent);
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
    fn dev_badges_foreground_and_start_icon() {
        let tmp = TempDir::new().unwrap();
        let media = tmp.path().join("AppScope/resources/base/media");
        let fg = media.join("foreground.png");
        let start = media.join("startIcon.png");
        write_png(&fg, 216, [0x20, 0x40, 0x80, 0xFF]);
        write_png(&start, 216, [0x20, 0x40, 0x80, 0xFF]);
        let fg_before = fs::read(&fg).unwrap();

        assert!(badge_staged_icons(tmp.path(), AppEnv::Dev).unwrap());
        assert_ne!(fs::read(&fg).unwrap(), fg_before);
        let decoded = image::open(&fg).unwrap().to_rgba8();
        // 216 = 2×108: badge center is at 216 - 36 - 20 = 160.
        let center = decoded.get_pixel(160, 160).0;
        assert!(
            center[0] > 0xA0,
            "expected accent red at badge center, got {center:?}"
        );
        assert_eq!(decoded.get_pixel(8, 8), &Rgba([0x20, 0x40, 0x80, 0xFF]));
    }
}
