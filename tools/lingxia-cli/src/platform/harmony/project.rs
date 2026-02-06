use crate::config::HarmonyConfig;
use anyhow::{Context, Result, anyhow};
use std::path::{Path, PathBuf};

pub fn resolve_harmony_dir(
    project_root: &Path,
    _harmony_config: Option<&HarmonyConfig>,
) -> Result<PathBuf> {
    let harmony_dir = project_root.join("harmony");
    if harmony_dir.join("build-profile.json5").exists() {
        return Ok(harmony_dir);
    }

    if project_root.join("build-profile.json5").exists() {
        return Ok(project_root.to_path_buf());
    }

    Err(anyhow!(
        "HarmonyOS project not found.\n\
         Expected build-profile.json5 in: {}/harmony/",
        project_root.display()
    ))
}

pub fn resolve_harmony_rawfile_dir(project_root: &Path) -> Result<PathBuf> {
    let harmony_dir = project_root.join("harmony");
    if harmony_dir.exists() {
        Ok(harmony_dir.join("entry/src/main/resources/rawfile"))
    } else {
        Ok(project_root.join("entry/src/main/resources/rawfile"))
    }
}

pub fn read_bundle_name(harmony_dir: &Path) -> Result<String> {
    let app_json5_path = harmony_dir.join("AppScope/app.json5");
    if !app_json5_path.exists() {
        return Err(anyhow!(
            "AppScope/app.json5 not found in {}",
            harmony_dir.display()
        ));
    }

    let content = std::fs::read_to_string(&app_json5_path)
        .with_context(|| format!("Failed to read {}", app_json5_path.display()))?;

    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.contains("bundleName") {
            continue;
        }

        let Some(colon_pos) = trimmed.find(':') else {
            continue;
        };
        let value_part = trimmed[colon_pos + 1..].trim();
        let value = value_part
            .trim_matches(|c: char| c == '"' || c == '\'' || c == ',' || c.is_whitespace());
        if !value.is_empty() {
            return Ok(value.to_string());
        }
    }

    Err(anyhow!(
        "bundleName not found in {}",
        app_json5_path.display()
    ))
}

pub fn generate_icons(
    project_root: &Path,
    source_icon: &Path,
    background_color: &str,
    harmony_config: Option<&HarmonyConfig>,
) -> Result<()> {
    let harmony_dir = resolve_harmony_dir(project_root, harmony_config)?;
    crate::appicon::generate_harmony_icons(source_icon, &harmony_dir, background_color)
}
