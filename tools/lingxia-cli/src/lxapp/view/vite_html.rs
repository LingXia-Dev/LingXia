use super::PageAction;
use super::bridge_metadata_script;
use super::vite_assets::copy_dir_recursive;
use crate::lxapp::project::Project;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

pub(super) fn copy_html_page(project: &Project, page_path: &str) -> Result<()> {
    let source_path = project.root.join(page_path);
    let output_path = project.output_dir.join(page_path);
    let page_dir = source_path.parent().unwrap_or_else(|| Path::new(""));
    let output_dir = output_path.parent().unwrap_or_else(|| Path::new(""));
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let html = fs::read_to_string(&source_path)
        .with_context(|| format!("Failed to read {}", source_path.display()))?;
    let output_html = inject_runtime_script(html);
    fs::write(&output_path, &output_html)
        .with_context(|| format!("Failed to write {}", output_path.display()))?;

    let base = Path::new(page_path)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("index");
    let config_path = source_path
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .join(format!("{base}.json"));
    if config_path.exists() {
        let output_config = output_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(format!("{base}.json"));
        fs::copy(&config_path, output_config)?;
    }

    copy_html_page_support_files(page_dir, output_dir, base)?;
    Ok(())
}

pub(super) fn html_pages_require_bundling(project: &Project) -> Result<bool> {
    for page_path in &project.pages {
        let source_path = project.root.join(page_path);
        let html = fs::read_to_string(&source_path)
            .with_context(|| format!("Failed to read {}", source_path.display()))?;
        if html_source_requires_bundling(&html) {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn html_view_target(project: &Project) -> Result<Option<String>> {
    let config_path = project.root.join("lxapp.config.ts");
    if !config_path.exists() {
        return Ok(None);
    }
    let source = fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read {}", config_path.display()))?;
    Ok(extract_view_target(&source))
}

pub(super) fn copy_html_page_support_files(
    source_dir: &Path,
    output_dir: &Path,
    base: &str,
) -> Result<()> {
    if !source_dir.exists() {
        return Ok(());
    }

    let html_name = format!("{base}.html");
    let config_name = format!("{base}.json");
    let logic_ts_name = format!("{base}.ts");
    let logic_js_name = format!("{base}.js");

    for entry in fs::read_dir(source_dir)? {
        let entry = entry?;
        let source_path = entry.path();
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();
        let destination_path = output_dir.join(&file_name);

        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&source_path, &destination_path)?;
            continue;
        }

        if file_name_str == html_name
            || file_name_str == config_name
            || file_name_str == logic_ts_name
            || file_name_str == logic_js_name
        {
            continue;
        }

        fs::copy(&source_path, &destination_path).with_context(|| {
            format!(
                "Failed to copy {} -> {}",
                source_path.display(),
                destination_path.display()
            )
        })?;
    }

    Ok(())
}

pub(super) fn rewrite_entry_script_path(mut html: String, page_id: &str) -> String {
    let expected = format!("src=\"/pages/{page_id}/{page_id}.js\"");
    let expected_rel = format!("src=\"./{page_id}.js\"");
    let expected_plain = format!("src=\"{page_id}.js\"");
    for pattern in [
        expected.as_str(),
        expected_rel.as_str(),
        expected_plain.as_str(),
    ] {
        if html.contains(pattern) {
            html = html.replace(pattern, "src=\"./view.js\"");
        }
    }
    html
}

pub(super) fn rewrite_module_entry_script(
    mut html: String,
    original_src: &str,
    new_src: &str,
) -> String {
    let patterns = [
        format!("type=\"module\" src=\"{original_src}\""),
        format!("type='module' src='{original_src}'"),
        format!("TYPE='MODULE' src='{original_src}'"),
        format!("type = \"module\" src=\"{original_src}\""),
        format!("type=module src={original_src}"),
        format!("defer type=module src={original_src}"),
        format!("src=\"{original_src}\" type=\"module\""),
        format!("src='{original_src}' type='module'"),
    ];

    for pattern in patterns {
        if html.contains(&pattern) {
            let replacement = format!("src=\"{new_src}\"");
            html = html.replace(&pattern, &replacement);
        }
    }

    html
}

pub(super) fn extract_html_module_entry_script_path(source: &str) -> Option<String> {
    let bytes = source.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        i += 1;

        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || matches!(bytes[i], b'/' | b'!' | b'?') {
            continue;
        }

        let tag_start = i;
        while i < bytes.len() && is_html_name_char(bytes[i]) {
            i += 1;
        }
        if !source[tag_start..i].eq_ignore_ascii_case("script") {
            continue;
        }

        let mut is_module = false;
        let mut src = None;

        while i < bytes.len() {
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i >= bytes.len() || bytes[i] == b'>' {
                break;
            }
            if bytes[i] == b'/' {
                i += 1;
                continue;
            }

            let attr_start = i;
            while i < bytes.len() && is_html_name_char(bytes[i]) {
                i += 1;
            }
            if attr_start == i {
                i += 1;
                continue;
            }
            let attr_name = &source[attr_start..i];

            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }

            let mut value = None;
            if i < bytes.len() && bytes[i] == b'=' {
                i += 1;
                while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
                if i >= bytes.len() {
                    break;
                }
                let value_start;
                let value_end;
                if matches!(bytes[i], b'"' | b'\'') {
                    let quote = bytes[i];
                    i += 1;
                    value_start = i;
                    while i < bytes.len() && bytes[i] != quote {
                        i += 1;
                    }
                    value_end = i.min(bytes.len());
                    if i < bytes.len() {
                        i += 1;
                    }
                } else {
                    value_start = i;
                    while i < bytes.len()
                        && !bytes[i].is_ascii_whitespace()
                        && !matches!(bytes[i], b'>' | b'/')
                    {
                        i += 1;
                    }
                    value_end = i;
                }
                value = Some(&source[value_start..value_end]);
            }

            if attr_name.eq_ignore_ascii_case("type")
                && value.is_some_and(|v| v.eq_ignore_ascii_case("module"))
            {
                is_module = true;
            }
            if attr_name.eq_ignore_ascii_case("src") {
                src = value.map(str::to_string);
            }
        }

        if is_module && src.is_some() {
            return src;
        }
    }

    None
}

