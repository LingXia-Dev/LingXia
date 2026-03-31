mod html;
mod react;
mod vue;

use anyhow::{Result, anyhow};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

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
    let package_json = project_root.join("package.json");
    if package_json.exists() {
        let content = fs::read_to_string(&package_json)?;
        let value: Value = serde_json::from_str(&content)?;
        if let Some(framework) = value
            .get("lingxia")
            .and_then(|value| value.get("framework"))
            .and_then(Value::as_str)
        {
            return parse_framework(framework);
        }
    }

    detect_framework_from_pages(project_root)
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

fn parse_framework(value: &str) -> Result<ProjectFramework> {
    match value {
        "react" => Ok(ProjectFramework::React),
        "vue" => Ok(ProjectFramework::Vue),
        "html" => Ok(ProjectFramework::Html),
        _ => Err(anyhow!("Unsupported LingXia framework: {value}")),
    }
}

fn detect_framework_from_pages(project_root: &Path) -> Option<ProjectFramework> {
    let pages_dir = project_root.join("pages");
    let entries = fs::read_dir(&pages_dir).ok()?;

    for entry in entries.flatten() {
        let file_type = entry.file_type().ok()?;
        if !file_type.is_dir() {
            continue;
        }
        let dir = entry.path();
        if find_first_file_with_ext(&dir, &["tsx", "jsx"]).is_some() {
            return Some(ProjectFramework::React);
        }
        if find_first_file_with_ext(&dir, &["vue"]).is_some() {
            return Some(ProjectFramework::Vue);
        }
        if find_first_file_with_ext(&dir, &["html"]).is_some() {
            return Some(ProjectFramework::Html);
        }
    }

    None
}

fn find_first_file_with_ext(dir: &Path, extensions: &[&str]) -> Option<PathBuf> {
    for ext in extensions {
        let candidate = dir.join(format!("index.{ext}"));
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
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
