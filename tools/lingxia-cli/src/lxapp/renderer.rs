//! An lxapp's renderer follows from its pages' View files: a named `render` export is a
//! GPU View page; a default export or an `index.html` is a WebView page. The build writes the
//! answer into the built `lxapp.json` as `"renderer": "gpu"`, which is what the runtime reads.

use anyhow::{Result, anyhow};
use oxc_allocator::Allocator;
use oxc_ast::ast::{Declaration, ModuleExportName, Statement};
use oxc_parser::Parser;
use oxc_span::SourceType;
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Renderer {
    Gpu,
    Webview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PageKind {
    Gpu,
    React,
    Html,
}

const SHAPES: &str = "  export function render() { ... }        a GPU View page\n  \
                      export default function Page() { ... }  a WebView page (React)";

/// A source manifest's renderer, read from its pages; a built one names it under `renderer`.
/// No manifest, or no page with a View file, is WebView.
pub(crate) fn lxapp_renderer(root: &Path) -> Result<Renderer> {
    let Ok(text) = fs::read_to_string(root.join("lxapp.json")) else {
        return Ok(Renderer::Webview);
    };
    let manifest: Value = serde_json::from_str(&text)?;
    if let Some(declared) = manifest.get("renderer") {
        return Ok(if declared == "gpu" {
            Renderer::Gpu
        } else {
            Renderer::Webview
        });
    }
    derive(root, &text, &manifest)
}

/// The renderer the pages' View files make, whatever the manifest says.
pub(crate) fn pages_renderer(root: &Path) -> Result<Renderer> {
    let text = fs::read_to_string(root.join("lxapp.json"))?;
    let manifest: Value = serde_json::from_str(&text)?;
    derive(root, &text, &manifest)
}

fn derive(root: &Path, text: &str, manifest: &Value) -> Result<Renderer> {
    let pages: Vec<&str> = manifest
        .get("pages")
        .and_then(Value::as_array)
        .map(|pages| {
            pages
                .iter()
                .filter_map(|page| page.get("path").and_then(Value::as_str))
                .collect()
        })
        .unwrap_or_default();
    cached(root, text, &pages)
}

type Stamp = Option<(SystemTime, u64)>;

struct Entry {
    manifest: String,
    files: Vec<(PathBuf, Stamp)>,
    result: std::result::Result<Renderer, String>,
}

/// Every page path helper asks per page; parsing is paid again only when a file it read changes.
fn cached(root: &Path, manifest: &str, pages: &[&str]) -> Result<Renderer> {
    static CACHE: Mutex<Option<HashMap<PathBuf, Entry>>> = Mutex::new(None);
    let mut cache = CACHE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let cache = cache.get_or_insert_with(HashMap::new);
    if let Some(entry) = cache.get(root)
        && entry.manifest == manifest
        && entry
            .files
            .iter()
            .all(|(path, stamp)| stamp_of(path) == *stamp)
    {
        return entry.result.clone().map_err(|message| anyhow!(message));
    }
    let mut read = BTreeSet::new();
    let result = infer(root, pages, &mut read).map_err(|error| format!("{error:#}"));
    let files = read
        .into_iter()
        .map(|path| {
            let stamp = stamp_of(&path);
            (path, stamp)
        })
        .collect();
    cache.insert(
        root.to_path_buf(),
        Entry {
            manifest: manifest.to_owned(),
            files,
            result: result.clone(),
        },
    );
    result.map_err(|message| anyhow!(message))
}

fn stamp_of(path: &Path) -> Stamp {
    let metadata = fs::metadata(path).ok()?;
    Some((metadata.modified().ok()?, metadata.len()))
}

fn infer(root: &Path, pages: &[&str], read: &mut BTreeSet<PathBuf>) -> Result<Renderer> {
    let mut gpu = Vec::new();
    let mut webview = Vec::new();
    for page in pages {
        let Some((file, kind)) = classify(root, page, read)? else {
            continue;
        };
        match kind {
            PageKind::Gpu => gpu.push(file),
            PageKind::React => webview.push(file),
            PageKind::Html => webview.push(format!("{file} (HTML)")),
        }
    }
    match (gpu.is_empty(), webview.is_empty()) {
        (false, false) => Err(anyhow!(disagreement(&gpu, &webview))),
        (false, true) => Ok(Renderer::Gpu),
        _ => Ok(Renderer::Webview),
    }
}

/// The page's View file and what it is; `None` for a page with no View file, which page
/// resolution reports.
fn classify(
    root: &Path,
    page: &str,
    read: &mut BTreeSet<PathBuf>,
) -> Result<Option<(String, PageKind)>> {
    let path = Path::new(page);
    let candidates: Vec<String> = match path.extension().and_then(|ext| ext.to_str()) {
        Some("tsx" | "jsx" | "html") => vec![page.replace('\\', "/")],
        _ => {
            let base = crate::lxapp::framework::page_base(page);
            ["tsx", "jsx", "html"]
                .iter()
                .map(|ext| format!("{base}.{ext}"))
                .collect()
        }
    };
    for candidate in &candidates {
        read.insert(root.join(candidate));
    }
    let existing = |ext: &str| {
        candidates
            .iter()
            .find(|candidate| candidate.ends_with(ext) && root.join(candidate).is_file())
    };
    if let Some(module) = existing(".tsx").or_else(|| existing(".jsx")) {
        let exports = module_exports(&root.join(module), read, 0)?;
        let kind = match (exports.render, exports.default) {
            (true, false) => PageKind::Gpu,
            (false, true) => PageKind::React,
            (true, true) => {
                return Err(anyhow!(
                    "{module}: exports both `render` and a default component; a page's View exports one of:\n{SHAPES}"
                ));
            }
            (false, false) => {
                return Err(anyhow!(
                    "{module}: exports neither `render` nor a default component; a page's View exports one of:\n{SHAPES}"
                ));
            }
        };
        return Ok(Some((module.clone(), kind)));
    }
    Ok(existing(".html").map(|html| (html.clone(), PageKind::Html)))
}

fn disagreement(gpu: &[String], webview: &[String]) -> String {
    let left = "these pages export `render`:";
    let width = gpu
        .iter()
        .map(|page| page.len() + 2)
        .chain([left.len()])
        .max()
        .unwrap_or(0)
        + 4;
    let mut message = format!(
        "an lxapp uses one renderer, and its pages disagree:\n{left:<width$}these export a default component:\n"
    );
    for row in 0..gpu.len().max(webview.len()) {
        let cell = gpu
            .get(row)
            .map(|page| format!("  {page}"))
            .unwrap_or_default();
        let right = webview
            .get(row)
            .map(|page| format!("  {page}"))
            .unwrap_or_default();
        message.push_str(format!("{cell:<width$}{right}").trim_end());
        message.push('\n');
    }
    message.trim_end().to_string()
}

#[derive(Default)]
struct Exports {
    render: bool,
    default: bool,
}

const MAX_REEXPORT_DEPTH: usize = 8;

/// The module's value exports named `render` or `default`; `export * from './x'` is followed
/// for `render`, since it never forwards a default.
fn module_exports(file: &Path, read: &mut BTreeSet<PathBuf>, depth: usize) -> Result<Exports> {
    read.insert(file.to_path_buf());
    let source =
        fs::read_to_string(file).map_err(|error| anyhow!("{}: {error}", file.display()))?;
    let source_type = SourceType::from_path(file)
        .map_err(|_| anyhow!("{}: unsupported file type", file.display()))?;
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, &source, source_type).parse();
    if let Some(error) = parsed.diagnostics.first() {
        return Err(anyhow!("{}: {error}", file.display()));
    }
    let mut exports = Exports::default();
    let mut note = |name: &str| match name {
        "render" => exports.render = true,
        "default" => exports.default = true,
        _ => {}
    };
    let mut stars = Vec::new();
    for statement in &parsed.program.body {
        match statement {
            Statement::ExportDefaultDeclaration(_) => note("default"),
            Statement::ExportDeclaration(export) => match &export.declaration {
                Declaration::FunctionDeclaration(function) if !function.declare => {
                    if let Some(id) = &function.id {
                        note(id.name.as_str());
                    }
                }
                Declaration::ClassDeclaration(class) if !class.declare => {
                    if let Some(id) = &class.id {
                        note(id.name.as_str());
                    }
                }
                Declaration::VariableDeclaration(variables) if !variables.declare => {
                    for declarator in &variables.declarations {
                        for id in declarator.id.get_binding_identifiers() {
                            note(id.name.as_str());
                        }
                    }
                }
                _ => {}
            },
            Statement::ExportNamedDeclaration(export) if !export.export_kind.is_type() => {
                for specifier in &export.specifiers {
                    if !specifier.export_kind.is_type() {
                        note(&export_name(&specifier.exported));
                    }
                }
            }
            Statement::ExportFromDeclaration(export) if !export.export_kind.is_type() => {
                for specifier in &export.specifiers {
                    if !specifier.export_kind.is_type() {
                        note(&export_name(&specifier.exported));
                    }
                }
            }
            Statement::ExportAllDeclaration(export) if !export.export_kind.is_type() => {
                match &export.exported {
                    Some(name) => note(&export_name(name)),
                    None => stars.push(export.source.value.as_str().to_owned()),
                }
            }
            _ => {}
        }
    }
    if !exports.render && depth < MAX_REEXPORT_DEPTH {
        for specifier in stars {
            if let Some(target) = resolve_relative(file, &specifier, read) {
                exports.render |= module_exports(&target, read, depth + 1)?.render;
            }
        }
    }
    Ok(exports)
}