pub(super) fn inject_runtime_script(mut html: String) -> String {
    let runtime_script = "<script src=\"lx://assets/bridge-runtime.js\"></script>";
    if html.contains(runtime_script) {
        return html;
    }
    if let Some(index) = html.find("</head>") {
        html.insert_str(index, runtime_script);
        return html;
    }
    format!("{runtime_script}{html}")
}

pub(super) fn inject_bridge_metadata(mut html: String, actions: &[PageAction]) -> String {
    let metadata = format!("<script>\n{}\n</script>", bridge_metadata_script(actions));
    if html.contains(&metadata) {
        return html;
    }
    if let Some(index) = html.find("</body>") {
        html.insert_str(index, &metadata);
        return html;
    }
    html.push_str(&metadata);
    html
}

fn html_source_requires_bundling(source: &str) -> bool {
    html_source_has_module_script(source)
}

fn html_source_has_module_script(source: &str) -> bool {
    let bytes = source.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        i += 1;

        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || matches!(bytes[i], b'/' | b'!' | b'?') {
            continue;
        }

        let tag_start = i;
        while i < bytes.len() && is_html_name_char(bytes[i]) {
            i += 1;
        }
        if !source[tag_start..i].eq_ignore_ascii_case("script") {
            continue;
        }

        while i < bytes.len() {
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i >= bytes.len() || bytes[i] == b'>' {
                break;
            }
            if bytes[i] == b'/' {
                i += 1;
                continue;
            }

            let attr_start = i;
            while i < bytes.len() && is_html_name_char(bytes[i]) {
                i += 1;
            }
            if attr_start == i {
                i += 1;
                continue;
            }
            let attr_name = &source[attr_start..i];

            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }

            let mut value = None;
            if i < bytes.len() && bytes[i] == b'=' {
                i += 1;
                while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
                if i >= bytes.len() {
                    break;
                }
                let value_start;
                let value_end;
                if matches!(bytes[i], b'"' | b'\'') {
                    let quote = bytes[i];
                    i += 1;
                    value_start = i;
                    while i < bytes.len() && bytes[i] != quote {
                        i += 1;
                    }
                    value_end = i.min(bytes.len());
                    if i < bytes.len() {
                        i += 1;
                    }
                } else {
                    value_start = i;
                    while i < bytes.len()
                        && !bytes[i].is_ascii_whitespace()
                        && !matches!(bytes[i], b'>' | b'/')
                    {
                        i += 1;
                    }
                    value_end = i;
                }
                value = Some(&source[value_start..value_end]);
            }

            if attr_name.eq_ignore_ascii_case("type")
                && value.is_some_and(|v| v.eq_ignore_ascii_case("module"))
            {
                return true;
            }
        }
    }

    false
}

