mod html;
mod react;

use anyhow::{Result, anyhow};
use serde_json::Value;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectFramework {
    React,
    Html,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageActionMode {
    Call,
    Stream,
}

impl PageActionMode {
    pub fn as_str(self) -> &'static str {
        match self {
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
            Self::Html => "html",
        }
    }
}

/// `wait_for_state`: the page has Logic, so its View mounts only once the
/// page's first state has arrived (`mountPage` in `@lingxia/react/entry`).
pub fn scaffold(
    framework: ProjectFramework,
    page_title: &str,
    app_import: &str,
    page_bridge_import: &str,
    wait_for_state: bool,
) -> FrameworkScaffold {
    match framework {
        ProjectFramework::React => {
            react::scaffold(page_title, app_import, page_bridge_import, wait_for_state)
        }
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
    let base = page_base(page_path);
    let config = project_root.join(format!("{base}.json"));
    if fs::read_to_string(config)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .is_some_and(|value| value["renderer"] == "gpu")
    {
        let candidate = format!("{base}.ui.tsx");
        return project_root.join(&candidate).is_file().then_some(candidate);
    }
    if is_gpu_page(page_path) {
        return None;
    }
    let page_path = Path::new(page_path);
    if page_path.extension().is_some() {
        let full_path = project_root.join(page_path);
        return full_path
            .exists()
            .then(|| normalize_relative_path(page_path));
    }

    let mut extensions = preferred_extensions(framework).to_vec();
    for ext in ["tsx", "jsx", "html"] {
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

pub(crate) fn is_gpu_page(path: &str) -> bool {
    path.ends_with(".ui.tsx") || path.ends_with(".ui.js")
}

pub(crate) fn page_base(path: &str) -> String {
    for suffix in [".ui.tsx", ".ui.js"] {
        if let Some(base) = path.strip_suffix(suffix) {
            return base.replace('\\', "/");
        }
    }
    Path::new(path)
        .with_extension("")
        .to_string_lossy()
        .replace('\\', "/")
}

pub(crate) fn compiled_page_path(path: &str) -> String {
    if is_gpu_page(path) {
        format!("{}.ui.js", page_base(path))
    } else {
        path.to_owned()
    }
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
        // GPU pages do not select the WebView framework for their siblings.
        extract_first_manifest_page_entry(&manifest)?;
        let mut gpu = false;
        if let Some(pages) = manifest.get("pages").and_then(Value::as_array) {
            for page in pages {
                let Some(path) = page.get("path").and_then(Value::as_str) else {
                    continue;
                };
                let config = project_root.join(format!("{}.json", page_base(path)));
                if fs::read_to_string(config)
                    .ok()
                    .and_then(|s| serde_json::from_str::<Value>(&s).ok())
                    .is_some_and(|v| v["renderer"] == "gpu")
                {
                    gpu = true;
                    continue;
                }
                if let Some(framework) = detect_framework_for_page_path(project_root, path)? {
                    return Ok(Some(framework));
                }
            }
        }
        if gpu {
            return Ok(Some(ProjectFramework::Html));
        }
    }

    Ok(None)
}

fn parse_framework(value: &str) -> Result<ProjectFramework> {
    match value {
        "react" => Ok(ProjectFramework::React),
        "html" => Ok(ProjectFramework::Html),
        "vue" => Err(anyhow!(
            "Vue is no longer a LingXia view framework. Use react or html."
        )),
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
                    .get("path")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow!("manifest pages entries must include path"))
            })
            .transpose(),
        _ => Err(anyhow!(
            "manifest pages must be an array of objects with name/path"
        )),
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
        _ => {
            let options = candidates
                .iter()
                .map(|framework| framework.as_str())
                .collect::<Vec<_>>()
                .join("|");
            Err(anyhow!(
                "Multiple framework implementations found for {}. \
                 Set \"framework\" to one of {options} in lxapp.json.",
                normalize_relative_path(page_path)
            ))
        }
    }
}

fn framework_for_extension(extension: &str) -> Option<ProjectFramework> {
    match extension {
        "tsx" | "jsx" => Some(ProjectFramework::React),
        "html" => Some(ProjectFramework::Html),
        _ => None,
    }
}

fn preferred_extensions(framework: ProjectFramework) -> &'static [&'static str] {
    match framework {
        ProjectFramework::React => &["tsx", "jsx"],
        ProjectFramework::Html => &["html"],
    }
}

fn normalize_relative_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_page_does_not_select_react_for_webview_siblings() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("lxapp.json"),
            r#"{"pages":[{"name":"native","path":"native"},{"name":"web","path":"web"}]}"#,
        )
        .unwrap();
        fs::write(dir.path().join("native.json"), r#"{"renderer":"gpu"}"#).unwrap();
        fs::write(dir.path().join("native.ui.tsx"), "").unwrap();
        fs::write(dir.path().join("web.html"), "").unwrap();
        assert_eq!(
            detect_project_framework(dir.path()).unwrap(),
            ProjectFramework::Html
        );
        assert_eq!(
            resolve_page_path(dir.path(), "native", ProjectFramework::Html).as_deref(),
            Some("native.ui.tsx")
        );
        assert_eq!(page_base("pages/home/index.ui.tsx"), "pages/home/index");
        assert_eq!(
            compiled_page_path("pages/home/index.ui.tsx"),
            "pages/home/index.ui.js"
        );
    }

    #[test]
    fn ambiguity_error_lists_only_actual_candidates() {
        let dir = std::env::temp_dir().join("lingxia-framework-test");
        let page_dir = dir.join("pages/home");
        std::fs::create_dir_all(&page_dir).unwrap();
        std::fs::write(page_dir.join("index.tsx"), "").unwrap();
        std::fs::write(page_dir.join("index.html"), "").unwrap();

        let err = detect_framework_for_page_path(&dir, "pages/home/index").unwrap_err();
        let message = err.to_string();
        assert!(message.contains("react|html"), "{message}");
        assert!(message.contains("lxapp.json"), "{message}");
        assert!(!message.contains("--framework"), "{message}");

        std::fs::remove_dir_all(&dir).ok();
    }
}
