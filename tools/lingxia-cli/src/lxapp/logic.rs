use crate::lxapp::options::BuildOptions;
use crate::lxapp::project::{Project, ProjectKind};
use anyhow::{Context, Result, anyhow, bail};
use indicatif::ProgressBar;
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Declaration, ExportDefaultDeclarationKind, Expression, ImportDeclarationSpecifier,
    ImportOrExportKind, ModuleExportName, ObjectPropertyKind, PropertyKey, Statement,
};
use oxc_codegen::{Codegen, CodegenOptions};
use oxc_parser::Parser;
use oxc_resolver::{
    ModuleType, ResolveOptions, Resolver, TsconfigDiscovery, TsconfigOptions, TsconfigReferences,
};
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, SourceType};
use oxc_transformer::{TransformOptions, Transformer};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub enum LogicBuildStatus {
    Built { output_path: PathBuf },
    Disabled,
    Skipped,
}

#[derive(Debug, Clone)]
pub struct LogicBuildReport {
    pub status: LogicBuildStatus,
}

pub fn build(
    project: &Project,
    options: &BuildOptions,
    progress: Option<ProgressBar>,
) -> Result<LogicBuildReport> {
    if let Some(progress) = &progress {
        progress.set_message(format!(
            "{} scanning entries",
            console::style("Logic").cyan()
        ));
    }

    let Some(logic_entry) = &project.logic_entry else {
        return Ok(LogicBuildReport {
            status: LogicBuildStatus::Disabled,
        });
    };

    let app_logic = discover_app_logic(project.root.as_path())?;
    let (page_logic_entries, view_only_pages) = discover_page_logic_entries(project)?;

    if app_logic.is_none() && page_logic_entries.is_empty() {
        return Ok(LogicBuildReport {
            status: LogicBuildStatus::Skipped,
        });
    }

    if let Some(progress) = &progress {
        progress.set_message(format!(
            "{} bundling modules",
            console::style("Logic").cyan()
        ));
    }

    let mut bundler = LogicBundler::new(project);
    // Each entry's refusal is collected, so one build names every page that has one.
    let mut failures = Vec::new();
    // The generated client installs `page.native`, so it ships first whether or not a module
    // imports it; an `@lingxia/native` import resolves to this same module.
    let native_client = native_client_path(&project.root);
    let entries = native_client
        .is_file()
        .then_some((native_client, ModuleRole::Plain))
        .into_iter()
        .chain(app_logic.map(|path| (path, ModuleRole::App)))
        .chain(
            page_logic_entries
                .into_iter()
                .map(|(path, page_path)| (path, ModuleRole::Page { page_path })),
        );
    for (path, role) in entries {
        if let Err(error) = bundler.add_entry(path, role) {
            // A module abandoned mid-compile is not a cycle for the next entry that imports it.
            bundler.visiting.clear();
            let message = format!("{error:#}");
            // A shared module's refusal reached again through another page is said once.
            if !failures.contains(&message) {
                failures.push(message);
            }
        }
    }
    if !failures.is_empty() {
        bail!(failures.join("\n"));
    }
    for page_path in view_only_pages {
        bundler.add_view_only_page(&page_path)?;
    }

    let bundle = bundler.render_bundle(options.release)?;
    let output_path = project.output_dir.join(logic_entry);
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    if let Some(progress) = &progress {
        progress.set_message(format!("{} writing bundle", console::style("Logic").cyan()));
    }
    fs::write(&output_path, bundle)
        .with_context(|| format!("Failed to write {}", output_path.display()))?;
    Ok(LogicBuildReport {
        status: LogicBuildStatus::Built { output_path },
    })
}

#[derive(Debug, Clone)]
enum ModuleRole {
    Plain,
    App,
    Page {
        page_path: String,
    },
    /// `mocks/index.ts`: the bundle's value is its default export.
    Mocks,
}

/// An lxapp's bundled `mocks/index.ts`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MocksBundle {
    /// A script whose value is the default export: the handler map.
    pub source: String,
    /// Its keys, in object order.
    pub keys: Vec<String>,
    /// `mocks/index.ts` or `mocks/index.js`, relative to the lxapp root.
    pub entry: String,
}

/// The lxapp's handlers file, when it has one.
pub fn mocks_entry(root: &Path) -> Result<Option<PathBuf>> {
    let dir = root.join(lingxia_control_protocol::mock::MOCKS_DIR);
    let found: Vec<PathBuf> = lingxia_control_protocol::mock::HANDLERS_FILES
        .iter()
        .map(|name| dir.join(name))
        .filter(|path| path.is_file())
        .collect();
    match found.as_slice() {
        [] => Ok(None),
        [one] => Ok(Some(one.clone())),
        _ => bail!("mocks/: found both index.ts and index.js; keep one"),
    }
}

/// Bundle `mocks/index.ts` with the Logic bundler. Its default export must
/// be an object literal whose keys are HTTP targets (`'GET **/x'`), so the
/// complete set of handlers is visible in one place. `None` when the lxapp
/// has no `mocks/index.ts`.
pub fn build_mocks(root: &Path) -> Result<Option<MocksBundle>> {
    let Some(entry) = mocks_entry(root)? else {
        return Ok(None);
    };
    // Mocks are plain modules: no app or page registration, so the
    // bundler needs only the root.
    let project = &Project {
        root: root.to_path_buf(),
        kind: ProjectKind::LxApp,
        framework: crate::lxapp::ProjectFramework::Html,
        output_dir: root.join("dist"),
        pages: Vec::new(),
        page_names: Vec::new(),
        logic_entry: None,
        plugin_id: None,
        package_name: None,
        version: String::new(),
    };
    let label = relative_to(&entry, project.root.as_path());
    let source = fs::read_to_string(&entry)
        .with_context(|| format!("Failed to read {}", entry.display()))?;
    let keys = mock_keys(&entry, &source).map_err(|err| anyhow!("{label}: {err}"))?;
    let mut bundler = LogicBundler::new(project);
    let module_var = bundler.add_entry(entry, ModuleRole::Mocks)?;
    let mut output = String::from("(function() {\n\n");
    for module in bundler.modules {
        output.push_str(&module.rendered);
        output.push('\n');
    }
    output.push_str(&format!(
        "return {};\n}})();\n",
        module_export_access_expr(&module_var, "default")
    ));
    Ok(Some(MocksBundle {
        source: output,
        keys,
        entry: label,
    }))
}

/// The handler keys of a `mocks/index.ts` default export, in order.
fn mock_keys(path: &Path, source: &str) -> Result<Vec<String>> {
    use lingxia_control_protocol::mock::{MockTarget, SCENARIO_ONLY_FIELDS, parse_handler_key};
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(path).map_err(|_| anyhow!("unsupported file type"))?;
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.diagnostics.is_empty() {
        bail!(
            "failed to parse: {}",
            format_diagnostics(&parsed.diagnostics)
        );
    }
    let program = parsed.program;
    let shape = "the default export must be an object of handlers: \
                 export default { 'GET **/path': (req) => ({ json: … }) } satisfies Mocks";
    let default = program
        .body
        .iter()
        .find_map(|statement| match statement {
            Statement::ExportDefaultDeclaration(export) => Some(&export.declaration),
            _ => None,
        })
        .ok_or_else(|| anyhow!("{shape}"))?;
    let expression = default
        .as_expression()
        .map(unwrap_expression)
        .ok_or_else(|| anyhow!("{shape}"))?;
    // `export default mocks` of a `const mocks = { … }` in the same file.
    let expression = match expression {
        Expression::Identifier(identifier) => program
            .body
            .iter()
            .find_map(|statement| {
                let declaration = match statement {
                    Statement::VariableDeclaration(declaration) => declaration,
                    Statement::ExportDeclaration(export) => match &export.declaration {
                        Declaration::VariableDeclaration(declaration) => declaration,
                        _ => return None,
                    },
                    _ => return None,
                };
                declaration.declarations.iter().find_map(|declarator| {
                    match (&declarator.id, &declarator.init) {
                        (oxc_ast::ast::BindingPattern::BindingIdentifier(id), Some(init))
                            if id.name == identifier.name =>
                        {
                            Some(unwrap_expression(init))
                        }
                        _ => None,
                    }
                })
            })
            .ok_or_else(|| anyhow!("{shape}"))?,
        other => other,
    };
    let Expression::ObjectExpression(object) = expression else {
        bail!("{shape}");
    };
    let mut keys: Vec<String> = Vec::new();
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            bail!(
                "list every handler in the default export under its own 'METHOD url-glob' or \
                 'native route' key; a spread (...) hides which calls it answers"
            );
        };
        let key = match &property.key {
            PropertyKey::StringLiteral(literal) if !property.computed => {
                literal.value.as_str().to_string()
            }
            PropertyKey::StaticIdentifier(identifier) if !property.computed => {
                identifier.name.as_str().to_string()
            }
            _ => bail!(
                "a handler key must be a 'METHOD url-glob' or 'native route' string, like \
                 'GET **/devices/*'; a computed key hides which calls it answers"
            ),
        };
        let target = parse_handler_key(&key).map_err(|err| anyhow!("{err}"))?;
        if keys.contains(&key) {
            bail!("'{key}' is listed twice");
        }
        // A native answer is the route's result, whose fields are its own.
        if let (MockTarget::Http { .. }, Expression::ObjectExpression(answer)) =
            (&target, unwrap_expression(&property.value))
        {
            for field in &answer.properties {
                if let ObjectPropertyKind::ObjectProperty(field) = field
                    && let Some(name) = field.key.static_name()
                    && SCENARIO_ONLY_FIELDS.contains(&name.as_ref())
                {
                    bail!(
                        "'{key}': '{name}' is a scenario field; a handler returns one answer per \
                         call"
                    );
                }
            }
        }
        keys.push(key);
    }
    Ok(keys)
}

/// The error for product code that imports `mocks/`.
fn mocks_import_error(importer: &str, imported: &str) -> anyhow::Error {
    anyhow!(
        "Logic build failed: {importer} imports from mocks/ ({imported}). Product code never \
         imports mocks; the app calls fetch and mocks/index.ts answers it in dev."
    )
}

