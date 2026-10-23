//! `--changed` / `--changed-since REF`: run only the specs a change can
//! affect.
//!
//! The unit is the spec module, the file whose `spec(…)` calls register
//! specs: the runtime's `locations` control selects by it, so the run keeps
//! its entries (their order, hooks and platform subset) and only narrows
//! whose specs run. A spec module is affected when a changed file is in its
//! import graph, belongs to a page its test code names, is an entry that
//! includes it, or is a non-script file its test code names. A project-wide
//! file, or one the graph cannot attribute (an asset, a nested config),
//! affects every spec: missing a spec is the failure to avoid. Only script
//! modules nothing imports and docs affect none.

use crate::test_bundle::source_name;
use crate::test_select::{Selection, display_path, usage};
use anyhow::{Context, Result};
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, CallExpression, ExportAllDeclaration, ExportFromDeclaration, Expression,
    ImportDeclaration, ImportExpression, StringLiteral, TemplateLiteral,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_resolver::{ResolveOptions, Resolver, TsconfigDiscovery};
use oxc_span::SourceType;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Component, Path, PathBuf};
use std::process::Command;

/// Files that change every spec's outcome, by name in the project root or an
/// lxapp's directory.
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
    "bun.lock",
];

/// `*.config.*` files beside those that only lint or format.
const INERT_CONFIGS: &[&str] = &["eslint", "prettier", "stylelint", "commitlint"];

const SCRIPT_EXTENSIONS: &[&str] = &["js", "mjs", "cjs", "ts", "mts", "cts", "jsx", "tsx"];

/// Why a spec module is affected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    /// A changed file is in its import graph (or is the module itself).
    Imports(PathBuf),
    /// A changed file belongs to a page its test code names.
    Page { page: String, file: PathBuf },
    /// A changed entry or aggregator includes it.
    IncludedBy(PathBuf),
    /// Its test code names a changed non-script file (a fixture).
    Names(PathBuf),
}

/// Why every spec is affected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Everything {
    /// lxdev.json, lxapp.json, the app entry, a package or build file.
    ProjectWide(PathBuf),
    /// No import, page or name ties it to a spec, and it may matter.
    Unattributed(PathBuf),
}

/// The changed files under the project root.
#[derive(Debug, Default)]
pub struct Changes {
    pub files: Vec<PathBuf>,
    /// Changed files outside the project root, which are not considered.
    pub outside: usize,
    /// `None` for the uncommitted work, else `main (merge base 1a2b3c4)`.
    pub base: Option<String>,
}

/// What `--changed` found.
#[derive(Debug, Default)]
pub struct ChangedSelection {
    pub changes: Changes,
    /// The spec modules of the run.
    pub spec_modules: usize,
    /// Set when every spec is affected; `selected` is then empty.
    pub everything: Option<Everything>,
    /// Affected spec modules, each with one reason.
    pub selected: Vec<(PathBuf, Reason)>,
    /// Changed files that affect no spec of the run.
    pub unaffected: Vec<PathBuf>,
    /// The entries that reach an affected spec module.
    entries: Vec<PathBuf>,
}

impl ChangedSelection {
    pub fn is_empty(&self) -> bool {
        self.everything.is_none() && self.selected.is_empty()
    }
}

