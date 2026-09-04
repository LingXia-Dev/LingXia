//! Recognizing a built lxapp bundle. `lingxia dev` may be run inside a dist
//! directory; the build output identifies itself: page entries in the
//! transformed `lxapp.json` resolve to compiled HTML documents (a source
//! `.tsx`/`.vue` page can never be one), and an explicitly named logic
//! bundle is present. No extra state files are involved.

use serde_json::Value;
use std::fs;
use std::io::Read;
use std::path::Path;

pub(crate) fn is_built_bundle_dir(dir: &Path) -> bool {
    let Ok(manifest) = fs::read_to_string(dir.join("lxapp.json")) else {
        return false;
    };
    let Ok(manifest) = serde_json::from_str::<Value>(&manifest) else {
        return false;
    };
    // A `"logic"` entry naming a file explicitly must have been built.
    if let Some(Value::String(entry)) = manifest.get("logic")
        && !dir.join(entry.trim()).is_file()
    {
        return false;
    }
    let Some(pages) = manifest.get("pages").and_then(|pages| pages.as_array()) else {
        return false;
    };
    !pages.is_empty()
        && pages.iter().all(|page| {
            page.get("path")
                .and_then(|path| path.as_str())
                .is_some_and(|path| is_built_page(&dir.join(path), path))
        })
}

fn is_built_page(file: &Path, manifest_path: &str) -> bool {
    let Some(head) = read_head(file) else {
        return false;
    };
    let head = String::from_utf8_lossy(&head).to_lowercase();
    let trimmed = head.trim_start();
    let is_html_document = trimmed.starts_with("<!doctype html")
        || trimmed.starts_with("<html")
        || (head.contains("<head") && head.contains("<body"));
    if manifest_path.ends_with(".html") {
        // An html-framework SOURCE page is an HTML document too; only the
        // build injects the bridge runtime reference.
        is_html_document && head.contains("lx://assets/bridge-runtime.js")
    } else {
        is_html_document
    }
}

fn read_head(path: &Path) -> Option<Vec<u8>> {
    let mut head = vec![0u8; 8192];
    let mut file = fs::File::open(path).ok()?;
    let read = file.read(&mut head).ok()?;
    head.truncate(read);
    Some(head)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_manifest(dir: &Path, pages: &[(&str, &str)], logic: Option<&str>) {
        let pages = pages
            .iter()
            .map(|(name, path)| format!(r#"{{"name":"{name}","path":"{path}"}}"#))
            .collect::<Vec<_>>()
            .join(",");
        let logic = logic
            .map(|entry| format!(r#""logic":"{entry}","#))
            .unwrap_or_default();
        fs::write(
            dir.join("lxapp.json"),
            format!(
                r#"{{"appId":"demo","appName":"Demo","version":"1.0.0",{logic}"pages":[{pages}]}}"#
            ),
        )
        .unwrap();
    }

    #[test]
    fn recognizes_built_dist_with_html_document_pages() {
        let temp = tempfile::tempdir().unwrap();
        write_manifest(temp.path(), &[("chat", "pages/chat/index.tsx")], None);
        fs::create_dir_all(temp.path().join("pages/chat")).unwrap();
        fs::write(
            temp.path().join("pages/chat/index.tsx"),
            "<!DOCTYPE html>\n<html><head></head><body></body></html>",
        )
        .unwrap();

        assert!(is_built_bundle_dir(temp.path()));
    }

    #[test]
    fn rejects_source_project_with_tsx_page() {
        let temp = tempfile::tempdir().unwrap();
        write_manifest(temp.path(), &[("chat", "pages/chat/index.tsx")], None);
        fs::create_dir_all(temp.path().join("pages/chat")).unwrap();
        fs::write(
            temp.path().join("pages/chat/index.tsx"),
            "import React from 'react';\nexport default () => <div />;",
        )
        .unwrap();

        assert!(!is_built_bundle_dir(temp.path()));
    }

    #[test]
    fn rejects_unresolved_page_paths() {
        let temp = tempfile::tempdir().unwrap();
        write_manifest(temp.path(), &[("chat", "pages/chat/index")], None);
        fs::create_dir_all(temp.path().join("pages/chat")).unwrap();
        fs::write(temp.path().join("pages/chat/index.tsx"), "source").unwrap();

        assert!(!is_built_bundle_dir(temp.path()));
    }

    #[test]
    fn html_page_requires_injected_bridge_runtime() {
        let temp = tempfile::tempdir().unwrap();
        write_manifest(temp.path(), &[("home", "pages/home/index.html")], None);
        fs::create_dir_all(temp.path().join("pages/home")).unwrap();
        let source = "<!DOCTYPE html>\n<html><head></head><body>hi</body></html>";
        fs::write(temp.path().join("pages/home/index.html"), source).unwrap();
        assert!(!is_built_bundle_dir(temp.path()));

        let built = "<!DOCTYPE html>\n<html><head>\
            <script data-lingxia-bridge-runtime=\"v3-bootstrap\" src=\"lx://assets/bridge-runtime.js\"></script>\
            </head><body>hi</body></html>";
        fs::write(temp.path().join("pages/home/index.html"), built).unwrap();
        assert!(is_built_bundle_dir(temp.path()));

        // Existing V2 bundles remain recognized. They just lack the sentinel
        // required by trusted direct-load bootstrap.
        let legacy = "<!DOCTYPE html>\n<html><head>\
            <script src=\"lx://assets/bridge-runtime.js\"></script>\
            </head><body>hi</body></html>";
        fs::write(temp.path().join("pages/home/index.html"), legacy).unwrap();
        assert!(is_built_bundle_dir(temp.path()));
    }

    #[test]
    fn requires_explicitly_named_logic_bundle() {
        let temp = tempfile::tempdir().unwrap();
        write_manifest(
            temp.path(),
            &[("chat", "pages/chat/index.tsx")],
            Some("app-logic.js"),
        );
        fs::create_dir_all(temp.path().join("pages/chat")).unwrap();
        fs::write(
            temp.path().join("pages/chat/index.tsx"),
            "<!DOCTYPE html><html><head></head><body></body></html>",
        )
        .unwrap();
        assert!(!is_built_bundle_dir(temp.path()));

        fs::write(temp.path().join("app-logic.js"), "// bundled").unwrap();
        assert!(is_built_bundle_dir(temp.path()));
    }
}
