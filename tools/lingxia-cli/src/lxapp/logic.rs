use crate::lxapp::options::BuildOptions;
use crate::lxapp::project::{Project, ProjectKind};
use anyhow::{Context, Result, anyhow, bail};
use indicatif::ProgressBar;
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Declaration, ExportDefaultDeclarationKind, Expression, ImportDeclarationSpecifier,
    ModuleExportName, ObjectPropertyKind, PropertyKey, Statement,
};
use oxc_codegen::{Codegen, CodegenOptions};
use oxc_parser::Parser;
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
    let page_logic_entries = discover_page_logic_entries(project)?;

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
    if let Some(app_logic) = app_logic {
        bundler.add_entry(app_logic, ModuleRole::App)?;
    }
    for (logic_path, page_path) in page_logic_entries {
        bundler.add_entry(logic_path, ModuleRole::Page { page_path })?;
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
    Page { page_path: String },
}

#[derive(Debug, Clone)]
struct ImportBinding {
    imported: Option<String>,
    local: String,
    namespace: bool,
}

#[derive(Debug, Clone)]
struct ImportRecord {
    statement_span: oxc_span::Span,
    resolved_local: Option<PathBuf>,
    bindings: Vec<ImportBinding>,
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
}

impl<'a> LogicBundler<'a> {
    fn new(project: &'a Project) -> Self {
        Self {
            project,
            modules: Vec::new(),
            module_vars: HashMap::new(),
            visiting: HashSet::new(),
        }
    }

    fn add_entry(&mut self, path: PathBuf, role: ModuleRole) -> Result<String> {
        self.compile_module(path, role)
    }

    fn render_bundle(self, _release: bool) -> Result<String> {
        let mut output = String::from("(function() {\n\n");
        for module in self.modules {
            output.push_str(&module.rendered);
            output.push('\n');
        }
        output.push_str("})();\n");
        Ok(output)
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
        if !parse_result.errors.is_empty() {
            bail!(
                "Failed to parse logic module {}: {}",
                path.display(),
                format_diagnostics(&parse_result.errors)
            );
        }
        let program = parse_result.program;

        let imports = collect_imports(&program, &source, &path, self.project.root.as_path())?;
        let mut dependency_vars = BTreeMap::new();
        for import in &imports {
            if let Some(local_path) = &import.resolved_local {
                let module_var = self.compile_module(local_path.clone(), ModuleRole::Plain)?;
                dependency_vars.insert(local_path.clone(), module_var);
            }
        }

        let rewritten = rewrite_module_source(
            self.project,
            &program,
            &source,
            &path,
            &role,
            &imports,
            &dependency_vars,
        )?;
        let transpiled = transpile_module(&path, &rewritten)?;

        let module_var = format!("__lx_mod_{}", self.modules.len());
        let rendered = format!(
            "//#region {}\nconst {} = (() => {{\n{}\n  return __lx_module_exports;\n}})();\n//#endregion",
            relative_to(&path, self.project.root.as_path()),
            module_var,
            indent_block(&transpiled, "  ")
        );
        self.module_vars.insert(path.clone(), module_var.clone());
        self.modules.push(ModuleArtifact { rendered });
        self.visiting.remove(&path);
        Ok(module_var)
    }
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

fn discover_page_logic_entries(project: &Project) -> Result<Vec<(PathBuf, String)>> {
    let mut entries = Vec::new();
    for page_path in &project.pages {
        let page_path_obj = Path::new(page_path);
        let without_ext = page_path_obj.with_extension("");
        let ts_path = project.root.join(&without_ext).with_extension("ts");
        let js_path = project.root.join(&without_ext).with_extension("js");
        match (ts_path.exists(), js_path.exists()) {
            (true, true) => {
                return Err(anyhow!(
                    "Logic layer conflict for {page_path}: found both .ts and .js"
                ));
            }
            (true, false) => entries.push((ts_path, page_path.clone())),
            (false, true) => entries.push((js_path, page_path.clone())),
            (false, false) => {}
        }
    }
    Ok(entries)
}

fn collect_imports(
    program: &oxc_ast::ast::Program<'_>,
    source: &str,
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
                        }),
                        ImportDeclarationSpecifier::ImportDefaultSpecifier(spec) => {
                            Ok(ImportBinding {
                                imported: Some("default".to_string()),
                                local: spec.local.name.as_str().to_string(),
                                namespace: false,
                            })
                        }
                        ImportDeclarationSpecifier::ImportNamespaceSpecifier(spec) => {
                            Ok(ImportBinding {
                                imported: None,
                                local: spec.local.name.as_str().to_string(),
                                namespace: true,
                            })
                        }
                    })
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?
            .unwrap_or_default();

        let resolved_local = if is_local_specifier(&import_source) {
            Some(resolve_local_import(
                module_path,
                &import_source,
                project_root,
            )?)
        } else {
            if bindings.is_empty() {
                bail!(
                    "Unsupported side-effect bare import {import_source:?} in {}",
                    module_path.display()
                );
            }
            let binding_names = bindings.iter().map(|binding| binding.local.as_str());
            let used = binding_names
                .into_iter()
                .any(|name| identifier_used_outside_span(source, import_decl.span, name));
            if used {
                bail!(
                    "Unsupported bare logic import {import_source:?} in {}. \
Only local relative imports are supported by the Rust logic builder for now.",
                    module_path.display()
                );
            }
            None
        };

        imports.push(ImportRecord {
            statement_span: import_decl.span,
            resolved_local,
            bindings,
        });
    }
    Ok(imports)
}