#[derive(Debug, Clone)]
struct ImportBinding {
    imported: Option<String>,
    local: String,
    namespace: bool,
    type_only: bool,
}

#[derive(Debug, Clone)]
struct ImportRecord {
    statement_span: oxc_span::Span,
    resolved_local: Option<PathBuf>,
    bindings: Vec<ImportBinding>,
}

#[derive(Debug, Clone)]
struct ExportDependencyRecord {
    statement_span: oxc_span::Span,
    resolved_local: Option<PathBuf>,
    export_all: bool,
}

#[derive(Debug, Clone)]
struct ModuleArtifact {
    rendered: String,
}

struct LogicBundler<'a> {
    project: &'a Project,
    modules: Vec<ModuleArtifact>,
    module_vars: HashMap<PathBuf, String>,
    visiting: HashSet<PathBuf>,
    view_only: Vec<String>,
}

impl<'a> LogicBundler<'a> {
    fn new(project: &'a Project) -> Self {
        Self {
            project,
            modules: Vec::new(),
            module_vars: HashMap::new(),
            visiting: HashSet::new(),
            view_only: Vec::new(),
        }
    }

    fn add_entry(&mut self, path: PathBuf, role: ModuleRole) -> Result<String> {
        self.compile_module(path, role)
    }

    /// A page without Logic still gets a service once the lxapp has Logic, so it
    /// registers an empty definition: no data, no actions.
    fn add_view_only_page(&mut self, page_path: &str) -> Result<()> {
        self.view_only.push(format!(
            "globalThis.__registerDefinedPage({}, Object.defineProperty({{}}, Symbol.for(\"lingxia.page\"), {{ value: true }}));\n",
            json_string_literal(&page_registration_path(self.project, page_path)?)
        ));
        Ok(())
    }

    fn render_bundle(self, release: bool) -> Result<String> {
        let mut output = String::from("(function() {\n\n");
        for module in self.modules {
            output.push_str(&module.rendered);
            output.push('\n');
        }
        for registration in self.view_only {
            output.push_str(&registration);
        }
        output.push_str("})();\n");
        if release {
            crate::lxapp::hardening::harden_logic_bundle(&output)
        } else {
            Ok(output)
        }
    }

    fn compile_module(&mut self, path: PathBuf, role: ModuleRole) -> Result<String> {
        let path = normalize_path(&path)?;
        if let Some(module_var) = self.module_vars.get(&path) {
            return Ok(module_var.clone());
        }
        if !self.visiting.insert(path.clone()) {
            return Err(anyhow!(
                "Circular logic import detected at {}",
                path.display()
            ));
        }

        let source = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read logic module {}", path.display()))?;
        let source_type = SourceType::from_path(&path)
            .map_err(|_| anyhow!("Unsupported logic file {}", path.display()))?;
        let allocator = Allocator::default();
        let parse_result = Parser::new(&allocator, &source, source_type).parse();
        if !parse_result.diagnostics.is_empty() {
            bail!(
                "Failed to parse logic module {}: {}",
                path.display(),
                format_diagnostics(&parse_result.diagnostics)
            );
        }
        let program = parse_result.program;
        if !path
            .components()
            .any(|part| part.as_os_str() == "node_modules")
            && let Some(line) = unimported_lx_line(&program, &source)
        {
            bail!(
                "{}:{line}: `lx` is not imported; add `import {{ lx }} from '@lingxia/logic'`",
                relative_to(
                    &path,
                    &normalize_path(&self.project.root)
                        .unwrap_or_else(|_| self.project.root.clone())
                )
            );
        }
        if normalize_path(&native_client_path(&self.project.root))
            .ok()
            .as_deref()
            != Some(path.as_path())
            && let Some(line) = native_bridge_line(&program, &source)
        {
            bail!(
                "{}:{line}: `{NATIVE_BRIDGE_GLOBAL}` is the runtime's private native bridge; \
                 call routes through `page.native` or `import {{ native }} from '@lingxia/native'`",
                relative_to(
                    &path,
                    &normalize_path(&self.project.root)
                        .unwrap_or_else(|_| self.project.root.clone())
                )
            );
        }

        let imports = collect_imports(&program, &source, &path, self.project.root.as_path())?;
        let export_dependencies =
            collect_export_dependencies(&program, &path, self.project.root.as_path())?;
        let mut dependency_vars = BTreeMap::new();
        // Mocks never ship: nothing outside `mocks/` may import from it.
        let mocks_dir = normalize_path(&self.project.root)
            .unwrap_or_else(|_| self.project.root.clone())
            .join(lingxia_control_protocol::mock::MOCKS_DIR);
        let root = normalize_path(&self.project.root).unwrap_or_else(|_| self.project.root.clone());
        for local_path in
            dependency_paths_in_source_order(&program, &imports, &export_dependencies)?
        {
            if local_path.starts_with(&mocks_dir) && !path.starts_with(&mocks_dir) {
                return Err(mocks_import_error(
                    &relative_to(&path, &root),
                    &relative_to(&local_path, &root),
                ));
            }
            let module_var = self.compile_module(local_path.clone(), ModuleRole::Plain)?;
            dependency_vars.insert(local_path, module_var);
        }

        let rewritten = rewrite_module_source(
            self.project,
            &program,
            &source,
            &path,
            &role,
            &imports,
            &export_dependencies,
            &dependency_vars,
        )?;
        let transpiled = transpile_module(&path, &rewritten)?;

        let module_var = format!("__lx_mod_{}", self.modules.len());
        // The module body goes in as transpiled, not re-indented: a line-based
        // indent also shifts every continuation line of a multi-line template
        // literal, which changes the string the program sees.
        let rendered = format!(
            "//#region {}\nconst {} = (() => {{\n{}\n  return __lx_module_exports;\n}})();\n//#endregion",
            relative_to(&path, self.project.root.as_path()),
            module_var,
            transpiled.trim_end_matches('\n')
        );
        self.module_vars.insert(path.clone(), module_var.clone());
        self.modules.push(ModuleArtifact { rendered });
        self.visiting.remove(&path);
        Ok(module_var)
    }
}

fn dependency_paths_in_source_order(
    program: &oxc_ast::ast::Program<'_>,
    imports: &[ImportRecord],
    export_dependencies: &[ExportDependencyRecord],
) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    let mut seen = HashSet::new();
    for statement in &program.body {
        let path = match statement {
            Statement::ImportDeclaration(declaration) => imports
                .iter()
                .find(|record| record.statement_span == declaration.span)
                .ok_or_else(|| anyhow!("Internal import bookkeeping mismatch"))?
                .resolved_local
                .as_ref(),
            Statement::ExportFromDeclaration(declaration) => export_dependencies
                .iter()
                .find(|record| record.statement_span == declaration.span)
                .ok_or_else(|| anyhow!("Internal export bookkeeping mismatch"))?
                .resolved_local
                .as_ref(),
            Statement::ExportAllDeclaration(declaration) => export_dependencies
                .iter()
                .find(|record| record.statement_span == declaration.span)
                .ok_or_else(|| anyhow!("Internal export bookkeeping mismatch"))?
                .resolved_local
                .as_ref(),
            _ => None,
        };
        if let Some(path) = path
            && seen.insert(path.clone())
        {
            paths.push(path.clone());
        }
    }
    Ok(paths)
}

fn discover_app_logic(project_root: &Path) -> Result<Option<PathBuf>> {
    let ts_path = project_root.join("lxapp.ts");
    let js_path = project_root.join("lxapp.js");
    match (ts_path.exists(), js_path.exists()) {
        (true, true) => Err(anyhow!(
            "Logic layer conflict: found both lxapp.ts and lxapp.js"
        )),
        (true, false) => Ok(Some(ts_path)),
        (false, true) => Ok(Some(js_path)),
        (false, false) => Ok(None),
    }
}

/// `(module, compiled page path)` for each page with Logic, then the compiled
/// paths of the pages without it.
type PageLogicEntries = (Vec<(PathBuf, String)>, Vec<String>);

fn discover_page_logic_entries(project: &Project) -> Result<PageLogicEntries> {
    let mut entries = Vec::new();
    let mut view_only = Vec::new();
    for page_path in &project.pages {
        let without_ext = PathBuf::from(crate::lxapp::framework::page_base(page_path));
        let ts_path = project.root.join(&without_ext).with_extension("ts");
        let js_path = project.root.join(&without_ext).with_extension("js");
        match (ts_path.exists(), js_path.exists()) {
            (true, true) => {
                return Err(anyhow!(
                    "Logic layer conflict for {page_path}: found both .ts and .js"
                ));
            }
            (true, false) => entries.push((
                ts_path,
                crate::lxapp::framework::compiled_page_path(&project.root, page_path),
            )),
            (false, true) => entries.push((
                js_path,
                crate::lxapp::framework::compiled_page_path(&project.root, page_path),
            )),
            (false, false) => view_only.push(crate::lxapp::framework::compiled_page_path(
                &project.root,
                page_path,
            )),
        }
    }
    Ok((entries, view_only))
}

