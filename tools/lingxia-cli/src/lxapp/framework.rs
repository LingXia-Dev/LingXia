mod html;
mod react;
mod vue;

use anyhow::{Result, anyhow};
use serde_json::Value;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectFramework {
    React,
    Vue,
    Html,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageActionMode {
    Notify,
    Call,
    Stream,
}

impl PageActionMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Notify => "notify",
            Self::Call => "call",
            Self::Stream => "stream",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageAction {
    pub name: String,
    pub mode: PageActionMode,
}

#[derive(Debug, Clone)]
pub struct FrameworkScaffold {
    pub index_html: String,
    pub main_entry_filename: &'static str,
    pub main_entry: String,
    pub output_extension: &'static str,
}

impl ProjectFramework {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::React => "react",
            Self::Vue => "vue",
            Self::Html => "html",
        }
    }
}

pub fn scaffold(
    framework: ProjectFramework,
    page_title: &str,
    app_import: &str,
    page_bridge_import: &str,
) -> FrameworkScaffold {
    match framework {
        ProjectFramework::React => react::scaffold(page_title, app_import, page_bridge_import),
        ProjectFramework::Vue => vue::scaffold(page_title, app_import, page_bridge_import),
        ProjectFramework::Html => html::scaffold(page_title, app_import, page_bridge_import),
    }
}

pub fn detect_project_framework(project_root: &Path) -> Result<ProjectFramework> {
    detect_framework_from_manifest(project_root)?
        .ok_or_else(|| anyhow!("Cannot determine LingXia project framework"))
}

pub fn resolve_page_path(
    project_root: &Path,
    page_path: &str,
    framework: ProjectFramework,
) -> Option<String> {
    let page_path = Path::new(page_path);
    if page_path.extension().is_some() {
        let full_path = project_root.join(page_path);
        return full_path
            .exists()
            .then(|| normalize_relative_path(page_path));
    }

    let mut extensions = preferred_extensions(framework).to_vec();
    for ext in ["tsx", "jsx", "vue", "html"] {
        if !extensions.contains(&ext) {
            extensions.push(ext);
        }
    }

    let page_dir = page_path.parent().unwrap_or_else(|| Path::new(""));
    let base_name = page_path.file_name()?.to_str()?;

    for ext in extensions {
        let candidate = page_dir.join(format!("{base_name}.{ext}"));
        if project_root.join(&candidate).exists() {
            return Some(normalize_relative_path(&candidate));
        }
    }

    None
}

fn detect_framework_from_manifest(project_root: &Path) -> Result<Option<ProjectFramework>> {
    for manifest_name in ["lxapp.json", "lxplugin.json"] {
        let manifest_path = project_root.join(manifest_name);
        if !manifest_path.exists() {
            continue;
        }

        let content = fs::read_to_string(&manifest_path)?;
        let manifest: Value = serde_json::from_str(&content)?;
        if let Some(framework) = manifest.get("framework").and_then(Value::as_str) {
            return Ok(Some(parse_framework(framework)?));
        }
        let Some(page_path) = extract_first_manifest_page_entry(&manifest)? else {
            continue;
        };
        if let Some(framework) = detect_framework_for_page_path(project_root, page_path)? {
            return Ok(Some(framework));
        }
    }

    Ok(None)
}

fn parse_framework(value: &str) -> Result<ProjectFramework> {
    match value {
        "react" => Ok(ProjectFramework::React),
        "vue" => Ok(ProjectFramework::Vue),
        "html" => Ok(ProjectFramework::Html),
        _ => Err(anyhow!("Unsupported LingXia framework: {value}")),
    }
}

fn extract_first_manifest_page_entry(manifest: &Value) -> Result<Option<&str>> {
    let Some(pages) = manifest.get("pages") else {
        return Ok(None);
    };

    match pages {
        Value::Array(entries) => entries
            .first()
            .map(|value| {
                value
                    .as_str()
                    .ok_or_else(|| anyhow!("manifest pages entries must be strings"))
            })
            .transpose(),
        Value::Object(entries) => entries
            .values()
            .next()
            .map(|value| {
                value
                    .as_str()
                    .ok_or_else(|| anyhow!("manifest named pages entries must be strings"))
            })
            .transpose(),
        _ => Err(anyhow!("manifest pages must be an array or object")),
    }
}

fn detect_framework_for_page_path(
    project_root: &Path,
    page_path: &str,
) -> Result<Option<ProjectFramework>> {
    let page_path = Path::new(page_path);
    if let Some(extension) = page_path.extension().and_then(|ext| ext.to_str()) {
        return Ok(framework_for_extension(extension));
    }

    let mut candidates = Vec::new();
    for (framework, extensions) in [
        (ProjectFramework::React, &["tsx", "jsx"][..]),
        (ProjectFramework::Vue, &["vue"][..]),
        (ProjectFramework::Html, &["html"][..]),
    ] {
        if extensions
            .iter()
            .any(|ext| project_root.join(page_path).with_extension(ext).exists())
        {
            candidates.push(framework);
        }
    }

    match candidates.as_slice() {
        [] => Ok(None),
        [framework] => Ok(Some(*framework)),
        _ => Err(anyhow!(
            "Multiple framework candidates found for {}. Pass --framework react|vue|html.",
            normalize_relative_path(page_path)
        )),
    }
}

fn framework_for_extension(extension: &str) -> Option<ProjectFramework> {
    match extension {
        "tsx" | "jsx" => Some(ProjectFramework::React),
        "vue" => Some(ProjectFramework::Vue),
        "html" => Some(ProjectFramework::Html),
        _ => None,
    }
}

fn preferred_extensions(framework: ProjectFramework) -> &'static [&'static str] {
    match framework {
        ProjectFramework::React => &["tsx", "jsx"],
        ProjectFramework::Vue => &["vue"],
        ProjectFramework::Html => &["html"],
    }
}

fn normalize_relative_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
