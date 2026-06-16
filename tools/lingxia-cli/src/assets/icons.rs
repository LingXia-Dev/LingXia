use crate::config::LingXiaConfig;
use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use include_dir::{Dir, DirEntry, include_dir};
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use super::hash::sha256_hex;
use super::sync::write_if_changed;
use super::ui::{TERMINAL_ICON_SOURCE, effective_ui_config};

const APP_UI_ICON_DIR: &str = "icons";
const WINDOWS_DESIGN_ICON_DIR: &str = "icons/design";
const WINDOWS_DESIGN_ICON_PNG_SIZE: u32 = 64;
const MAX_APP_UI_ICON_BYTES: u64 = 512 * 1024;
const MIN_APP_UI_ICON_SIZE: f32 = 16.0;
const MAX_APP_UI_ICON_SIZE: f32 = 512.0;
const WINDOWS_APP_UI_ICON_PNG_SIZE: u32 = 64;
const TERMINAL_ICON_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="0 0 64 64"><rect x="7" y="11" width="50" height="42" rx="8" fill="none" stroke="#000000" stroke-width="5"/><path d="M17 25l8 7-8 7" fill="none" stroke="#000000" stroke-width="5" stroke-linecap="round" stroke-linejoin="round"/><path d="M31 40h15" fill="none" stroke="#000000" stroke-width="5" stroke-linecap="round"/></svg>"##;

static WINDOWS_DESIGN_ICON_SVGS: Dir<'_> =
    include_dir!("$CARGO_MANIFEST_DIR/assets/design-icons/svg");

#[derive(Clone, Debug)]
pub(super) struct PreparedAppUiIcon {
    pub(super) relative_path: String,
    pub(super) windows_relative_path: String,
    pub(super) source_path: String,
    pub(super) bytes: Vec<u8>,
    pub(super) windows_bytes: Vec<u8>,
    pub(super) hash: String,
    pub(super) windows_hash: String,
}

#[derive(Clone, Debug)]
pub(super) struct PreparedWindowsDesignIcon {
    pub(super) relative_path: String,
    pub(super) source_path: String,
    pub(super) bytes: Vec<u8>,
    pub(super) hash: String,
}

pub(super) fn prepare_app_ui_icons(
    project_root: &Path,
    config: &LingXiaConfig,
) -> Result<Vec<PreparedAppUiIcon>> {
    let Some(ui) = effective_ui_config(config)? else {
        return Ok(Vec::new());
    };

    let mut icon_sources = Vec::new();
    collect_app_ui_icon_sources(&ui, &mut icon_sources)?;
    icon_sources.sort();
    icon_sources.dedup();

    let mut prepared = Vec::new();
    for source in icon_sources {
        if source == TERMINAL_ICON_SOURCE {
            prepared.push(prepare_builtin_terminal_icon()?);
            continue;
        }
        let source_path = resolve_project_relative_file(project_root, &source)
            .with_context(|| format!("Invalid ui activator icon '{}'", source))?;
        if !source_path.exists() {
            return Err(anyhow!(
                "UI activator icon not found: {}",
                source_path.display()
            ));
        }
        let ext = source_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or_default();
        if !ext.eq_ignore_ascii_case("svg") {
            return Err(anyhow!(
                "Invalid ui activator icon '{}': only SVG source icons are supported",
                source
            ));
        }
        let metadata = fs::metadata(&source_path)
            .with_context(|| format!("Failed to stat icon {}", source_path.display()))?;
        if metadata.len() > MAX_APP_UI_ICON_BYTES {
            return Err(anyhow!(
                "Invalid ui activator icon '{}': file is {} bytes, max is {} bytes",
                source,
                metadata.len(),
                MAX_APP_UI_ICON_BYTES
            ));
        }

        let svg = fs::read_to_string(&source_path)
            .with_context(|| format!("Failed to read SVG icon {}", source_path.display()))?;
        validate_app_ui_svg_icon(&source, &svg)?;
        let pdf = crate::r#gen::icons::svg_to_pdf_bytes(&svg)
            .with_context(|| format!("Failed to convert SVG icon '{}' to PDF", source))?;
        let windows_png = crate::r#gen::icons::svg_to_png_bytes(&svg, WINDOWS_APP_UI_ICON_PNG_SIZE)
            .with_context(|| format!("Failed to convert SVG icon '{}' to Windows PNG", source))?;
        let hash = sha256_hex(&pdf);
        let windows_hash = sha256_hex(&windows_png);
        let stem = source_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(sanitize_asset_stem)
            .filter(|stem| !stem.is_empty())
            .unwrap_or_else(|| "icon".to_string());
        let relative_path = format!("{}/{}-{}.pdf", APP_UI_ICON_DIR, stem, &hash[..12]);
        let windows_relative_path =
            format!("{}/{}-{}.png", APP_UI_ICON_DIR, stem, &windows_hash[..12]);

        prepared.push(PreparedAppUiIcon {
            relative_path,
            windows_relative_path,
            source_path: source,
            bytes: pdf,
            windows_bytes: windows_png,
            hash,
            windows_hash,
        });
    }

    Ok(prepared)
}