fn collect_imports(
    program: &oxc_ast::ast::Program<'_>,
    _source: &str,
    module_path: &Path,
    project_root: &Path,
) -> Result<Vec<ImportRecord>> {
    let mut imports = Vec::new();
    for statement in &program.body {
        let Statement::ImportDeclaration(import_decl) = statement else {
            continue;
        };

        let import_source = import_decl.source.value.as_str().to_string();
        let bindings = import_decl
            .specifiers
            .as_ref()
            .map(|specifiers| {
                specifiers
                    .iter()
                    .map(|specifier| match specifier {
                        ImportDeclarationSpecifier::ImportSpecifier(spec) => Ok(ImportBinding {
                            imported: Some(module_export_name(&spec.imported).ok_or_else(
                                || anyhow!("Unsupported import name in {}", module_path.display()),
                            )?),
                            local: spec.local.name.as_str().to_string(),
                            namespace: false,
                            type_only: import_decl.import_kind == ImportOrExportKind::Type
                                || spec.import_kind == ImportOrExportKind::Type,
                        }),
                        ImportDeclarationSpecifier::ImportDefaultSpecifier(spec) => {
                            Ok(ImportBinding {
                                imported: Some("default".to_string()),
                                local: spec.local.name.as_str().to_string(),
                                namespace: false,
                                type_only: import_decl.import_kind == ImportOrExportKind::Type,
                            })
                        }
                        ImportDeclarationSpecifier::ImportNamespaceSpecifier(spec) => {
                            Ok(ImportBinding {
                                imported: None,
                                local: spec.local.name.as_str().to_string(),
                                namespace: true,
                                type_only: import_decl.import_kind == ImportOrExportKind::Type,
                            })
                        }
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?
            .unwrap_or_default();

        let side_effect_only = import_decl
            .specifiers
            .as_ref()
            .is_none_or(|specifiers| specifiers.is_empty());
        let has_runtime_bindings = bindings.iter().any(|binding| !binding.type_only);
        let resolved_local = if import_decl.import_kind == ImportOrExportKind::Type
            || (!has_runtime_bindings && !side_effect_only)
        {
            None
        } else if is_local_specifier(&import_source) {
            Some(resolve_local_import(
                module_path,
                &import_source,
                project_root,
            )?)
        } else {
            Some(resolve_bare_import(
                module_path,
                &import_source,
                project_root,
            )?)
        };

        imports.push(ImportRecord {
            statement_span: import_decl.span,
            resolved_local,
            bindings,
        });
    }
    Ok(imports)
}

fn collect_export_dependencies(
    program: &oxc_ast::ast::Program<'_>,
    module_path: &Path,
    project_root: &Path,
) -> Result<Vec<ExportDependencyRecord>> {
    let mut exports = Vec::new();
    for statement in &program.body {
        match statement {
            // `export { a }` and `export const a = 1` name no module, so only
            // the `from` forms below carry a dependency to record.
            Statement::ExportFromDeclaration(export_decl) => {
                let has_runtime_specifiers = export_decl.export_kind != ImportOrExportKind::Type
                    && export_decl
                        .specifiers
                        .iter()
                        .any(|specifier| specifier.export_kind != ImportOrExportKind::Type);
                let resolved_local = if has_runtime_specifiers {
                    Some(resolve_import_specifier(
                        module_path,
                        export_decl.source.value.as_str(),
                        project_root,
                    )?)
                } else {
                    None
                };
                exports.push(ExportDependencyRecord {
                    statement_span: export_decl.span,
                    resolved_local,
                    export_all: false,
                });
            }
            Statement::ExportAllDeclaration(export_decl) => {
                let resolved_local = if export_decl.export_kind == ImportOrExportKind::Type {
                    None
                } else {
                    Some(resolve_import_specifier(
                        module_path,
                        export_decl.source.value.as_str(),
                        project_root,
                    )?)
                };
                exports.push(ExportDependencyRecord {
                    statement_span: export_decl.span,
                    resolved_local,
                    export_all: export_decl.exported.is_none(),
                });
            }
            _ => {}
        }
    }
    Ok(exports)
}

fn rewrite_module_source(
    project: &Project,
    program: &oxc_ast::ast::Program<'_>,
    source: &str,
    module_path: &Path,
    role: &ModuleRole,
    imports: &[ImportRecord],
    export_dependencies: &[ExportDependencyRecord],
    dependency_vars: &BTreeMap<PathBuf, String>,
) -> Result<String> {
    let mut output = String::new();
    let mut cursor = 0usize;
    let mut exports = Vec::<(String, String)>::new();
    let mut star_exports = Vec::<String>::new();

    for statement in &program.body {
        let span = statement.span();
        let start = span.start as usize;
        let end = span.end as usize;
        output.push_str(&source[cursor..start]);

        match statement {
            Statement::ImportDeclaration(import_decl) => {
                let import = imports
                    .iter()
                    .find(|record| record.statement_span == import_decl.span)
                    .ok_or_else(|| anyhow!("Internal import bookkeeping mismatch"))?;
                output.push_str(&render_import_stub(import, dependency_vars)?);
            }
            Statement::ExportFromDeclaration(export_decl) => {
                if export_decl.export_kind != ImportOrExportKind::Type {
                    let export = export_dependencies
                        .iter()
                        .find(|record| record.statement_span == export_decl.span)
                        .ok_or_else(|| anyhow!("Internal export bookkeeping mismatch"))?;
                    if let Some(local_path) = &export.resolved_local {
                        let module_var = dependency_vars.get(local_path).ok_or_else(|| {
                            anyhow!("Missing dependency module for {}", local_path.display())
                        })?;
                        for specifier in &export_decl.specifiers {
                            if specifier.export_kind == ImportOrExportKind::Type {
                                continue;
                            }
                            let exported =
                                module_export_name(&specifier.exported).ok_or_else(|| {
                                    anyhow!(
                                        "Unsupported exported name in {}",
                                        module_path.display()
                                    )
                                })?;
                            let local = module_export_name(&specifier.local).ok_or_else(|| {
                                anyhow!(
                                    "Unsupported local export name in {}",
                                    module_path.display()
                                )
                            })?;
                            exports.push((exported, module_export_access_expr(module_var, &local)));
                        }
                    }
                }
            }
            Statement::ExportDeclaration(export_decl) => {
                let declaration = &export_decl.declaration;
                output.push_str(slice(source, declaration.span())?);
                collect_exports_from_declaration(declaration, &mut exports)?;
            }
            Statement::ExportNamedDeclaration(export_decl) => {
                for specifier in &export_decl.specifiers {
                    if export_decl.export_kind == ImportOrExportKind::Type
                        || specifier.export_kind == ImportOrExportKind::Type
                    {
                        continue;
                    }
                    // `export { T }` of a type-only import has no runtime value.
                    let local = module_export_name(&specifier.local);
                    if imports.iter().any(|import| {
                        import.bindings.iter().any(|binding| {
                            binding.type_only && Some(&binding.local) == local.as_ref()
                        })
                    }) {
                        continue;
                    }
                    exports.push((
                        module_export_name(&specifier.exported).ok_or_else(|| {
                            anyhow!("Unsupported exported name in {}", module_path.display())
                        })?,
                        module_export_name(&specifier.local).ok_or_else(|| {
                            anyhow!("Unsupported local export name in {}", module_path.display())
                        })?,
                    ));
                }
            }
            Statement::ExportAllDeclaration(export_all) => {
                if export_all.export_kind != ImportOrExportKind::Type {
                    let export = export_dependencies
                        .iter()
                        .find(|record| record.statement_span == export_all.span)
                        .ok_or_else(|| anyhow!("Internal export bookkeeping mismatch"))?;
                    if let Some(local_path) = &export.resolved_local {
                        let module_var = dependency_vars.get(local_path).ok_or_else(|| {
                            anyhow!("Missing dependency module for {}", local_path.display())
                        })?;
                        if export.export_all {
                            star_exports.push(module_var.clone());
                        } else if let Some(exported) = &export_all.exported {
                            exports.push((
                                module_export_name(exported).ok_or_else(|| {
                                    anyhow!(
                                        "Unsupported exported name in {}",
                                        module_path.display()
                                    )
                                })?,
                                module_var.clone(),
                            ));
                        }
                    }
                }
            }
            Statement::ExportDefaultDeclaration(export_default) => {
                if let Some(expression) = export_default.declaration.as_expression() {
                    refuse_removed_registration(expression, role)?;
                }
                if matches!(role, ModuleRole::App)
                    && !may_be_defined_app(&export_default.declaration)
                {
                    bail!(
                        "lxapp.ts must default-export defineApp({{ onLaunch }}) from '@lingxia/logic'"
                    );
                }
                let (replacement, export_name) =
                    rewrite_export_default(source, &export_default.declaration)?;
                output.push_str(&replacement);
                exports.push(("default".to_string(), export_name));
            }
            Statement::ExpressionStatement(expr_stmt) => {
                refuse_removed_registration(&expr_stmt.expression, role)?;
                output.push_str(slice(source, span)?);
            }
            _ => {
                output.push_str(slice(source, span)?);
            }
        }

        cursor = end;
    }

    output.push_str(&source[cursor..]);
    output.push_str(
        "\nconst __lx_module_exports = {};\nconst __lx_star_export_names = new Set();\nconst __lx_ambiguous_star_exports = new Set();\n",
    );
    for module_var in &star_exports {
        output.push_str(&format!(
            "for (const __lx_export_name of Object.keys({module_var})) {{\n  if (__lx_export_name === \"default\") continue;\n  if (__lx_ambiguous_star_exports.has(__lx_export_name)) continue;\n  if (__lx_star_export_names.has(__lx_export_name)) {{\n    __lx_ambiguous_star_exports.add(__lx_export_name);\n    delete __lx_module_exports[__lx_export_name];\n    continue;\n  }}\n  __lx_star_export_names.add(__lx_export_name);\n  __lx_module_exports[__lx_export_name] = {module_var}[__lx_export_name];\n}}\n"
        ));
    }
    for (exported, local) in &exports {
        output.push_str(&format!("__lx_module_exports[\"{exported}\"] = {local};\n"));
    }
    // `export default defineApp({ ... })` is the app; `export default definePage({ ... })` is a page.
    if matches!(role, ModuleRole::App) {
        if !exports.iter().any(|(exported, _)| exported == "default") {
            bail!("lxapp.ts must default-export defineApp({{ onLaunch }}) from '@lingxia/logic'");
        }
        output.push_str("globalThis.__registerDefinedApp(__lx_module_exports[\"default\"]);\n");
    }
    if let ModuleRole::Page { page_path } = role {
        if !exports.iter().any(|(exported, _)| exported == "default") {
            bail!(
                "{page_path}: a page's Logic must `export default definePage({{ data, setup }})` from '@lingxia/logic'"
            );
        }
        output.push_str(&format!(
            "globalThis.__registerDefinedPage({}, __lx_module_exports[\"default\"]);\n",
            json_string_literal(&page_registration_path(project, page_path)?)
        ));
    }
    Ok(output)
}

fn render_import_stub(
    import: &ImportRecord,
    dependency_vars: &BTreeMap<PathBuf, String>,
) -> Result<String> {
    let Some(local_path) = &import.resolved_local else {
        return Ok(String::new());
    };
    let module_var = dependency_vars
        .get(local_path)
        .ok_or_else(|| anyhow!("Missing dependency module for {}", local_path.display()))?;

    if import.bindings.is_empty() {
        return Ok(format!("void {module_var};"));
    }

    let mut lines = Vec::new();
    let mut named_parts = Vec::new();
    for binding in &import.bindings {
        if binding.type_only {
            continue;
        }
        if binding.namespace {
            lines.push(format!("const {} = {};", binding.local, module_var));
            continue;
        }
        let imported = binding
            .imported
            .as_deref()
            .ok_or_else(|| anyhow!("Missing imported binding"))?;
        if imported == binding.local {
            named_parts.push(imported.to_string());
        } else {
            named_parts.push(format!("{imported}: {}", binding.local));
        }
    }

    if !named_parts.is_empty() {
        lines.push(format!(
            "const {{ {} }} = {};",
            named_parts.join(", "),
            module_var
        ));
    }

    Ok(lines.join("\n"))
}

fn module_export_access_expr(module_var: &str, export_name: &str) -> String {
    format!("{module_var}[{}]", json_string_literal(export_name))
}

fn rewrite_export_default(
    source: &str,
    declaration: &ExportDefaultDeclarationKind<'_>,
) -> Result<(String, String)> {
    match declaration {
        ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
            let name = function
                .id
                .as_ref()
                .map(|id| id.name.as_str().to_string())
                .unwrap_or_else(|| "__lx_default__".to_string());
            let mut text = slice(source, declaration.span())?.to_string();
            if function.id.is_none() {
                text = format!("const {name} = {text};");
            }
            Ok((text, name))
        }
        ExportDefaultDeclarationKind::ClassDeclaration(class) => {
            let name = class
                .id
                .as_ref()
                .map(|id| id.name.as_str().to_string())
                .unwrap_or_else(|| "__lx_default__".to_string());
            let mut text = slice(source, declaration.span())?.to_string();
            if class.id.is_none() {
                text = format!("const {name} = {text};");
            }
            Ok((text, name))
        }
        ExportDefaultDeclarationKind::TSInterfaceDeclaration(_) => {
            Ok((String::new(), "__lx_default__".to_string()))
        }
        _ => {
            let expr = slice(source, declaration.span())?;
            Ok((
                format!("const __lx_default__ = {expr};"),
                "__lx_default__".to_string(),
            ))
        }
    }
}

fn collect_exports_from_declaration(
    declaration: &Declaration<'_>,
    exports: &mut Vec<(String, String)>,
) -> Result<()> {
    match declaration {
        Declaration::FunctionDeclaration(function) => {
            let name = function
                .id
                .as_ref()
                .ok_or_else(|| anyhow!("Anonymous exported function is unsupported"))?
                .name
                .as_str()
                .to_string();
            exports.push((name.clone(), name));
        }
        Declaration::ClassDeclaration(class) => {
            let name = class
                .id
                .as_ref()
                .ok_or_else(|| anyhow!("Anonymous exported class is unsupported"))?
                .name
                .as_str()
                .to_string();
            exports.push((name.clone(), name));
        }
        Declaration::VariableDeclaration(declaration) => {
            let mut names = Vec::new();
            for declarator in &declaration.declarations {
                collect_binding_names(&declarator.id, &mut names);
            }
            for name in names {
                exports.push((name.clone(), name));
            }
        }
        Declaration::TSEnumDeclaration(enum_decl) if !enum_decl.r#const => {
            let name = enum_decl.id.name.as_str().to_string();
            exports.push((name.clone(), name));
        }
        Declaration::TSTypeAliasDeclaration(_)
        | Declaration::TSInterfaceDeclaration(_)
        | Declaration::TSEnumDeclaration(_)
        | Declaration::TSExternalModuleDeclaration(_)
        | Declaration::TSNamespaceDeclaration(_)
        | Declaration::TSGlobalDeclaration(_)
        | Declaration::TSImportEqualsDeclaration(_) => {}
    }
    Ok(())
}

fn collect_binding_names(pattern: &oxc_ast::ast::BindingPattern<'_>, output: &mut Vec<String>) {
    match pattern {
        oxc_ast::ast::BindingPattern::BindingIdentifier(identifier) => {
            output.push(identifier.name.as_str().to_string());
        }
        oxc_ast::ast::BindingPattern::ObjectPattern(pattern) => {
            for property in &pattern.properties {
                collect_binding_names(&property.value, output);
            }
            if let Some(rest) = &pattern.rest {
                collect_binding_names(&rest.argument, output);
            }
        }
        oxc_ast::ast::BindingPattern::ArrayPattern(pattern) => {
            for element in pattern.elements.iter().flatten() {
                collect_binding_names(element, output);
            }
            if let Some(rest) = &pattern.rest {
                collect_binding_names(&rest.argument, output);
            }
        }
        oxc_ast::ast::BindingPattern::AssignmentPattern(pattern) => {
            collect_binding_names(&pattern.left, output);
        }
    }
}

/// `Page(`, `PageInstance(` and `App(` name what to write instead.
fn refuse_removed_registration(expression: &Expression<'_>, role: &ModuleRole) -> Result<()> {
    let Expression::CallExpression(call_expr) = unwrap_expression(expression) else {
        return Ok(());
    };
    let Expression::Identifier(identifier) = unwrap_expression(&call_expr.callee) else {
        return Ok(());
    };
    match identifier.name.as_str() {
        "Page" | "PageInstance" if matches!(role, ModuleRole::Page { .. }) => bail!(
            "{}() was removed: a page's Logic default-exports definePage({{ data, setup }}) from '@lingxia/logic'",
            identifier.name.as_str()
        ),
        "App" if matches!(role, ModuleRole::App) => bail!(
            "App() was removed: the app entry default-exports defineApp({{ onLaunch }}) from '@lingxia/logic'"
        ),
        _ => Ok(()),
    }
}

/// The path a page registers under: plugin pages are namespaced by plugin.
fn page_registration_path(project: &Project, page_path: &str) -> Result<String> {
    Ok(match project.kind {
        ProjectKind::LxApp => page_path.to_string(),
        ProjectKind::LxPlugin => format!(
            "plugin/{}/{}",
            project
                .plugin_id
                .as_deref()
                .ok_or_else(|| anyhow!("Missing plugin id"))?,
            page_path
        ),
    })
}

fn module_export_name(name: &ModuleExportName<'_>) -> Option<String> {
    match name {
        ModuleExportName::IdentifierName(identifier) => Some(identifier.name.as_str().to_string()),
        ModuleExportName::IdentifierReference(identifier) => {
            Some(identifier.name.as_str().to_string())
        }
        ModuleExportName::StringLiteral(literal) => Some(literal.value.as_str().to_string()),
    }
}

/// Whether an app's default export can be `defineApp(...)`. Only a value that is plainly
/// something else fails here; a call or a binding (possibly aliased) is left to the
/// runtime's brand check.
fn may_be_defined_app(declaration: &ExportDefaultDeclarationKind<'_>) -> bool {
    let Some(expression) = declaration.as_expression() else {
        return false;
    };
    !matches!(
        unwrap_expression(expression),
        Expression::ObjectExpression(_)
            | Expression::ArrayExpression(_)
            | Expression::ArrowFunctionExpression(_)
            | Expression::FunctionExpression(_)
            | Expression::ClassExpression(_)
            | Expression::StringLiteral(_)
            | Expression::NumericLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_)
            | Expression::TemplateLiteral(_)
    )
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

/// The line of the first `lx` that no import or local binding provides: the runtime has a
/// global `lx`, but Logic imports it so a View cannot reach it by mistake.
fn unimported_lx_line(program: &oxc_ast::ast::Program<'_>, source: &str) -> Option<usize> {
    use oxc_ast_visit::Visit;
    struct Uses<'s> {
        scoping: &'s oxc_semantic::Scoping,
        first: Option<u32>,
    }
    impl<'a> Visit<'a> for Uses<'_> {
        fn visit_identifier_reference(&mut self, ident: &oxc_ast::ast::IdentifierReference<'a>) {
            let unresolved = ident
                .reference_id
                .get()
                .is_some_and(|id| self.scoping.get_reference(id).symbol_id().is_none());
            if ident.name == "lx" && unresolved {
                self.first = Some(
                    self.first
                        .map_or(ident.span.start, |f| f.min(ident.span.start)),
                );
            }
        }
    }
    let semantic = SemanticBuilder::new().build(program).semantic;
    let mut uses = Uses {
        scoping: semantic.scoping(),
        first: None,
    };
    uses.visit_program(program);
    let start = uses.first?;
    Some(source[..start as usize].matches('\n').count() + 1)
}