fn extract_view_target(source: &str) -> Option<String> {
    let view_index = source.find("view")?;
    let target_index = source[view_index..].find("target")? + view_index;
    let after_target = &source[target_index + "target".len()..];
    let colon_index = after_target.find(':')?;
    let value = after_target[colon_index + 1..].trim_start();
    let quote = value.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let rest = &value[quote.len_utf8()..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

fn is_html_name_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'_' | b'-')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lxapp::framework::{PageAction, PageActionMode, ProjectFramework};
    use crate::lxapp::project::ProjectKind;
    use tempfile::tempdir;

    fn write_file(root: &Path, relative: &str, content: &str) {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    fn make_project(root: &Path) -> Project {
        Project {
            root: root.to_path_buf(),
            kind: ProjectKind::LxApp,
            framework: ProjectFramework::Html,
            output_dir: root.join("dist"),
            pages: vec!["pages/home/index.html".to_string()],
            logic_entry: None,
            plugin_id: None,
            package_name: Some("demo".to_string()),
            version: "1.0.0".to_string(),
        }
    }

    #[test]
    fn html_page_copy_injects_runtime_script() {
        let temp = tempdir().unwrap();
        let project = Project {
            pages: vec!["pages/settings/index.html".to_string()],
            ..make_project(temp.path())
        };
        write_file(
            temp.path(),
            "pages/settings/index.html",
            "<!DOCTYPE html><html><head><title>Settings</title></head><body></body></html>",
        );

        copy_html_page(&project, "pages/settings/index.html").unwrap();

        let output =
            fs::read_to_string(project.output_dir.join("pages/settings/index.html")).unwrap();
        assert!(output.contains("lx://assets/bridge-runtime.js"));
    }

    #[test]
    fn html_page_copy_preserves_support_files() {
        let temp = tempdir().unwrap();
        let project = Project {
            pages: vec!["pages/downloads/index.html".to_string()],
            ..make_project(temp.path())
        };
        write_file(
            temp.path(),
            "pages/downloads/index.html",
            "<!DOCTYPE html><html><body><script src=\"./download.js\"></script></body></html>",
        );
        write_file(
            temp.path(),
            "pages/downloads/download.js",
            "console.log('download helper');",
        );
        write_file(temp.path(), "pages/downloads/index.ts", "Page({});");

        copy_html_page(&project, "pages/downloads/index.html").unwrap();

        assert!(
            project
                .output_dir
                .join("pages/downloads/download.js")
                .exists()
        );
        assert!(!project.output_dir.join("pages/downloads/index.ts").exists());
    }

    #[test]
    fn html_pages_require_bundling_for_module_scripts() {
        let temp = tempdir().unwrap();
        let project = make_project(temp.path());
        write_file(
            temp.path(),
            "pages/home/index.html",
            "<!DOCTYPE html><html><body><script type=\"module\" src=\"./entry.js\"></script></body></html>",
        );

        assert!(html_pages_require_bundling(&project).unwrap());
    }

    #[test]
    fn html_source_requires_bundling_for_module_type_variants() {
        for source in [
            "<script type = \"module\" src=\"./entry.js\"></script>",
            "<script TYPE='MODULE' src='./entry.js'></script>",
            "<script defer type=module src=./entry.js></script>",
            "<SCRIPT type = 'module'></SCRIPT>",
        ] {
            assert!(html_source_requires_bundling(source), "{source}");
        }
    }

    #[test]
    fn html_source_requires_bundling_ignores_non_module_scripts() {
        for source in [
            "<script src=\"./entry.js\"></script>",
            "<script type=\"text/javascript\" src=\"./entry.js\"></script>",
            "<div type=\"module\"></div>",
        ] {
            assert!(!html_source_requires_bundling(source), "{source}");
        }
    }

    #[test]
    fn inject_bridge_metadata_adds_names() {
        let html = inject_bridge_metadata(
            "<!DOCTYPE html><html><body></body></html>".to_string(),
            &[PageAction {
                name: "confirmOrientation".to_string(),
                mode: PageActionMode::Call,
            }],
        );

        assert!(html.contains("\"confirmOrientation\""));
        assert!(html.contains("__names"));
    }

    #[test]
    fn extract_module_entry_script_path_variants() {
        assert_eq!(
            extract_html_module_entry_script_path(
                "<script type = \"module\" src=\"./entry.js\"></script>"
            )
            .as_deref(),
            Some("./entry.js")
        );
        assert_eq!(
            extract_html_module_entry_script_path(
                "<script TYPE='MODULE' src='./entry.js'></script>"
            )
            .as_deref(),
            Some("./entry.js")
        );
    }

    #[test]
    fn rewrite_module_entry_script_to_classic_script() {
        let html = "<script type=\"module\" src=\"./entry.js\"></script>".to_string();
        let rewritten = rewrite_module_entry_script(html, "./entry.js", "./view.js");
        assert_eq!(rewritten, "<script src=\"./view.js\"></script>");
    }

    #[test]
    fn extract_view_target_from_config_source() {
        assert_eq!(
            extract_view_target("export default { view: { target: 'es5' } };").as_deref(),
            Some("es5")
        );
        assert_eq!(
            extract_view_target("export default { view: { target: \"es2020\" } };").as_deref(),
            Some("es2020")
        );
    }
}
