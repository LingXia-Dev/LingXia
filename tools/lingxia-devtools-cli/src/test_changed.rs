//! `--changed [BASE]`: the spec files a change can affect.
//!
//! A spec file is selected when a changed file is in its own import graph,
//! or belongs to a page the spec names (a string literal equal to the page's
//! name or path, in the spec's own test-side modules), or is project-wide
//! (lxdev.json and the files it names, lxapp.json, the app entry, package
//! manifests, root tsconfigs, `--openapi` documents): then every spec is.
//! Over-selecting is safe; missing a spec is not, so anything the graph
//! cannot read is read as "may depend".

use crate::test_bundle::source_name;
use crate::test_select::{Selection, display_path, usage};
use anyhow::{Context, Result};
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    ExportAllDeclaration, ExportFromDeclaration, ImportDeclaration, ImportExpression,
    StringLiteral, TemplateLiteral,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_resolver::{ResolveOptions, Resolver, TsconfigDiscovery};
use oxc_span::SourceType;
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Files that change every spec's outcome, by name at the project root.
const PROJECT_WIDE: &[&str] = &[
    "lxdev.json",
    "lxapp.json",
    "lxapp.ts",
    "lxapp.js",
    "app.ts",
    "app.js",
    "package.json",
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "bun.lockb",
];

const SCRIPT_EXTENSIONS: &[&str] = &["js", "mjs", "cjs", "ts", "mts", "cts", "jsx", "tsx"];

/// Why a spec file was selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    /// A changed file is in its import graph.
    Imports(PathBuf),
    /// A changed file belongs to a page it names.
    Page { page: String, file: PathBuf },
    /// A project-wide file changed.
    ProjectWide(PathBuf),
}

/// What `--changed` found.
#[derive(Debug)]
pub struct ChangedSelection {
    /// The changed files under the project root.
    pub changed: Vec<PathBuf>,
    /// Selected spec files, in the selection's order, each with one reason.
    pub selected: Vec<(PathBuf, Reason)>,
    /// Changed files none of the candidate specs depends on.
    pub unused: Vec<PathBuf>,
}

/// The files that differ from `base` (its merge base with HEAD; `HEAD` for
/// the uncommitted work): committed since, staged, unstaged and untracked,
/// deleted ones included. Only files under `root`.
pub fn changed_files(root: &Path, base: &str) -> Result<Vec<PathBuf>> {
    let toplevel = git(root, &["rev-parse", "--show-toplevel"]).map_err(|_| {
        usage(format!(
            "--changed needs a git repository; {} is not in one",
            root.display()
        ))
    })?;
    let toplevel = canonical(Path::new(toplevel.trim()));
    git(
        root,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{base}^{{commit}}"),
        ],
    )
    .map_err(|_| usage(format!("--changed {base}: no such commit or branch")))?;
    let since = git(root, &["merge-base", base, "HEAD"])
        .with_context(|| format!("--changed {base}: no merge base with HEAD"))?
        .trim()
        .to_string();
    let mut names = git(root, &["diff", "--name-only", "--no-renames", "-z", &since])?;
    names.push_str(&git(
        root,
        &[
            "ls-files",
            "--others",
            "--exclude-standard",
            "-z",
            "--full-name",
        ],
    )?);
    let root = canonical(root);
    let mut files = BTreeSet::new();
    for name in names.split('\0').filter(|name| !name.is_empty()) {
        let path = canonical(&toplevel.join(name));
        if path.starts_with(&root) {
            files.insert(path);
        }
    }
    Ok(files.into_iter().collect())
}

fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .context("could not run git")?;
    if !output.status.success() {
        anyhow::bail!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// A deleted file has no real path: its directory's, joined with its name.
fn canonical(path: &Path) -> PathBuf {
    if let Ok(path) = std::fs::canonicalize(path) {
        return path;
    }
    match (path.parent(), path.file_name()) {
        (Some(parent), Some(name)) => canonical(parent).join(name),
        _ => path.to_path_buf(),
    }
}

/// Select, among `candidates`, the spec files `changed` can affect.
/// `extra_wide` adds project-wide files (the run's `--openapi` documents).
pub fn select(
    root: &Path,
    candidates: &[PathBuf],
    changed: &[PathBuf],
    extra_wide: &[PathBuf],
) -> ChangedSelection {
    let root = canonical(root);
    let changed_set: BTreeSet<&PathBuf> = changed.iter().collect();
    let mut wide = project_wide(&root);
    wide.extend(extra_wide.iter().map(|path| canonical(&root.join(path))));
    let wide_hit = changed.iter().find(|file| wide.contains(*file)).cloned();

    let mut graph = Graph::new(&root);
    let pages = pages(&root, &mut graph);
    let test_dirs = test_dirs(&root, candidates);
    let mut used: BTreeSet<PathBuf> = BTreeSet::new();
    let mut selected = Vec::new();

    for candidate in candidates {
        let spec = canonical(candidate);
        let closure = graph.closure(std::slice::from_ref(&spec));
        let mut reason = wide_hit.clone().map(Reason::ProjectWide);
        for file in closure.iter().filter(|file| changed_set.contains(file)) {
            used.insert(file.clone());
            reason.get_or_insert_with(|| Reason::Imports(file.clone()));
        }
        // The pages the spec names, in its test-side modules only: a
        // product module it imports may mention any page.
        let named: BTreeSet<&String> = closure
            .iter()
            .filter(|module| is_test_side(module, &test_dirs))
            .flat_map(|module| graph.strings(module).iter())
            .collect();
        for page in &pages {
            if !named.iter().any(|literal| page.named_by(literal)) {
                continue;
            }
            for file in changed.iter().filter(|file| page.owns(file)) {
                used.insert(file.clone());
                reason.get_or_insert_with(|| Reason::Page {
                    page: page.name.clone(),
                    file: file.clone(),
                });
            }
        }
        if let Some(reason) = reason {
            selected.push((candidate.clone(), reason));
        }
    }
    if let Some(hit) = &wide_hit {
        used.insert(hit.clone());
    }
    ChangedSelection {
        changed: changed.to_vec(),
        unused: changed
            .iter()
            .filter(|file| !used.contains(*file))
            .cloned()
            .collect(),
        selected,
    }
}

/// Project-wide files: the fixed names, root tsconfigs, and lxdev.json's
/// OpenAPI documents (a routed answer is checked against them).
fn project_wide(root: &Path) -> BTreeSet<PathBuf> {
    let mut files: BTreeSet<PathBuf> = PROJECT_WIDE.iter().map(|name| root.join(name)).collect();
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with("tsconfig") && name.ends_with(".json") {
                files.insert(root.join(name));
            }
        }
    }
    let config = std::fs::read_to_string(root.join("lxdev.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok());
    for document in config.as_ref().map(contract_documents).unwrap_or_default() {
        files.insert(canonical(&root.join(document)));
    }
    files
}

/// lxdev.json's OpenAPI documents: `test.openapi`, and every `--openapi`
/// value of a preset.
fn contract_documents(config: &serde_json::Value) -> Vec<String> {
    let test = config.get("test");
    let mut documents: Vec<String> = test
        .and_then(|test| test.get("openapi"))
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str().map(str::to_string))
        .collect();
    let presets = test
        .and_then(|test| test.get("presets"))
        .and_then(|value| value.as_object());
    for args in presets.into_iter().flat_map(|presets| presets.values()) {
        let args: Vec<&str> = args
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|arg| arg.as_str())
            .collect();
        for (index, arg) in args.iter().enumerate() {
            if let Some(value) = arg.strip_prefix("--openapi=") {
                documents.push(value.to_string());
            } else if *arg == "--openapi"
                && let Some(value) = args.get(index + 1)
            {
                documents.push(value.to_string());
            }
        }
    }
    documents
}

/// Directories holding the candidate specs (and their parents up to the
/// root, exclusive): a module there is test code, not product code.
fn test_dirs(root: &Path, candidates: &[PathBuf]) -> BTreeSet<PathBuf> {
    let mut dirs = BTreeSet::new();
    for candidate in candidates {
        let mut dir = canonical(candidate).parent().map(Path::to_path_buf);
        while let Some(current) = dir {
            if current == root || !current.starts_with(root) {
                break;
            }
            dir = current.parent().map(Path::to_path_buf);
            dirs.insert(current);
        }
    }
    dirs
}

fn is_test_side(module: &Path, test_dirs: &BTreeSet<PathBuf>) -> bool {
    module.parent().is_some_and(|dir| test_dirs.contains(dir))
}