/// The Logic global the generated native client dispatches through; only that client may name it.
const NATIVE_BRIDGE_GLOBAL: &str = "__lingxiaNative";

/// The line of the first reach for the native bridge: the global itself (also when declared
/// ambient), the name as a property or key, or the name as a string
/// (`globalThis["__lingxiaNative"]`). Local bindings and comments are not reaches.
fn native_bridge_line(program: &oxc_ast::ast::Program<'_>, source: &str) -> Option<usize> {
    use oxc_ast_visit::{Visit, walk};
    struct Reaches<'s> {
        scoping: &'s oxc_semantic::Scoping,
        in_global: bool,
        first: Option<u32>,
    }
    impl Reaches<'_> {
        fn hit(&mut self, start: u32) {
            self.first = Some(self.first.map_or(start, |f| f.min(start)));
        }
    }
    impl<'a> Visit<'a> for Reaches<'_> {
        fn visit_identifier_reference(&mut self, ident: &oxc_ast::ast::IdentifierReference<'a>) {
            let global = ident.reference_id.get().is_some_and(|id| {
                self.scoping
                    .get_reference(id)
                    .symbol_id()
                    .is_none_or(|symbol| {
                        self.scoping
                            .symbol_flags(symbol)
                            .contains(oxc_semantic::SymbolFlags::Ambient)
                    })
            });
            if ident.name == NATIVE_BRIDGE_GLOBAL && global {
                self.hit(ident.span.start);
            }
        }
        fn visit_binding_identifier(&mut self, ident: &oxc_ast::ast::BindingIdentifier<'a>) {
            if self.in_global && ident.name == NATIVE_BRIDGE_GLOBAL {
                self.hit(ident.span.start);
            }
        }
        fn visit_ts_global_declaration(&mut self, decl: &oxc_ast::ast::TSGlobalDeclaration<'a>) {
            let outer = std::mem::replace(&mut self.in_global, true);
            walk::walk_ts_global_declaration(self, decl);
            self.in_global = outer;
        }
        fn visit_identifier_name(&mut self, ident: &oxc_ast::ast::IdentifierName<'a>) {
            if ident.name == NATIVE_BRIDGE_GLOBAL {
                self.hit(ident.span.start);
            }
        }
        fn visit_string_literal(&mut self, literal: &oxc_ast::ast::StringLiteral<'a>) {
            if literal.value.as_str() == NATIVE_BRIDGE_GLOBAL {
                self.hit(literal.span.start);
            }
        }
        fn visit_template_literal(&mut self, literal: &oxc_ast::ast::TemplateLiteral<'a>) {
            let plain = literal.expressions.is_empty()
                && literal
                    .quasis
                    .first()
                    .and_then(|q| q.value.cooked.as_ref())
                    .is_some_and(|cooked| cooked.as_str() == NATIVE_BRIDGE_GLOBAL);
            if plain {
                self.hit(literal.span.start);
            }
            walk::walk_template_literal(self, literal);
        }
    }
    let semantic = SemanticBuilder::new().build(program).semantic;
    let mut reaches = Reaches {
        scoping: semantic.scoping(),
        in_global: false,
        first: None,
    };
    reaches.visit_program(program);
    let start = reaches.first?;
    Some(source[..start as usize].matches('\n').count() + 1)
}

