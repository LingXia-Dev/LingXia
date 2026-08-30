use anyhow::{Context, Result, bail};
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, BindingPattern, Expression, ImportDeclarationSpecifier, ModuleExportName, Statement,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_span::SourceType;
use std::collections::HashMap;
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
    let mut unscannable = Vec::new();
    for path in collect_files(output_dir)? {
        let rel = relative_path(output_dir, &path);
        match extension(&path).as_deref() {
            Some("html" | "htm") => {
                let source = fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read {}", path.display()))?;
                scan_html(&rel, &source, &mut findings, &mut unscannable);
            }
            Some("js" | "mjs" | "cjs") => {
                let source = fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read {}", path.display()))?;
                scan_javascript(&rel, &source, 0, &source, &mut findings, &mut unscannable);
            }
            Some("tsx" | "vue" | "ts") => {
                let source = fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read {}", path.display()))?;
                if is_html_document(&source) {
                    scan_html(&rel, &source, &mut findings, &mut unscannable);
                }
            }
            _ => {}
        }
    }

    if !unscannable.is_empty() {
        unscannable.sort();
        unscannable.dedup();
        eprintln!(
            "warning: media audit could not parse {} bundle file(s); they were not checked:\n{}",
            unscannable.len(),
            unscannable
                .iter()
                .map(|path| format!("  {path}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
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
    BareLxVideo,
}

impl MediaKind {
    fn message(self) -> &'static str {
        match self {
            Self::VideoElement => {
                "`<video>` is not allowed in an lxapp View. Use `<lx-video>` as a direct child of `<lx-native-root>`; live/pushed streams go through `lx.createVideoContext(id).setStreamSource(...)`"
            }
            Self::AudioElement => {
                "`<audio>` is not allowed in an lxapp View. Audio playback is not available yet; `lx.audio` is planned"
            }
            Self::NewAudio => {
                "`new Audio()` is not allowed in an lxapp View. Audio playback is not available yet; `lx.audio` is planned"
            }
            Self::BareLxVideo => {
                "bare `<lx-video>` is not allowed. Make it a direct child of `<lx-native-root>`"
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

fn scan_html(path: &str, source: &str, findings: &mut Vec<Finding>, unscannable: &mut Vec<String>) {
    let bytes = source.as_bytes();
    let mut i = 0;
    let mut stack: Vec<String> = Vec::new();
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

        if is_close {
            if let Some(position) = stack.iter().rposition(|open| open == &name_lower) {
                stack.truncate(position);
            }
            i += 1;
            continue;
        }

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
            "lx-video" if stack.last().map(String::as_str) != Some("lx-native-root") => findings
                .push(Finding {
                    path: path.to_string(),
                    line: line_number(source, i),
                    kind: MediaKind::BareLxVideo,
                }),
            "script" => {
                if let Some(content) = script_body(source, name_start) {
                    scan_javascript(
                        path,
                        content.body,
                        content.start,
                        source,
                        findings,
                        unscannable,
                    );
                    i = content.end;
                    continue;
                }
            }
            "style" => {
                // `<style/>` closes itself; skipping to the next `</style>`
                // would step over everything between the two.
                if let Some(end) = style_body_end(source, name_start) {
                    i = end;
                    continue;
                }
            }
            _ => {}
        }

        if let Some(tag_end) = source[name_start..].find('>') {
            let open_end = name_start + tag_end;
            if !source[..open_end].trim_end().ends_with('/') {
                stack.push(name_lower);
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

/// End of a `<style>` element's body, or `None` when it is self-closing.
fn style_body_end(source: &str, name_start: usize) -> Option<usize> {
    let rel_gt = source[name_start..].find('>')?;
    let open_end = name_start + rel_gt;
    if source[..open_end].trim_end().ends_with('/') {
        return None;
    }
    find_closing_tag(source, open_end + 1, "style")
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
    unscannable: &mut Vec<String>,
) {
    if source.trim().is_empty() {
        return;
    }

    let allocator = Allocator::default();
    // `.cjs` and a classic inline `<script>` are scripts, not modules; parsing
    // them as modules fails on `with`, on a bare `return`, and on the sloppy
    // syntax a legacy chunk is allowed to use.
    let source_type = if path.ends_with(".cjs") {
        SourceType::cjs()
    } else {
        SourceType::mjs()
    };
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.diagnostics.is_empty() {
        // Retry as the other goal before giving up: a chunk's extension does
        // not always match how it was written.
        let fallback = Parser::new(&allocator, source, SourceType::cjs()).parse();
        if fallback.diagnostics.is_empty() {
            let factory_locals = collect_factory_locals(&fallback.program);
            let mut visitor = MediaVisitor {
                path,
                base_offset,
                line_source,
                findings,
                factory_locals: &factory_locals,
            };
            visitor.visit_program(&fallback.program);
            return;
        }
        // Say so rather than pass: a file this scan could not read is not a
        // file it cleared.
        unscannable.push(path.to_string());
        return;
    }
    let factory_locals = collect_factory_locals(&parsed.program);
    let mut visitor = MediaVisitor {
        path,
        base_offset,
        line_source,
        findings,
        factory_locals: &factory_locals,
    };
    visitor.visit_program(&parsed.program);
}

struct FactoryOrigin {
    imported: Option<String>,
    source: String,
}

struct MediaVisitor<'a, 'b> {
    path: &'a str,
    base_offset: usize,
    line_source: &'a str,
    findings: &'b mut Vec<Finding>,
    factory_locals: &'a HashMap<String, FactoryOrigin>,
}

impl MediaVisitor<'_, '_> {
    fn push(&mut self, offset: usize, kind: MediaKind) {
        self.findings.push(Finding {
            path: self.path.to_string(),
            line: line_number(self.line_source, self.base_offset + offset),
            kind,
        });
    }

    /// Markup handed to an HTML sink, whether written inline or built up.
    fn scan_markup_expression(&mut self, expression: &Expression<'_>) {
        match unwrap_expression(expression) {
            Expression::StringLiteral(literal) => {
                self.scan_markup_string(literal.value.as_str(), literal.span.start as usize);
            }
            Expression::TemplateLiteral(literal) => {
                for quasi in &literal.quasis {
                    let text = quasi
                        .value
                        .cooked
                        .as_deref()
                        .unwrap_or(quasi.value.raw.as_str());
                    self.scan_markup_string(text, quasi.span.start as usize);
                }
            }
            Expression::BinaryExpression(binary) => {
                self.scan_markup_expression(&binary.left);
                self.scan_markup_expression(&binary.right);
            }
            _ => {}
        }
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
        if let Some(kind) = factory_media_kind(
            unwrap_expression(&it.callee),
            &it.arguments,
            self.factory_locals,
        ) {
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

    /// A string is only markup when something parses it as markup. Scanning
    /// every literal flags an error message, a doc comment, or a sanitizer
    /// allowlist that merely names the tag — in a dependency the developer
    /// cannot edit.
    fn visit_assignment_expression(&mut self, it: &oxc_ast::ast::AssignmentExpression<'a>) {
        if let Some(property) = assignment_target_property(&it.left)
            && matches!(property, "innerHTML" | "outerHTML")
        {
            self.scan_markup_expression(&it.right);
        }
        walk::walk_assignment_expression(self, it);
    }

    fn visit_object_property(&mut self, it: &oxc_ast::ast::ObjectProperty<'a>) {
        // React's escape hatch: `dangerouslySetInnerHTML: { __html: ... }`.
        if it.key.static_name().as_deref() == Some("__html") {
            self.scan_markup_expression(&it.value);
        }
        walk::walk_object_property(self, it);
    }
}

fn factory_media_kind(
    callee: &Expression<'_>,
    arguments: &[Argument<'_>],
    factory_locals: &HashMap<String, FactoryOrigin>,
) -> Option<MediaKind> {
    if let Some(kind) = callee_factory_kind(callee, factory_locals) {
        let arg_index = match kind {
            ElementFactory::TagFirst => 0,
            ElementFactory::TagSecond => 1,
        };
        return media_element_kind(string_argument(arguments.get(arg_index)?)?);
    }
    None
}

fn callee_factory_kind(
    callee: &Expression<'_>,
    factory_locals: &HashMap<String, FactoryOrigin>,
) -> Option<ElementFactory> {
    if let Some(name) = callee_leaf_name(callee)
        && let Some(kind) = element_factory_kind(name)
    {
        return Some(kind);
    }

    let Expression::Identifier(identifier) = callee else {
        return None;
    };
    let origin = factory_locals.get(identifier.name.as_str())?;
    if origin
        .imported
        .as_deref()
        .and_then(element_factory_kind)
        .is_some()
        || is_factory_module(&origin.source)
    {
        return Some(ElementFactory::TagFirst);
    }
    None
}

/// Locals that hold a JSX factory, by where they came from.
///
/// A bundler renames the factory to a single letter, so the call site alone
/// says nothing — `e("video", {…})` and `t("video", {count})` are the same
/// shape. Only provenance separates them, so every alias is traced back to an
/// import or to another name already known to hold a factory.
struct FactoryCollector {
    locals: HashMap<String, FactoryOrigin>,
}

impl<'a> Visit<'a> for FactoryCollector {
    fn visit_variable_declarator(&mut self, it: &oxc_ast::ast::VariableDeclarator<'a>) {
        if let Some(init) = &it.init
            && let BindingPattern::BindingIdentifier(id) = &it.id
            && let Some(name) = callee_leaf_name(unwrap_expression(init))
            && element_factory_kind(name).is_some()
        {
            self.locals.insert(
                id.name.as_str().to_string(),
                FactoryOrigin {
                    imported: Some(name.to_string()),
                    source: String::new(),
                },
            );
        }
        walk::walk_variable_declarator(self, it);
    }
}

fn collect_factory_locals(program: &oxc_ast::ast::Program<'_>) -> HashMap<String, FactoryOrigin> {
    let mut collector = FactoryCollector {
        locals: HashMap::new(),
    };
    collector.visit_program(program);
    let mut locals = collector.locals;
    for statement in &program.body {
        let Statement::ImportDeclaration(declaration) = statement else {
            continue;
        };
        let source = declaration.source.value.as_str();
        let Some(specifiers) = &declaration.specifiers else {
            continue;
        };
        for specifier in specifiers {
            match specifier {
                ImportDeclarationSpecifier::ImportSpecifier(spec) => {
                    locals.insert(
                        spec.local.name.as_str().to_string(),
                        FactoryOrigin {
                            imported: export_name(&spec.imported),
                            source: source.to_string(),
                        },
                    );
                }
                ImportDeclarationSpecifier::ImportDefaultSpecifier(spec) => {
                    locals.insert(
                        spec.local.name.as_str().to_string(),
                        FactoryOrigin {
                            imported: Some("default".to_string()),
                            source: source.to_string(),
                        },
                    );
                }
                ImportDeclarationSpecifier::ImportNamespaceSpecifier(_) => {}
            }
        }
    }
    locals
}

fn export_name(name: &ModuleExportName<'_>) -> Option<String> {
    match name {
        ModuleExportName::IdentifierName(identifier) => Some(identifier.name.as_str().to_string()),
        ModuleExportName::IdentifierReference(identifier) => {
            Some(identifier.name.as_str().to_string())
        }
        ModuleExportName::StringLiteral(literal) => Some(literal.value.as_str().to_string()),
    }
}

fn is_factory_module(source: &str) -> bool {
    let source = source.replace('\\', "/").to_ascii_lowercase();
    const MARKERS: &[&str] = &[
        "react/jsx-runtime",
        "react/jsx-dev-runtime",
        "react-runtime",
        "preact/jsx-runtime",
        "preact/compat",
        "vue-runtime",
        "@vue/runtime",
        "solid-js/jsx-runtime",
    ];
    MARKERS.iter().any(|marker| source.contains(marker))
        || matches!(source.as_str(), "react" | "vue" | "preact" | "solid-js")
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
        // `(0, ns.jsx)(...)` — what esbuild and Rollup emit for a namespace
        // call. The callee is the last element, not the sequence.
        Expression::SequenceExpression(expr) => match expr.expressions.last() {
            Some(last) => unwrap_expression(last),
            None => expression,
        },
        Expression::TSAsExpression(expr) => unwrap_expression(&expr.expression),
        Expression::TSSatisfiesExpression(expr) => unwrap_expression(&expr.expression),
        Expression::TSTypeAssertion(expr) => unwrap_expression(&expr.expression),
        Expression::TSNonNullExpression(expr) => unwrap_expression(&expr.expression),
        _ => expression,
    }
}

/// The property name an assignment writes to, for `a.b = …` and `a["b"] = …`.
fn assignment_target_property<'a>(
    target: &'a oxc_ast::ast::AssignmentTarget<'a>,
) -> Option<&'a str> {
    match target {
        oxc_ast::ast::AssignmentTarget::StaticMemberExpression(member) => {
            Some(member.property.name.as_str())
        }
        oxc_ast::ast::AssignmentTarget::ComputedMemberExpression(member) => {
            match unwrap_expression(&member.expression) {
                Expression::StringLiteral(literal) => Some(literal.value.as_str()),
                _ => None,
            }
        }
        _ => None,
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

    /// A call is a factory call because of where its callee came from, never
    /// because the arguments line up. `t("video", { count })` is an i18n key.
    #[test]
    fn leaves_calls_that_only_look_like_a_factory_alone() {
        audit_one(
            "assets/app.js",
            concat!(
                "const label = t(\"video\", { count: 2 });\n",
                "track(\"video\", null);\n",
                "inject(\"audio\", undefined);\n",
            ),
        )
        .expect("a string argument is not a factory call");
    }

    /// The scan covers vendor chunks, so a string that merely names the tag —
    /// an error message, a sanitizer allowlist — must not fail the build.
    #[test]
    fn leaves_strings_that_are_never_parsed_as_markup_alone() {
        audit_one(
            "assets/vendor.js",
            concat!(
                "const msg = \"expected <video> element\";\n",
                "const allow = [\"<video\", \"<audio\"];\n",
            ),
        )
        .expect("a string is only markup when something parses it as markup");
    }

    #[test]
    fn rejects_markup_assigned_to_an_html_sink() {
        let err = audit_one(
            "assets/app.js",
            "el.innerHTML = \"<video src=x></video>\";\n",
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("assets/app.js:1:"), "{err}");
    }

    /// `(0, ns.jsx)(...)` is what esbuild and Rollup emit for a namespace call.
    #[test]
    fn rejects_a_sequence_wrapped_factory_call() {
        let err = audit_one(
            "assets/chunk.js",
            concat!(
                "var m = require(\"react/jsx-runtime\");\n",
                "export const B = () => (0, m.jsx)(\"video\", { src: \"./b.mp4\" });\n",
            ),
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("assets/chunk.js:2:"), "{err}");
    }

    /// Minification renames the factory to a local, so the alias is traced
    /// back to the import rather than guessed from the call.
    #[test]
    fn rejects_a_factory_reached_through_a_local_alias() {
        let err = audit_one(
            "assets/min.js",
            concat!(
                "import * as m from \"react/jsx-runtime\";\n",
                "var e = m.jsx;\n",
                "export const C = () => e(\"video\", { src: \"./c.mp4\" });\n",
            ),
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("assets/min.js:3:"), "{err}");
    }

    /// `<style/>` closes itself; skipping to the next `</style>` would step
    /// over everything between the two.
    #[test]
    fn rejects_video_after_a_self_closing_style() {
        let err = audit_one(
            "pages/home/index.html",
            "<style/>\n<video src=\"x\"></video>\n<style>.a{}</style>\n",
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("pages/home/index.html:2:"), "{err}");
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
    fn allows_root_wrapped_lx_video() {
        audit_one(
            "pages/player/index.html",
            r#"<lx-native-root class="player"><lx-video src="./clip.mp4" controls="false"></lx-video></lx-native-root>"#,
        )
        .unwrap();
    }

    #[test]
    fn rejects_bare_lx_video_but_not_media_swiper_data() {
        let err = audit_one(
            "index.html",
            r#"<lx-video id="hero" src="./clip.mp4"></lx-video>
<script>const item = { type: "video", src: "./clip.mp4" };</script>"#,
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("direct child"), "{err}");
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
    fn rejects_minified_jsx_and_vite_runtime_chunks() {
        let renamed = audit_one(
            "view.js",
            "import { jsx as e } from 'react/jsx-runtime';\ne(\"video\", { src: \"./a.mp4\" });\n",
        )
        .unwrap_err()
        .to_string();
        assert!(renamed.contains("`<video>`"), "{renamed}");

        let chunk = audit_one(
            "pages/home/home.js",
            "import { A as e } from '../../assets/react-runtime-a1b2c3.js';\ne(\"video\", { src: \"./a.mp4\" });\n",
        )
        .unwrap_err()
        .to_string();
        assert!(chunk.contains("`<video>`"), "{chunk}");

        let one_arg = audit_one(
            "pages/home/home.js",
            "import { A as e } from '../../assets/react-runtime-a1b2c3.js';\ne(\"video\");\n",
        )
        .unwrap_err()
        .to_string();
        assert!(one_arg.contains("`<video>`"), "{one_arg}");

        // A factory the bundler inlined as a local function carries no
        // provenance, and its call is indistinguishable from `t("audio", {…})`
        // — an i18n key, an analytics event. Matching it would fail builds
        // over a dependency's string, so this one is a known gap rather than
        // a guess.
        audit_one(
            "assets/page-abc.js",
            "function e(t,n){return t}\ne(\"audio\",{src:\"./beep.mp3\"});\n",
        )
        .expect("an inlined factory has no provenance to match on");
    }

    #[test]
    fn allows_one_arg_helper_named_video() {
        audit_one(
            "view.js",
            "export const label = t => t;\nlabel(\"video\");\n",
        )
        .unwrap();
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
