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
use image::ImageFormat;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::EnvVersion;
use crate::platform::env_badge::{composite_badge, env_badge};

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
