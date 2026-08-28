mod react;
mod vite_assets;
mod vite_html;
mod vite_pipeline;
mod vite_tooling;
mod vue;

pub(crate) use vite_assets::native_client_output_path;
pub(crate) use vite_html::view_target_from_dir;

use crate::lxapp::framework::{PageAction, PageActionMode, ProjectFramework};
use crate::lxapp::options::BuildOptions;
use crate::lxapp::project::Project;
use anyhow::{Result, anyhow, bail};
use indicatif::ProgressBar;
use oxc_allocator::Allocator;
use oxc_ast::ast::{BindingPattern, Expression, ObjectPropertyKind, PropertyKey, Statement};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_span::SourceType;
use serde_json::Value;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

const PAGE_BRIDGE_RUNTIME_MODULE: &str =
    include_str!("../../templates/builder-frameworks/page_bridge_runtime.js");

#[derive(Debug, Clone)]
pub struct ViewBuildReport {
    pub framework: ProjectFramework,
    pub page_count: usize,
    pub install_duration: Option<Duration>,
    pub prepare_duration: Duration,
    pub bundle_duration: Duration,
    pub finalize_duration: Duration,
}

#[derive(Clone)]
pub struct ViewProgress {
    bar: ProgressBar,
}

impl ViewProgress {
    pub fn new(bar: ProgressBar) -> Self {
        Self { bar }
    }

    pub fn ensuring_tooling(&self) {
        self.bar.set_message(format!(
            "{} checking tooling",
            console::style("View").cyan()
        ));
    }

    pub fn installing_project_deps(&self) {
        self.bar
            .set_message(format!("{} installing deps", console::style("View").cyan()));
    }

    pub fn preparing_pages(&self, _page_count: usize, framework: ProjectFramework) {
        self.bar.set_message(format!(
            "{} preparing {} entries",
            console::style("View").cyan(),
            framework.as_str()
        ));
    }

    pub fn bundling_pages(&self, _page_count: usize, framework: ProjectFramework) {
        self.bar.set_message(format!(
            "{} bundling {} assets",
            console::style("View").cyan(),
            framework.as_str()
        ));
    }

    pub fn finalizing_pages(&self, _page_count: usize) {
        self.bar.set_message(format!(
            "{} finalizing outputs",
            console::style("View").cyan()
        ));
    }
}

pub fn build(
    project: &Project,
    options: &BuildOptions,
    progress: Option<ViewProgress>,
    install_duration_hint: Option<Duration>,
) -> Result<ViewBuildReport> {
    crate::commands::project_upgrade::warn_if_behind(&project.root);
    vite_pipeline::build(project, options, progress, install_duration_hint)
}

pub fn prepare_tooling(
    project: &Project,
    options: &BuildOptions,
    progress: Option<ViewProgress>,
) -> Result<Option<Duration>> {
    vite_pipeline::prepare_tooling(project, options, progress)
}

pub(crate) fn page_logic_path(project: &Project, page_path: &str) -> Result<Option<PathBuf>> {
    let page_path_obj = Path::new(page_path);
    let without_ext = page_path_obj.with_extension("");
    let ts_path = project.root.join(&without_ext).with_extension("ts");
    let js_path = project.root.join(&without_ext).with_extension("js");
    match (ts_path.exists(), js_path.exists()) {
        (true, true) => bail!("Logic layer conflict for {page_path}: found both .ts and .js"),
        (true, false) => Ok(Some(ts_path)),
        (false, true) => Ok(Some(js_path)),
        (false, false) => Ok(None),
    }
}

