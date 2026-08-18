use anyhow::{Context, Result, bail};
use oxc_allocator::Allocator;
use oxc_ast::ast::{Argument, Expression};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_span::SourceType;
use std::fs;
use std::path::{Path, PathBuf};

/// Scan a built lxapp `dist/` for Web media the View must not own.
///
/// `<video>` / `<audio>` / `new Audio()` belong to the native player
/// (`<lx-video>` / `lx.createVideoContext`) or are not available yet
/// (`lx.audio`). The check walks the **built** bundle so a tag inside a
/// third-party component is caught the same way as authored markup.
pub(crate) fn audit_output_media(output_dir: &Path) -> Result<()> {
    let mut findings = Vec::new();
    for path in collect_files(output_dir)? {
        let rel = relative_path(output_dir, &path);
        match extension(&path).as_deref() {
            Some("html" | "htm") => {
                let source = fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read {}", path.display()))?;
                scan_html(&rel, &source, &mut findings);
            }
            Some("js" | "mjs" | "cjs") => {
                let source = fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read {}", path.display()))?;
                scan_javascript(&rel, &source, 0, &source, &mut findings);
            }
            Some("tsx" | "vue" | "ts") => {
                let source = fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read {}", path.display()))?;
                if is_html_document(&source) {
                    scan_html(&rel, &source, &mut findings);
                }
            }
            _ => {}
        }
    }

    if findings.is_empty() {
        return Ok(());
    }

    findings.sort();
    findings.dedup();

    bail!(
        "LxApp View media is native-owned; Web `<video>` / `<audio>` / `new Audio()` are rejected:\n{}",
        findings
            .into_iter()
            .map(|finding| format!(
                "  {}:{}: {}",
                finding.path,
                finding.line,
                finding.kind.message()
            ))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum MediaKind {
    VideoElement,
    AudioElement,
    NewAudio,
}

impl MediaKind {
    fn message(self) -> &'static str {
        match self {
            Self::VideoElement => {
                "`<video>` is not allowed in an lxapp View. Use `<lx-video>`; live/pushed streams go through `lx.createVideoContext(id).setStreamSource(...)`"
            }
            Self::AudioElement => {
                "`<audio>` is not allowed in an lxapp View. Audio playback is not available yet; `lx.audio` is planned"
            }
            Self::NewAudio => {
                "`new Audio()` is not allowed in an lxapp View. Audio playback is not available yet; `lx.audio` is planned"
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Finding {
    path: String,
    line: usize,
    kind: MediaKind,
}

fn scan_html(path: &str, source: &str, findings: &mut Vec<Finding>) {
    let bytes = source.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        // `i` is on ASCII `<`, so every `&source[i..]` slice below is a
        // char boundary even when the page contains CJK or other multibyte text.
        if starts_with_ignore_ascii_case(&source[i..], "<!--") {
            i += 4;
            match source[i..].find("-->") {
                Some(end) => i += end + 3,
                None => break,
            }
            continue;
        }

        let after_lt = i + 1;
        if after_lt >= source.len() {
            break;
        }
        let (is_close, name_start) = if source.as_bytes()[after_lt] == b'/' {
            (true, after_lt + 1)
        } else {
            (false, after_lt)
        };
        let name = read_tag_name(&source[name_start..]);
        if name.is_empty() {
            i += 1;
            continue;
        }
        let name_lower = name.to_ascii_lowercase();

        if !is_close {
            match name_lower.as_str() {
                "video" => findings.push(Finding {
                    path: path.to_string(),
                    line: line_number(source, i),
                    kind: MediaKind::VideoElement,
                }),
                "audio" => findings.push(Finding {
                    path: path.to_string(),
                    line: line_number(source, i),
                    kind: MediaKind::AudioElement,
                }),
                "script" => {
                    if let Some(content) = script_body(source, name_start) {
                        scan_javascript(path, content.body, content.start, source, findings);
                        i = content.end;
                        continue;
                    }
                }
                "style" => {
                    if let Some(end) = find_closing_tag(source, name_start, "style") {
                        i = end;
                        continue;
                    }
                }
                _ => {}
            }
        }

        i += 1;
    }
}

struct ScriptBody<'a> {
    body: &'a str,
    start: usize,
    end: usize,
}

fn script_body(source: &str, name_start: usize) -> Option<ScriptBody<'_>> {
    let rel_gt = source[name_start..].find('>')?;
    let open_end = name_start + rel_gt;
    if source[..open_end].trim_end().ends_with('/') {
        return None;
    }
    let body_start = open_end + 1;
    let close = find_closing_tag(source, body_start, "script")?;
    let close_start = source[..close].rfind('<').unwrap_or(close);
    Some(ScriptBody {
        body: &source[body_start..close_start],
        start: body_start,
        end: close,
    })
}

fn find_closing_tag(source: &str, from: usize, name: &str) -> Option<usize> {
    let needle = format!("</{name}");
    let rel = find_ignore_ascii_case(&source[from..], &needle)?;
    let close_gt = source[from + rel..].find('>')?;
    Some(from + rel + close_gt + 1)
}

fn scan_javascript(
    path: &str,
    source: &str,
    base_offset: usize,
    line_source: &str,
    findings: &mut Vec<Finding>,
) {
    if source.trim().is_empty() {
        return;
    }

    let allocator = Allocator::default();
    let source_type = SourceType::mjs();
    let parsed = Parser::new(&allocator, source, source_type).parse();
    let mut visitor = MediaVisitor {
        path,
        base_offset,
        line_source,
        findings,
    };
    visitor.visit_program(&parsed.program);
}

struct MediaVisitor<'a, 'b> {
    path: &'a str,
    base_offset: usize,
    line_source: &'a str,
    findings: &'b mut Vec<Finding>,
}

impl MediaVisitor<'_, '_> {
    fn push(&mut self, offset: usize, kind: MediaKind) {
        self.findings.push(Finding {
            path: self.path.to_string(),
            line: line_number(self.line_source, self.base_offset + offset),
            kind,
        });
    }

    fn scan_markup_string(&mut self, value: &str, literal_start: usize) {
        // Report the literal's span, not a cooked-relative offset: those
        // two coordinate spaces diverge as soon as the string has escapes
        // or non-ASCII before the tag.
        for (_rel, kind) in html_media_tag_offsets(value) {
            self.push(literal_start, kind);
        }
    }
}

impl<'a> Visit<'a> for MediaVisitor<'_, '_> {
    fn visit_call_expression(&mut self, it: &oxc_ast::ast::CallExpression<'a>) {
        if let Some(kind) = factory_media_kind(unwrap_expression(&it.callee), &it.arguments) {
            self.push(it.span.start as usize, kind);
        }
        walk::walk_call_expression(self, it);
    }

    fn visit_new_expression(&mut self, it: &oxc_ast::ast::NewExpression<'a>) {
        if callee_leaf_name(unwrap_expression(&it.callee)) == Some("Audio") {
            self.push(it.span.start as usize, MediaKind::NewAudio);
        }
        walk::walk_new_expression(self, it);
    }

    fn visit_string_literal(&mut self, it: &oxc_ast::ast::StringLiteral<'a>) {
        self.scan_markup_string(it.value.as_str(), it.span.start as usize);
        walk::walk_string_literal(self, it);
    }

    fn visit_template_literal(&mut self, it: &oxc_ast::ast::TemplateLiteral<'a>) {
        for quasi in &it.quasis {
            let text = quasi
                .value
                .cooked
                .as_deref()
                .unwrap_or(quasi.value.raw.as_str());
            self.scan_markup_string(text, quasi.span.start as usize);
        }
        walk::walk_template_literal(self, it);
    }
}

fn factory_media_kind(callee: &Expression<'_>, arguments: &[Argument<'_>]) -> Option<MediaKind> {
    let name = callee_leaf_name(callee)?;
    let kind = element_factory_kind(name)?;
    let arg_index = match kind {
        ElementFactory::TagFirst => 0,
        ElementFactory::TagSecond => 1,
    };
    let tag = string_argument(arguments.get(arg_index)?)?;
    media_element_kind(tag)
}

#[derive(Clone, Copy)]
enum ElementFactory {
    TagFirst,
    TagSecond,
}

fn element_factory_kind(name: &str) -> Option<ElementFactory> {
    let trimmed = name.trim_start_matches('_');
    if trimmed.eq_ignore_ascii_case("createElementNS") {
        return Some(ElementFactory::TagSecond);
    }
    if trimmed.eq_ignore_ascii_case("createElement")
        || trimmed.eq_ignore_ascii_case("createElementVNode")
        || trimmed.eq_ignore_ascii_case("createElementBlock")
        || trimmed.eq_ignore_ascii_case("createVNode")
        || trimmed.eq_ignore_ascii_case("jsx")
        || trimmed.eq_ignore_ascii_case("jsxs")
        || trimmed.eq_ignore_ascii_case("jsxDEV")
        || trimmed == "h"
    {
        return Some(ElementFactory::TagFirst);
    }
    None
}

fn callee_leaf_name<'a>(expression: &'a Expression<'a>) -> Option<&'a str> {
    match unwrap_expression(expression) {
        Expression::Identifier(identifier) => Some(identifier.name.as_str()),
        Expression::StaticMemberExpression(member) => Some(member.property.name.as_str()),
        _ => None,
    }
}

fn string_argument<'a>(argument: &'a Argument<'a>) -> Option<&'a str> {
    match argument {
        Argument::StringLiteral(literal) => Some(literal.value.as_str()),
        Argument::TemplateLiteral(template)
            if template.expressions.is_empty() && template.quasis.len() == 1 =>
        {
            template.quasis[0]
                .value
                .cooked
                .as_deref()
                .or(Some(template.quasis[0].value.raw.as_str()))
        }
        _ => None,
    }
}

fn media_element_kind(tag: &str) -> Option<MediaKind> {
    if tag.eq_ignore_ascii_case("video") {
        Some(MediaKind::VideoElement)
    } else if tag.eq_ignore_ascii_case("audio") {
        Some(MediaKind::AudioElement)
    } else {
        None
    }
}

fn html_media_tag_offsets(source: &str) -> Vec<(usize, MediaKind)> {
    let mut out = Vec::new();
    let bytes = source.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        let after = i + 1;
        if after >= source.len() {
            break;
        }
        if source.as_bytes()[after] == b'/' {
            i += 1;
            continue;
        }
        let name = read_tag_name(&source[after..]);
        if let Some(kind) = media_element_kind(name) {
            out.push((i, kind));
        }
        i += 1;
    }
    out
}

