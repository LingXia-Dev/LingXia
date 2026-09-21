//! Apple host-env launcher-icon overlay.
//!
//! Mirrors the Android `prepare_res_overlay` flow: when the active
//! env is `dev`, build a parallel `Assets.xcassets` under
//! `<target>/lingxia/<platform>/overlay/<env>/Resources/` whose `AppIcon.appiconset`
//! has each PNG composited with the shared Android-matching badge (filled
//! circle + vector "D"). The build then points `actool` at the staging
//! resources dir so the source asset catalog is never mutated and
//! dev/prod can be installed side by side and visually distinguished
//! on the home screen.

use anyhow::{Context, Result};
use image::ImageFormat;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::AppEnv;
use crate::platform::env_badge::{composite_corner_badge, env_badge};

/// If the active env needs a badge, stage a copy of `Assets.xcassets` with a
/// badged `AppIcon.appiconset` and return the staging *resources_dir*; the
/// caller should pass that to `compile_asset_catalog` instead of the source
/// dir. Returns `None` when no badge applies (release, or no source catalog).
///
/// `opaque`: iOS icons are square and must carry no alpha; macOS icons are
/// the rounded square itself and must keep it, or the corners come out black.
/// Placement is the shared corner seat on the visible plate — iOS's full-bleed
/// square uses the 22% squircle corner, macOS uses the artwork's alpha plate.
pub fn prepare_overlay_resources_dir(
    staging_base: &Path,
    resources_dir: &Path,
    env: AppEnv,
    opaque: bool,
) -> Result<Option<PathBuf>> {
    let Some((letter, accent)) = env_badge(env) else {
        return Ok(None);
    };
    let original_xcassets = resources_dir.join("Assets.xcassets");
    let original_appicon = original_xcassets.join("AppIcon.appiconset");
    if !original_appicon.exists() {
        return Ok(None);
    }

    let staging_root = staging_base.join("overlay").join(env.as_str());
    let staging_resources = staging_root.join("Resources");
    let staging_xcassets = staging_resources.join("Assets.xcassets");
    if staging_root.exists() {
        fs::remove_dir_all(&staging_root)
            .with_context(|| format!("Failed to clean {}", staging_root.display()))?;
    }
    copy_dir_recursive(&original_xcassets, &staging_xcassets)?;

    let staging_appicon = staging_xcassets.join("AppIcon.appiconset");
    badge_appiconset(&staging_appicon, letter, accent, opaque)?;

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

fn badge_appiconset(dir: &Path, letter: char, accent: [u8; 4], opaque: bool) -> Result<()> {
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
        // Seat on the visible plate's 22% corner — iOS squircles that
        // corner, macOS already drew it in alpha. The previous Inside
        // inset floated the badge off the mask and looked unlike Android.
        composite_corner_badge(&mut rgba, letter, accent);
        // iOS app icons must be opaque: an alpha channel — even a fully
        // opaque one — makes the home screen composite the icon over black,
        // which reads as a ghosted tile. macOS is the opposite: the icon is
        // the rounded square, and flattening fills its corners with black.
        if opaque {
            image::DynamicImage::ImageRgba8(rgba)
                .to_rgb8()
                .save_with_format(&path, ImageFormat::Png)
        } else {
            rgba.save_with_format(&path, ImageFormat::Png)
        }
        .with_context(|| format!("Failed to write {}", path.display()))?;
    }
    Ok(())
}
