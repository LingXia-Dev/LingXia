use crate::config::LingXiaConfig;
use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use super::hash::sha256_hex;
use super::sync::write_if_changed;

const APP_UI_ICON_DIR: &str = "icons";
const MAX_APP_UI_ICON_BYTES: u64 = 512 * 1024;
const MIN_APP_UI_ICON_SIZE: f32 = 16.0;
const MAX_APP_UI_ICON_SIZE: f32 = 512.0;

#[derive(Clone, Debug)]
pub(super) struct PreparedAppUiIcon {
    pub(super) relative_path: String,
    pub(super) source_path: String,
    pub(super) bytes: Vec<u8>,
    pub(super) hash: String,
}

pub(super) fn prepare_app_ui_icons(
    project_root: &Path,
    config: &LingXiaConfig,
) -> Result<Vec<PreparedAppUiIcon>> {
    let Some(ui) = config.ui.as_ref() else {
        return Ok(Vec::new());
    };

    let mut icon_sources = Vec::new();
    collect_app_ui_icon_sources(ui, &mut icon_sources)?;
    icon_sources.sort();
    icon_sources.dedup();

    let mut prepared = Vec::new();
    for source in icon_sources {
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
        let pdf = lingxia_gen::icons::svg_to_pdf_bytes(&svg)
            .with_context(|| format!("Failed to convert SVG icon '{}' to PDF", source))?;
        let hash = sha256_hex(&pdf);
        let stem = source_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(sanitize_asset_stem)
            .filter(|stem| !stem.is_empty())
            .unwrap_or_else(|| "icon".to_string());
        let relative_path = format!("{}/{}-{}.pdf", APP_UI_ICON_DIR, stem, &hash[..12]);

        prepared.push(PreparedAppUiIcon {
            relative_path,
            source_path: source,
            bytes: pdf,
            hash,
        });
    }

    Ok(prepared)
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

pub(super) fn app_ui_icon_hashes(icons: &[PreparedAppUiIcon]) -> BTreeMap<String, String> {
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
                "  {} icon {} → {}",
                "✓".green(),
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

pub(super) fn validate_app_ui_svg_icon(label: &str, svg: &str) -> Result<()> {
    let (width, height) = lingxia_gen::icons::svg_size(svg)
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