pub(crate) fn page_title(project: &Project, page_path: &str) -> Result<String> {
    let page_path_obj = Path::new(page_path);
    let base = page_path_obj
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("Page");
    let config_path = project
        .root
        .join(page_path_obj.parent().unwrap_or_else(|| Path::new("")))
        .join(format!("{base}.json"));
    if config_path.exists() {
        let content = fs::read_to_string(&config_path)?;
        let value: Value = serde_json::from_str(&content)?;
        if let Some(title) = value
            .get("navigationBar")
            .and_then(|navigation_bar| navigation_bar.get("title"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Ok(title.to_string());
        }
    }

    let mut chars = base.chars();
    let first = chars.next().unwrap_or('P').to_ascii_uppercase();
    Ok(format!("{first}{}", chars.collect::<String>()))
}

#[derive(Debug, Clone)]
pub(crate) struct ViewUsageAudit {
    pub used_actions: BTreeSet<String>,
    /// Whether `used_actions` is complete enough to call the rest dead. False
    /// when the actions object reached code this audit could not read.
    pub unused_reportable: bool,
}

#[derive(Debug, Clone)]
struct ViewDirectLxUse {
    member: String,
    line: usize,
    column: usize,
    origin: &'static str,
}

#[derive(Debug, Default, Clone)]
pub(super) struct ViewBindingAnalyzer {
    pub(super) action_object_aliases: HashSet<String>,
    pub(super) local_action_aliases: HashMap<String, String>,
    pub(super) used_actions: BTreeSet<String>,
    pub(super) direct_lx_uses: Vec<(usize, String)>,
    /// True once an actions alias is referenced as a whole value rather than
    /// read through (`actions.foo`) or destructured.
    pub(super) actions_escaped: bool,
    /// Spans of alias references already accounted for, so the generic
    /// identifier visit does not read them as an escape.
    consumed_alias_spans: HashSet<u32>,
}

impl ViewBindingAnalyzer {
    pub(super) fn with_bindings(
        action_object_aliases: HashSet<String>,
        local_action_aliases: HashMap<String, String>,
    ) -> Self {
        Self {
            action_object_aliases,
            local_action_aliases,
            ..Self::default()
        }
    }

    fn register_binding_from_init(&mut self, pattern: &BindingPattern<'_>, init: &Expression<'_>) {
        let init = unwrap_expression(init);
        if is_use_lx_page_call(init) {
            self.register_use_lx_page_pattern(pattern);
            return;
        }

        if let Expression::Identifier(identifier) = init {
            let name = identifier.name.as_str();
            if self.action_object_aliases.contains(name) {
                self.consumed_alias_spans.insert(identifier.span.start);
                match pattern {
                    // `const other = actions` just renames the object.
                    BindingPattern::BindingIdentifier(binding) => {
                        self.action_object_aliases
                            .insert(binding.name.as_str().to_string());
                    }
                    _ => self.register_action_object_pattern(pattern),
                }
                return;
            }
            if let Some(action_name) = self.local_action_aliases.get(name).cloned() {
                self.register_local_action_alias(pattern, &action_name);
            }
            return;
        }

        if let Some(action_name) = self.resolve_action_member(init) {
            self.register_local_action_alias(pattern, &action_name);
        }
    }

    fn register_use_lx_page_pattern(&mut self, pattern: &BindingPattern<'_>) {
        let BindingPattern::ObjectPattern(object) = pattern else {
            return;
        };

        for property in &object.properties {
            let Some(name) = property_name(&property.key) else {
                continue;
            };
            if name != "actions" {
                continue;
            }
            self.register_actions_binding(&property.value);
        }
    }

    fn register_actions_binding(&mut self, pattern: &BindingPattern<'_>) {
        match pattern {
            BindingPattern::BindingIdentifier(identifier) => {
                self.action_object_aliases
                    .insert(identifier.name.as_str().to_string());
            }
            BindingPattern::ObjectPattern(object) => {
                self.register_action_object_pattern_from_object(object);
            }
            BindingPattern::AssignmentPattern(pattern) => {
                self.register_actions_binding(&pattern.left);
            }
            _ => {}
        }
    }

    fn register_action_object_pattern(&mut self, pattern: &BindingPattern<'_>) {
        match pattern {
            BindingPattern::ObjectPattern(object) => {
                self.register_action_object_pattern_from_object(object);
            }
            BindingPattern::AssignmentPattern(pattern) => {
                self.register_action_object_pattern(&pattern.left);
            }
            _ => {}
        }
    }

    fn register_action_object_pattern_from_object(
        &mut self,
        object: &oxc_ast::ast::ObjectPattern<'_>,
    ) {
        for property in &object.properties {
            let Some(action_name) = property_name(&property.key) else {
                continue;
            };
            let mut bound_names = Vec::new();
            collect_binding_names(&property.value, &mut bound_names);
            for bound_name in bound_names {
                self.local_action_aliases
                    .insert(bound_name, action_name.clone());
            }
        }
    }

    fn register_local_action_alias(&mut self, pattern: &BindingPattern<'_>, action_name: &str) {
        let mut bound_names = Vec::new();
        collect_binding_names(pattern, &mut bound_names);
        for bound_name in bound_names {
            self.local_action_aliases
                .insert(bound_name, action_name.to_string());
        }
    }

    fn resolve_action_member(&self, expression: &Expression<'_>) -> Option<String> {
        match expression {
            Expression::Identifier(identifier) => self
                .local_action_aliases
                .get(identifier.name.as_str())
                .cloned(),
            Expression::StaticMemberExpression(member) => {
                let Expression::Identifier(identifier) = unwrap_expression(&member.object) else {
                    return None;
                };
                self.action_object_aliases
                    .contains(identifier.name.as_str())
                    .then(|| member.property.name.as_str().to_string())
            }
            _ => None,
        }
    }
}

impl<'a> Visit<'a> for ViewBindingAnalyzer {
    fn visit_variable_declarator(&mut self, it: &oxc_ast::ast::VariableDeclarator<'a>) {
        if let Some(init) = &it.init {
            self.register_binding_from_init(&it.id, init);
            if let Some(action_name) = self.resolve_action_member(init) {
                self.used_actions.insert(action_name);
            }
        }
        walk::walk_variable_declarator(self, it);
    }

    fn visit_identifier_reference(&mut self, it: &oxc_ast::ast::IdentifierReference<'a>) {
        if let Some(action_name) = self.local_action_aliases.get(it.name.as_str()) {
            self.used_actions.insert(action_name.clone());
        }
        if self.action_object_aliases.contains(it.name.as_str())
            && !self.consumed_alias_spans.contains(&it.span.start)
        {
            self.actions_escaped = true;
        }
        walk::walk_identifier_reference(self, it);
    }

    fn visit_static_member_expression(&mut self, it: &oxc_ast::ast::StaticMemberExpression<'a>) {
        if let Expression::Identifier(identifier) = unwrap_expression(&it.object) {
            let object_name = identifier.name.as_str();
            if object_name == "lx" {
                self.direct_lx_uses.push((
                    it.span.start as usize,
                    it.property.name.as_str().to_string(),
                ));
            } else if self.action_object_aliases.contains(object_name) {
                self.consumed_alias_spans.insert(identifier.span.start);
                self.used_actions
                    .insert(it.property.name.as_str().to_string());
            }
        } else if let Expression::StaticMemberExpression(inner) = unwrap_expression(&it.object)
            && inner.property.name.as_str() == "actions"
        {
            // A view handed the page as one prop reaches through it:
            // `page.actions.foo`.
            self.used_actions
                .insert(it.property.name.as_str().to_string());
        }
        walk::walk_static_member_expression(self, it);
    }

    fn visit_computed_member_expression(
        &mut self,
        it: &oxc_ast::ast::ComputedMemberExpression<'a>,
    ) {
        if let Expression::Identifier(identifier) = unwrap_expression(&it.object)
            && identifier.name.as_str() == "lx"
        {
            self.direct_lx_uses
                .push((it.span.start as usize, "<computed>".to_string()));
        }
        walk::walk_computed_member_expression(self, it);
    }
}

pub(crate) fn validate_component_view_bindings(
    project: &Project,
    page_path: &str,
    actions: &[PageAction],
) -> Result<ViewUsageAudit> {
    match project.framework {
        ProjectFramework::React => react::validate_react_bindings(project, page_path, actions),
        ProjectFramework::Vue => vue::validate_vue_bindings(project, page_path, actions),
        ProjectFramework::Html => Ok(ViewUsageAudit {
            used_actions: BTreeSet::new(),
            // HTML pages bind through the bridge, so this audit sees no actions.
            unused_reportable: false,
        }),
    }
}

pub(super) fn analyze_script_bindings(
    source: &str,
    source_type: SourceType,
    bindings: Option<(HashSet<String>, HashMap<String, String>)>,
) -> Result<ViewBindingAnalyzer> {
    let allocator = Allocator::default();
    let parse_result = Parser::new(&allocator, source, source_type).parse();
    if !parse_result.diagnostics.is_empty() {
        bail!("Failed to parse view source");
    }

    let mut analyzer = if let Some((action_object_aliases, local_action_aliases)) = bindings {
        ViewBindingAnalyzer::with_bindings(action_object_aliases, local_action_aliases)
    } else {
        ViewBindingAnalyzer::default()
    };
    analyzer.visit_program(&parse_result.program);
    Ok(analyzer)
}

/// Collects the relative import specifiers a view source declares, so the
/// action audit can follow the components a page composition root renders.
fn local_import_specifiers(source: &str, source_type: SourceType) -> Vec<String> {
    let allocator = Allocator::default();
    let parse_result = Parser::new(&allocator, source, source_type).parse();
    let mut specifiers = Vec::new();
    for statement in &parse_result.program.body {
        let Statement::ImportDeclaration(declaration) = statement else {
            continue;
        };
        let specifier = declaration.source.value.as_str();
        if specifier.starts_with("./") || specifier.starts_with("../") {
            specifiers.push(specifier.to_string());
        }
    }
    specifiers
}

/// Resolves an import specifier the way the bundler does, but only far enough
/// to find a view source inside the project. Non-script imports (CSS, assets)
/// resolve to nothing — they hold no action bindings.
fn resolve_local_import(project: &Project, importer: &Path, specifier: &str) -> Option<PathBuf> {
    const EXTENSIONS: [&str; 5] = ["tsx", "ts", "jsx", "js", "vue"];
    let base = importer.parent()?.join(specifier);
    let candidates = std::iter::once(base.clone())
        .chain(EXTENSIONS.iter().map(|ext| base.with_extension(ext)))
        .chain(
            EXTENSIONS
                .iter()
                .map(|ext| base.join(format!("index.{ext}"))),
        );
    for candidate in candidates {
        let is_script = candidate
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| EXTENSIONS.contains(&ext));
        if is_script && candidate.is_file() && candidate.starts_with(&project.root) {
            return Some(candidate);
        }
    }
    None
}