/// The files that differ from `base` (`None`: the uncommitted work; else its
/// merge base with HEAD): committed since, staged, unstaged and untracked,
/// deleted ones included.
pub fn changed_files(root: &Path, base: Option<&str>) -> Result<Changes> {
    let toplevel = git(root, &["rev-parse", "--show-toplevel"]).map_err(|_| {
        usage(format!(
            "--changed needs a git repository; {} is not in one",
            root.display()
        ))
    })?;
    let toplevel = canonical(Path::new(toplevel.trim()));
    let (since, label) = match base {
        None => {
            git(root, &["rev-parse", "--verify", "--quiet", "HEAD^{commit}"])
                .map_err(|_| usage("--changed needs a commit to compare with"))?;
            ("HEAD".to_string(), None)
        }
        Some(base) => {
            git(
                root,
                &[
                    "rev-parse",
                    "--verify",
                    "--quiet",
                    &format!("{base}^{{commit}}"),
                ],
            )
            .map_err(|_| usage(format!("--changed-since {base}: no such commit or branch")))?;
            let since = git(root, &["merge-base", base, "HEAD"])
                .with_context(|| format!("--changed-since {base}: no merge base with HEAD"))?
                .trim()
                .to_string();
            let short = since.get(..7).unwrap_or(&since);
            let label = format!("{base} (merge base {short})");
            (since, Some(label))
        }
    };
    let mut names = git(
        &toplevel,
        &[
            "diff",
            "--name-only",
            "--no-renames",
            "--no-relative",
            "-z",
            &since,
        ],
    )?;
    names.push_str(&git(
        &toplevel,
        &["ls-files", "--others", "--exclude-standard", "-z"],
    )?);
    let root = canonical(root);
    let mut files = BTreeSet::new();
    let mut outside = BTreeSet::new();
    for name in names.split('\0').filter(|name| !name.is_empty()) {
        let path = canonical(&toplevel.join(name));
        if path.starts_with(&root) {
            files.insert(path);
        } else {
            outside.insert(path);
        }
    }
    Ok(Changes {
        files: files.into_iter().collect(),
        outside: outside.len(),
        base: label,
    })
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

/// Select, among the spec modules `entries` reach, those `changed` can
/// affect. `extra_wide` adds project-wide files (the run's `--openapi`
/// documents).
pub fn select(
    root: &Path,
    entries: &[PathBuf],
    changes: Changes,
    extra_wide: &[PathBuf],
) -> ChangedSelection {
    let root = canonical(root);
    let changed = &changes.files;
    let apps = apps(&root);
    let deleted: Vec<PathBuf> = changed
        .iter()
        .filter(|file| !file.exists() && is_script(file))
        .cloned()
        .collect();
    let mut graph = Graph::new(&root, deleted);

    let mut wide = project_wide(&root, &apps);
    wide.extend(extra_wide.iter().map(|path| canonical(&root.join(path))));
    let pages = pages(&apps, &mut graph);
    let app_entries: Vec<PathBuf> = apps.iter().flat_map(|app| app.entries.clone()).collect();
    let mut product = graph.closure(&app_entries);
    for page in &pages {
        product.extend(page.files.iter().cloned());
    }

    let entries: Vec<PathBuf> = entries.iter().map(|entry| canonical(entry)).collect();
    let run = graph.closure(&entries);
    let spec_modules: Vec<PathBuf> = run
        .iter()
        .filter(|module| !product.contains(*module) && graph.registers_specs(module))
        .cloned()
        .collect();
    let mut found = ChangedSelection {
        spec_modules: spec_modules.len(),
        ..Default::default()
    };

    if let Some(file) = changed.iter().find(|file| wide.contains(*file)) {
        found.everything = Some(Everything::ProjectWide(file.clone()));
        found.entries = entries;
        found.changes = changes;
        return found;
    }

    let closures: BTreeMap<PathBuf, BTreeSet<PathBuf>> = spec_modules
        .iter()
        .map(|module| (module.clone(), graph.closure(std::slice::from_ref(module))))
        .collect();
    // A changed test-side module reselects every spec module it includes.
    let included: Vec<(PathBuf, BTreeSet<PathBuf>)> = changed
        .iter()
        .filter(|file| run.contains(*file) && !product.contains(*file))
        .map(|file| (file.clone(), graph.closure(std::slice::from_ref(file))))
        .collect();
    let mut used: BTreeSet<PathBuf> = BTreeSet::new();
    let mut selected: BTreeMap<PathBuf, Reason> = BTreeMap::new();

    for (module, closure) in &closures {
        let mut reason = None;
        for file in changed.iter().filter(|file| closure.contains(*file)) {
            used.insert(file.clone());
            reason.get_or_insert_with(|| Reason::Imports(file.clone()));
        }
        for (file, includes) in &included {
            if file != module && includes.contains(module) {
                used.insert(file.clone());
                reason.get_or_insert_with(|| Reason::IncludedBy(file.clone()));
            }
        }
        // Pages are named in test code only: a product module it imports may
        // mention any page.
        let named: BTreeSet<&String> = closure
            .iter()
            .filter(|file| !product.contains(*file))
            .flat_map(|file| graph.strings(file).iter())
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
            selected.insert(module.clone(), reason);
        }
    }

    let reports = report_inputs(&root);
    let reached: BTreeSet<&PathBuf> = run
        .iter()
        .chain(product.iter())
        .chain(reports.iter())
        .collect();
    for file in changed.iter().filter(|file| !used.contains(*file)) {
        if reached.contains(file)
            || pages.iter().any(|page| page.owns(file))
            || is_script(file)
            || is_inert(&root, file)
        {
            found.unaffected.push(file.clone());
            continue;
        }
        // A fixture: the test code that names it.
        let Some(name) = file.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let naming: Vec<&PathBuf> = run
            .iter()
            .filter(|module| !product.contains(*module))
            .filter(|module| graph.strings(module).iter().any(|s| s.contains(name)))
            .collect();
        if naming.is_empty() {
            found.everything = Some(Everything::Unattributed(file.clone()));
            break;
        }
        for (module, closure) in &closures {
            if naming.iter().any(|named| closure.contains(*named)) {
                selected
                    .entry(module.clone())
                    .or_insert_with(|| Reason::Names(file.clone()));
            }
        }
    }
    if found.everything.is_some() {
        found.unaffected.clear();
        found.entries = entries;
    } else {
        found.entries = entries
            .into_iter()
            .filter(|entry| {
                graph
                    .closure(std::slice::from_ref(entry))
                    .iter()
                    .any(|module| selected.contains_key(module))
            })
            .collect();
        found.selected = selected.into_iter().collect();
    }
    found.changes = changes;
    found
}