fn prepare_builtin_terminal_icon() -> Result<PreparedAppUiIcon> {
    validate_app_ui_svg_icon(TERMINAL_ICON_SOURCE, TERMINAL_ICON_SVG)?;
    let pdf = crate::r#gen::icons::svg_to_pdf_bytes(TERMINAL_ICON_SVG)
        .with_context(|| "Failed to convert built-in terminal icon to PDF")?;
    let windows_png =
        crate::r#gen::icons::svg_to_png_bytes(TERMINAL_ICON_SVG, WINDOWS_APP_UI_ICON_PNG_SIZE)
            .with_context(|| "Failed to convert built-in terminal icon to Windows PNG")?;
    let hash = sha256_hex(&pdf);
    let windows_hash = sha256_hex(&windows_png);
    Ok(PreparedAppUiIcon {
        relative_path: format!("{}/terminal-{}.pdf", APP_UI_ICON_DIR, &hash[..12]),
        windows_relative_path: format!("{}/terminal-{}.png", APP_UI_ICON_DIR, &windows_hash[..12]),
        source_path: TERMINAL_ICON_SOURCE.to_string(),
        bytes: pdf,
        windows_bytes: windows_png,
        hash,
        windows_hash,
    })
}

pub(super) fn rewrite_app_ui_icon_paths(
    ui: &mut serde_json::Value,
    by_source: &HashMap<&str, &str>,
) -> Result<()> {
    let Some(activators) = ui
        .get_mut("activators")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return Ok(());
    };

    for (index, activator) in activators.iter_mut().enumerate() {
        let Some(icon) = activator.get_mut("icon") else {
            continue;
        };
        let source = icon
            .as_str()
            .ok_or_else(|| anyhow!("ui.activators[{index}].icon must be a string"))?;
        let generated = by_source
            .get(source)
            .ok_or_else(|| anyhow!("Internal error: icon '{}' was not prepared", source))?;
        *icon = serde_json::Value::String((*generated).to_string());
    }
    Ok(())
}

pub(super) fn rewrite_windows_app_ui_icon_paths(
    ui: &mut serde_json::Value,
    by_source: &HashMap<&str, &str>,
) -> Result<()> {
    rewrite_app_ui_icon_paths(ui, by_source)
}

pub(super) fn app_ui_icon_hashes(icons: &[PreparedAppUiIcon]) -> BTreeMap<String, String> {
    icons
        .iter()
        .map(|icon| (icon.relative_path.clone(), icon.hash.clone()))
        .collect()
}

pub(super) fn windows_app_ui_icon_hashes(icons: &[PreparedAppUiIcon]) -> BTreeMap<String, String> {
    icons
        .iter()
        .map(|icon| {
            (
                icon.windows_relative_path.clone(),
                icon.windows_hash.clone(),
            )
        })
        .collect()
}

pub(super) fn prepare_windows_design_icons() -> Result<Vec<PreparedWindowsDesignIcon>> {
    let mut entries: Vec<_> = WINDOWS_DESIGN_ICON_SVGS
        .entries()
        .iter()
        .filter_map(|entry| match entry {
            DirEntry::File(file)
                if file
                    .path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("svg")) =>
            {
                Some(file)
            }
            _ => None,
        })
        .collect();
    entries.sort_by_key(|file| file.path().file_name().map(|name| name.to_os_string()));

    let mut prepared = Vec::new();
    for file in entries {
        let file_stem = file
            .path()
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| anyhow!("Invalid design icon name: {}", file.path().display()))?;
        let source_path = format!("design/icons/svg/{file_stem}.svg");
        let svg = std::str::from_utf8(file.contents())
            .with_context(|| format!("Failed to read embedded design icon {source_path}"))?;
        let bytes = crate::r#gen::icons::svg_to_png_bytes(svg, WINDOWS_DESIGN_ICON_PNG_SIZE)
            .with_context(|| {
                format!("Failed to convert design icon {source_path} to Windows PNG")
            })?;
        let hash = sha256_hex(&bytes);
        prepared.push(PreparedWindowsDesignIcon {
            relative_path: format!("{WINDOWS_DESIGN_ICON_DIR}/{file_stem}.png"),
            source_path,
            bytes,
            hash,
        });
    }

    Ok(prepared)
}