/// Unions the action usage of every local view file reachable from `entry`.
///
/// A composition root hands `actions` to the views it renders, so the entry
/// alone cannot tell a wired action from a dead one. Children receive the
/// object under a prop, so `actions` is treated as the object name there — a
/// seed that can only add usage, never invent a missing binding.
///
/// The second field is false when a reachable file could not be read or
/// parsed: usage may live in it, so nothing may be called dead.
fn downstream_action_usage(project: &Project, entry: &Path) -> (BTreeSet<String>, bool) {
    let mut used_actions = BTreeSet::new();
    let mut complete = true;
    let mut visited = HashSet::from([entry.to_path_buf()]);
    let mut queue = vec![entry.to_path_buf()];

    while let Some(path) = queue.pop() {
        let Ok(source) = fs::read_to_string(&path) else {
            complete = false;
            continue;
        };
        let is_vue = path.extension().is_some_and(|extension| extension == "vue");
        let (script, script_source_type) = if is_vue {
            let sections = vue::sfc_sections(&source);
            used_actions.extend(vue::template_action_names(&sections.template));
            (sections.script, sections.script_source_type)
        } else {
            let Ok(source_type) = SourceType::from_path(&path) else {
                complete = false;
                continue;
            };
            (source, source_type)
        };

        if path != entry {
            let seed = HashSet::from(["actions".to_string()]);
            match analyze_script_bindings(&script, script_source_type, Some((seed, HashMap::new())))
            {
                Ok(analyzer) => used_actions.extend(analyzer.used_actions),
                Err(_) => complete = false,
            }
        }

        for specifier in local_import_specifiers(&script, script_source_type) {
            if let Some(resolved) = resolve_local_import(project, &path, &specifier)
                && visited.insert(resolved.clone())
            {
                queue.push(resolved);
            }
        }
    }

    (used_actions, complete)
}

pub(super) fn ensure_no_direct_lx_usage(
    page_path: &str,
    source: &str,
    uses: &[(usize, String)],
    origin: &'static str,
) -> Result<()> {
    if uses.is_empty() {
        return Ok(());
    }

    let mut locations = Vec::new();
    for (offset, member) in uses.iter().take(5) {
        let (line, column) = line_col_for_offset(source, *offset);
        locations.push(ViewDirectLxUse {
            member: member.clone(),
            line,
            column,
            origin,
        });
    }

    let detail = locations
        .iter()
        .map(|usage| {
            format!(
                "{}:{} {} lx.{}",
                usage.line, usage.column, usage.origin, usage.member
            )
        })
        .collect::<Vec<_>>()
        .join(", ");

    bail!(
        "View {page_path} must not call lx.* directly. Move host calls into Page(...) logic actions. Found: {detail}"
    );
}