/// One page of an lxapp: its files and how a spec names it.
#[derive(Debug)]
struct Page {
    name: String,
    /// `pages/home/index`: the declared path without extension.
    stem: String,
    /// Its directory, when no other page shares it.
    dir: Option<PathBuf>,
    /// The import graph of its entries (Logic, View, page config).
    files: BTreeSet<PathBuf>,
}

impl Page {
    fn named_by(&self, literal: &str) -> bool {
        let literal = literal.trim_start_matches('/');
        literal == self.name
            || literal == self.stem
            || literal.starts_with(&format!("{}.", self.stem))
    }

    fn owns(&self, file: &Path) -> bool {
        self.files.contains(file) || self.dir.as_ref().is_some_and(|dir| file.starts_with(dir))
    }
}

/// The pages of the project's lxapp (lxapp.json at the root, or one level
/// down for a host app with an embedded lxapp).
fn pages(root: &Path, graph: &mut Graph) -> Vec<Page> {
    let mut manifests = vec![root.join("lxapp.json")];
    if let Ok(entries) = std::fs::read_dir(root) {
        manifests.extend(
            entries
                .flatten()
                .map(|entry| entry.path().join("lxapp.json")),
        );
    }
    let mut pages = Vec::new();
    for manifest in manifests.iter().filter(|path| path.is_file()) {
        let app = manifest.parent().unwrap_or(root);
        let Some(config) = std::fs::read_to_string(manifest)
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        else {
            continue;
        };
        let declared: Vec<(String, String)> = config
            .get("pages")
            .and_then(|pages| pages.as_array())
            .into_iter()
            .flatten()
            .filter_map(|page| {
                let name = page.get("name")?.as_str()?;
                let path = page.get("path")?.as_str()?;
                Some((name.to_string(), path.trim_start_matches("./").to_string()))
            })
            .collect();
        let mut dir_count: HashMap<PathBuf, usize> = HashMap::new();
        for (_, path) in &declared {
            if let Some(dir) = app.join(path).parent() {
                *dir_count.entry(canonical(dir)).or_default() += 1;
            }
        }
        for (name, path) in declared {
            let declared_path = app.join(&path);
            let stem_path = declared_path.with_extension("");
            let dir = declared_path.parent().map(canonical);
            let stem_name = stem_path.file_name().map(|name| name.to_os_string());
            // The page's entries: every file beside it with its stem
            // (index.ts Logic, index.tsx View, index.json config).
            let entries: Vec<PathBuf> = dir
                .as_ref()
                .and_then(|dir| std::fs::read_dir(dir).ok())
                .into_iter()
                .flatten()
                .flatten()
                .map(|entry| entry.path())
                .filter(|file| {
                    file.is_file() && file.file_stem().map(|stem| stem.to_os_string()) == stem_name
                })
                .map(|file| canonical(&file))
                .collect();
            let stem = source_name(&stem_path, app);
            pages.push(Page {
                files: graph.closure(&entries),
                dir: dir.filter(|dir| dir_count.get(dir) == Some(&1)),
                name,
                stem,
            });
        }
    }
    pages
}

/// Static imports between the project's own files, read with oxc. A file
/// that cannot be parsed or a specifier that cannot be resolved is left
/// out of the graph: a page's owned directory and project-wide files still
/// cover it.
struct Graph {
    resolver: Resolver,
    nodes: HashMap<PathBuf, Node>,
}

#[derive(Default)]
struct Node {
    deps: Vec<PathBuf>,
    strings: Vec<String>,
}

impl Graph {
    fn new(root: &Path) -> Self {
        let mut options =
            ResolveOptions::default().with_condition_names(&["import", "module", "default"]);
        options.extensions = [
            ".ts", ".tsx", ".mts", ".js", ".jsx", ".mjs", ".cjs", ".json", ".vue",
        ]
        .iter()
        .map(|ext| ext.to_string())
        .collect();
        options.extension_alias = vec![(
            ".js".into(),
            vec![".ts".into(), ".tsx".into(), ".js".into(), ".jsx".into()],
        )];
        options.tsconfig = Some(TsconfigDiscovery::Auto);
        options.roots = vec![root.to_path_buf()];
        Self {
            resolver: Resolver::new(options),
            nodes: HashMap::new(),
        }
    }