fn rewrite_module_source(
    project: &Project,
    program: &oxc_ast::ast::Program<'_>,
    source: &str,
    module_path: &Path,
    role: &ModuleRole,
    imports: &[ImportRecord],
    dependency_vars: &BTreeMap<PathBuf, String>,
) -> Result<String> {
    let mut output = String::new();
    let mut cursor = 0usize;
    let mut exports = Vec::<(String, String)>::new();

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
            Statement::ExportNamedDeclaration(export_decl) => {
                if export_decl.source.is_some() {
                    bail!(
                        "Re-export syntax is not supported in Rust logic builder: {}",
                        module_path.display()
                    );
                }

                if let Some(declaration) = &export_decl.declaration {
                    output.push_str(slice(source, declaration.span())?);
                    collect_exports_from_declaration(declaration, &mut exports)?;
                } else {
                    for specifier in &export_decl.specifiers {
                        exports.push((
                            module_export_name(&specifier.exported).ok_or_else(|| {
                                anyhow!("Unsupported exported name in {}", module_path.display())
                            })?,
                            module_export_name(&specifier.local).ok_or_else(|| {
                                anyhow!(
                                    "Unsupported local export name in {}",
                                    module_path.display()
                                )
                            })?,
                        ));
                    }
                }
            }
            Statement::ExportDefaultDeclaration(export_default) => {
                let (replacement, export_name) =
                    rewrite_export_default(source, &export_default.declaration)?;
                output.push_str(&replacement);
                exports.push(("default".to_string(), export_name));
            }
            Statement::ExpressionStatement(expr_stmt) => {
                if let Some(rewritten) =
                    rewrite_registration_call(project, source, &expr_stmt.expression, role)?
                {
                    output.push_str(&rewritten);
                } else {
                    output.push_str(slice(source, span)?);
                }
            }
            _ => {
                output.push_str(slice(source, span)?);
            }
        }

        cursor = end;
    }

    output.push_str(&source[cursor..]);
    output.push_str("\nconst __lx_module_exports = {");
    if !exports.is_empty() {
        output.push('\n');
        for (index, (exported, local)) in exports.iter().enumerate() {
            let comma = if index + 1 == exports.len() { "" } else { "," };
            output.push_str(&format!("  \"{exported}\": {local}{comma}\n"));
        }
    }
    output.push_str("};\n");
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
        Declaration::TSTypeAliasDeclaration(_)
        | Declaration::TSInterfaceDeclaration(_)
        | Declaration::TSEnumDeclaration(_)
        | Declaration::TSModuleDeclaration(_)
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