fn is_script(file: &Path) -> bool {
    file.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| SCRIPT_EXTENSIONS.contains(&ext))
}

/// Docs, and dot files and directories (VCS, editor and tool metadata; a
/// project-wide `.env` is matched before), which no run reads.
fn is_inert(root: &Path, file: &Path) -> bool {
    let ext = file
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default();
    let dotted = file.strip_prefix(root).is_ok_and(|relative| {
        relative
            .components()
            .any(|part| part.as_os_str().to_string_lossy().starts_with('.'))
    });
    let name = file
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    dotted || matches!(ext, "md" | "mdx" | "txt") || name.starts_with("LICENSE")
}

/// One lxapp in the project: the root itself, or a directory one level down
/// for a host app with an embedded lxapp.
struct App {
    dir: PathBuf,
    manifest: serde_json::Value,
    /// The app's Logic entry (`lxapp.ts`, `app.ts`, …).
    entries: Vec<PathBuf>,
}

fn apps(root: &Path) -> Vec<App> {
    let mut dirs = vec![root.to_path_buf()];
    if let Ok(entries) = std::fs::read_dir(root) {
        dirs.extend(
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.is_dir()),
        );
    }
    dirs.into_iter()
        .filter_map(|dir| {
            let text = std::fs::read_to_string(dir.join("lxapp.json")).ok()?;
            let manifest = serde_json::from_str(&text).ok()?;
            let entries = ["lxapp.ts", "lxapp.js", "app.ts", "app.js"]
                .iter()
                .map(|name| dir.join(name))
                .filter(|path| path.is_file())
                .map(|path| canonical(&path))
                .collect();
            Some(App {
                dir: canonical(&dir),
                manifest,
                entries,
            })
        })
        .collect()
}

