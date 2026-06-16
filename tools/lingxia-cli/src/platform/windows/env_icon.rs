//! Windows env-version launcher-icon overlay.
//!
//! Windows hosts load a root `assets/AppIcon.png` when present, falling back to
//! the home lxapp public icon. Dev/preview badging must follow that same
//! runtime lookup while avoiding mutations to live prepared assets.

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::EnvVersion;
use crate::platform::env_badge::{badge_png_file, env_badge};

/// Resolve the launcher icon `lingxia-windows-sdk` loads at runtime from a
/// prepared assets dir: `<assets>/AppIcon.png` first, then
/// `<assets>/<home_app_id>/public/AppIcon.png`.
pub fn resolve_windows_app_icon(assets_dir: &Path, home_app_id: Option<&str>) -> Option<PathBuf> {
    let root_icon = assets_dir.join("AppIcon.png");
    if root_icon.is_file() {
        return Some(root_icon);
    }
    if let Some(id) = home_app_id.map(str::trim).filter(|id| !id.is_empty()) {
        let home_icon = assets_dir.join(id).join("public").join("AppIcon.png");
        if home_icon.is_file() {
            return Some(home_icon);
        }
    }
    None
}

/// Stage a badged host launcher icon in the assembled dist (the `lingxia build`
/// path). The host window/taskbar icon must stay distinct from the lxapp's
/// served `<home>/public/AppIcon.png`, which is app content.
pub fn stage_dist_host_icon(
    assets_dir: &Path,
    home_app_id: Option<&str>,
    version: EnvVersion,
) -> Result<bool> {
    if env_badge(version).is_none() {
        return Ok(false);
    }
    let Some(src) = resolve_windows_app_icon(assets_dir, home_app_id) else {
        return Ok(false);
    };
    let host_icon = assets_dir.join("AppIcon.png");
    if src != host_icon {
        fs::copy(&src, &host_icon).with_context(|| {
            format!(
                "Failed to copy {} -> {}",
                src.display(),
                host_icon.display()
            )
        })?;
    }
    badge_png_file(&host_icon, version)
}

/// Stage a badged copy of the launcher icon for `lingxia dev`, returning the
/// staged path (which the caller points `LINGXIA_APP_ICON_PATH` at).
pub fn stage_dev_badged_icon(
    assets_dir: &Path,
    home_app_id: Option<&str>,
    overlay_dir: &Path,
    version: EnvVersion,
) -> Result<Option<PathBuf>> {
    if env_badge(version).is_none() {
        return Ok(None);
    }
    let Some(src) = resolve_windows_app_icon(assets_dir, home_app_id) else {
        return Ok(None);
    };
    fs::create_dir_all(overlay_dir)
        .with_context(|| format!("Failed to create {}", overlay_dir.display()))?;
    let dest = overlay_dir.join("AppIcon.png");
    fs::copy(&src, &dest)
        .with_context(|| format!("Failed to copy {} -> {}", src.display(), dest.display()))?;
    badge_png_file(&dest, version)?;
    Ok(Some(dest))
}

#[cfg(test)]
mod tests {
    use super::{resolve_windows_app_icon, stage_dev_badged_icon, stage_dist_host_icon};
    use crate::config::EnvVersion;
    use image::{ImageFormat, Rgba, RgbaImage};
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    fn write_png_color(path: &Path, size: u32, color: [u8; 4]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let img = RgbaImage::from_pixel(size, size, Rgba(color));
        img.save_with_format(path, ImageFormat::Png).unwrap();
    }

    fn write_png(path: &Path, size: u32) {
        write_png_color(path, size, [0x20, 0x40, 0x80, 0xFF]);
    }

    #[test]
    fn resolve_windows_app_icon_prefers_root_then_home() {
        let tmp = TempDir::new().unwrap();
        let assets = tmp.path();

        assert_eq!(resolve_windows_app_icon(assets, Some("home")), None);

        let home = assets.join("home").join("public").join("AppIcon.png");
        write_png(&home, 64);
        assert_eq!(resolve_windows_app_icon(assets, Some("home")), Some(home));

        let root = assets.join("AppIcon.png");
        write_png(&root, 64);
        assert_eq!(resolve_windows_app_icon(assets, Some("home")), Some(root));
    }

    #[test]
    fn stage_dist_host_icon_badges_host_copy_and_leaves_served_content_intact() {
        let tmp = TempDir::new().unwrap();
        let assets = tmp.path();
        let content = assets
            .join("lingxia-showcase")
            .join("public")
            .join("AppIcon.png");
        write_png(&content, 256);
        let content_before = fs::read(&content).unwrap();
        let host_icon = assets.join("AppIcon.png");

        assert!(
            !stage_dist_host_icon(assets, Some("lingxia-showcase"), EnvVersion::Release).unwrap()
        );
        assert!(!host_icon.exists());
        assert_eq!(fs::read(&content).unwrap(), content_before);

        assert!(
            stage_dist_host_icon(assets, Some("lingxia-showcase"), EnvVersion::Developer).unwrap()
        );
        assert!(host_icon.is_file());
        assert_eq!(fs::read(&content).unwrap(), content_before);
        assert_ne!(fs::read(&host_icon).unwrap(), content_before);
    }

    #[test]
    fn stage_dist_host_icon_preserves_existing_root_icon_source() {
        let tmp = TempDir::new().unwrap();
        let assets = tmp.path();
        let content = assets.join("home").join("public").join("AppIcon.png");
        let host_icon = assets.join("AppIcon.png");
        write_png_color(&content, 256, [0xE0, 0x10, 0x10, 0xFF]);
        write_png_color(&host_icon, 256, [0x10, 0x40, 0xE0, 0xFF]);

        let content_before = fs::read(&content).unwrap();
        let host_before = fs::read(&host_icon).unwrap();
        assert!(stage_dist_host_icon(assets, Some("home"), EnvVersion::Developer).unwrap());

        assert_eq!(fs::read(&content).unwrap(), content_before);
        let host_after = fs::read(&host_icon).unwrap();
        assert_ne!(host_after, host_before);
        assert_ne!(host_after, content_before);

        let decoded = image::open(&host_icon).unwrap().to_rgba8();
        assert_eq!(decoded.get_pixel(0, 0), &Rgba([0x10, 0x40, 0xE0, 0xFF]));
    }

    #[test]
    fn stage_dev_badged_icon_badges_a_copy_and_leaves_source_intact() {
        let tmp = TempDir::new().unwrap();
        let assets = tmp.path().join("assets");
        let src = assets.join("home").join("public").join("AppIcon.png");
        write_png(&src, 256);
        let src_before = fs::read(&src).unwrap();
        let overlay = tmp.path().join("overlay").join("developer");

        assert!(
            stage_dev_badged_icon(&assets, Some("home"), &overlay, EnvVersion::Release)
                .unwrap()
                .is_none()
        );

        let staged = stage_dev_badged_icon(&assets, Some("home"), &overlay, EnvVersion::Developer)
            .unwrap()
            .expect("developer env stages a badged icon");
        assert!(staged.is_file());
        assert_eq!(fs::read(&src).unwrap(), src_before);
        assert_ne!(fs::read(&staged).unwrap(), src_before);
    }
}
