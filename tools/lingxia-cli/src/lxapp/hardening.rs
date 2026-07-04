use crate::lxapp::project::Project;
use anyhow::{Context, Result, anyhow, bail};
use oxc_allocator::Allocator;
use oxc_codegen::{Codegen, CodegenOptions};
use oxc_minifier::{
    CompressOptions, MangleOptions, MangleOptionsKeepNames, Minifier, MinifierOptions,
};
use oxc_parser::Parser;
use oxc_span::SourceType;
use serde::Serialize;
use sha2::Digest;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) const INTEGRITY_MANIFEST: &str = "lxapp.integrity.json";

pub(crate) fn harden_release_output(project: &Project) -> Result<()> {
    harden_text_artifacts(&project.output_dir)?;
    audit_release_output(project)?;
    write_integrity_manifest(project)?;
    Ok(())
}

pub(crate) fn harden_javascript_source(path: &Path, source: &str) -> Result<String> {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(path)
        .map_err(|_| anyhow!("Unsupported JavaScript release artifact {}", path.display()))?;
    minify_javascript_with_source_type(&allocator, path, source, source_type)
}

pub(crate) fn harden_logic_bundle(source: &str) -> Result<String> {
    let allocator = Allocator::default();
    minify_javascript_with_source_type(
        &allocator,
        Path::new("logic.js"),
        source,
        SourceType::script(),
    )
}

fn minify_javascript_with_source_type<'a>(
    allocator: &'a Allocator,
    path: &Path,
    source: &'a str,
    source_type: SourceType,
) -> Result<String> {
    let parse_result = Parser::new(allocator, source, source_type).parse();
    if !parse_result.diagnostics.is_empty() {
        bail!(
            "Failed to parse release JavaScript artifact {}: {}",
            path.display(),
            format_diagnostics(&parse_result.diagnostics)
        );
    }

    let mut program = parse_result.program;
    let minifier_return = Minifier::new(MinifierOptions {
        compress: Some(CompressOptions {
            drop_console: true,
            drop_debugger: true,
            ..CompressOptions::smallest()
        }),
        mangle: Some(MangleOptions {
            top_level: Some(true),
            keep_names: MangleOptionsKeepNames {
                function: false,
                class: false,
            },
            debug: false,
        }),
    })
    .minify(allocator, &mut program);

    let output = Codegen::new()
        .with_options(CodegenOptions::minify())
        .with_scoping(minifier_return.scoping)
        .with_private_member_mappings(minifier_return.class_private_mappings)
        .build(&program)
        .code;
    Ok(output)
}