/// Project-wide files: the fixed names, tsconfigs, build configs and env
/// files in the root and each lxapp's directory, and lxdev.json's OpenAPI
/// documents (a routed answer is checked against them).
fn project_wide(root: &Path, apps: &[App]) -> BTreeSet<PathBuf> {
    let mut dirs = vec![root.to_path_buf()];
    dirs.extend(apps.iter().map(|app| app.dir.clone()));
    let mut files = BTreeSet::new();
    for dir in &dirs {
        files.extend(PROJECT_WIDE.iter().map(|name| dir.join(name)));
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let tsconfig = name.starts_with("tsconfig") && name.ends_with(".json");
            let config = name.split('.').nth(1) == Some("config")
                && !INERT_CONFIGS.iter().any(|tool| name.starts_with(tool));
            if tsconfig || config || name.starts_with(".env") {
                files.insert(dir.join(name));
            }
        }
    }
    let documents = lxdev_config(root)
        .map(|config| config_documents(&config, "--openapi"))
        .unwrap_or_default();
    files.extend(
        documents
            .iter()
            .map(|document| canonical(&root.join(document))),
    );
    files
}

/// The documents lxdev.json gives a flag (`test.openapi` for `--openapi`,
/// and every preset's value of `flag`).
fn config_documents(config: &serde_json::Value, flag: &str) -> Vec<String> {
    let test = config.get("test");
    let mut documents: Vec<String> = Vec::new();
    if flag == "--openapi" {
        match test.and_then(|test| test.get("openapi")) {
            Some(serde_json::Value::String(document)) => documents.push(document.clone()),
            Some(serde_json::Value::Array(values)) => documents.extend(
                values
                    .iter()
                    .filter_map(|value| value.as_str().map(str::to_string)),
            ),
            _ => {}
        }
    }
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
            if let Some(value) = arg
                .strip_prefix(flag)
                .and_then(|rest| rest.strip_prefix('='))
            {
                documents.push(value.to_string());
            } else if *arg == flag
                && let Some(value) = args.get(index + 1)
            {
                documents.push(value.to_string());
            }
        }
    }
    documents
}

/// lxdev.json, parsed.
fn lxdev_config(root: &Path) -> Option<serde_json::Value> {
    let text = std::fs::read_to_string(root.join("lxdev.json")).ok()?;
    serde_json::from_str(&text).ok()
}

/// Files only a run's report reads (`--covers-manifest`): no spec's outcome
/// depends on them.
fn report_inputs(root: &Path) -> BTreeSet<PathBuf> {
    lxdev_config(root)
        .map(|config| config_documents(&config, "--covers-manifest"))
        .unwrap_or_default()
        .iter()
        .map(|document| canonical(&root.join(document)))
        .collect()
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

fn pages(apps: &[App], graph: &mut Graph) -> Vec<Page> {
    let mut pages = Vec::new();
    for app in apps {
        let declared: Vec<(String, String)> = app
            .manifest
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
            if let Some(dir) = app.dir.join(path).parent() {
                *dir_count.entry(canonical(dir)).or_default() += 1;
            }
        }
        for (name, path) in declared {
            let declared_path = app.dir.join(&path);
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
            pages.push(Page {
                files: graph.closure(&entries),
                dir: dir.filter(|dir| dir_count.get(dir) == Some(&1)),
                stem: source_name(&stem_path, &app.dir),
                name,
            });
        }
    }
    pages
}

/// Static imports between the project's own files, read with oxc. A
/// specifier that no longer resolves because the change deleted its target
/// still links to the deleted file.
struct Graph {
    resolver: Resolver,
    nodes: HashMap<PathBuf, Node>,
    deleted: Vec<PathBuf>,
}

#[derive(Default)]
struct Node {
    deps: Vec<PathBuf>,
    strings: Vec<String>,
    registers_specs: bool,
}