    /// Every project file reachable from `entries`, the entries included.
    fn closure(&mut self, entries: &[PathBuf]) -> BTreeSet<PathBuf> {
        let mut seen = BTreeSet::new();
        let mut stack: Vec<PathBuf> = entries.to_vec();
        while let Some(file) = stack.pop() {
            if !seen.insert(file.clone()) {
                continue;
            }
            self.load(&file);
            stack.extend(
                self.nodes[&file]
                    .deps
                    .iter()
                    .filter(|dep| !seen.contains(*dep))
                    .cloned(),
            );
        }
        seen
    }

    fn strings(&self, file: &Path) -> &[String] {
        self.nodes
            .get(file)
            .map(|node| node.strings.as_slice())
            .unwrap_or_default()
    }

    fn load(&mut self, file: &Path) {
        if self.nodes.contains_key(file) {
            return;
        }
        let node = self.read(file).unwrap_or_default();
        self.nodes.insert(file.to_path_buf(), node);
    }

    fn read(&self, file: &Path) -> Option<Node> {
        let extension = file.extension()?.to_str()?;
        let text = std::fs::read_to_string(file).ok()?;
        let scripts: Vec<(String, SourceType)> = if SCRIPT_EXTENSIONS.contains(&extension) {
            vec![(text, SourceType::from_path(file).ok()?)]
        } else if extension == "vue" {
            vue_scripts(&text)
        } else {
            return None;
        };
        let mut found = Found::default();
        for (source, source_type) in &scripts {
            let allocator = Allocator::default();
            let parsed = Parser::new(&allocator, source, *source_type).parse();
            found.visit_program(&parsed.program);
        }
        let dir = file.parent()?;
        let deps = found
            .specifiers
            .iter()
            .filter_map(|specifier| self.resolver.resolve(dir, specifier).ok())
            .map(|resolution| resolution.into_path_buf())
            .filter(|path| {
                !path
                    .components()
                    .any(|part| part.as_os_str() == "node_modules")
            })
            .map(|path| canonical(&path))
            .collect();
        Some(Node {
            deps,
            strings: found.strings,
        })
    }
}

/// The `<script>` blocks of a Vue single-file component, as TypeScript.
fn vue_scripts(text: &str) -> Vec<(String, SourceType)> {
    let mut scripts = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find("<script") {
        let after = &rest[open..];
        let Some(tag_end) = after.find('>') else {
            break;
        };
        let body = &after[tag_end + 1..];
        let Some(close) = body.find("</script>") else {
            break;
        };
        scripts.push((body[..close].to_string(), SourceType::tsx()));
        rest = &body[close..];
    }
    scripts
}

#[derive(Default)]
struct Found {
    specifiers: Vec<String>,
    strings: Vec<String>,
}

impl<'a> Visit<'a> for Found {
    fn visit_import_declaration(&mut self, it: &ImportDeclaration<'a>) {
        self.specifiers.push(it.source.value.to_string());
    }

    fn visit_export_from_declaration(&mut self, it: &ExportFromDeclaration<'a>) {
        self.specifiers.push(it.source.value.to_string());
    }

    fn visit_export_all_declaration(&mut self, it: &ExportAllDeclaration<'a>) {
        self.specifiers.push(it.source.value.to_string());
    }

    fn visit_import_expression(&mut self, it: &ImportExpression<'a>) {
        if let oxc_ast::ast::Expression::StringLiteral(source) = &it.source {
            self.specifiers.push(source.value.to_string());
        }
        walk::walk_import_expression(self, it);
    }

    fn visit_string_literal(&mut self, it: &StringLiteral<'a>) {
        self.strings.push(it.value.to_string());
    }

    fn visit_template_literal(&mut self, it: &TemplateLiteral<'a>) {
        if it.expressions.is_empty() {
            for quasi in &it.quasis {
                if let Some(cooked) = &quasi.value.cooked {
                    self.strings.push(cooked.to_string());
                }
            }
        }
        walk::walk_template_literal(self, it);
    }
}