pub(crate) fn transpile_module(path: &Path, source: &str) -> Result<String> {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(path)
        .map_err(|_| anyhow!("Unsupported logic file {}", path.display()))?;
    let parse_result = Parser::new(&allocator, source, source_type).parse();
    if !parse_result.diagnostics.is_empty() {
        bail!(
            "Failed to parse rewritten logic module {}: {}",
            path.display(),
            format_diagnostics(&parse_result.diagnostics)
        );
    }

    let mut program = parse_result.program;
    let semantic = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .with_enum_eval(true)
        .build(&program);
    if !semantic.diagnostics.is_empty() {
        bail!(
            "Semantic analysis failed for {}: {}",
            path.display(),
            format_diagnostics(&semantic.diagnostics)
        );
    }

    let transformer_return = Transformer::new(&allocator, path, &TransformOptions::default())
        .build_with_scoping(semantic.semantic.into_scoping(), &mut program);
    if !transformer_return.diagnostics.is_empty() {
        bail!(
            "Failed to transform {}: {}",
            path.display(),
            format_diagnostics(&transformer_return.diagnostics)
        );
    }

    let mut codegen = Codegen::new();
    codegen = codegen.with_options(CodegenOptions::default());
    Ok(codegen.build(&program).code)
}

fn resolve_local_import(
    from_module: &Path,
    specifier: &str,
    project_root: &Path,
) -> Result<PathBuf> {
    let base_dir = from_module
        .parent()
        .ok_or_else(|| anyhow!("Missing parent directory for {}", from_module.display()))?;
    let candidate_base = if specifier.starts_with('/') {
        project_root.join(specifier.trim_start_matches('/'))
    } else {
        base_dir.join(specifier)
    };

    for candidate in candidate_candidates(&candidate_base) {
        if candidate.exists() {
            return normalize_path(&candidate);
        }
    }

    Err(anyhow!(
        "Failed to resolve local logic import {:?} from {}",
        specifier,
        from_module.display()
    ))
}

/// The module name the generated Logic native client is imported by.
const NATIVE_CLIENT_SPECIFIER: &str = "@lingxia/native";

fn native_client_path(project_root: &Path) -> PathBuf {
    project_root
        .join(".lingxia")
        .join(lingxia_native_codegen::LOGIC_CLIENT)
}

fn resolve_bare_import(
    from_module: &Path,
    specifier: &str,
    project_root: &Path,
) -> Result<PathBuf> {
    if specifier == NATIVE_CLIENT_SPECIFIER {
        let client = native_client_path(project_root);
        if !client.is_file() {
            bail!(
                "{} imports {specifier}, but this lxapp has no native client: no Rust crate serves it",
                relative_to(from_module, project_root)
            );
        }
        return normalize_path(&client);
    }
    let mut options = ResolveOptions::default()
        .with_condition_names(&["import", "module", "default"])
        .with_builtin_modules(true);
    options.module_type = true;
    options.main_fields = vec!["module".into(), "main".into()];
    options.extensions = vec![
        ".ts".into(),
        ".tsx".into(),
        ".mts".into(),
        ".js".into(),
        ".jsx".into(),
        ".mjs".into(),
        ".json".into(),
    ];
    options.extension_alias = vec![
        (
            ".js".into(),
            vec![
                ".ts".into(),
                ".tsx".into(),
                ".js".into(),
                ".jsx".into(),
                ".mjs".into(),
            ],
        ),
        (
            ".mjs".into(),
            vec![".mts".into(), ".mjs".into(), ".js".into()],
        ),
    ];

    // A runtime import through a tsconfig `paths` alias resolves as the type check resolved it.
    let config_file = project_root.join("tsconfig.json");
    if config_file.is_file() {
        options.tsconfig = Some(TsconfigDiscovery::Manual(TsconfigOptions {
            config_file,
            references: TsconfigReferences::Auto,
        }));
    }

    let resolver = Resolver::new(options);
    let resolution = resolver
        .resolve_file(from_module, specifier)
        .with_context(|| {
            format!(
                "Failed to resolve logic import {specifier:?} from {}. \
Run npm install in the lxapp root if this package is missing.",
                relative_to(from_module, project_root)
            )
        })?;

    if is_commonjs_resolution(&resolution) {
        bail!(
            "Unsupported CommonJS logic import {specifier:?} from {} -> {}. \
The logic bundler inlines modules as ESM, so a CommonJS file would throw on \
`require` at evaluation time and take the whole logic layer down. \
Use an ESM package or ESM entrypoint for logic-layer imports.",
            relative_to(from_module, project_root),
            relative_to(resolution.path(), project_root)
        );
    }

    normalize_path(resolution.path())
}

/// `oxc_resolver` reports a module type only when the extension or the nearest
/// `"type"` field settles it. A `.js` file in a package that declares no
/// `"type"` comes back as `None`, even though Node treats it as CommonJS — that
/// gap is how a CommonJS dependency used to be inlined verbatim, leaving the
/// bundle to throw `require is not defined` at eval time with no build error.
/// Fall back to the source itself there, so packages that ship extensionless
/// ESM keep resolving while real CommonJS is rejected at build time.
fn is_commonjs_resolution(resolution: &oxc_resolver::Resolution) -> bool {
    match resolution.module_type() {
        Some(ModuleType::CommonJs) => true,
        Some(_) => false,
        None => source_looks_commonjs(resolution.path()),
    }
}

fn source_looks_commonjs(path: &Path) -> bool {
    if !matches!(path.extension().and_then(|ext| ext.to_str()), Some("js")) {
        return false;
    }
    let Ok(source) = fs::read_to_string(path) else {
        return false;
    };
    if !source.contains("require(")
        && !source.contains("module.exports")
        && !source.contains("exports.")
    {
        return false;
    }
    let Ok(source_type) = SourceType::from_path(path) else {
        return false;
    };
    let allocator = Allocator::default();
    let parse_result = Parser::new(&allocator, &source, source_type).parse();
    if !parse_result.diagnostics.is_empty() {
        return false;
    }
    !parse_result.program.body.iter().any(|statement| {
        matches!(
            statement,
            Statement::ImportDeclaration(_)
                | Statement::ExportAllDeclaration(_)
                | Statement::ExportDeclaration(_)
                | Statement::ExportDefaultDeclaration(_)
                | Statement::ExportNamedDeclaration(_)
                | Statement::ExportFromDeclaration(_)
        )
    })
}

fn resolve_import_specifier(
    from_module: &Path,
    specifier: &str,
    project_root: &Path,
) -> Result<PathBuf> {
    if is_local_specifier(specifier) {
        resolve_local_import(from_module, specifier, project_root)
    } else {
        resolve_bare_import(from_module, specifier, project_root)
    }
}

fn candidate_candidates(base: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if base.extension().is_some() {
        candidates.push(base.to_path_buf());
    } else {
        candidates.push(base.with_extension("ts"));
        candidates.push(base.with_extension("js"));
        candidates.push(base.with_extension("mts"));
        candidates.push(base.with_extension("mjs"));
        candidates.push(base.join("index.ts"));
        candidates.push(base.join("index.js"));
        candidates.push(base.join("index.mts"));
        candidates.push(base.join("index.mjs"));
    }
    candidates
}

fn is_local_specifier(specifier: &str) -> bool {
    specifier.starts_with("./") || specifier.starts_with("../") || specifier.starts_with('/')
}

fn slice(source: &str, span: oxc_span::Span) -> Result<&str> {
    source
        .get(span.start as usize..span.end as usize)
        .ok_or_else(|| anyhow!("Invalid source span"))
}