impl Graph {
    fn new(root: &Path, deleted: Vec<PathBuf>) -> Self {
        let mut options =
            ResolveOptions::default().with_condition_names(&["import", "module", "default"]);
        options.extensions = [
            ".ts", ".tsx", ".mts", ".js", ".jsx", ".mjs", ".cjs", ".json",
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
            deleted,
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

    fn registers_specs(&self, file: &Path) -> bool {
        self.nodes
            .get(file)
            .is_some_and(|node| node.registers_specs)
    }

    fn load(&mut self, file: &Path) {
        if self.nodes.contains_key(file) {
            return;
        }
        let node = self.read(file).unwrap_or_default();
        self.nodes.insert(file.to_path_buf(), node);
    }

    fn read(&self, file: &Path) -> Option<Node> {
        let text = std::fs::read_to_string(file).ok()?;
        let scripts: Vec<(String, SourceType)> = if is_script(file) {
            vec![(text, SourceType::from_path(file).ok()?)]
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
        let mut deps = Vec::new();
        for specifier in &found.specifiers {
            match self.resolver.resolve(dir, specifier) {
                Ok(resolution) => {
                    let path = resolution.into_path_buf();
                    if !path
                        .components()
                        .any(|part| part.as_os_str() == "node_modules")
                    {
                        deps.push(canonical(&path));
                    }
                }
                Err(_) => deps.extend(
                    self.deleted
                        .iter()
                        .filter(|deleted| names_deleted(dir, specifier, deleted))
                        .cloned(),
                ),
            }
        }
        Some(Node {
            deps,
            strings: found.strings,
            registers_specs: found.registers_specs,
        })
    }
}

/// Whether an unresolved `specifier` imported from `dir` meant `deleted`. A
/// relative one is compared by path; an alias or package-like one, by its
/// last segment only, which may over-match.
fn names_deleted(dir: &Path, specifier: &str, deleted: &Path) -> bool {
    let target = deleted.with_extension("");
    let is_index = target.file_name().is_some_and(|name| name == "index");
    if specifier.starts_with('.') {
        let path = normalize(&dir.join(specifier));
        return path.with_extension("") == target
            || path == target
            || (is_index && Some(path.as_path()) == target.parent());
    }
    let last = Path::new(specifier.rsplit('/').next().unwrap_or(specifier)).with_extension("");
    let stem = if is_index {
        target.parent().and_then(|dir| dir.file_name())
    } else {
        target.file_name()
    };
    Some(last.as_os_str()) == stem
}

/// `a/./b/../c` → `a/c`, without touching the file system.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

#[derive(Default)]
struct Found {
    specifiers: Vec<String>,
    strings: Vec<String>,
    registers_specs: bool,
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
        if let Expression::StringLiteral(source) = &it.source {
            self.specifiers.push(source.value.to_string());
        }
        walk::walk_import_expression(self, it);
    }

    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        // The bundle only accepts direct `spec(…)` / `spec.x(…)` calls
        // (test_spec_check), so the callee tells a spec module.
        let callee = match &it.callee {
            Expression::Identifier(id) => Some(id.name.as_str()),
            Expression::StaticMemberExpression(member) => match &member.object {
                Expression::Identifier(id) => Some(id.name.as_str()),
                _ => None,
            },
            _ => None,
        };
        match callee {
            Some("spec") => self.registers_specs = true,
            Some("require") if matches!(&it.callee, Expression::Identifier(_)) => {
                if let Some(Argument::StringLiteral(source)) = it.arguments.first() {
                    self.specifiers.push(source.value.to_string());
                }
            }
            _ => {}
        }
        walk::walk_call_expression(self, it);
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

/// Narrow `selection` to the spec modules the change affects: its entries
/// stay (dropping those that reach none), and the `locations` control names
/// the affected modules. An empty result is for the caller to report; it
/// starts nothing.
pub fn narrow(
    selection: &mut Selection,
    base: Option<&str>,
    extra_wide: &[PathBuf],
) -> Result<ChangedSelection> {
    let changes = changed_files(&selection.root, base)?;
    let mut found = select(&selection.root, &selection.files, changes, extra_wide);
    if found.everything.is_some() {
        return Ok(found);
    }
    let root = selection.root.clone();
    let name = |file: &Path| source_name(file, &root);
    match &mut selection.locations {
        // `FILE:LINE` already narrowed to its entry files: keep the affected.
        Some(locations) => {
            let affected: BTreeSet<String> =
                found.selected.iter().map(|(file, _)| name(file)).collect();
            locations.retain(|file, _| affected.contains(file));
            found
                .selected
                .retain(|(file, _)| locations.contains_key(&name(file)));
        }
        None => {
            selection.locations = Some(
                found
                    .selected
                    .iter()
                    .map(|(file, _)| (name(file), None))
                    .collect(),
            );
        }
    }
    if !found.selected.is_empty() {
        selection.files = std::mem::take(&mut found.entries);
    }
    Ok(found)
}

/// Changed files the report names before it only counts the rest.
const UNAFFECTED_SHOWN: usize = 8;

/// The text report of `--changed`: what changed, which spec modules run and
/// why, and what affects none.
pub fn describe(found: &ChangedSelection, cwd: &Path) -> String {
    let show = |path: &Path| display_path(path, cwd);
    let changes = &found.changes;
    let count = changes.files.len();
    let plural = if count == 1 { "" } else { "s" };
    let mut out = match (&changes.base, count) {
        (None, 0) => "--changed: no uncommitted changes".to_string(),
        (Some(base), 0) => format!("--changed: nothing changed since {base}"),
        (None, _) => format!("--changed: {count} uncommitted file{plural}"),
        (Some(base), _) => format!("--changed: {count} file{plural} changed since {base}"),
    };
    match &found.everything {
        _ if count == 0 => {}
        Some(everything) => {
            let why = match everything {
                Everything::ProjectWide(file) => format!("{} is project-wide", show(file)),
                Everything::Unattributed(file) => {
                    format!("nothing tells what uses {}", show(file))
                }
            };
            out.push_str(&format!(
                ", all {} spec files run: {why}",
                found.spec_modules
            ));
        }
        None => {
            out.push_str(&format!(
                ", {} of {} spec files affected",
                found.selected.len(),
                found.spec_modules
            ));
            let rows: Vec<(String, String)> = found
                .selected
                .iter()
                .map(|(module, reason)| {
                    let why = match reason {
                        Reason::Imports(file) if file == module => "changed".to_string(),
                        Reason::Imports(file) => format!("imports {}", show(file)),
                        Reason::Page { page, file } => format!("page {page}: {}", show(file)),
                        Reason::IncludedBy(file) => format!("included by {}", show(file)),
                        Reason::Names(file) => format!("names {}", show(file)),
                    };
                    (show(module), why)
                })
                .collect();
            let width = rows.iter().map(|(file, _)| file.len()).max().unwrap_or(0);
            for (file, why) in rows {
                out.push_str(&format!("\n  {file:<width$}  {why}"));
            }
        }
    }
    if !found.unaffected.is_empty() {
        let mut names: Vec<String> = found
            .unaffected
            .iter()
            .take(UNAFFECTED_SHOWN)
            .map(|file| show(file))
            .collect();
        if found.unaffected.len() > UNAFFECTED_SHOWN {
            names.push(format!(
                "and {} more",
                found.unaffected.len() - UNAFFECTED_SHOWN
            ));
        }
        out.push_str(&format!("\n  affects no spec: {}", names.join(", ")));
    }
    if changes.outside > 0 {
        out.push_str(&format!(
            "\n  not considered: {} changed file{} outside the project",
            changes.outside,
            if changes.outside == 1 { "" } else { "s" }
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

    /// Two pages, a shared module one of them imports, three spec modules
    /// (one per page, one unit spec of the shared module) and an aggregator
    /// entry that includes them, as `test.entry` would name it.
    fn project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "package.json", "{}");
        write(
            root,
            "lxdev.json",
            r#"{ "test": { "entry": "tests/all.test.ts", "openapi": "api/openapi.yaml", "presets": { "ci": ["--openapi", "api/ci.yaml"], "nightly": ["--covers-manifest=tests/coverage.yaml"] } } }"#,
        );
        write(root, "api/openapi.yaml", "openapi: 3.0.3");
        write(root, "api/ci.yaml", "openapi: 3.0.3");
        write(root, "lxapp.config.ts", "export default {};\n");
        write(root, "public/logo.svg", "<svg/>");
        write(root, "README.md", "# app\n");
        write(root, ".tool/project.json", "{}");
        write(root, "tests/coverage.yaml", "- { id: a }\n");
        write(
            root,
            "lxapp.json",
            r#"{ "pages": [ { "name": "home", "path": "pages/home/index.tsx" }, { "name": "ask", "path": "pages/ask/index" } ] }"#,
        );
        write(root, "lxapp.ts", "App({});\n");
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
        // A helper in a directory no spec lives in names the page.
        write(
            root,
            "tests/helpers/nav.ts",
            "export const openHome = () => 'home';\n",
        );
        write(root, "tests/fixtures/answer.json", "{}");
        write(
            root,
            "tests/pages/home.test.ts",
            "import { spec } from '@lingxia/test';\nimport { openHome } from '../helpers/nav';\nspec('home', async () => { openHome(); });\n",
        );
        write(
            root,
            "tests/pages/ask.test.ts",
            "import { spec } from '@lingxia/test';\nspec('ask', async (t) => { await t.app.nav.relaunch({ page: 'ask' }); await t.fixture('fixtures/answer.json'); });\n",
        );
        write(
            root,
            "tests/unit/fmt.test.ts",
            "import { spec } from '@lingxia/test';\nimport { fmt } from '../../shared/fmt';\nspec('fmt', () => { fmt(1); });\n",
        );
        write(
            root,
            "tests/all.test.ts",
            "import './pages/home.test.js';\nimport './pages/ask.test.js';\nimport './unit/fmt.test.js';\n",
        );
        dir
    }

    fn changes(root: &Path, changed: &[&str]) -> Changes {
        Changes {
            files: changed
                .iter()
                .map(|path| canonical(&root.join(path)))
                .collect(),
            ..Default::default()
        }
    }

    /// The affected spec modules through the aggregator entry, and the
    /// changed files that affect none; `None` when every spec runs.
    fn picked(root: &Path, changed: &[&str]) -> Option<(Vec<String>, Vec<String>)> {
        let entries = [canonical(&root.join("tests/all.test.ts"))];
        let found = select(root, &entries, changes(root, changed), &[]);
        assert_eq!(found.spec_modules, 3);
        if found.everything.is_some() {
            return None;
        }
        let name = |path: &PathBuf| source_name(path, &canonical(root));
        Some((
            found.selected.iter().map(|(file, _)| name(file)).collect(),
            found.unaffected.iter().map(name).collect(),
        ))
    }

    #[test]
    fn a_page_change_selects_the_specs_that_name_the_page() {
        let dir = project();
        let (selected, unaffected) = picked(dir.path(), &["pages/ask/index.tsx"]).unwrap();
        assert_eq!(selected, ["tests/pages/ask.test.ts"]);
        assert!(unaffected.is_empty());
        // Named from a helper; and a file under the page's directory belongs
        // to it even when nothing imports it yet, or it was deleted.
        let (selected, _) = picked(dir.path(), &["pages/home/views/gone.tsx"]).unwrap();
        assert_eq!(selected, ["tests/pages/home.test.ts"]);
    }

    #[test]
    fn a_shared_module_selects_its_importers_and_the_pages_using_it() {
        let dir = project();
        let (selected, _) = picked(dir.path(), &["shared/fmt.ts"]).unwrap();
        assert_eq!(
            selected,
            ["tests/pages/home.test.ts", "tests/unit/fmt.test.ts"]
        );
    }

    #[test]
    fn test_code_changes_select_their_specs() {
        let dir = project();
        let (selected, _) = picked(dir.path(), &["tests/helpers/nav.ts"]).unwrap();
        assert_eq!(selected, ["tests/pages/home.test.ts"]);
        let (selected, _) = picked(dir.path(), &["tests/unit/fmt.test.ts"]).unwrap();
        assert_eq!(selected, ["tests/unit/fmt.test.ts"]);
        // A changed aggregator reselects what it includes.
        let (selected, _) = picked(dir.path(), &["tests/all.test.ts"]).unwrap();
        assert_eq!(selected.len(), 3);
        // A fixture selects the specs whose test code names it.
        let (selected, _) = picked(dir.path(), &["tests/fixtures/answer.json"]).unwrap();
        assert_eq!(selected, ["tests/pages/ask.test.ts"]);
    }

    #[test]
    fn a_deleted_module_selects_its_importers() {
        let dir = project();
        fs::remove_file(dir.path().join("shared/fmt.ts")).unwrap();
        let (selected, _) = picked(dir.path(), &["shared/fmt.ts"]).unwrap();
        assert_eq!(
            selected,
            ["tests/pages/home.test.ts", "tests/unit/fmt.test.ts"]
        );
    }

    #[test]
    fn unreached_scripts_and_docs_affect_nothing() {
        let dir = project();
        let (selected, unaffected) = picked(
            dir.path(),
            &[
                "shared/unused.ts",
                "README.md",
                ".tool/project.json",
                "tests/coverage.yaml",
            ],
        )
        .unwrap();
        assert!(selected.is_empty());
        assert_eq!(unaffected.len(), 4);
    }

    #[test]
    fn project_wide_and_unattributed_files_select_everything() {
        let dir = project();
        for file in [
            "lxdev.json",
            "lxapp.json",
            "lxapp.ts",
            "lxapp.config.ts",
            "package.json",
            "api/openapi.yaml",
            "api/ci.yaml",
            "public/logo.svg",
        ] {
            assert!(picked(dir.path(), &[file]).is_none(), "{file}");
        }
    }

    #[test]
    fn narrowing_keeps_the_entry_and_names_the_modules() {
        let dir = project();
        let root = canonical(dir.path());
        let entries = [
            root.join("tests/all.test.ts"),
            root.join("tests/unit/fmt.test.ts"),
        ];
        let found = select(
            &root,
            &entries,
            changes(&root, &["pages/ask/index.ts"]),
            &[],
        );
        assert_eq!(found.entries, [root.join("tests/all.test.ts")]);
        let text = describe(&found, &root);
        assert!(text.contains("1 of 3 spec files affected"), "{text}");
        assert!(
            text.contains("tests/pages/ask.test.ts  page ask: pages/ask/index.ts"),
            "{text}"
        );
    }

    #[test]
    fn changed_files_come_from_git_since_the_merge_base() {
        let parent = tempfile::tempdir().unwrap();
        let repo = parent.path();
        let dir = project();
        let root = repo.join("app");
        fs::rename(dir.path(), &root).unwrap();
        let run = |args: &[&str]| {
            assert!(
                Command::new("git")
                    .args(["-c", "user.email=a@b", "-c", "user.name=a"])
                    .args(args)
                    .current_dir(repo)
                    .output()
                    .unwrap()
                    .status
                    .success()
            )
        };
        run(&["init", "-q", "-b", "main"]);
        run(&["add", "."]);
        run(&["commit", "-qm", "base"]);
        run(&["checkout", "-qb", "topic"]);
        write(
            &root,
            "shared/fmt.ts",
            "export const fmt = (n: number) => `#${n}`;\n",
        );
        run(&["commit", "-qam", "topic"]);
        write(&root, "pages/ask/index.ts", "Page({ data: {} });\n");
        write(&root, "pages/ask/new.ts", "export {};\n");
        write(repo, "native/src/lib.rs", "\n");
        let names = |base: Option<&str>| -> (Vec<String>, usize) {
            let changes = changed_files(&root, base).unwrap();
            (
                changes
                    .files
                    .iter()
                    .map(|path| source_name(path, &canonical(&root)))
                    .collect(),
                changes.outside,
            )
        };
        assert_eq!(
            names(None),
            (
                vec!["pages/ask/index.ts".into(), "pages/ask/new.ts".into()],
                1
            )
        );
        assert_eq!(
            names(Some("main")).0,
            ["pages/ask/index.ts", "pages/ask/new.ts", "shared/fmt.ts"]
        );
        assert!(changed_files(&root, Some("nope")).is_err());
    }
}