fn rewrite_registration_call(
    project: &Project,
    source: &str,
    expression: &Expression<'_>,
    role: &ModuleRole,
) -> Result<Option<String>> {
    let Expression::CallExpression(call_expr) = unwrap_expression(expression) else {
        return Ok(None);
    };
    let Expression::Identifier(identifier) = unwrap_expression(&call_expr.callee) else {
        return Ok(None);
    };

    match identifier.name.as_str() {
        "App" => {
            if !matches!(role, ModuleRole::App) {
                return Ok(None);
            }
            let first_arg = call_expr
                .arguments
                .first()
                .ok_or_else(|| anyhow!("App() must be called with a configuration object"))?;
            let config_expr = argument_source(source, first_arg.span())?;
            let handler_names = match unwrap_expression(first_arg.to_expression()) {
                Expression::ObjectExpression(object) => collect_app_handler_names(object),
                _ => Vec::new(),
            };
            let handler_json = serde_json::to_string(&handler_names)?;
            Ok(Some(format!(
                "globalThis.__registerApp({}, {});",
                config_expr,
                json_string_literal(&handler_json)
            )))
        }
        "Page" => {
            let ModuleRole::Page { page_path } = role else {
                return Ok(None);
            };
            let first_arg = call_expr
                .arguments
                .first()
                .ok_or_else(|| anyhow!("Page() must be called with a configuration expression"))?;
            let config_expr = argument_source(source, first_arg.span())?;
            let binding_meta_json = match unwrap_expression(first_arg.to_expression()) {
                Expression::ObjectExpression(object) => serde_json::to_string(&BindingMeta {
                    handlers: collect_page_handler_names(object),
                })?,
                _ => serde_json::to_string(&BindingMeta {
                    handlers: Vec::new(),
                })?,
            };
            let final_path = match project.kind {
                ProjectKind::LxApp => page_path.clone(),
                ProjectKind::LxPlugin => format!(
                    "plugin/{}/{}",
                    project
                        .plugin_id
                        .as_deref()
                        .ok_or_else(|| anyhow!("Missing plugin id"))?,
                    page_path
                ),
            };
            Ok(Some(format!(
                "globalThis.__registerPage({}, {}, {});",
                json_string_literal(&final_path),
                config_expr,
                json_string_literal(&binding_meta_json)
            )))
        }
        _ => Ok(None),
    }
}

#[derive(serde::Serialize)]
struct BindingMeta {
    handlers: Vec<String>,
}

fn collect_page_handler_names(object: &oxc_ast::ast::ObjectExpression<'_>) -> Vec<String> {
    let lifecycle_names: HashSet<&str> = [
        "onLoad",
        "onShow",
        "onReady",
        "onHide",
        "onUnload",
        "onPullDownRefresh",
        "onReachBottom",
        "onShareAppMessage",
        "onPageScroll",
        "onResize",
        "onTabItemTap",
    ]
    .into_iter()
    .collect();
    collect_object_handler_names(object, |name| {
        name != "data" && !name.starts_with('_') && !lifecycle_names.contains(name)
    })
}

fn collect_app_handler_names(object: &oxc_ast::ast::ObjectExpression<'_>) -> Vec<String> {
    let allowed: HashSet<&str> = ["onLaunch", "onShow", "onHide", "onUserCaptureScreen"]
        .into_iter()
        .collect();
    collect_object_handler_names(object, |name| allowed.contains(name))
}