fn unwrap_expression<'a>(expression: &'a Expression<'a>) -> &'a Expression<'a> {
    match expression {
        Expression::ParenthesizedExpression(expr) => unwrap_expression(&expr.expression),
        Expression::TSAsExpression(expr) => unwrap_expression(&expr.expression),
        Expression::TSSatisfiesExpression(expr) => unwrap_expression(&expr.expression),
        Expression::TSTypeAssertion(expr) => unwrap_expression(&expr.expression),
        Expression::TSNonNullExpression(expr) => unwrap_expression(&expr.expression),
        _ => expression,
    }
}

fn read_tag_name(source: &str) -> &str {
    let end = source
        .char_indices()
        .find(|(_, ch)| !matches!(ch, 'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | ':'))
        .map(|(idx, _)| idx)
        .unwrap_or(source.len());
    &source[..end]
}

fn starts_with_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    haystack.len() >= needle.len()
        && haystack.as_bytes()[..needle.len()].eq_ignore_ascii_case(needle.as_bytes())
}

fn find_ignore_ascii_case(haystack: &str, needle: &str) -> Option<usize> {
    let needle = needle.as_bytes();
    if needle.is_empty() {
        return Some(0);
    }
    haystack
        .as_bytes()
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle))
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

fn line_number(source: &str, offset: usize) -> usize {
    let mut end = offset.min(source.len());
    while end > 0 && !source.is_char_boundary(end) {
        end -= 1;
    }
    source[..end].bytes().filter(|b| *b == b'\n').count() + 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn audit_one(name: &str, source: &str) -> Result<()> {
        let temp = tempdir().unwrap();
        let path = temp.path().join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, source).unwrap();
        audit_output_media(temp.path())
    }

    #[test]
    fn rejects_html_video_with_file_line_and_replacement() {
        let err = audit_one(
            "pages/home/index.html",
            "<html><body>\n<video src=\"./clip.mp4\"></video>\n</body></html>\n",
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("pages/home/index.html:2:"), "{err}");
        assert!(err.contains("`<video>`"), "{err}");
        assert!(err.contains("<lx-video>"), "{err}");
        assert!(err.contains("setStreamSource"), "{err}");
    }

    #[test]
    fn rejects_html_audio() {
        let err = audit_one("index.html", "<audio src=\"./beep.mp3\"></audio>\n")
            .unwrap_err()
            .to_string();
        assert!(err.contains("index.html:1:"), "{err}");
        assert!(err.contains("`<audio>`"), "{err}");
        assert!(err.contains("`lx.audio` is planned"), "{err}");
    }

    #[test]
    fn allows_lx_video_and_media_swiper_type() {
        audit_one(
            "index.html",
            r#"<lx-video id="hero" src="./clip.mp4"></lx-video>
<script>const item = { type: "video", src: "./clip.mp4" };</script>"#,
        )
        .unwrap();
    }

    #[test]
    fn ignores_html_comment_and_style() {
        audit_one(
            "index.html",
            "<!-- <video src=x></video> -->\n<style>video { color: red; }</style>\n<div></div>\n",
        )
        .unwrap();
    }

    #[test]
    fn rejects_create_element_video_in_bundle_js() {
        let err = audit_one(
            "pages/home/view.js",
            "export function mount(el) {\n  el.appendChild(document.createElement(\"video\"));\n}\n",
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("pages/home/view.js:2:"), "{err}");
        assert!(err.contains("`<video>`"), "{err}");
        assert!(err.contains("<lx-video>"), "{err}");
    }

    #[test]
    fn rejects_jsx_and_vue_factories() {
        let jsx = audit_one("view.js", "import { jsx as _jsx } from 'react/jsx-runtime';\n_jsx(\"video\", { src: \"./a.mp4\" });\n")
            .unwrap_err()
            .to_string();
        assert!(jsx.contains("`<video>`"), "{jsx}");

        let vue = audit_one(
            "chunk.js",
            "import { createElementVNode as _createElementVNode } from 'vue';\n_createElementVNode(\"audio\", null);\n",
        )
        .unwrap_err()
        .to_string();
        assert!(vue.contains("`<audio>`"), "{vue}");

        let block = audit_one(
            "block.js",
            "import { createElementBlock as _createElementBlock } from 'vue';\n_createElementBlock(\"video\", { src: \"./a.mp4\" });\n",
        )
        .unwrap_err()
        .to_string();
        assert!(block.contains("`<video>`"), "{block}");
        assert!(block.contains("<lx-video>"), "{block}");
    }

    #[test]
    fn allows_cjk_html_without_media() {
        audit_one(
            "pages/home/index.html",
            "<!doctype html><html><head><title>首页</title></head><body><div>播放列表</div></body></html>\n",
        )
        .unwrap();
    }

    #[test]
    fn rejects_innerhtml_video_after_cjk_without_panic() {
        let err = audit_one(
            "lib.js",
            "export function render(el) { el.innerHTML = \"播放<video src=x></video>\"; }\n",
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("`<video>`"), "{err}");
        assert!(err.contains("lib.js:"), "{err}");
    }

    #[test]
    fn rejects_new_audio() {
        let err = audit_one(
            "vendor.js",
            "export const beep = () => new Audio(\"./beep.mp3\");\n",
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("vendor.js:1:"), "{err}");
        assert!(err.contains("`new Audio()`"), "{err}");
        assert!(err.contains("`lx.audio` is planned"), "{err}");
    }

    #[test]
    fn rejects_innerhtml_video_string_from_third_party() {
        let err = audit_one(
            "lib.js",
            "export function render(el) { el.innerHTML = \"<video src=x></video>\"; }\n",
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("`<video>`"), "{err}");
    }

    #[test]
    fn allows_create_element_lx_video_and_audio_context() {
        audit_one(
            "view.js",
            r#"document.createElement("lx-video");
document.createElement("div");
new AudioContext();
const kind = "video";
"#,
        )
        .unwrap();
    }

    #[test]
    fn scans_generated_html_document_with_tsx_extension() {
        let err = audit_one(
            "pages/home/index.tsx",
            "<!doctype html><html><body><video></video></body></html>\n",
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("pages/home/index.tsx:1:"), "{err}");
    }
}