pub(super) fn windows_design_icon_hashes(
    icons: &[PreparedWindowsDesignIcon],
) -> BTreeMap<String, String> {
    icons
        .iter()
        .map(|icon| (icon.relative_path.clone(), icon.hash.clone()))
        .collect()
}

pub(super) fn sync_app_ui_icons(
    target_root: &Path,
    icons: &[PreparedAppUiIcon],
    prev_hashes: Option<&BTreeMap<String, String>>,
) -> Result<bool> {
    let desired_hashes = app_ui_icon_hashes(icons);
    let mut changed = false;

    if let Some(prev_hashes) = prev_hashes {
        for prev_name in prev_hashes.keys() {
            if !desired_hashes.contains_key(prev_name) {
                let stale = target_root.join(prev_name);
                if stale.exists() {
                    fs::remove_file(&stale)
                        .with_context(|| format!("Failed to remove {}", stale.display()))?;
                    remove_empty_parent_dirs_until(target_root, stale.parent());
                    changed = true;
                }
            }
        }
    }

    for icon in icons {
        let target = target_root.join(&icon.relative_path);
        let prev_hash = prev_hashes.and_then(|hashes| hashes.get(&icon.relative_path));
        if prev_hash == Some(&icon.hash) && target.exists() {
            continue;
        }
        if write_if_changed(&target, &icon.bytes)? {
            println!(
                "  {} icon {} -> {}",
                "ok".green(),
                icon.source_path,
                target.display()
            );
            changed = true;
        }
    }

    let icon_dir = target_root.join(APP_UI_ICON_DIR);
    if icons.is_empty() && prev_hashes.is_some() && icon_dir.exists() {
        let is_empty = fs::read_dir(&icon_dir)
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(false);
        if is_empty {
            fs::remove_dir(&icon_dir)
                .with_context(|| format!("Failed to remove {}", icon_dir.display()))?;
            changed = true;
        }
    }

    Ok(changed)
}

pub(super) fn sync_windows_app_ui_icons(
    target_root: &Path,
    icons: &[PreparedAppUiIcon],
    prev_hashes: Option<&BTreeMap<String, String>>,
) -> Result<bool> {
    let desired_hashes = windows_app_ui_icon_hashes(icons);
    let mut changed = false;

    if let Some(prev_hashes) = prev_hashes {
        for prev_name in prev_hashes.keys() {
            if !desired_hashes.contains_key(prev_name) {
                let stale = target_root.join(prev_name);
                if stale.exists() {
                    fs::remove_file(&stale)
                        .with_context(|| format!("Failed to remove {}", stale.display()))?;
                    remove_empty_parent_dirs_until(target_root, stale.parent());
                    changed = true;
                }
            }
        }
    }

    for icon in icons {
        let target = target_root.join(&icon.windows_relative_path);
        let prev_hash = prev_hashes.and_then(|hashes| hashes.get(&icon.windows_relative_path));
        if prev_hash == Some(&icon.windows_hash) && target.exists() {
            continue;
        }
        if write_if_changed(&target, &icon.windows_bytes)? {
            println!(
                "  {} icon {} -> {}",
                "ok".green(),
                icon.source_path,
                target.display()
            );
            changed = true;
        }
    }

    let icon_dir = target_root.join(APP_UI_ICON_DIR);
    if icons.is_empty() && prev_hashes.is_some() && icon_dir.exists() {
        let is_empty = fs::read_dir(&icon_dir)
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(false);
        if is_empty {
            fs::remove_dir(&icon_dir)
                .with_context(|| format!("Failed to remove {}", icon_dir.display()))?;
            changed = true;
        }
    }

    Ok(changed)
}

pub(super) fn sync_windows_design_icons(
    target_root: &Path,
    icons: &[PreparedWindowsDesignIcon],
    prev_hashes: Option<&BTreeMap<String, String>>,
) -> Result<bool> {
    let desired_hashes = windows_design_icon_hashes(icons);
    let mut changed = false;

    if let Some(prev_hashes) = prev_hashes {
        for prev_name in prev_hashes.keys() {
            if !desired_hashes.contains_key(prev_name) {
                let stale = target_root.join(prev_name);
                if stale.exists() {
                    fs::remove_file(&stale)
                        .with_context(|| format!("Failed to remove {}", stale.display()))?;
                    remove_empty_parent_dirs_until(target_root, stale.parent());
                    changed = true;
                }
            }
        }
    }

    let design_icon_dir = target_root.join(WINDOWS_DESIGN_ICON_DIR);
    if design_icon_dir.exists() {
        for entry in fs::read_dir(&design_icon_dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
            {
                continue;
            }
            let Ok(relative_path) = path.strip_prefix(target_root) else {
                continue;
            };
            let relative_path = relative_path.to_string_lossy().replace('\\', "/");
            if !desired_hashes.contains_key(&relative_path) {
                fs::remove_file(&path)
                    .with_context(|| format!("Failed to remove {}", path.display()))?;
                changed = true;
            }
        }
        remove_empty_parent_dirs_until(target_root, Some(&design_icon_dir));
    }

    for icon in icons {
        let target = target_root.join(&icon.relative_path);
        let prev_hash = prev_hashes.and_then(|hashes| hashes.get(&icon.relative_path));
        if prev_hash == Some(&icon.hash) && target.exists() {
            continue;
        }
        if write_if_changed(&target, &icon.bytes)? {
            println!(
                "  {} design icon {} -> {}",
                "ok".green(),
                icon.source_path,
                target.display()
            );
            changed = true;
        }
    }

    Ok(changed)
}