fn collect_object_handler_names<F>(
    object: &oxc_ast::ast::ObjectExpression<'_>,
    include: F,
) -> Vec<String>
where
    F: Fn(&str) -> bool,
{
    let mut names = Vec::new();
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            continue;
        };
        let Some(name) = property_name(&property.key) else {
            continue;
        };
        if !include(&name) {
            continue;
        }
        if is_function_like_property(property) {
            names.push(name);
        }
    }
    names
}

fn is_function_like_property(property: &oxc_ast::ast::ObjectProperty<'_>) -> bool {
    if property.method {
        return true;
    }
    matches!(
        unwrap_expression(&property.value),
        Expression::FunctionExpression(_)
            | Expression::ArrowFunctionExpression(_)
            | Expression::Identifier(_)
            | Expression::StaticMemberExpression(_)
            | Expression::ComputedMemberExpression(_)
    )
}

fn property_name(key: &PropertyKey<'_>) -> Option<String> {
    match key {
        PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.as_str().to_string()),
        PropertyKey::StringLiteral(literal) => Some(literal.value.as_str().to_string()),
        _ => None,
    }
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

fn argument_source<'a>(source: &'a str, span: oxc_span::Span) -> Result<&'a str> {
    slice(source, span)
}

fn transpile_module(path: &Path, source: &str) -> Result<String> {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(path)
        .map_err(|_| anyhow!("Unsupported logic file {}", path.display()))?;
    let parse_result = Parser::new(&allocator, source, source_type).parse();
    if !parse_result.errors.is_empty() {
        bail!(
            "Failed to parse rewritten logic module {}: {}",
            path.display(),
            format_diagnostics(&parse_result.errors)
        );
    }

    let mut program = parse_result.program;
    let semantic = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&program);
    if !semantic.errors.is_empty() {
        bail!(
            "Semantic analysis failed for {}: {}",
            path.display(),
            format_diagnostics(&semantic.errors)
        );
    }

    let transformer_return = Transformer::new(&allocator, path, &TransformOptions::default())
        .build_with_scoping(semantic.semantic.into_scoping(), &mut program);
    if !transformer_return.errors.is_empty() {
        bail!(
            "Failed to transform {}: {}",
            path.display(),
            format_diagnostics(&transformer_return.errors)
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

fn identifier_used_outside_span(source: &str, span: oxc_span::Span, name: &str) -> bool {
    let start = span.start as usize;
    let end = span.end as usize;
    contains_identifier(&source[..start], name) || contains_identifier(&source[end..], name)
}

fn contains_identifier(haystack: &str, name: &str) -> bool {
    let bytes = haystack.as_bytes();
    let needle = name.as_bytes();
    if needle.is_empty() {
        return false;
    }
    let mut index = 0usize;
    while let Some(found) = haystack[index..].find(name) {
        let pos = index + found;
        let before_ok = pos == 0 || !is_ident_char(bytes[pos - 1]);
        let after_index = pos + needle.len();
        let after_ok = after_index >= bytes.len() || !is_ident_char(bytes[after_index]);
        if before_ok && after_ok {
            return true;
        }
        index = pos + needle.len();
    }
    false
}

fn is_ident_char(ch: u8) -> bool {
    ch.is_ascii_alphanumeric() || ch == b'_' || ch == b'$'
}

fn slice(source: &str, span: oxc_span::Span) -> Result<&str> {
    source
        .get(span.start as usize..span.end as usize)
        .ok_or_else(|| anyhow!("Invalid source span"))
}

fn json_string_literal(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

fn indent_block(source: &str, indent: &str) -> String {
    source
        .lines()
        .map(|line| {
            if line.is_empty() {
                String::new()
            } else {
                format!("{indent}{line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn relative_to(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn normalize_path(path: &Path) -> Result<PathBuf> {
    path.canonicalize()
        .with_context(|| format!("Failed to canonicalize {}", path.display()))
}

fn format_diagnostics<T: std::fmt::Debug>(diagnostics: &[T]) -> String {
    diagnostics
        .iter()
        .map(|error| format!("{error:?}"))
        .collect::<Vec<_>>()
        .join("\n")
}