fn harden_text_artifacts(output_dir: &Path) -> Result<()> {
    for path in collect_files(output_dir)? {
        match extension(&path).as_deref() {
            Some("js") | Some("mjs") | Some("cjs") => {
                let source = fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read {}", path.display()))?;
                let hardened = harden_javascript_source(&path, &source)?;
                fs::write(&path, hardened)
                    .with_context(|| format!("Failed to write {}", path.display()))?;
            }
            Some("html") | Some("htm") => {
                let source = fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read {}", path.display()))?;
                fs::write(&path, minify_html(&source))
                    .with_context(|| format!("Failed to write {}", path.display()))?;
            }
            Some("ts") | Some("tsx") | Some("vue") => {
                let source = fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read {}", path.display()))?;
                if is_html_document(&source) {
                    fs::write(&path, minify_html(&source))
                        .with_context(|| format!("Failed to write {}", path.display()))?;
                }
            }
            Some("css") => {
                let source = fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read {}", path.display()))?;
                fs::write(&path, minify_css(&source))
                    .with_context(|| format!("Failed to write {}", path.display()))?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn audit_release_output(project: &Project) -> Result<()> {
    let mut violations = Vec::new();
    let project_root = project.root.to_string_lossy().replace('\\', "/");
    for path in collect_files(&project.output_dir)? {
        let rel = relative_path(&project.output_dir, &path);
        match extension(&path).as_deref() {
            Some("map") => {
                violations.push(format!("{rel}: sourcemap files are not allowed in release"));
                continue;
            }
            Some("ts") | Some("tsx") | Some("vue") => {
                let Ok(source) = fs::read_to_string(&path) else {
                    violations.push(format!("{rel}: source files are not allowed in release"));
                    continue;
                };
                if !is_html_document(&source) {
                    violations.push(format!("{rel}: source files are not allowed in release"));
                    continue;
                }
            }
            _ => {}
        }

        if matches!(extension(&path).as_deref(), Some("ts" | "tsx" | "vue")) {
            if let Ok(source) = fs::read_to_string(&path) {
                audit_text_artifact(&rel, &source, &project_root, &mut violations);
            }
            continue;
        }

        if !is_text_artifact(&path) {
            continue;
        }
        let Ok(source) = fs::read_to_string(&path) else {
            continue;
        };
        audit_text_artifact(&rel, &source, &project_root, &mut violations);
    }

    if violations.is_empty() {
        Ok(())
    } else {
        bail!(
            "Release hardening audit failed:\n{}",
            violations
                .into_iter()
                .map(|item| format!("  - {item}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}

fn audit_text_artifact(rel: &str, source: &str, project_root: &str, violations: &mut Vec<String>) {
    for pattern in ["sourceMappingURL", "//# sourceMappingURL", "//#region"] {
        if source.contains(pattern) {
            violations.push(format!(
                "{rel}: contains forbidden release marker {pattern:?}"
            ));
        }
    }
    if !project_root.is_empty() && source.replace('\\', "/").contains(project_root) {
        violations.push(format!("{rel}: contains the project root path"));
    }
}

fn is_html_document(source: &str) -> bool {
    let trimmed = source.trim_start();
    let lower = trimmed
        .chars()
        .take(128)
        .collect::<String>()
        .to_ascii_lowercase();
    lower.starts_with("<!doctype html")
        || lower.starts_with("<html")
        || (lower.contains("<head") && lower.contains("<body"))
}

fn write_integrity_manifest(project: &Project) -> Result<()> {
    let mut files = Vec::new();
    for path in collect_files(&project.output_dir)? {
        let rel = relative_path(&project.output_dir, &path);
        if rel == INTEGRITY_MANIFEST {
            continue;
        }
        let bytes =
            fs::read(&path).with_context(|| format!("Failed to read {}", path.display()))?;
        files.push(IntegrityFile {
            path: rel,
            size: bytes.len() as u64,
            sha256: sha256_hex(&bytes),
        });
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));

    let manifest = IntegrityManifest {
        version: 1,
        algorithm: "sha256",
        files,
    };
    let json = serde_json::to_string_pretty(&manifest)?;
    fs::write(project.output_dir.join(INTEGRITY_MANIFEST), json).with_context(|| {
        format!(
            "Failed to write {}",
            project.output_dir.join(INTEGRITY_MANIFEST).display()
        )
    })?;
    Ok(())
}

#[derive(Serialize)]
struct IntegrityManifest<'a> {
    version: u8,
    algorithm: &'a str,
    files: Vec<IntegrityFile>,
}

#[derive(Serialize)]
struct IntegrityFile {
    path: String,
    size: u64,
    sha256: String,
}

fn collect_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_files_inner(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_files_inner(path: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(path).with_context(|| format!("Failed to read {}", path.display()))? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_files_inner(&path, files)?;
        } else if path.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

fn is_text_artifact(path: &Path) -> bool {
    matches!(
        extension(path).as_deref(),
        Some("js" | "mjs" | "cjs" | "html" | "htm" | "css" | "json" | "txt" | "svg" | "xml")
    )
}

fn extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn minify_html(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut in_quote = None;
    let mut pending_space = false;
    for ch in source.chars() {
        if matches!(in_quote, Some(q) if q == ch) {
            in_quote = None;
            out.push(ch);
            pending_space = false;
            continue;
        }
        if in_quote.is_none() && matches!(ch, '"' | '\'') {
            in_quote = Some(ch);
            if pending_space && needs_space_before(out.chars().last(), Some(ch)) {
                out.push(' ');
            }
            out.push(ch);
            pending_space = false;
            continue;
        }
        if in_quote.is_none() && ch.is_whitespace() {
            pending_space = true;
            continue;
        }
        if pending_space && needs_space_before(out.chars().last(), Some(ch)) {
            out.push(' ');
        }
        out.push(ch);
        pending_space = false;
    }
    out
}

fn minify_css(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut in_comment = false;
    let mut pending_space = false;
    while let Some(ch) = chars.next() {
        if in_comment {
            if ch == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_comment = false;
            }
            continue;
        }
        if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            in_comment = true;
            continue;
        }
        if ch.is_whitespace() {
            pending_space = true;
            continue;
        }
        if pending_space && needs_space_before(out.chars().last(), Some(ch)) {
            out.push(' ');
        }
        if matches!(ch, '{' | '}' | ':' | ';' | ',' | '>' | '+') && out.ends_with(' ') {
            out.pop();
        }
        out.push(ch);
        pending_space = false;
    }
    out
}

fn needs_space_before(prev: Option<char>, next: Option<char>) -> bool {
    matches!(
        (prev, next),
        (Some(a), Some(b))
            if (a.is_ascii_alphanumeric() || matches!(a, '_' | '-'))
                && (b.is_ascii_alphanumeric() || matches!(b, '_' | '-'))
    )
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = sha2::Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn format_diagnostics<T: std::fmt::Debug>(diagnostics: &[T]) -> String {
    diagnostics
        .iter()
        .map(|error| format!("{error:?}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lxapp::framework::ProjectFramework;
    use crate::lxapp::project::{Project, ProjectKind};
    use tempfile::tempdir;

    fn project(root: &Path) -> Project {
        Project {
            root: root.to_path_buf(),
            kind: ProjectKind::LxApp,
            framework: ProjectFramework::Html,
            output_dir: root.join("dist"),
            pages: Vec::new(),
            logic_entry: Some("logic.js".to_string()),
            plugin_id: None,
            package_name: Some("demo".to_string()),
            version: "1.0.0".to_string(),
        }
    }

    #[test]
    fn hardens_javascript_and_drops_console() {
        let source = "function verboseName(value) { console.log(value); return value + 1; }\nverboseName(1);";
        let output = harden_javascript_source(Path::new("app.js"), source).unwrap();
        assert!(!output.contains("verboseName"));
        assert!(!output.contains("console.log"));
        assert!(!output.contains('\n'));
    }

    #[test]
    fn release_audit_rejects_sourcemaps_and_source_markers() {
        let temp = tempdir().unwrap();
        let project = project(temp.path());
        fs::create_dir_all(&project.output_dir).unwrap();
        fs::write(
            project.output_dir.join("view.js"),
            "console.log(1)\n//# sourceMappingURL=view.js.map",
        )
        .unwrap();
        fs::write(project.output_dir.join("view.js.map"), "{}").unwrap();

        let err = audit_release_output(&project).unwrap_err().to_string();
        assert!(err.contains("sourcemap"));
        assert!(err.contains("sourceMappingURL"));
    }

    #[test]
    fn release_allows_generated_react_page_documents() {
        let temp = tempdir().unwrap();
        let project = project(temp.path());
        fs::create_dir_all(project.output_dir.join("pages/home")).unwrap();
        fs::write(
            project.output_dir.join("pages/home/index.tsx"),
            "<!doctype html>\n<html>\n  <head></head>\n  <body>\n    <div id=\"root\"></div>\n  </body>\n</html>\n",
        )
        .unwrap();
        fs::write(
            project.output_dir.join("logic.js"),
            "console.log('debug'); Page({});",
        )
        .unwrap();

        harden_release_output(&project).unwrap();

        let page = fs::read_to_string(project.output_dir.join("pages/home/index.tsx")).unwrap();
        let logic = fs::read_to_string(project.output_dir.join("logic.js")).unwrap();
        assert!(page.starts_with("<!doctype html><html>"));
        assert!(!logic.contains("console.log"));
        assert!(project.output_dir.join(INTEGRITY_MANIFEST).is_file());
    }

    #[test]
    fn release_allows_generated_vue_page_documents() {
        let temp = tempdir().unwrap();
        let project = project(temp.path());
        fs::create_dir_all(project.output_dir.join("pages/home")).unwrap();
        fs::write(
            project.output_dir.join("pages/home/index.vue"),
            "<html>\n  <head></head>\n  <body><div id=\"root\"></div></body>\n</html>\n",
        )
        .unwrap();

        harden_release_output(&project).unwrap();

        let page = fs::read_to_string(project.output_dir.join("pages/home/index.vue")).unwrap();
        assert_eq!(
            page,
            "<html><head></head><body><div id=\"root\"></div></body></html>"
        );
    }

    #[test]
    fn release_rejects_real_tsx_source_files() {
        let temp = tempdir().unwrap();
        let project = project(temp.path());
        fs::create_dir_all(project.output_dir.join("pages/home")).unwrap();
        fs::write(
            project.output_dir.join("pages/home/index.tsx"),
            "export default function Home() { return <div />; }",
        )
        .unwrap();

        let err = harden_release_output(&project).unwrap_err().to_string();
        assert!(err.contains("source files are not allowed in release"));
    }

    #[test]
    fn release_rejects_real_vue_source_files() {
        let temp = tempdir().unwrap();
        let project = project(temp.path());
        fs::create_dir_all(project.output_dir.join("pages/home")).unwrap();
        fs::write(
            project.output_dir.join("pages/home/index.vue"),
            "<template><div /></template><script setup>console.log('debug')</script>",
        )
        .unwrap();

        let err = harden_release_output(&project).unwrap_err().to_string();
        assert!(err.contains("source files are not allowed in release"));
    }

    #[test]
    fn writes_integrity_manifest_with_hashes() {
        let temp = tempdir().unwrap();
        let project = project(temp.path());
        fs::create_dir_all(&project.output_dir).unwrap();
        fs::write(project.output_dir.join("logic.js"), "(()=>{})();").unwrap();

        write_integrity_manifest(&project).unwrap();

        let manifest = fs::read_to_string(project.output_dir.join(INTEGRITY_MANIFEST)).unwrap();
        assert!(manifest.contains("\"algorithm\": \"sha256\""));
        assert!(manifest.contains("\"path\": \"logic.js\""));
    }
}