/// Every action a child view calls must be one the entry actually handed it.
///
/// An entry that re-wraps `actions` into its own object literal —
/// `actions: { save: actions.save, … }` — is writing a whitelist, and a child
/// calling something the whitelist omits gets `undefined` at runtime. Nothing
/// else catches it: the action exists on `Page(...)`, the child's call is
/// well-typed against the page contract, and the omission is a missing line
/// rather than a wrong one. The first symptom is a tap that does nothing.
///
/// Only reported when the downstream scan was complete. A view this analysis
/// could not read might be forwarding the action some other way, and a build
/// error has to be certain.
pub(super) fn ensure_forwarded_actions_cover_downstream(
    page_path: &str,
    forwarded: &BTreeSet<String>,
    downstream: &BTreeSet<String>,
    downstream_complete: bool,
) -> Result<()> {
    if !downstream_complete {
        return Ok(());
    }
    let missing = downstream
        .iter()
        .filter(|name| !forwarded.contains(name.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return Ok(());
    }

    bail!(
        "View {page_path} does not forward Page(...) actions its child views call: {}. \
         The entry builds its own actions object, so anything left out of it is \
         undefined at runtime even though the action exists and the call type-checks. \
         Add each one to the object the entry passes down, or pass `actions` through \
         whole.",
        missing.join(", ")
    );
}

pub(super) fn ensure_used_actions_exist(
    page_path: &str,
    actions: &[PageAction],
    used_actions: &BTreeSet<String>,
) -> Result<()> {
    let defined = actions
        .iter()
        .map(|action| action.name.as_str())
        .collect::<HashSet<_>>();
    let missing = used_actions
        .iter()
        .filter(|name| !defined.contains(name.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return Ok(());
    }

    bail!(
        "View {page_path} references missing Page(...) actions: {}",
        missing.join(", ")
    );
}

fn line_col_for_offset(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut column = 1usize;
    for ch in source[..offset.min(source.len())].chars() {
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}

fn line_number_for_offset(source: &str, offset: usize) -> usize {
    line_col_for_offset(source, offset).0
}

fn collect_binding_names(pattern: &BindingPattern<'_>, output: &mut Vec<String>) {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => {
            output.push(identifier.name.as_str().to_string());
        }
        BindingPattern::ObjectPattern(pattern) => {
            for property in &pattern.properties {
                collect_binding_names(&property.value, output);
            }
            if let Some(rest) = &pattern.rest {
                collect_binding_names(&rest.argument, output);
            }
        }
        BindingPattern::ArrayPattern(pattern) => {
            for element in pattern.elements.iter().flatten() {
                collect_binding_names(element, output);
            }
            if let Some(rest) = &pattern.rest {
                collect_binding_names(&rest.argument, output);
            }
        }
        BindingPattern::AssignmentPattern(pattern) => {
            collect_binding_names(&pattern.left, output);
        }
    }
}

fn is_use_lx_page_call(expression: &Expression<'_>) -> bool {
    let Expression::CallExpression(call_expr) = expression else {
        return false;
    };
    let Expression::Identifier(identifier) = unwrap_expression(&call_expr.callee) else {
        return false;
    };
    identifier.name.as_str() == "useLxPage"
}

pub(crate) fn extract_page_actions(logic_path: Option<&Path>) -> Result<Vec<PageAction>> {
    let Some(logic_path) = logic_path else {
        return Ok(Vec::new());
    };
    let source = fs::read_to_string(logic_path)?;
    let source_type = SourceType::from_path(logic_path)
        .map_err(|_| anyhow!("Unsupported page logic file {}", logic_path.display()))?;
    let allocator = Allocator::default();
    let parse_result = Parser::new(&allocator, &source, source_type).parse();
    if !parse_result.diagnostics.is_empty() {
        bail!("Failed to parse page logic {}", logic_path.display());
    }
    let function_bindings = collect_top_level_function_bindings(&parse_result.program);

    for statement in &parse_result.program.body {
        let Statement::ExpressionStatement(expression_statement) = statement else {
            continue;
        };
        let Expression::CallExpression(call_expr) =
            unwrap_expression(&expression_statement.expression)
        else {
            continue;
        };
        let Expression::Identifier(identifier) = unwrap_expression(&call_expr.callee) else {
            continue;
        };
        if identifier.name.as_str() != "Page" {
            continue;
        }

        let Some(first_arg) = call_expr.arguments.first() else {
            return Ok(Vec::new());
        };
        let Expression::ObjectExpression(object) = unwrap_expression(first_arg.to_expression())
        else {
            return Ok(Vec::new());
        };

        let mut actions = Vec::new();
        for property in &object.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                continue;
            };
            let Some(name) = property_name(&property.key) else {
                continue;
            };
            if name == "data" || name.starts_with('_') || super::is_page_lifecycle(&name) {
                continue;
            }
            if !is_function_like_property(property, &function_bindings) {
                continue;
            }
            actions.push(PageAction {
                name,
                mode: infer_property_mode(property, &function_bindings),
            });
        }
        return Ok(actions);
    }

    Ok(Vec::new())
}

#[derive(Clone, Copy)]
struct FunctionBinding<'a> {
    body: Option<&'a oxc_ast::ast::FunctionBody<'a>>,
    is_generator: bool,
    returns_expression: bool,
}