fn json_string_literal(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

fn relative_to(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn normalize_path(path: &Path) -> Result<PathBuf> {
    let canonical = path
        .canonicalize()
        .with_context(|| format!("Failed to canonicalize {}", path.display()))?;
    Ok(strip_verbatim_prefix(canonical))
}

/// Windows `canonicalize` yields `\\?\D:\…`. That prefix never matches the
/// project root, so diagnostics printed absolute verbatim paths and every
/// module id carried it; keep plain drive paths.
#[cfg(windows)]
fn strip_verbatim_prefix(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        path
    }
}

#[cfg(not(windows))]
fn strip_verbatim_prefix(path: PathBuf) -> PathBuf {
    path
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
    use crate::lxapp::project::Project;
    use std::fs;
    use tempfile::TempDir;

    fn parse_program<'a>(
        allocator: &'a Allocator,
        path: &Path,
        source: &'a str,
    ) -> oxc_ast::ast::Program<'a> {
        let source_type = SourceType::from_path(path).unwrap();
        let parse_result = Parser::new(allocator, source, source_type).parse();
        assert!(
            parse_result.diagnostics.is_empty(),
            "parse errors: {}",
            format_diagnostics(&parse_result.diagnostics)
        );
        parse_result.program
    }

    fn build_test_bundle(root: &Path, entry: &str) -> String {
        let project = Project {
            root: root.to_path_buf(),
            kind: ProjectKind::LxApp,
            framework: ProjectFramework::Html,
            output_dir: root.join("dist"),
            pages: Vec::new(),
            page_names: Vec::new(),
            logic_entry: Some("logic.js".to_string()),
            plugin_id: None,
            package_name: Some("@test/app".to_string()),
            version: "1.0.0".to_string(),
        };
        let mut bundler = LogicBundler::new(&project);
        bundler
            .add_entry(root.join(entry), ModuleRole::Plain)
            .unwrap();
        bundler.render_bundle(false).unwrap()
    }

    fn test_project(root: &Path) -> Project {
        Project {
            root: root.to_path_buf(),
            kind: ProjectKind::LxApp,
            framework: ProjectFramework::Html,
            output_dir: root.join("dist"),
            pages: Vec::new(),
            page_names: Vec::new(),
            logic_entry: Some("logic.js".to_string()),
            plugin_id: None,
            package_name: Some("@test/app".to_string()),
            version: "1.0.0".to_string(),
        }
    }

    fn write(root: &Path, path: &str, text: &str) {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    #[test]
    fn logic_using_lx_without_the_import_names_the_import() {
        let temp = TempDir::new().unwrap();
        let error = page_build_error(temp.path(), "export default {};\n\nlx.nav.back();\n");
        assert!(
            error.contains("pages/home/index.ts:3: `lx` is not imported; add `import { lx } from '@lingxia/logic'`"),
            "{error}"
        );
    }

    #[test]
    fn logic_reaching_for_the_native_bridge_names_the_generated_client() {
        for reach in [
            "__lingxiaNative.invoke('a.b');",
            "globalThis.__lingxiaNative.invoke('a.b');",
            "globalThis['__lingxiaNative'].invoke('a.b');",
            "globalThis[`__lingxiaNative`].invoke('a.b');",
            "const { __lingxiaNative: bridge } = globalThis;",
            "const present = '__lingxiaNative' in globalThis;",
            "declare const __lingxiaNative: any; __lingxiaNative.invoke('a.b');",
            "declare global { var __lingxiaNative: any }",
        ] {
            let temp = TempDir::new().unwrap();
            let error =
                page_build_error(temp.path(), &format!("export default {{}};\n\n{reach}\n"));
            assert!(
                error.contains("pages/home/index.ts:3: `__lingxiaNative` is the runtime's private native bridge; call routes through `page.native` or `import { native } from '@lingxia/native'`"),
                "{reach}: {error}"
            );
        }
    }

    #[test]
    fn a_local_binding_or_comment_named_like_the_native_bridge_builds() {
        let temp = TempDir::new().unwrap();
        write(
            temp.path(),
            "lxapp.ts",
            "// __lingxiaNative stays private\nconst __lingxiaNative = 1;\nglobalThis.n = __lingxiaNative + 1;\nexport default {};\n",
        );
        build_test_bundle(temp.path(), "lxapp.ts");
    }

    /// The exemption is the generated client's own path, not its contents.
    #[test]
    fn a_copy_of_the_generated_client_outside_its_path_is_refused() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        write(
            root,
            "rust/lib.rs",
            "#[lingxia::native(\"greet.hello\")]\npub async fn hello() -> lingxia::Result<()> { todo!() }\n",
        );
        lingxia_native_codegen::generate_clients(
            &[root.join("rust")],
            &root.join(".lingxia"),
            None,
            lingxia_native_codegen::CallerClass::StandardApp,
        )
        .unwrap();
        let client = fs::read_to_string(
            root.join(".lingxia")
                .join(lingxia_native_codegen::LOGIC_CLIENT),
        )
        .unwrap();
        let error = page_build_error(root, &client);
        assert!(
            error.contains("pages/home/index.ts:") && error.contains("private native bridge"),
            "{error}"
        );
    }

    /// The real `@lingxia/logic` runtime entry, bundled as a project would, run on this host's
    /// engine: the app entry and every page see the one injected `lx`, the app entry first.
    #[test]
    fn imported_lx_is_the_injected_object_in_the_app_entry_and_pages() {
        use rong::{JSEngine, RongJS, Source};
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        write(
            root,
            "node_modules/@lingxia/logic/package.json",
            include_str!("../../../../packages/lingxia-logic/package.json"),
        );
        write(
            root,
            "node_modules/@lingxia/logic/index.js",
            include_str!("../../../../packages/lingxia-logic/index.js"),
        );
        write(
            root,
            "node_modules/@lingxia/logic/package-text.js",
            include_str!("../../../../packages/lingxia-logic/package-text.js"),
        );
        write(
            root,
            "node_modules/@lingxia/types/package.json",
            r#"{"name":"@lingxia/types","type":"module","exports":{"./page":"./page.js","./error":"./error.js"}}"#,
        );
        write(
            root,
            "node_modules/@lingxia/types/page.js",
            "export function definePage(d) { return d; }\nexport function defineApp(d) { return d; }\n",
        );
        write(
            root,
            "node_modules/@lingxia/types/error.js",
            "export function isLxError() { return false; }\n",
        );
        write(
            root,
            "lxapp.ts",
            "import { defineApp, lx } from '@lingxia/logic';\nglobalThis.seen.push(['app', lx]);\nexport default defineApp({});\n",
        );
        write(
            root,
            "pages/home/index.ts",
            "import { definePage, lx } from '@lingxia/logic';\nglobalThis.seen.push(['page', lx]);\nexport default definePage({});\n",
        );
        let project = test_project(root);
        let mut bundler = LogicBundler::new(&project);
        bundler
            .add_entry(root.join("lxapp.ts"), ModuleRole::App)
            .unwrap();
        bundler
            .add_entry(
                root.join("pages/home/index.ts"),
                ModuleRole::Page {
                    page_path: "pages/home/index".to_string(),
                },
            )
            .unwrap();
        let bundle = bundler.render_bundle(false).unwrap();

        let runtime = RongJS::runtime();
        let ctx = runtime.context();
        let prelude = "globalThis.lx = { nav: {} }; globalThis.seen = []; \
                       globalThis.__registerDefinedApp = () => seen.push(['App', null]); \
                       globalThis.__registerDefinedPage = () => seen.push(['register', null]);";
        ctx.eval::<()>(Source::from_bytes(prelude)).unwrap();
        ctx.eval::<()>(Source::from_bytes(bundle)).unwrap();
        let order: String = ctx
            .eval(Source::from_bytes(
                "JSON.stringify(seen.map(([who, value]) => [who, value === null || value === globalThis.lx]))",
            ))
            .unwrap();
        assert_eq!(
            order,
            r#"[["app",true],["App",true],["page",true],["register",true]]"#
        );
    }

    /// A View-only page in an lxapp with Logic registers an empty, branded definition.
    #[test]
    fn a_page_without_logic_registers_an_empty_definition() {
        use rong::{JSEngine, RongJS, Source};
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        write(
            root,
            "pages/home/index.tsx",
            "export default function Home() {}\n",
        );
        write(
            root,
            "pages/about/index.ts",
            "export default { data: {} };\n",
        );
        let mut project = test_project(root);
        project.pages = vec!["pages/home/index".into(), "pages/about/index".into()];
        let (entries, view_only) = discover_page_logic_entries(&project).unwrap();
        assert_eq!(entries.len(), 1);
        let home = crate::lxapp::framework::compiled_page_path(root, "pages/home/index");
        assert_eq!(view_only, std::slice::from_ref(&home));

        let mut bundler = LogicBundler::new(&project);
        for page_path in &view_only {
            bundler.add_view_only_page(page_path).unwrap();
        }
        let bundle = bundler.render_bundle(false).unwrap();
        let runtime = RongJS::runtime();
        let ctx = runtime.context();
        ctx.eval::<()>(Source::from_bytes(
            "globalThis.seen = []; globalThis.__registerDefinedPage = (path, d) => \
             seen.push([path, d[Symbol.for('lingxia.page')] === true, Object.keys(d).length]);",
        ))
        .unwrap();
        ctx.eval::<()>(Source::from_bytes(bundle)).unwrap();
        let seen: String = ctx
            .eval(Source::from_bytes("JSON.stringify(seen)"))
            .unwrap();
        assert_eq!(seen, format!(r#"[["{home}",true,0]]"#));
    }

    #[test]
    fn a_page_module_default_export_is_its_page() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        write(
            root,
            "define.ts",
            "export const definePage = (d: object) => d;
",
        );
        write(
            root,
            "pages/home/index.ts",
            "import { definePage } from '../../define';
             export default definePage({ data: { n: 0 } });
",
        );
        let project = test_project(root);
        let mut bundler = LogicBundler::new(&project);
        bundler
            .add_entry(
                root.join("pages/home/index.ts"),
                ModuleRole::Page {
                    page_path: "pages/home/index".to_string(),
                },
            )
            .unwrap();
        let bundle = bundler.render_bundle(false).unwrap();
        assert!(
            bundle.contains(
                "globalThis.__registerDefinedPage(\"pages/home/index\", __lx_module_exports[\"default\"]);"
            ),
            "{bundle}"
        );
    }

    #[test]
    fn a_page_calling_page_fails_with_the_replacement() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        write(
            root,
            "pages/home/index.ts",
            "Page({ data: {} });
",
        );
        let project = test_project(root);
        let mut bundler = LogicBundler::new(&project);
        let error = bundler
            .add_entry(
                root.join("pages/home/index.ts"),
                ModuleRole::Page {
                    page_path: "pages/home/index".to_string(),
                },
            )
            .and_then(|_| bundler.render_bundle(false))
            .unwrap_err();
        assert!(format!("{error:#}").contains("definePage"), "{error:#}");
    }

    fn page_build_error(root: &Path, source: &str) -> String {
        write(root, "pages/home/index.ts", source);
        let project = test_project(root);
        let mut bundler = LogicBundler::new(&project);
        let error = bundler
            .add_entry(
                root.join("pages/home/index.ts"),
                ModuleRole::Page {
                    page_path: "pages/home/index".to_string(),
                },
            )
            .and_then(|_| bundler.render_bundle(false))
            .unwrap_err();
        format!("{error:#}")
    }

    #[test]
    fn a_page_without_a_default_export_fails_with_the_fix() {
        let temp = TempDir::new().unwrap();
        let error = page_build_error(temp.path(), "export const data = { n: 0 };\n");
        assert!(
            error.contains("pages/home/index") && error.contains("export default definePage"),
            "{error}"
        );
        let temp = TempDir::new().unwrap();
        let error = page_build_error(temp.path(), "export default Page({ data: {} });\n");
        assert!(error.contains("Page() was removed"), "{error}");
    }

    #[test]
    fn an_app_calling_app_or_missing_default_export_fails_with_the_replacement() {
        for source in [
            "App({ onLaunch() {} });\n",
            "export default App({});\n",
            "export const x = 1;\n",
            "export default { onLaunch() {} };\n",
            "export default ({ onLaunch() {} }) as const;\n",
            "export default function onLaunch() {}\n",
        ] {
            let temp = TempDir::new().unwrap();
            let root = temp.path();
            write(root, "lxapp.ts", source);
            let project = test_project(root);
            let mut bundler = LogicBundler::new(&project);
            let error = bundler
                .add_entry(root.join("lxapp.ts"), ModuleRole::App)
                .and_then(|_| bundler.render_bundle(false))
                .unwrap_err();
            assert!(
                format!("{error:#}").contains("defineApp({ onLaunch })"),
                "{error:#}"
            );
        }
    }

    /// A binding or a call may still be `defineApp(...)` (aliased or re-exported), so only the
    /// runtime's brand check can refuse it.
    #[test]
    fn an_app_exporting_a_binding_or_call_builds() {
        for source in [
            "const app = globalThis.made;\nexport default app;\n",
            "export default globalThis.make({});\n",
        ] {
            let temp = TempDir::new().unwrap();
            let root = temp.path();
            write(root, "lxapp.ts", source);
            let project = test_project(root);
            let mut bundler = LogicBundler::new(&project);
            bundler
                .add_entry(root.join("lxapp.ts"), ModuleRole::App)
                .unwrap();
            let bundle = bundler.render_bundle(false).unwrap();
            assert!(bundle.contains("__registerDefinedApp"), "{bundle}");
        }
    }

    #[test]
    fn mocks_bundle_to_a_script_whose_value_is_the_default_export() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        assert_eq!(build_mocks(root).unwrap(), None);
        write(
            root,
            "shared/format.ts",
            "export const label = (n: number) => `#${n}`;\n",
        );
        write(
            root,
            "mocks/fixtures.ts",
            "export const DEVICES = [{ id: 'd1' }];\n",
        );
        write(
            root,
            "mocks/index.ts",
            "import type { Mocks } from '@lingxia/types/mocks';\n\
             import { DEVICES } from './fixtures';\n\
             import { label } from '../shared/format';\n\
             let signedIn = true;\n\
             export default {\n\
               'GET **/devices': () => (signedIn ? { json: DEVICES } : { status: 401 }),\n\
               \"DELETE **/sessions/current\": () => { signedIn = false; return { status: 204 }; },\n\
               'GET **/label': { json: label(1) },\n\
             } satisfies Mocks;\n",
        );
        let bundle = build_mocks(root).unwrap().unwrap();
        assert_eq!(bundle.entry, "mocks/index.ts");
        assert_eq!(
            bundle.keys,
            [
                "GET **/devices",
                "DELETE **/sessions/current",
                "GET **/label"
            ]
        );
        assert!(
            bundle.source.starts_with("(function() {"),
            "{}",
            bundle.source
        );
        assert!(
            bundle.source.trim_end().ends_with("[\"default\"];\n})();")
                || bundle.source.contains("[\"default\"];\n})();"),
            "{}",
            bundle.source
        );
        assert!(!bundle.source.contains("satisfies"), "{}", bundle.source);
        assert!(bundle.source.contains("DEVICES"), "{}", bundle.source);
    }

    #[test]
    fn mock_keys_must_be_visible_http_targets() {
        let cases = [
            (
                "export default { ...other } ;",
                "a spread (...) hides which calls it answers",
            ),
            (
                "const k = 'GET **/x'; export default { [k]: {} };",
                "a computed key hides which calls it answers",
            ),
            (
                "export default { 'GET/x': {} };",
                "'GET/x' is not a target: a handler key is 'METHOD url-glob'",
            ),
            (
                "export default { 'GET **/x': {}, 'GET **/x': {} };",
                "'GET **/x' is listed twice",
            ),
            (
                "export default { 'GET **/x': { sequence: [] } };",
                "'GET **/x': 'sequence' is a scenario field; a handler returns one answer per call",
            ),
            (
                "export default [];",
                "the default export must be an object of handlers",
            ),
            (
                "export const x = 1;",
                "the default export must be an object of handlers",
            ),
        ];
        for (source, message) in cases {
            let err = mock_keys(Path::new("index.ts"), source)
                .unwrap_err()
                .to_string();
            assert!(err.contains(message), "{source}: {err}");
        }
        assert_eq!(
            mock_keys(
                Path::new("index.ts"),
                "const mocks = { '* **/y': { continue: true } } as const;\nexport default mocks;"
            )
            .unwrap(),
            ["* **/y"]
        );
        assert_eq!(
            mock_keys(
                Path::new("index.ts"),
                "export default { 'native notes.list': [{ match: 1 }], 'native notes.*': () => null };"
            )
            .unwrap(),
            ["native notes.list", "native notes.*"]
        );
    }

    #[test]
    fn product_code_that_imports_mocks_fails_every_build() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        write(
            root,
            "mocks/fixtures.ts",
            "export const ME = { id: 'me' };\n",
        );
        write(
            root,
            "pages/home/index.ts",
            "import { ME } from '../../mocks/fixtures';\nexport const me = ME;\n",
        );
        let project = test_project(root);
        let mut bundler = LogicBundler::new(&project);
        let err = bundler
            .add_entry(root.join("pages/home/index.ts"), ModuleRole::App)
            .unwrap_err()
            .to_string();
        assert_eq!(
            err,
            "Logic build failed: pages/home/index.ts imports from mocks/ (mocks/fixtures.ts). \
             Product code never imports mocks; the app calls fetch and mocks/index.ts answers it \
             in dev."
        );
        // Through a shared module too.
        write(
            root,
            "shared/me.ts",
            "export { ME } from '../mocks/fixtures';\n",
        );
        write(
            root,
            "lxapp.ts",
            "import { ME } from './shared/me';\nvoid ME;\n",
        );
        let mut bundler = LogicBundler::new(&project);
        let err = bundler
            .add_entry(root.join("lxapp.ts"), ModuleRole::App)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("shared/me.ts imports from mocks/ (mocks/fixtures.ts)"),
            "{err}"
        );
    }

    #[test]
    fn allows_type_only_bare_imports_without_resolution() {
        let temp = TempDir::new().unwrap();
        let module_path = temp.path().join("index.ts");
        fs::write(
            &module_path,
            "import type { PreviewMediaOptions } from '@lingxia/types';\nconst ok = true;\n",
        )
        .unwrap();
        let source = fs::read_to_string(&module_path).unwrap();
        let allocator = Allocator::default();
        let program = parse_program(&allocator, &module_path, &source);

        let imports = collect_imports(&program, &source, &module_path, temp.path()).unwrap();
        assert_eq!(imports.len(), 1);
        assert!(imports[0].resolved_local.is_none());
        assert!(imports[0].bindings.iter().all(|binding| binding.type_only));
    }

    #[test]
    fn a_paths_alias_resolves_through_the_project_reference_that_includes_the_file() {
        let temp = TempDir::new().unwrap();
        let module_path = temp.path().join("pages/home/index.ts");
        fs::create_dir_all(module_path.parent().unwrap()).unwrap();
        fs::create_dir_all(temp.path().join("shared")).unwrap();
        fs::write(&module_path, "import { label } from '@shared/format';\n").unwrap();
        fs::write(
            temp.path().join("shared/format.ts"),
            "export const label = 1;\n",
        )
        .unwrap();
        fs::write(
            temp.path().join("tsconfig.json"),
            r#"{ "files": [], "references": [{ "path": "./tsconfig.logic.json" }] }"#,
        )
        .unwrap();
        fs::write(
            temp.path().join("tsconfig.logic.json"),
            r#"{ "compilerOptions": { "composite": true, "paths": { "@shared/*": ["./shared/*"] } }, "include": ["pages/**/*.ts", "shared/**/*.ts"] }"#,
        )
        .unwrap();

        let resolved = resolve_bare_import(&module_path, "@shared/format", temp.path()).unwrap();
        assert_eq!(
            resolved,
            normalize_path(&temp.path().join("shared/format.ts")).unwrap()
        );
    }

    #[test]
    fn resolves_bare_esm_package_from_node_modules() {
        let temp = TempDir::new().unwrap();
        let module_path = temp.path().join("pages").join("home.ts");
        fs::create_dir_all(module_path.parent().unwrap()).unwrap();
        fs::create_dir_all(temp.path().join("node_modules/demo-pkg/dist")).unwrap();
        fs::write(
            temp.path().join("node_modules/demo-pkg/package.json"),
            r#"{
  "name": "demo-pkg",
  "exports": {
    "import": "./dist/index.mjs"
  }
}"#,
        )
        .unwrap();
        fs::write(
            temp.path().join("node_modules/demo-pkg/dist/index.mjs"),
            "export const demo = 1;\n",
        )
        .unwrap();

        let resolved = resolve_bare_import(&module_path, "demo-pkg", temp.path()).unwrap();
        assert_eq!(
            resolved,
            normalize_path(&temp.path().join("node_modules/demo-pkg/dist/index.mjs")).unwrap()
        );
    }

    #[test]
    fn rejects_commonjs_package_without_a_type_field() {
        // Regression: `@lingxia/types` shipped a CommonJS `dist/index.js` behind
        // the `import` condition and declared no `"type"`, so the resolver
        // reported no module type and the file was inlined verbatim. The bundle
        // then threw `require is not defined` while evaluating, registering no
        // page at all.
        let temp = TempDir::new().unwrap();
        let module_path = temp.path().join("pages").join("home.ts");
        fs::create_dir_all(module_path.parent().unwrap()).unwrap();
        fs::create_dir_all(temp.path().join("node_modules/cjs-pkg/dist")).unwrap();
        fs::write(
            temp.path().join("node_modules/cjs-pkg/package.json"),
            r#"{
  "name": "cjs-pkg",
  "exports": {
    "import": "./dist/index.js",
    "require": "./dist/index.js"
  }
}"#,
        )
        .unwrap();
        fs::write(
            temp.path().join("node_modules/cjs-pkg/dist/index.js"),
            "\"use strict\";\nObject.defineProperty(exports, \"__esModule\", { value: true });\nexports.demo = require(\"./demo\");\n",
        )
        .unwrap();

        let error = resolve_bare_import(&module_path, "cjs-pkg", temp.path()).unwrap_err();
        let message = format!("{error:#}");
        assert!(
            message.contains("Unsupported CommonJS logic import"),
            "{message}"
        );
    }

    #[test]
    fn allows_extensionless_esm_package_without_a_type_field() {
        let temp = TempDir::new().unwrap();
        let module_path = temp.path().join("pages").join("home.ts");
        fs::create_dir_all(module_path.parent().unwrap()).unwrap();
        fs::create_dir_all(temp.path().join("node_modules/esm-pkg/dist")).unwrap();
        fs::write(
            temp.path().join("node_modules/esm-pkg/package.json"),
            r#"{
  "name": "esm-pkg",
  "exports": {
    "import": "./dist/index.js"
  }
}"#,
        )
        .unwrap();
        fs::write(
            temp.path().join("node_modules/esm-pkg/dist/index.js"),
            "export const demo = 1;\n",
        )
        .unwrap();

        let resolved = resolve_bare_import(&module_path, "esm-pkg", temp.path()).unwrap();
        assert_eq!(
            resolved,
            normalize_path(&temp.path().join("node_modules/esm-pkg/dist/index.js")).unwrap()
        );
    }

    #[test]
    fn supports_named_reexports_from_dependencies() {
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("dep.ts"),
            "export const alpha = 1;\nexport default 2;\n",
        )
        .unwrap();
        fs::write(
            temp.path().join("entry.ts"),
            "export { alpha as beta, default as gamma } from './dep';\n",
        )
        .unwrap();

        let bundle = build_test_bundle(temp.path(), "entry.ts");
        assert!(
            bundle.contains("__lx_module_exports[\"beta\"] = __lx_mod_0[\"alpha\"];"),
            "{bundle}"
        );
        assert!(
            bundle.contains("__lx_module_exports[\"gamma\"] = __lx_mod_0[\"default\"];"),
            "{bundle}"
        );
    }

    #[test]
    fn keeps_multi_line_template_literals_verbatim() {
        // Regression: every module body was re-indented line by line, and the
        // continuation lines of a multi-line template literal took the indent
        // too — a Markdown string's `## Section` reached the program as
        // `  ## Section`.
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("entry.ts"),
            "export const text = `# Title\n\n## Section\nbody\n`;\n",
        )
        .unwrap();

        let bundle = build_test_bundle(temp.path(), "entry.ts");
        assert!(
            bundle.contains("`# Title\n\n## Section\nbody\n`"),
            "{bundle}"
        );
    }

    #[test]
    fn type_only_reexports_emit_no_runtime_export() {
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("types.ts"),
            "export interface Chart { x: number }\nexport interface Point { y: number }\n",
        )
        .unwrap();
        fs::write(
            temp.path().join("entry.ts"),
            "import type { Chart, Point } from './types';\nexport type { Chart };\nexport { Point };\nexport const beta = 3;\n",
        )
        .unwrap();

        let bundle = build_test_bundle(temp.path(), "entry.ts");
        assert!(
            !bundle.contains("__lx_module_exports[\"Chart\"]"),
            "{bundle}"
        );
        assert!(
            !bundle.contains("__lx_module_exports[\"Point\"]"),
            "{bundle}"
        );
        assert!(
            bundle.contains("__lx_module_exports[\"beta\"] = beta;"),
            "{bundle}"
        );
    }

    #[test]
    fn supports_export_all_reexports_from_dependencies() {
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("dep.ts"),
            "export const alpha = 1;\nexport default 2;\n",
        )
        .unwrap();
        fs::write(
            temp.path().join("entry.ts"),
            "export * from './dep';\nexport const beta = 3;\n",
        )
        .unwrap();

        let bundle = build_test_bundle(temp.path(), "entry.ts");
        assert!(
            bundle.contains("for (const __lx_export_name of Object.keys(__lx_mod_0)) {"),
            "{bundle}"
        );
        assert!(
            bundle.contains("if (__lx_export_name === \"default\") continue;"),
            "{bundle}"
        );
        assert!(
            bundle.contains("__lx_module_exports[\"beta\"] = beta;"),
            "{bundle}"
        );
    }

    #[test]
    fn export_all_conflicts_do_not_silently_override_previous_exports() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("dep-a.ts"), "export const alpha = 1;\n").unwrap();
        fs::write(temp.path().join("dep-b.ts"), "export const alpha = 2;\n").unwrap();
        fs::write(
            temp.path().join("entry.ts"),
            "export * from './dep-a';\nexport * from './dep-b';\n",
        )
        .unwrap();

        let bundle = build_test_bundle(temp.path(), "entry.ts");
        assert!(
            bundle.contains("__lx_ambiguous_star_exports.add(__lx_export_name);"),
            "{bundle}"
        );
        assert!(
            bundle.contains("delete __lx_module_exports[__lx_export_name];"),
            "{bundle}"
        );
    }

    #[test]
    fn exports_runtime_enum() {
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("entry.ts"),
            "export enum Color { Red, Green }\nexport const enum Erased { A }\n",
        )
        .unwrap();

        let bundle = build_test_bundle(temp.path(), "entry.ts");
        assert!(
            bundle.contains("__lx_module_exports[\"Color\"]"),
            "{bundle}"
        );
        assert!(!bundle.contains("__lx_module_exports[\"Erased\"]"));
    }

    /// An lxapp served by one `greet.hello` route, its client generated as the build does.
    fn native_project(root: &Path, app_logic: &str) -> String {
        write(
            root,
            "rust/lib.rs",
            r#"
            #[derive(serde::Deserialize)]
            pub struct HelloInput { pub name: String }
            #[lingxia::native("greet.hello")]
            pub async fn hello(input: HelloInput) -> lingxia::Result<String> { todo!() }
            "#,
        );
        lingxia_native_codegen::generate_clients(
            &[root.join("rust")],
            &root.join(".lingxia"),
            None,
            lingxia_native_codegen::CallerClass::StandardApp,
        )
        .unwrap();
        write(root, "lxapp.ts", app_logic);
        let options = BuildOptions {
            dev_session: false,
            release: false,
            package: false,
            typecheck: false,
            progress: crate::lxapp::options::ProgressMode::Plain,
            inputs_record: None,
            messages: None,
        };
        let report = build(&test_project(root), &options, None).unwrap();
        let LogicBuildStatus::Built { output_path } = report.status else {
            panic!("logic was not built");
        };
        fs::read_to_string(output_path).unwrap()
    }

    /// Runs `bundle` with a bridge that answers each call with its route, input name and page.
    fn run_with_bridge(bundle: &str, script: &str) -> String {
        use rong::{JSEngine, RongJS, Source};
        let runtime = RongJS::runtime();
        let ctx = runtime.context();
        ctx.eval::<()>(Source::from_bytes(
            "globalThis.__registerDefinedApp = () => {}; globalThis.defineApp = (app) => app; \
             globalThis.__lingxiaNative = { invoke: (route, input, pageId) => `${route}:${input.name}:${pageId}` };",
        ))
        .unwrap();
        ctx.eval::<()>(Source::from_bytes(bundle)).unwrap();
        ctx.eval(Source::from_bytes(script)).unwrap()
    }

    #[test]
    fn the_generated_native_client_ships_first_without_an_import() {
        let temp = TempDir::new().unwrap();
        let bundle = native_project(
            &temp.path().canonicalize().unwrap(),
            "export default globalThis.defineApp({});\n",
        );
        let client = bundle
            .find("//#region .lingxia/native.logic.ts")
            .expect("client bundled");
        assert!(
            client < bundle.find("//#region lxapp.ts").unwrap(),
            "{bundle}"
        );
        // What `page.native` builds for a page: its calls carry that page's id.
        let answer = run_with_bridge(
            &bundle,
            r#"globalThis.__lingxiaCreatePageNative("p1").greet.hello({ name: "ada" })"#,
        );
        assert_eq!(answer, "greet.hello:ada:p1");
    }

    #[test]
    fn an_lingxia_native_import_is_the_bundled_client() {
        let temp = TempDir::new().unwrap();
        let bundle = native_project(
            &temp.path().canonicalize().unwrap(),
            "import { native } from '@lingxia/native';\nglobalThis.client = native;\nexport default globalThis.defineApp({});\n",
        );
        assert_eq!(
            bundle.matches("//#region .lingxia/native.logic.ts").count(),
            1
        );
        let answer = run_with_bridge(&bundle, r#"client.greet.hello({ name: "ada" })"#);
        assert_eq!(answer, "greet.hello:ada:undefined");
    }

    #[test]
    fn an_lingxia_native_import_without_a_client_names_the_cause() {
        let temp = TempDir::new().unwrap();
        write(
            temp.path(),
            "lxapp.ts",
            "import { native } from '@lingxia/native';\nexport default { native };\n",
        );
        let project = test_project(temp.path());
        let mut bundler = LogicBundler::new(&project);
        let error = bundler
            .add_entry(temp.path().join("lxapp.ts"), ModuleRole::App)
            .unwrap_err();
        assert!(
            format!("{error:#}")
                .contains("lxapp.ts imports @lingxia/native, but this lxapp has no native client"),
            "{error:#}"
        );
    }
}