pub(super) fn validate_app_ui_svg_icon(label: &str, svg: &str) -> Result<()> {
    let (width, height) = crate::r#gen::icons::svg_size(svg)
        .with_context(|| format!("Failed to parse SVG icon '{}'", label))?;
    if !(MIN_APP_UI_ICON_SIZE..=MAX_APP_UI_ICON_SIZE).contains(&width)
        || !(MIN_APP_UI_ICON_SIZE..=MAX_APP_UI_ICON_SIZE).contains(&height)
    {
        return Err(anyhow!(
            "Invalid ui activator icon '{}': SVG size must be between {} and {} px, got {}x{}",
            label,
            MIN_APP_UI_ICON_SIZE as u32,
            MAX_APP_UI_ICON_SIZE as u32,
            width,
            height
        ));
    }
    let ratio = width / height;
    if !(0.9..=1.1).contains(&ratio) {
        return Err(anyhow!(
            "Invalid ui activator icon '{}': SVG must be square, got {}x{}",
            label,
            width,
            height
        ));
    }
    Ok(())
}

fn collect_app_ui_icon_sources(ui: &serde_json::Value, out: &mut Vec<String>) -> Result<()> {
    let Some(activators) = ui.get("activators").and_then(serde_json::Value::as_array) else {
        return Ok(());
    };

    for (index, activator) in activators.iter().enumerate() {
        let Some(icon) = activator.get("icon") else {
            continue;
        };
        let icon = icon
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                anyhow!("ui.activators[{index}].icon must be a non-empty string when present")
            })?;
        out.push(icon.to_string());
    }
    Ok(())
}

fn resolve_project_relative_file(project_root: &Path, raw: &str) -> Result<PathBuf> {
    let raw_path = Path::new(raw);
    if raw_path.is_absolute() {
        return Err(anyhow!("path must be relative to project root"));
    }
    let mut relative = PathBuf::new();
    for component in raw_path.components() {
        match component {
            Component::Normal(part) => relative.push(part),
            Component::CurDir => {}
            Component::ParentDir => return Err(anyhow!("path must not contain '..'")),
            Component::RootDir | Component::Prefix(_) => {
                return Err(anyhow!("path must be relative to project root"));
            }
        }
    }
    if relative.as_os_str().is_empty() {
        return Err(anyhow!("path must not be empty"));
    }
    Ok(project_root.join(relative))
}

fn sanitize_asset_stem(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if matches!(ch, '-' | '_' | '.') && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

fn remove_empty_parent_dirs_until(root: &Path, start: Option<&Path>) {
    let Ok(root) = root.canonicalize() else {
        return;
    };
    let mut current = start.map(Path::to_path_buf);
    while let Some(dir) = current {
        let Ok(canonical) = dir.canonicalize() else {
            break;
        };
        if canonical == root || !canonical.starts_with(&root) {
            break;
        }
        let is_empty = fs::read_dir(&canonical)
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(false);
        if !is_empty || fs::remove_dir(&canonical).is_err() {
            break;
        }
        current = canonical.parent().map(Path::to_path_buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn sync_windows_design_icons_removes_stale_generated_pngs() {
        let temp = tempdir().unwrap();
        let stale = temp.path().join(WINDOWS_DESIGN_ICON_DIR).join("old.png");
        fs::create_dir_all(stale.parent().unwrap()).unwrap();
        fs::write(&stale, b"old").unwrap();

        let icons = vec![PreparedWindowsDesignIcon {
            relative_path: format!("{WINDOWS_DESIGN_ICON_DIR}/new.png"),
            source_path: "design/icons/svg/new.svg".to_string(),
            bytes: b"new".to_vec(),
            hash: "new-hash".to_string(),
        }];

        assert!(sync_windows_design_icons(temp.path(), &icons, None).unwrap());
        assert!(!stale.exists());
        assert!(
            temp.path()
                .join(WINDOWS_DESIGN_ICON_DIR)
                .join("new.png")
                .exists()
        );
    }
}