fn collect_top_level_function_bindings<'a>(
    program: &'a oxc_ast::ast::Program<'a>,
) -> HashMap<String, FunctionBinding<'a>> {
    let mut direct = HashMap::new();
    let mut aliases = HashMap::new();

    for statement in &program.body {
        match statement {
            Statement::FunctionDeclaration(function) => {
                let Some(identifier) = &function.id else {
                    continue;
                };
                direct.insert(
                    identifier.name.as_str().to_string(),
                    FunctionBinding {
                        body: function.body.as_deref(),
                        is_generator: function.generator,
                        returns_expression: false,
                    },
                );
            }
            Statement::VariableDeclaration(declaration) => {
                for declarator in &declaration.declarations {
                    let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
                        continue;
                    };
                    let Some(init) = declarator.init.as_ref().map(unwrap_expression) else {
                        continue;
                    };
                    match init {
                        Expression::FunctionExpression(function) => {
                            direct.insert(
                                identifier.name.as_str().to_string(),
                                FunctionBinding {
                                    body: function.body.as_deref(),
                                    is_generator: function.generator,
                                    returns_expression: false,
                                },
                            );
                        }
                        Expression::ArrowFunctionExpression(function) => {
                            let (body, returns_expression) = arrow_body(function);
                            direct.insert(
                                identifier.name.as_str().to_string(),
                                FunctionBinding {
                                    body,
                                    is_generator: false,
                                    returns_expression,
                                },
                            );
                        }
                        Expression::Identifier(target) => {
                            aliases.insert(
                                identifier.name.as_str().to_string(),
                                target.name.as_str().to_string(),
                            );
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    let mut resolved = direct.clone();
    for (alias, target) in aliases {
        let cursor = target.as_str();
        let mut visited = HashSet::new();
        // Resolves a single alias hop; the visited-set/while scaffold is kept
        // for future cycle-safe chain walking.
        #[allow(clippy::never_loop)]
        while visited.insert(cursor.to_string()) {
            if let Some(binding) = direct.get(cursor).copied() {
                resolved.insert(alias.clone(), binding);
                break;
            }
            let Some(next) = resolved
                .get(cursor)
                .copied()
                .or_else(|| direct.get(cursor).copied())
            else {
                break;
            };
            resolved.insert(alias.clone(), next);
            break;
        }
    }
    resolved
}

pub(crate) fn render_page_bridge_runtime_module() -> &'static str {
    PAGE_BRIDGE_RUNTIME_MODULE
}

pub(crate) fn render_page_bridge_module(actions: &[PageAction], runtime_import: &str) -> String {
    if actions.is_empty() {
        return "export const __names = [];\n".to_string();
    }

    let mut output = String::from("// Auto-generated by lingxia-cli. Do not edit.\n");
    output.push_str(&format!(
        "import {{ __lx_define_page_bridge }} from '{}';\n\n",
        runtime_import
    ));

    for action in actions {
        let name_json = serde_json::to_string(&action.name).unwrap_or_else(|_| "\"\"".to_string());
        let mode_json = serde_json::to_string(action.mode.as_str())
            .unwrap_or_else(|_| "\"notify\"".to_string());
        output.push_str(&format!(
            "export const {} = __lx_define_page_bridge({}, {});\n\n",
            action.name, name_json, mode_json,
        ));
    }

    let names = actions
        .iter()
        .map(|action| action.name.as_str())
        .collect::<Vec<_>>();
    output.push_str(&format!(
        "export const __names = {};\n",
        serde_json::to_string(&names).unwrap_or_else(|_| "[]".to_string())
    ));
    output
}

pub(crate) fn render_page_bridge_import() -> String {
    "import * as __pageBridge from './__page_bridge__.js';\nwindow.__pageBridge = __pageBridge;"
        .to_string()
}

pub(crate) fn bridge_metadata_script(actions: &[PageAction]) -> String {
    let names = actions
        .iter()
        .map(|action| action.name.as_str())
        .collect::<Vec<_>>();
    let modes = actions
        .iter()
        .map(|action| (action.name.as_str(), action.mode.as_str()))
        .collect::<std::collections::BTreeMap<_, _>>();
    format!(
        "window.__pageBridge = {{ __names: {}, __modes: {} }};",
        serde_json::to_string(&names).unwrap_or_else(|_| "[]".to_string()),
        serde_json::to_string(&modes).unwrap_or_else(|_| "{{}}".to_string())
    )
}

fn infer_property_mode(
    property: &oxc_ast::ast::ObjectProperty<'_>,
    function_bindings: &HashMap<String, FunctionBinding<'_>>,
) -> PageActionMode {
    if property.method {
        if let Expression::FunctionExpression(function) = unwrap_expression(&property.value) {
            return function_mode(function.body.as_deref(), function.generator, false);
        }
        return PageActionMode::Notify;
    }

    match unwrap_expression(&property.value) {
        Expression::FunctionExpression(function) => {
            function_mode(function.body.as_deref(), function.generator, false)
        }
        Expression::ArrowFunctionExpression(function) => {
            let (body, returns_expression) = arrow_body(function);
            function_mode(body, false, returns_expression)
        }
        Expression::Identifier(identifier) => function_bindings
            .get(identifier.name.as_str())
            .map(|binding| {
                function_mode(
                    binding.body,
                    binding.is_generator,
                    binding.returns_expression,
                )
            })
            .unwrap_or(PageActionMode::Notify),
        _ => PageActionMode::Notify,
    }
}

/// Splits an arrow's body into the block form and whether it is a concise body,
/// whose expression is itself the return value.
fn arrow_body<'a>(
    function: &'a oxc_ast::ast::ArrowFunctionExpression<'a>,
) -> (Option<&'a oxc_ast::ast::FunctionBody<'a>>, bool) {
    match &function.body {
        oxc_ast::ast::ArrowFunctionBody::FunctionBody(body) => (Some(body), false),
        _ => (None, true),
    }
}

fn function_mode(
    body: Option<&oxc_ast::ast::FunctionBody<'_>>,
    is_generator: bool,
    returns_expression: bool,
) -> PageActionMode {
    if is_generator {
        PageActionMode::Stream
    } else if returns_expression || body.is_some_and(function_body_returns_value) {
        PageActionMode::Call
    } else {
        PageActionMode::Notify
    }
}

fn function_body_returns_value(body: &oxc_ast::ast::FunctionBody<'_>) -> bool {
    statements_return_value(&body.statements)
}

fn statements_return_value(statements: &[Statement<'_>]) -> bool {
    statements.iter().any(statement_returns_value)
}

fn statement_returns_value(statement: &Statement<'_>) -> bool {
    match statement {
        Statement::ReturnStatement(return_statement) => return_statement.argument.is_some(),
        Statement::BlockStatement(block) => statements_return_value(&block.body),
        Statement::IfStatement(if_statement) => {
            statement_returns_value(&if_statement.consequent)
                || if_statement
                    .alternate
                    .as_ref()
                    .is_some_and(statement_returns_value)
        }
        Statement::SwitchStatement(switch_statement) => switch_statement
            .cases
            .iter()
            .any(|case| statements_return_value(&case.consequent)),
        Statement::LabeledStatement(labeled_statement) => {
            statement_returns_value(&labeled_statement.body)
        }
        Statement::DoWhileStatement(do_while_statement) => {
            statement_returns_value(&do_while_statement.body)
        }
        Statement::WhileStatement(while_statement) => {
            statement_returns_value(&while_statement.body)
        }
        Statement::ForStatement(for_statement) => statement_returns_value(&for_statement.body),
        Statement::ForInStatement(for_in_statement) => {
            statement_returns_value(&for_in_statement.body)
        }
        Statement::ForOfStatement(for_of_statement) => {
            statement_returns_value(&for_of_statement.body)
        }
        Statement::TryStatement(try_statement) => {
            statements_return_value(&try_statement.block.body)
                || try_statement
                    .handler
                    .as_ref()
                    .is_some_and(|handler| statements_return_value(&handler.body.body))
                || try_statement
                    .finalizer
                    .as_ref()
                    .is_some_and(|finalizer| statements_return_value(&finalizer.body))
        }
        _ => false,
    }
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

fn property_name(key: &PropertyKey<'_>) -> Option<String> {
    match key {
        PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.as_str().to_string()),
        PropertyKey::StringLiteral(literal) => Some(literal.value.as_str().to_string()),
        _ => None,
    }
}

fn is_function_like_property(
    property: &oxc_ast::ast::ObjectProperty<'_>,
    function_bindings: &HashMap<String, FunctionBinding<'_>>,
) -> bool {
    if property.method {
        return true;
    }
    match unwrap_expression(&property.value) {
        Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => true,
        Expression::Identifier(identifier) => {
            function_bindings.contains_key(identifier.name.as_str())
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn extract_action_modes(source: &str) -> Vec<(String, PageActionMode)> {
        let temp = tempdir().unwrap();
        let logic_path = temp.path().join("index.ts");
        fs::write(&logic_path, source).unwrap();
        extract_page_actions(Some(&logic_path))
            .unwrap()
            .into_iter()
            .map(|action| (action.name, action.mode))
            .collect()
    }

    fn create_project(root: &Path, framework: ProjectFramework, page: &str) -> Project {
        Project {
            root: root.to_path_buf(),
            kind: crate::lxapp::project::ProjectKind::LxApp,
            framework,
            output_dir: root.join("dist"),
            pages: vec![page.to_string()],
            logic_entry: None,
            plugin_id: None,
            package_name: None,
            version: "1.0.0".to_string(),
        }
    }

    #[test]
    fn async_navigation_actions_without_return_are_notify() {
        let actions = extract_action_modes(
            r#"
            Page({
              navigateToUIPage: async function (params) {
                await lx.navigateTo({ page: "ui", query: { type: "toast" } });
              }
            });
            "#,
        );

        assert_eq!(
            actions,
            vec![("navigateToUIPage".to_string(), PageActionMode::Notify)]
        );
    }

    #[test]
    fn explicit_return_actions_are_call() {
        let actions = extract_action_modes(
            r#"
            Page({
              updateNavigationBarTitle: function (options) {
                return lx.navigationBar.update(options);
              },
              chooseToastIcon: async function () {
                const result = await lx.showActionSheet({ itemList: ["a", "b"] });
                return result;
              },
              directArrow: (payload) => lx.callSomething(payload)
            });
            "#,
        );

        assert_eq!(
            actions,
            vec![
                ("updateNavigationBarTitle".to_string(), PageActionMode::Call),
                ("chooseToastIcon".to_string(), PageActionMode::Call),
                ("directArrow".to_string(), PageActionMode::Call),
            ]
        );
    }

    #[test]
    fn generator_actions_are_stream() {
        let actions = extract_action_modes(
            r#"
            Page({
              watchMessages: async function* () {
                yield "hello";
              }
            });
            "#,
        );

        assert_eq!(
            actions,
            vec![("watchMessages".to_string(), PageActionMode::Stream)]
        );
    }

    #[test]
    fn named_function_refs_are_extracted_and_lifecycle_private_are_filtered() {
        let actions = extract_action_modes(
            r#"
            async function navigateToUIPage() {
              await lx.navigateTo({ page: "ui", query: { type: "toast" } });
            }

            const chooseToastIcon = async () => {
              const result = await lx.showActionSheet({ itemList: ["a", "b"] });
              return result;
            };

            const aliasedAction = chooseToastIcon;

            Page({
              data: {},
              onLoad() {},
              _privateAction() {},
              navigateToUIPage,
              chooseToastIcon,
              aliasedAction,
            });
            "#,
        );

        assert_eq!(
            actions,
            vec![
                ("navigateToUIPage".to_string(), PageActionMode::Notify),
                ("chooseToastIcon".to_string(), PageActionMode::Call),
                ("aliasedAction".to_string(), PageActionMode::Call),
            ]
        );
    }

    #[test]
    fn non_function_page_fields_are_not_extracted() {
        let actions = extract_action_modes(
            r#"
            const title = "demo";
            const config = { enabled: true };

            Page({
              title,
              config,
              missingRef,
              doWork() {},
            });
            "#,
        );

        assert_eq!(
            actions,
            vec![("doWork".to_string(), PageActionMode::Notify)]
        );
    }

    #[test]
    fn bridge_metadata_includes_action_modes() {
        let script = bridge_metadata_script(&[
            PageAction {
                name: "confirmOrientation".to_string(),
                mode: PageActionMode::Call,
            },
            PageAction {
                name: "watchMessages".to_string(),
                mode: PageActionMode::Stream,
            },
        ]);

        assert!(script.contains("__names"));
        assert!(script.contains("__modes"));
        assert!(script.contains("\"confirmOrientation\":\"call\""));
        assert!(script.contains("\"watchMessages\":\"stream\""));
    }

    #[test]
    fn react_view_must_not_call_lx_directly() {
        let temp = tempdir().unwrap();
        let page_path = "pages/api/index.tsx";
        let full_path = temp.path().join(page_path);
        fs::create_dir_all(full_path.parent().unwrap()).unwrap();
        fs::write(
            &full_path,
            r#"
            export default function ApiPage() {
              return <button onClick={() => lx.showToast({ title: "bad" })}>bad</button>;
            }
            "#,
        )
        .unwrap();

        let project = create_project(temp.path(), ProjectFramework::React, page_path);
        let error = validate_component_view_bindings(
            &project,
            page_path,
            &[PageAction {
                name: "showToast".to_string(),
                mode: PageActionMode::Notify,
            }],
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("must not call lx.* directly"));
        assert!(error.contains("lx.showToast"));
    }

    #[test]
    fn react_view_missing_page_action_fails() {
        let temp = tempdir().unwrap();
        let page_path = "pages/api/index.tsx";
        let full_path = temp.path().join(page_path);
        fs::create_dir_all(full_path.parent().unwrap()).unwrap();
        fs::write(
            &full_path,
            r#"
            import { useLxPage } from "@lingxia/sdk";

            export default function ApiPage() {
              const { actions } = useLxPage();
              return <button onClick={() => actions.navigateToUIPage()}>go</button>;
            }
            "#,
        )
        .unwrap();

        let project = create_project(temp.path(), ProjectFramework::React, page_path);
        let error = validate_component_view_bindings(&project, page_path, &[])
            .unwrap_err()
            .to_string();

        assert!(error.contains("references missing Page(...) actions"));
        assert!(error.contains("navigateToUIPage"));
    }

    /// Build an entry that re-wraps `actions` into its own object and hands it
    /// to a child view, plus that child. `forwarded` is what the entry names.
    fn write_whitelist_entry(root: &Path, page_path: &str, forwarded: &[&str], child_calls: &str) {
        let full_path = root.join(page_path);
        fs::create_dir_all(full_path.parent().unwrap()).unwrap();
        let forwarded_lines = forwarded
            .iter()
            .map(|name| format!("{name}: actions.{name},"))
            .collect::<Vec<_>>()
            .join("\n            ");
        fs::write(
            &full_path,
            format!(
                r#"
            import {{ useLxPage }} from "@lingxia/sdk";
            import ChildView from "./views/child-view";

            export default function Page() {{
              const {{ data, actions }} = useLxPage();
              const page = {{
                data,
                actions: {{
            {forwarded_lines}
                }},
              }};
              return <ChildView page={{page}} />;
            }}
            "#
            ),
        )
        .unwrap();

        let child_path = full_path.parent().unwrap().join("views/child-view.tsx");
        fs::create_dir_all(child_path.parent().unwrap()).unwrap();
        fs::write(
            &child_path,
            format!(
                r#"
            export default function ChildView({{ page }}) {{
              const {{ actions }} = page;
              return <button onClick={{() => {child_calls}}}>go</button>;
            }}
            "#
            ),
        )
        .unwrap();
    }

    #[test]
    fn entry_whitelist_missing_a_child_action_fails_the_build() {
        let temp = tempdir().unwrap();
        let page_path = "pages/account/index.tsx";
        // The entry forwards `save` but the child also calls `invite` — exactly
        // the shape that type-checks and then does nothing when tapped.
        write_whitelist_entry(temp.path(), page_path, &["save"], "actions.invite()");

        let project = create_project(temp.path(), ProjectFramework::React, page_path);
        let error = validate_component_view_bindings(
            &project,
            page_path,
            &[
                PageAction {
                    name: "save".to_string(),
                    mode: PageActionMode::Notify,
                },
                PageAction {
                    name: "invite".to_string(),
                    mode: PageActionMode::Notify,
                },
            ],
        )
        .unwrap_err()
        .to_string();

        assert!(
            error.contains("does not forward Page(...) actions its child views call: invite"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn entry_whitelist_covering_every_child_action_passes() {
        let temp = tempdir().unwrap();
        let page_path = "pages/account/index.tsx";
        write_whitelist_entry(
            temp.path(),
            page_path,
            &["save", "invite"],
            "actions.invite()",
        );

        let project = create_project(temp.path(), ProjectFramework::React, page_path);
        validate_component_view_bindings(
            &project,
            page_path,
            &[
                PageAction {
                    name: "save".to_string(),
                    mode: PageActionMode::Notify,
                },
                PageAction {
                    name: "invite".to_string(),
                    mode: PageActionMode::Notify,
                },
            ],
        )
        .expect("a complete whitelist must build");
    }

    #[test]
    fn react_view_marks_used_page_actions() {
        let temp = tempdir().unwrap();
        let page_path = "pages/api/index.tsx";
        let full_path = temp.path().join(page_path);
        fs::create_dir_all(full_path.parent().unwrap()).unwrap();
        fs::write(
            &full_path,
            r#"
            import { useLxPage } from "@lingxia/sdk";

            export default function ApiPage() {
              const { actions: pageActions } = useLxPage();
              const { navigateToUIPage } = pageActions;
              return (
                <>
                  <button onClick={() => navigateToUIPage()}>go</button>
                  <button onClick={() => pageActions.openDeepSeek()}>deepseek</button>
                </>
              );
            }
            "#,
        )
        .unwrap();

        let project = create_project(temp.path(), ProjectFramework::React, page_path);
        let audit = validate_component_view_bindings(
            &project,
            page_path,
            &[
                PageAction {
                    name: "navigateToUIPage".to_string(),
                    mode: PageActionMode::Notify,
                },
                PageAction {
                    name: "openDeepSeek".to_string(),
                    mode: PageActionMode::Notify,
                },
            ],
        )
        .unwrap();

        assert_eq!(
            audit.used_actions,
            BTreeSet::from(["navigateToUIPage".to_string(), "openDeepSeek".to_string()])
        );
    }

    #[test]
    fn vue_template_marks_bare_action_identifiers() {
        let temp = tempdir().unwrap();
        let page_path = "pages/api/index.vue";
        let full_path = temp.path().join(page_path);
        fs::create_dir_all(full_path.parent().unwrap()).unwrap();
        fs::write(
            &full_path,
            r#"
            <template>
              <template v-if="enabled">
                <button
                  @click="primaryAction"
                  class="sm:hover:text-blue-500 primary"
                >
                  primary
                </button>
              </template>
              <button
                @click="secondaryAction"
                :class="enabled ? 'sm:text-green-500' : 'text-gray-500'"
              >
                secondary
              </button>
              <button @click="primaryAction(); secondaryAction()">both</button>
            </template>
            <script setup lang="ts">
            import { useLxPage } from "@lingxia/vue";
            const { actions } = useLxPage();
            const { primaryAction, secondaryAction } = actions;
            </script>
            "#,
        )
        .unwrap();

        let project = create_project(temp.path(), ProjectFramework::Vue, page_path);
        let audit = validate_component_view_bindings(
            &project,
            page_path,
            &[
                PageAction {
                    name: "primaryAction".to_string(),
                    mode: PageActionMode::Notify,
                },
                PageAction {
                    name: "secondaryAction".to_string(),
                    mode: PageActionMode::Notify,
                },
            ],
        )
        .unwrap();

        assert_eq!(
            audit.used_actions,
            BTreeSet::from(["primaryAction".to_string(), "secondaryAction".to_string()])
        );
    }

    fn write_file(root: &Path, relative: &str, contents: &str) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn action(name: &str) -> PageAction {
        PageAction {
            name: name.to_string(),
            mode: PageActionMode::Notify,
        }
    }

    const COMPOSITION_ROOT: &str = r#"
        import { useLxPage } from "@lingxia/react";
        import CompactView from "./views/compact-view";

        export default function Page() {
          const { data, actions } = useLxPage();
          const page = { data, actions };
          return <CompactView page={page} />;
        }
        "#;

    #[test]
    fn composition_root_credits_actions_wired_in_the_views_it_renders() {
        let temp = tempdir().unwrap();
        let page_path = "pages/speedtest/index.tsx";
        write_file(temp.path(), page_path, COMPOSITION_ROOT);
        write_file(
            temp.path(),
            "pages/speedtest/views/compact-view.tsx",
            r#"
            import Chart from "../components/chart";

            export default function CompactView({ page }) {
              const { actions } = page;
              return (
                <>
                  <button onClick={() => actions.start()}>start</button>
                  <Chart actions={actions} />
                </>
              );
            }
            "#,
        );
        write_file(
            temp.path(),
            "pages/speedtest/components/chart.tsx",
            r#"
            export default function Chart({ actions }) {
              return <button onClick={() => actions.reset()}>reset</button>;
            }
            "#,
        );

        let project = create_project(temp.path(), ProjectFramework::React, page_path);
        let audit = validate_component_view_bindings(
            &project,
            page_path,
            &[action("start"), action("reset")],
        )
        .unwrap();

        assert!(audit.unused_reportable);
        assert_eq!(
            audit.used_actions,
            BTreeSet::from(["reset".to_string(), "start".to_string()])
        );
    }

    #[test]
    fn composition_root_still_reports_an_action_no_view_wires() {
        let temp = tempdir().unwrap();
        let page_path = "pages/wifi/index.tsx";
        write_file(temp.path(), page_path, COMPOSITION_ROOT);
        write_file(
            temp.path(),
            "pages/wifi/views/compact-view.tsx",
            r#"
            export default function CompactView({ page }) {
              return <button onClick={() => page.actions.save()}>save</button>;
            }
            "#,
        );

        let project = create_project(temp.path(), ProjectFramework::React, page_path);
        let audit = validate_component_view_bindings(
            &project,
            page_path,
            &[action("save"), action("back")],
        )
        .unwrap();

        assert!(audit.unused_reportable);
        assert_eq!(audit.used_actions, BTreeSet::from(["save".to_string()]));
    }

    #[test]
    fn an_unreadable_view_keeps_every_action_unjudged() {
        let temp = tempdir().unwrap();
        let page_path = "pages/wifi/index.tsx";
        write_file(temp.path(), page_path, COMPOSITION_ROOT);
        // Reachable but unparseable: usage may live in it, so nothing is dead.
        write_file(
            temp.path(),
            "pages/wifi/views/compact-view.tsx",
            "export default function CompactView({ page } {",
        );

        let project = create_project(temp.path(), ProjectFramework::React, page_path);
        let audit =
            validate_component_view_bindings(&project, page_path, &[action("save")]).unwrap();

        assert!(!audit.unused_reportable);
    }

    #[test]
    fn a_view_that_keeps_actions_to_itself_is_judged_alone() {
        let temp = tempdir().unwrap();
        let page_path = "pages/api/index.tsx";
        write_file(
            temp.path(),
            page_path,
            r#"
            import { useLxPage } from "@lingxia/react";

            export default function Page() {
              const { actions } = useLxPage();
              return <button onClick={() => actions.run()}>run</button>;
            }
            "#,
        );

        let project = create_project(temp.path(), ProjectFramework::React, page_path);
        let audit =
            validate_component_view_bindings(&project, page_path, &[action("run"), action("idle")])
                .unwrap();

        assert!(audit.unused_reportable);
        assert_eq!(audit.used_actions, BTreeSet::from(["run".to_string()]));
    }

    #[test]
    fn vue_composition_root_credits_child_template_bindings() {
        let temp = tempdir().unwrap();
        let page_path = "pages/speedtest/index.vue";
        write_file(
            temp.path(),
            page_path,
            r#"
            <template>
              <CompactView :actions="actions" />
            </template>
            <script setup lang="ts">
            import { useLxPage } from "@lingxia/vue";
            import CompactView from "./views/compact-view.vue";
            const { actions } = useLxPage();
            </script>
            "#,
        );
        write_file(
            temp.path(),
            "pages/speedtest/views/compact-view.vue",
            r#"
            <template>
              <button @click="actions.start()">start</button>
            </template>
            <script setup lang="ts">
            defineProps<{ actions: unknown }>();
            </script>
            "#,
        );

        let project = create_project(temp.path(), ProjectFramework::Vue, page_path);
        let audit =
            validate_component_view_bindings(&project, page_path, &[action("start")]).unwrap();

        assert!(audit.unused_reportable);
        assert_eq!(audit.used_actions, BTreeSet::from(["start".to_string()]));
    }
}