/// Narrow `selection` to the spec files the change affects. An empty
/// selection is for the caller to report; it starts nothing.
pub fn narrow(
    selection: &mut Selection,
    base: &str,
    extra_wide: &[PathBuf],
    cwd: &Path,
) -> Result<ChangedSelection> {
    let changed = changed_files(&selection.root, base)?;
    let found = select(&selection.root, &selection.files, &changed, extra_wide);
    let keep: Vec<PathBuf> = found
        .selected
        .iter()
        .map(|(file, _)| file.clone())
        .collect();
    if let Some(locations) = &mut selection.locations {
        let names: BTreeSet<String> = keep
            .iter()
            .map(|file| source_name(file, &selection.root))
            .collect();
        locations.retain(|name, _| names.contains(name));
    }
    selection.shown = keep.iter().map(|file| display_path(file, cwd)).collect();
    if keep.len() == 1 {
        selection.identity = keep[0].clone();
    } else {
        selection.identity = selection.root.clone();
    }
    selection.files = keep;
    Ok(found)
}

/// Changed files the report names before it only counts the rest.
const UNUSED_SHOWN: usize = 8;

/// The text report of `--changed`: one line per selected file and why, and
/// what no spec depends on.
pub fn describe(found: &ChangedSelection, since: &str, cwd: &Path) -> String {
    let show = |path: &Path| display_path(path, cwd);
    let mut out = format!(
        "--changed: {} changed file{} since {since}, {} spec file{} selected",
        found.changed.len(),
        if found.changed.len() == 1 { "" } else { "s" },
        found.selected.len(),
        if found.selected.len() == 1 { "" } else { "s" },
    );
    if let Some(Reason::ProjectWide(changed)) = found.selected.first().map(|(_, reason)| reason) {
        out.push_str(&format!("\n  all of them: {} changed", show(changed)));
    } else {
        for (file, reason) in &found.selected {
            let why = match reason {
                Reason::Imports(changed) => format!("imports {}", show(changed)),
                Reason::Page { page, file } => format!("page {page}: {}", show(file)),
                Reason::ProjectWide(changed) => format!("{} changed", show(changed)),
            };
            out.push_str(&format!("\n  {}  ({why})", show(file)));
        }
    }
    if !found.unused.is_empty() {
        let mut names: Vec<String> = found
            .unused
            .iter()
            .take(UNUSED_SHOWN)
            .map(|file| show(file))
            .collect();
        if found.unused.len() > UNUSED_SHOWN {
            names.push(format!("and {} more", found.unused.len() - UNUSED_SHOWN));
        }
        out.push_str(&format!(
            "\n  no spec here depends on: {}",
            names.join(", ")
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(root: &Path, path: &str, text: &str) {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    /// Two pages, a shared module one of them imports, and three spec files:
    /// one per page and one unit spec of the shared module.
    fn project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "package.json", "{}");
        write(
            root,
            "lxdev.json",
            r#"{ "test": { "openapi": ["api/openapi.yaml"], "presets": { "ci": ["--openapi", "api/ci.yaml"] } } }"#,
        );
        write(root, "api/openapi.yaml", "openapi: 3.0.3");
        write(root, "api/ci.yaml", "openapi: 3.0.3");
        write(
            root,
            "lxapp.json",
            r#"{ "pages": [ { "name": "home", "path": "pages/home/index.tsx" }, { "name": "ask", "path": "pages/ask/index" } ] }"#,
        );
        write(
            root,
            "pages/home/index.tsx",
            "import View from './views/view';\nexport default View;\n",
        );
        write(
            root,
            "pages/home/views/view.tsx",
            "export default function View() { return null; }\n",
        );
        write(
            root,
            "pages/home/index.ts",
            "import { fmt } from '../../shared/fmt';\nPage({ data: { label: fmt(1) } });\n",
        );
        write(
            root,
            "pages/ask/index.tsx",
            "export default function Ask() { return null; }\n",
        );
        write(root, "pages/ask/index.ts", "Page({});\n");
        write(
            root,
            "shared/fmt.ts",
            "export const fmt = (n: number) => `${n}`;\n",
        );
        write(root, "shared/unused.ts", "export const nobody = 1;\n");
        write(
            root,
            "tests/helpers.ts",
            "export const open = (page: string) => page;\n",
        );
        write(
            root,
            "tests/home.test.ts",
            "import { spec } from '@lingxia/test';\nimport { open } from './helpers';\nspec('home', async () => { open('home'); });\n",
        );
        write(
            root,
            "tests/ask.test.ts",
            "import { spec } from '@lingxia/test';\nspec('ask', async (t) => { await t.app.nav.relaunch({ page: 'ask' }); });\n",
        );
        write(
            root,
            "tests/fmt.test.ts",
            "import { spec } from '@lingxia/test';\nimport { fmt } from '../shared/fmt';\nspec('fmt', () => { fmt(1); });\n",
        );
        dir
    }

    fn specs(root: &Path) -> Vec<PathBuf> {
        [
            "tests/home.test.ts",
            "tests/ask.test.ts",
            "tests/fmt.test.ts",
        ]
        .iter()
        .map(|path| canonical(&root.join(path)))
        .collect()
    }

    fn picked(root: &Path, changed: &[&str]) -> (Vec<String>, Vec<String>) {
        let changed: Vec<PathBuf> = changed
            .iter()
            .map(|path| canonical(&root.join(path)))
            .collect();
        let found = select(root, &specs(root), &changed, &[]);
        let name = |path: &PathBuf| source_name(path, &canonical(root));
        (
            found.selected.iter().map(|(file, _)| name(file)).collect(),
            found.unused.iter().map(name).collect(),
        )
    }

    #[test]
    fn a_page_change_selects_the_specs_that_name_the_page() {
        let dir = project();
        let (selected, unused) = picked(dir.path(), &["pages/ask/index.tsx"]);
        assert_eq!(selected, ["tests/ask.test.ts"]);
        assert!(unused.is_empty());
        // A file under the page's directory belongs to it even when nothing
        // imports it yet, and a deleted one too.
        let (selected, _) = picked(dir.path(), &["pages/home/views/gone.tsx"]);
        assert_eq!(selected, ["tests/home.test.ts"]);
    }

    #[test]
    fn a_shared_module_selects_its_importers_and_the_pages_using_it() {
        let dir = project();
        let (selected, _) = picked(dir.path(), &["shared/fmt.ts"]);
        assert_eq!(selected, ["tests/home.test.ts", "tests/fmt.test.ts"]);
    }

    #[test]
    fn a_test_helper_selects_the_specs_importing_it() {
        let dir = project();
        let (selected, _) = picked(dir.path(), &["tests/helpers.ts"]);
        assert_eq!(selected, ["tests/home.test.ts"]);
    }

    #[test]
    fn a_file_no_spec_reaches_is_reported_and_selects_nothing() {
        let dir = project();
        let (selected, unused) = picked(dir.path(), &["shared/unused.ts", "README.md"]);
        assert!(selected.is_empty());
        assert_eq!(unused, ["shared/unused.ts", "README.md"]);
    }

    #[test]
    fn project_wide_files_select_everything() {
        let dir = project();
        for file in [
            "lxdev.json",
            "lxapp.json",
            "package.json",
            "api/openapi.yaml",
            "api/ci.yaml",
        ] {
            let (selected, unused) = picked(dir.path(), &[file]);
            assert_eq!(selected.len(), 3, "{file}");
            assert!(unused.is_empty(), "{file}");
        }
    }

    #[test]
    fn changed_files_come_from_git_since_the_merge_base() {
        let dir = project();
        let root = dir.path();
        let run = |args: &[&str]| {
            assert!(
                Command::new("git")
                    .args(args)
                    .current_dir(root)
                    .output()
                    .unwrap()
                    .status
                    .success()
            )
        };
        run(&["init", "-q", "-b", "main"]);
        run(&["-c", "user.email=a@b", "-c", "user.name=a", "add", "."]);
        run(&[
            "-c",
            "user.email=a@b",
            "-c",
            "user.name=a",
            "commit",
            "-qm",
            "base",
        ]);
        run(&["checkout", "-qb", "topic"]);
        write(
            root,
            "shared/fmt.ts",
            "export const fmt = (n: number) => `#${n}`;\n",
        );
        run(&[
            "-c",
            "user.email=a@b",
            "-c",
            "user.name=a",
            "commit",
            "-qam",
            "topic",
        ]);
        write(root, "pages/ask/index.ts", "Page({ data: {} });\n");
        write(root, "pages/ask/new.ts", "export {};\n");
        let names = |base: &str| -> Vec<String> {
            changed_files(root, base)
                .unwrap()
                .iter()
                .map(|path| source_name(path, &canonical(root)))
                .collect()
        };
        assert_eq!(names("HEAD"), ["pages/ask/index.ts", "pages/ask/new.ts"]);
        assert_eq!(
            names("main"),
            ["pages/ask/index.ts", "pages/ask/new.ts", "shared/fmt.ts"]
        );
        assert!(changed_files(root, "nope").is_err());
    }
}