fn export_name(name: &ModuleExportName<'_>) -> String {
    match name {
        ModuleExportName::IdentifierName(identifier) => identifier.name.to_string(),
        ModuleExportName::IdentifierReference(identifier) => identifier.name.to_string(),
        ModuleExportName::StringLiteral(literal) => literal.value.to_string(),
    }
}

/// A relative module specifier as a bundler resolves it; packages are not followed.
fn resolve_relative(from: &Path, specifier: &str, read: &mut BTreeSet<PathBuf>) -> Option<PathBuf> {
    if !specifier.starts_with("./") && !specifier.starts_with("../") {
        return None;
    }
    let base = from.parent()?.join(specifier);
    let mut candidates = vec![base.clone()];
    for ext in ["tsx", "ts", "jsx", "js"] {
        candidates.push(PathBuf::from(format!("{}.{ext}", base.display())));
    }
    for ext in ["tsx", "ts", "jsx", "js"] {
        candidates.push(base.join(format!("index.{ext}")));
    }
    for candidate in &candidates {
        read.insert(candidate.clone());
    }
    candidates.into_iter().find(|candidate| candidate.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lxapp(pages: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let entries: Vec<Value> = pages
            .iter()
            .filter(|(path, _)| path.ends_with("index.tsx") || path.ends_with("index.html"))
            .map(|(path, _)| {
                let base = crate::lxapp::framework::page_base(path);
                serde_json::json!({ "name": base.replace('/', "-"), "path": base })
            })
            .collect();
        fs::write(
            dir.path().join("lxapp.json"),
            serde_json::json!({ "appId": "t", "version": "1.0.0", "pages": entries }).to_string(),
        )
        .unwrap();
        for (path, text) in pages {
            let path = dir.path().join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        dir
    }

    fn renderer(pages: &[(&str, &str)]) -> Result<Renderer> {
        lxapp_renderer(lxapp(pages).path())
    }

    #[test]
    fn named_render_pages_are_gpu() {
        assert_eq!(
            renderer(&[
                (
                    "a/index.tsx",
                    "export function render() { return <view/>; }"
                ),
                (
                    "b/index.tsx",
                    "const render = () => null;\nexport { render };"
                ),
                ("c/index.tsx", "export const render = () => null;"),
            ])
            .unwrap(),
            Renderer::Gpu
        );
    }

    #[test]
    fn re_exported_render_counts() {
        assert_eq!(
            renderer(&[
                ("a/index.tsx", "export { render } from './view';"),
                (
                    "b/index.tsx",
                    "function view() { return null; }\nexport { view as render };"
                ),
                ("c/index.tsx", "export * from './impl';"),
                ("c/impl.tsx", "export function render() { return null; }"),
            ])
            .unwrap(),
            Renderer::Gpu
        );
    }

    #[test]
    fn default_component_pages_are_webview() {
        assert_eq!(
            renderer(&[
                (
                    "a/index.tsx",
                    "export default function Page() { return <div/>; }"
                ),
                (
                    "b/index.tsx",
                    "function Page() { return null; }\nexport default memo(Page);"
                ),
                ("c/index.tsx", "export { default } from './page';"),
                (
                    "d/index.tsx",
                    "const Page = () => null;\nexport { Page as default };"
                ),
            ])
            .unwrap(),
            Renderer::Webview
        );
    }

    #[test]
    fn html_pages_are_webview() {
        assert_eq!(
            renderer(&[("a/index.html", "<!doctype html><html></html>")]).unwrap(),
            Renderer::Webview
        );
    }

    #[test]
    fn disagreeing_pages_are_listed_in_two_columns() {
        let message = renderer(&[
            ("a/index.tsx", "export function render() { return null; }"),
            (
                "b/index.tsx",
                "export default function Page() { return null; }",
            ),
            ("c/index.html", "<html></html>"),
        ])
        .unwrap_err()
        .to_string();
        assert!(message.contains("an lxapp uses one renderer"), "{message}");
        let lines: Vec<&str> = message.lines().collect();
        assert!(
            lines[1].starts_with("these pages export `render`:"),
            "{message}"
        );
        assert!(
            lines[1].ends_with("these export a default component:"),
            "{message}"
        );
        assert!(lines[2].starts_with("  a/index.tsx"), "{message}");
        assert!(lines[2].ends_with("  b/index.tsx"), "{message}");
        assert!(
            lines[3].trim_start().starts_with("c/index.html (HTML)"),
            "{message}"
        );
        assert_eq!(
            lines[2].find("b/index.tsx"),
            lines[3].find("c/index.html"),
            "{message}"
        );
    }

    #[test]
    fn a_view_with_both_or_neither_export_names_the_file_and_both_shapes() {
        let both = renderer(&[(
            "a/index.tsx",
            "export function render() { return null; }\nexport default function Page() { return null; }",
        )])
        .unwrap_err()
        .to_string();
        assert!(both.starts_with("a/index.tsx: exports both"), "{both}");
        assert!(both.contains("export function render()"), "{both}");
        assert!(both.contains("export default function Page()"), "{both}");

        let neither = renderer(&[(
            "a/index.tsx",
            "export const view = 1;\nexport type render = 1;",
        )])
        .unwrap_err()
        .to_string();
        assert!(
            neither.starts_with("a/index.tsx: exports neither"),
            "{neither}"
        );
        assert!(neither.contains("export function render()"), "{neither}");
    }

    #[test]
    fn pages_without_a_view_file_give_no_evidence() {
        let dir = lxapp(&[("a/index.tsx", "export function render() { return null; }")]);
        let manifest = r#"{"appId":"t","version":"1.0.0","pages":[{"name":"a","path":"a/index"},{"name":"logic","path":"logic/index"}]}"#;
        fs::write(dir.path().join("lxapp.json"), manifest).unwrap();
        assert_eq!(lxapp_renderer(dir.path()).unwrap(), Renderer::Gpu);
        let only_logic =
            r#"{"appId":"t","version":"1.0.0","pages":[{"name":"logic","path":"logic/index"}]}"#;
        fs::write(dir.path().join("lxapp.json"), only_logic).unwrap();
        assert_eq!(lxapp_renderer(dir.path()).unwrap(), Renderer::Webview);
    }

    #[test]
    fn an_edited_view_is_read_again() {
        let dir = lxapp(&[("a/index.tsx", "export function render() { return null; }")]);
        assert_eq!(lxapp_renderer(dir.path()).unwrap(), Renderer::Gpu);
        fs::write(
            dir.path().join("a/index.tsx"),
            "export default function Page() { return null; } // longer, so the length differs",
        )
        .unwrap();
        assert_eq!(lxapp_renderer(dir.path()).unwrap(), Renderer::Webview);
    }

    #[test]
    fn a_built_manifest_names_its_renderer() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("lxapp.json"),
            r#"{"renderer":"gpu","pages":[{"name":"a","path":"a/index.js"}]}"#,
        )
        .unwrap();
        assert_eq!(lxapp_renderer(dir.path()).unwrap(), Renderer::Gpu);
    }
}
