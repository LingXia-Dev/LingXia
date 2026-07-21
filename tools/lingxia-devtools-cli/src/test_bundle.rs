//! Dedicated Oxc bundler for `lxdev test` entries.
//!
//! Bundles a `.js`/`.ts` entry and its static ESM import graph into one
//! ES2020 classic script: an async IIFE whose promise the test runtime
//! awaits. Unlike the lxapp Logic bundler there are no `Page()`/`App()`
//! rewrites; dynamic `import()` and Node built-ins are rejected with a source
//! diagnostic. Statement rewrites preserve line counts so the inline source
//! map stays line-accurate against the original files.
//!
//! Two ESM semantics are intentionally not reproduced, both because faithful
//! support would require moving code off its original line and so break the
//! line-accurate source map (and both match the proven lxapp Logic bundler):
//! import bindings are rewritten in place rather than hoisted, so referencing
//! an imported binding textually *above* its `import` throws a TDZ error
//! instead of resolving; and exports are a value snapshot taken at module-body
//! end, so a mutated `export let` is not observed live by importers. Test
//! programs are linear scripts where neither pattern arises in practice.

use anyhow::{Context, Result, anyhow, bail};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Declaration, ExportDefaultDeclarationKind, ImportDeclarationSpecifier, ImportOrExportKind,
    ModuleExportName, Statement,
};
use oxc_ast_visit::Visit;
use oxc_codegen::{Codegen, CodegenOptions};
use oxc_parser::Parser;
use oxc_resolver::{ModuleType, ResolveError, ResolveOptions, Resolver};
use oxc_semantic::SemanticBuilder;
use oxc_sourcemap::{ConcatSourceMapBuilder, SourceMap};
use oxc_span::{GetSpan, SourceType, Span};
use oxc_transformer::{TransformOptions, Transformer};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// The unencoded bundle limit shared with the runtime.
pub const MAX_BUNDLE_BYTES: usize = 8 * 1024 * 1024;

pub struct TestBundle {
    /// The complete classic script, inline source map included.
    pub code: String,
    /// Stable bundle name used as the engine source name; stack frames
    /// referencing it are remapped through the source map.
    pub bundle_name: String,
    map: SourceMap<'static>,
}

/// A stack position mapped back to an original source file (1-based).
pub struct MappedPosition {
    pub source: String,
    pub line: u32,
    pub column: u32,
}

impl TestBundle {
    /// Replace `<bundle_name>:line:column` occurrences with original
    /// positions. Returns the rewritten stack and the first mapped frame.
    pub fn remap_stack(&self, stack: &str) -> (String, Option<MappedPosition>) {
        let table = self.map.generate_lookup_table();
        let mut output = String::with_capacity(stack.len());
        let mut primary: Option<MappedPosition> = None;
        let mut rest = stack;
        while let Some(found) = rest.find(&self.bundle_name) {
            let after = &rest[found + self.bundle_name.len()..];
            let Some((line, column, consumed)) = parse_position_suffix(after) else {
                output.push_str(&rest[..found + self.bundle_name.len()]);
                rest = after;
                continue;
            };
            output.push_str(&rest[..found]);
            match self.map.lookup_source_view_token(
                &table,
                line.saturating_sub(1),
                column.saturating_sub(1),
            ) {
                Some(token) => {
                    let source = token.get_source().unwrap_or("<unknown>").to_string();
                    let mapped = MappedPosition {
                        source: source.clone(),
                        line: token.get_src_line() + 1,
                        column: token.get_src_col() + 1,
                    };
                    output.push_str(&format!("{source}:{}:{}", mapped.line, mapped.column));
                    if primary.is_none() {
                        primary = Some(mapped);
                    }
                }
                None => output.push_str(&rest[found..found + self.bundle_name.len() + consumed]),
            }
            rest = &after[consumed..];
        }
        output.push_str(rest);
        (output, primary)
    }
}

/// Parse a `:line:column` suffix; returns (line, column, bytes consumed).
fn parse_position_suffix(text: &str) -> Option<(u32, u32, usize)> {
    let mut chars = text.char_indices().peekable();
    let mut numbers = [0u32; 2];
    let mut consumed = 0usize;
    for slot in &mut numbers {
        let (_, ':') = chars.next()? else {
            return None;
        };
        let mut digits = 0usize;
        while let Some((_, ch)) = chars.peek() {
            let Some(digit) = ch.to_digit(10) else { break };
            *slot = slot.saturating_mul(10).saturating_add(digit);
            digits += 1;
            chars.next();
        }
        if digits == 0 {
            return None;
        }
        consumed = chars.peek().map_or(text.len(), |(index, _)| *index);
    }
    Some((numbers[0], numbers[1], consumed))
}

pub fn bundle_test_entry(entry: &Path) -> Result<TestBundle> {
    let entry = normalize_path(entry)
        .with_context(|| format!("test entry not found: {}", entry.display()))?;
    ensure_supported_test_module(&entry)?;
    let root = find_project_root(&entry);

    let mut bundler = TestBundler {
        root,
        modules: Vec::new(),
        module_vars: HashMap::new(),
        visiting: HashSet::new(),
    };
    bundler.compile_module(entry.clone())?;

    let bundle_name = format!(
        "lxdev-test://{}",
        relative_display(&entry, &bundler.root).replace('\\', "/")
    );
    let bundle = assemble(bundler.modules, bundle_name);
    if bundle.code.len() > MAX_BUNDLE_BYTES {
        bail!(
            "bundled test is {} bytes; the limit is {MAX_BUNDLE_BYTES}",
            bundle.code.len()
        );
    }
    Ok(bundle)
}

struct CompiledModule {
    /// Rewritten + transpiled module body, trailing newline guaranteed.
    code: String,
    map: SourceMap<'static>,
    module_var: String,
}

struct TestBundler {
    root: PathBuf,
    modules: Vec<CompiledModule>,
    module_vars: HashMap<PathBuf, String>,
    visiting: HashSet<PathBuf>,
}

impl TestBundler {
    fn compile_module(&mut self, path: PathBuf) -> Result<String> {
        ensure_supported_test_module(&path)?;
        if let Some(module_var) = self.module_vars.get(&path) {
            return Ok(module_var.clone());
        }
        if !self.visiting.insert(path.clone()) {
            bail!("Circular test import detected at {}", path.display());
        }

        let source = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read test module {}", path.display()))?;
        let source_type = SourceType::from_path(&path)
            .map_err(|_| anyhow!("Unsupported test file {}", path.display()))?;
        let allocator = Allocator::default();
        let parse_result = Parser::new(&allocator, &source, source_type).parse();
        if !parse_result.diagnostics.is_empty() {
            bail!(
                "Failed to parse {}: {}",
                relative_display(&path, &self.root),
                format_diagnostics(&parse_result.diagnostics)
            );
        }
        let program = parse_result.program;

        reject_dynamic_imports(&program, &source, &path, &self.root)?;

        let imports = collect_imports(&program, &path, &self.root)?;
        let export_dependencies = collect_export_dependencies(&program, &path, &self.root)?;

        let mut dependency_vars = BTreeMap::new();
        for local_path in
            dependency_paths_in_source_order(&program, &imports, &export_dependencies)?
        {
            let module_var = self.compile_module(local_path.clone())?;
            dependency_vars.insert(local_path, module_var);
        }

        let rewritten = rewrite_module_source(
            &program,
            &source,
            &path,
            &imports,
            &export_dependencies,
            &dependency_vars,
        )?;
        let (code, map) = transpile_module(&path, &rewritten, &source, &self.root)?;

        let module_var = format!("__lx_mod_{}", self.modules.len());
        self.module_vars.insert(path.clone(), module_var.clone());
        self.modules.push(CompiledModule {
            code,
            map,
            module_var: module_var.clone(),
        });
        self.visiting.remove(&path);
        Ok(module_var)
    }
}

fn ensure_supported_test_module(path: &Path) -> Result<()> {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("js" | "ts" | "mjs" | "mts") => Ok(()),
        _ => bail!(
            "test modules must use .js, .ts, .mjs, or .mts; JSX/TSX is not available in the isolated runtime: {}",
            path.display()
        ),
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
            Statement::ExportNamedDeclaration(declaration) => export_dependencies
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

/// Every module — the entry included — becomes an awaited async IIFE inside
/// one outer async IIFE, so top-level `await` works everywhere and each
/// module keeps its own scope. The outer IIFE's promise is the run's result.
fn assemble(modules: Vec<CompiledModule>, bundle_name: String) -> TestBundle {
    let mut code = String::new();
    let mut concat = ConcatSourceMapBuilder::default();
    let mut line = 0u32;

    code.push_str(
        "(async () => {\n\"use strict\";\n\
const __lx_automation_host = globalThis.__LINGXIA_AUTOMATION_HOST__;\n\
globalThis.__RONG_TEST_HOST__ = {\n\
  args: __lx_automation_host.args,\n\
  attach: __lx_automation_host.attach,\n\
  report: (event) => __lx_automation_host.emit(event),\n\
};\n",
    );
    line += 8;

    for module in &modules {
        code.push_str(&format!(
            "const {} = await (async () => {{\n",
            module.module_var
        ));
        line += 1;
        concat.add_sourcemap(&module.map, line);
        code.push_str(&module.code);
        line += u32::try_from(module.code.matches('\n').count()).unwrap_or(u32::MAX);
        code.push_str("return __lx_module_exports;\n})();\n");
        line += 2;
    }
    code.push_str(
        "const __lx_test_framework = globalThis.__RONG_TEST__;\n\
if (!__lx_test_framework || typeof __lx_test_framework.run !== \"function\") {\n\
  throw new Error('No @rongjs/test framework was registered. Import test APIs from \"@rongjs/test\".');\n\
}\n\
return await __lx_test_framework.run();\n\
})()\n",
    );

    let map: SourceMap<'static> = concat.into_owned_sourcemap().into();
    code.push_str(&format!(
        "//# sourceMappingURL=data:application/json;charset=utf-8;base64,{}\n",
        BASE64.encode(map.to_json_string())
    ));
    code.push_str(&format!("//# sourceURL={bundle_name}\n"));

    TestBundle {
        code,
        bundle_name,
        map,
    }
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
    statement_span: Span,
    resolved_local: Option<PathBuf>,
    bindings: Vec<ImportBinding>,
}

#[derive(Debug, Clone)]
struct ExportDependencyRecord {
    statement_span: Span,
    resolved_local: Option<PathBuf>,
    export_all: bool,
}

fn collect_imports(
    program: &oxc_ast::ast::Program<'_>,
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

        // Unlike the Logic bundler, a side-effect import — bare `import
        // './setup'` or an empty list `import {} from './setup'` — is compiled
        // and executed in dependency order (ESM runs the module for its side
        // effects either way).
        let side_effect_only = import_decl
            .specifiers
            .as_ref()
            .is_none_or(|specifiers| specifiers.is_empty());
        let has_runtime_bindings = bindings.iter().any(|binding| !binding.type_only);
        let resolved_local = if import_decl.import_kind == ImportOrExportKind::Type
            || (!has_runtime_bindings && !side_effect_only)
        {
            None
        } else {
            Some(resolve_import_specifier(
                module_path,
                &import_source,
                project_root,
                import_decl.source.span,
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
            Statement::ExportNamedDeclaration(export_decl) => {
                let has_runtime_specifiers = export_decl.export_kind != ImportOrExportKind::Type
                    && export_decl
                        .specifiers
                        .iter()
                        .any(|specifier| specifier.export_kind != ImportOrExportKind::Type);
                let resolved_local = export_decl
                    .source
                    .as_ref()
                    .filter(|_| has_runtime_specifiers)
                    .map(|source| {
                        resolve_import_specifier(
                            module_path,
                            source.value.as_str(),
                            project_root,
                            source.span,
                        )
                    })
                    .transpose()?;
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
                        export_decl.source.span,
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

/// Splice statements while preserving line counts, so generated line numbers
/// stay aligned with the original file for everything the map touches. The
/// export table is appended after the last original line.
fn rewrite_module_source(
    program: &oxc_ast::ast::Program<'_>,
    source: &str,
    module_path: &Path,
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
        let original = slice(source, span)?;

        match statement {
            Statement::ImportDeclaration(import_decl) => {
                let import = imports
                    .iter()
                    .find(|record| record.statement_span == import_decl.span)
                    .ok_or_else(|| anyhow!("Internal import bookkeeping mismatch"))?;
                let stub = render_import_stub(import, dependency_vars)?;
                output.push_str(&pad_to_line_count(&stub, original));
            }
            Statement::ExportNamedDeclaration(export_decl) => {
                if export_decl.source.is_some() {
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
                                let exported = module_export_name(&specifier.exported)
                                    .ok_or_else(|| unsupported_name(module_path))?;
                                let local = module_export_name(&specifier.local)
                                    .ok_or_else(|| unsupported_name(module_path))?;
                                exports.push((
                                    exported,
                                    format!("{module_var}[{}]", json_string_literal(&local)),
                                ));
                            }
                        }
                    }
                    output.push_str(&pad_to_line_count("", original));
                } else if let Some(declaration) = &export_decl.declaration {
                    // Drop the `export ` prefix, keep the declaration text.
                    let decl_text = slice(source, declaration.span())?;
                    output.push_str(&pad_to_line_count(decl_text, original));
                    collect_exports_from_declaration(declaration, &mut exports)?;
                } else {
                    for specifier in &export_decl.specifiers {
                        if specifier.export_kind == ImportOrExportKind::Type {
                            continue;
                        }
                        exports.push((
                            module_export_name(&specifier.exported)
                                .ok_or_else(|| unsupported_name(module_path))?,
                            module_export_name(&specifier.local)
                                .ok_or_else(|| unsupported_name(module_path))?,
                        ));
                    }
                    output.push_str(&pad_to_line_count("", original));
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
                                module_export_name(exported)
                                    .ok_or_else(|| unsupported_name(module_path))?,
                                module_var.clone(),
                            ));
                        }
                    }
                }
                output.push_str(&pad_to_line_count("", original));
            }
            Statement::ExportDefaultDeclaration(export_default) => {
                let (replacement, export_name) =
                    rewrite_export_default(source, &export_default.declaration)?;
                output.push_str(&pad_to_line_count(&replacement, original));
                exports.push(("default".to_string(), export_name));
            }
            _ => {
                output.push_str(original);
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
        output.push_str(&format!(
            "__lx_module_exports[{}] = {local};\n",
            json_string_literal(exported)
        ));
    }
    Ok(output)
}

/// Pad `replacement` with newlines to occupy exactly as many lines as
/// `original` did, keeping later lines aligned for the source map.
fn pad_to_line_count(replacement: &str, original: &str) -> String {
    let original_newlines = original.matches('\n').count();
    let replacement_newlines = replacement.matches('\n').count();
    let mut output = replacement.to_string();
    for _ in replacement_newlines..original_newlines {
        output.push('\n');
    }
    output
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
    Ok(lines.join(" "))
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
        // A TS `enum` is a runtime value (the transformer emits an object), so
        // an exported enum must reach the export table. A `const enum` is
        // erased and correctly falls through.
        Declaration::TSEnumDeclaration(enum_decl) if !enum_decl.r#const => {
            let name = enum_decl.id.name.as_str().to_string();
            exports.push((name.clone(), name));
        }
        // Type-only declarations (type alias, interface, const enum, ambient
        // namespace) are erased and contribute no runtime export.
        _ => {}
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

struct DynamicImportFinder {
    spans: Vec<Span>,
}

impl<'a> Visit<'a> for DynamicImportFinder {
    fn visit_import_expression(&mut self, expr: &oxc_ast::ast::ImportExpression<'a>) {
        self.spans.push(expr.span);
    }
}

fn reject_dynamic_imports(
    program: &oxc_ast::ast::Program<'_>,
    source: &str,
    module_path: &Path,
    project_root: &Path,
) -> Result<()> {
    let mut finder = DynamicImportFinder { spans: Vec::new() };
    finder.visit_program(program);
    if let Some(span) = finder.spans.first() {
        let (line, column) = line_column(source, span.start as usize);
        bail!(
            "Dynamic import() is not supported in tests ({}:{line}:{column}); use a static import",
            relative_display(module_path, project_root)
        );
    }
    Ok(())
}

fn resolve_import_specifier(
    from_module: &Path,
    specifier: &str,
    project_root: &Path,
    span: Span,
) -> Result<PathBuf> {
    if specifier.starts_with("node:") || is_node_builtin(specifier) {
        bail!(
            "Node built-in module {specifier:?} is not available in the test runtime ({})",
            relative_display(from_module, project_root)
        );
    }
    let _ = span;
    if is_local_specifier(specifier) {
        resolve_local_import(from_module, specifier, project_root)
    } else {
        resolve_bare_import(from_module, specifier, project_root)
    }
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

    for candidate in local_candidates(&candidate_base) {
        if candidate.exists() {
            return normalize_path(&candidate);
        }
    }
    Err(anyhow!(
        "Failed to resolve test import {:?} from {}",
        specifier,
        relative_display(from_module, project_root)
    ))
}

fn resolve_bare_import(
    from_module: &Path,
    specifier: &str,
    project_root: &Path,
) -> Result<PathBuf> {
    let mut options = ResolveOptions::default()
        .with_condition_names(&["import", "module", "default"])
        .with_builtin_modules(true);
    options.module_type = true;
    options.main_fields = vec!["module".into(), "main".into()];
    options.extensions = vec![
        ".ts".into(),
        ".mts".into(),
        ".js".into(),
        ".mjs".into(),
        ".json".into(),
    ];
    options.extension_alias = vec![
        (
            ".js".into(),
            vec![".ts".into(), ".js".into(), ".mjs".into()],
        ),
        (
            ".mjs".into(),
            vec![".mts".into(), ".mjs".into(), ".js".into()],
        ),
    ];

    let resolver = Resolver::new(options);
    let resolution = match resolver.resolve_file(from_module, specifier) {
        Ok(resolution) => resolution,
        Err(ResolveError::Builtin { resolved, .. }) => bail!(
            "Node built-in module {resolved:?} is not available in the test runtime ({})",
            relative_display(from_module, project_root)
        ),
        Err(err) => {
            return Err(anyhow!(err)).with_context(|| {
                format!(
                    "Failed to resolve test import {specifier:?} from {}. \
Run npm install in the project root if this package is missing.",
                    relative_display(from_module, project_root)
                )
            });
        }
    };

    if matches!(resolution.module_type(), Some(ModuleType::CommonJs)) {
        bail!(
            "Unsupported CommonJS test import {specifier:?} from {} -> {}. \
Use an ESM package or ESM entrypoint.",
            relative_display(from_module, project_root),
            relative_display(resolution.path(), project_root)
        );
    }
    normalize_path(resolution.path())
}

fn is_node_builtin(specifier: &str) -> bool {
    const BUILTINS: [&str; 27] = [
        "assert",
        "buffer",
        "child_process",
        "cluster",
        "crypto",
        "dgram",
        "dns",
        "events",
        "fs",
        "http",
        "http2",
        "https",
        "module",
        "net",
        "os",
        "path",
        "perf_hooks",
        "process",
        "querystring",
        "readline",
        "stream",
        "tls",
        "tty",
        "url",
        "util",
        "worker_threads",
        "zlib",
    ];
    let root = specifier.split('/').next().unwrap_or(specifier);
    BUILTINS.contains(&root)
}

/// Transpile the rewritten module and produce its source map, with the map's
/// source name and content pointing at the *original* file.
fn transpile_module(
    path: &Path,
    rewritten: &str,
    original_source: &str,
    project_root: &Path,
) -> Result<(String, SourceMap<'static>)> {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(path)
        .map_err(|_| anyhow!("Unsupported test file {}", path.display()))?;
    let parse_result = Parser::new(&allocator, rewritten, source_type).parse();
    if !parse_result.diagnostics.is_empty() {
        bail!(
            "Failed to parse rewritten test module {}: {}",
            relative_display(path, project_root),
            format_diagnostics(&parse_result.diagnostics)
        );
    }

    let mut program = parse_result.program;
    // `with_enum_eval` is required for the transformer to lower TS `enum`
    // (it evaluates member initializers); without it the transformer panics
    // on any enum.
    let semantic = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .with_enum_eval(true)
        .build(&program);
    if !semantic.diagnostics.is_empty() {
        bail!(
            "Semantic analysis failed for {}: {}",
            relative_display(path, project_root),
            format_diagnostics(&semantic.diagnostics)
        );
    }

    let transformer_return = Transformer::new(&allocator, path, &TransformOptions::default())
        .build_with_scoping(semantic.semantic.into_scoping(), &mut program);
    if !transformer_return.diagnostics.is_empty() {
        bail!(
            "Failed to transform {}: {}",
            relative_display(path, project_root),
            format_diagnostics(&transformer_return.diagnostics)
        );
    }

    let ret = Codegen::new()
        .with_options(CodegenOptions {
            source_map_path: Some(path.to_path_buf()),
            ..CodegenOptions::default()
        })
        .build(&program);

    let mut code = ret.code;
    if !code.ends_with('\n') {
        code.push('\n');
    }
    let mut map = ret
        .map
        .ok_or_else(|| anyhow!("Codegen produced no source map for {}", path.display()))?;
    let display = relative_display(path, project_root).replace('\\', "/");
    map.set_sources([display.as_str()]);
    map.set_source_contents(vec![Some(original_source)]);
    Ok((code, map.into_owned()))
}

fn local_candidates(base: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    match base.extension().and_then(|ext| ext.to_str()) {
        // TypeScript NodeNext/ESM writes the *output* extension in the
        // specifier (`./x.js`) even when the file on disk is `x.ts`. Try the
        // literal path first, then the TypeScript sibling — matching the
        // bare-import resolver's extension_alias.
        Some("js") => {
            candidates.push(base.to_path_buf());
            candidates.push(base.with_extension("ts"));
        }
        Some("mjs") => {
            candidates.push(base.to_path_buf());
            candidates.push(base.with_extension("mts"));
        }
        Some(_) => candidates.push(base.to_path_buf()),
        None => {
            candidates.push(base.with_extension("ts"));
            candidates.push(base.with_extension("js"));
            candidates.push(base.with_extension("mts"));
            candidates.push(base.with_extension("mjs"));
            candidates.push(base.join("index.ts"));
            candidates.push(base.join("index.js"));
            candidates.push(base.join("index.mts"));
            candidates.push(base.join("index.mjs"));
        }
    }
    candidates
}

fn is_local_specifier(specifier: &str) -> bool {
    specifier.starts_with("./") || specifier.starts_with("../") || specifier.starts_with('/')
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

fn unsupported_name(module_path: &Path) -> anyhow::Error {
    anyhow!("Unsupported export name in {}", module_path.display())
}

fn slice(source: &str, span: Span) -> Result<&str> {
    source
        .get(span.start as usize..span.end as usize)
        .ok_or_else(|| anyhow!("Invalid source span"))
}

fn json_string_literal(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

fn line_column(source: &str, offset: usize) -> (usize, usize) {
    let prefix = &source[..offset.min(source.len())];
    let line = prefix.matches('\n').count() + 1;
    let column = prefix
        .rsplit_once('\n')
        .map_or(prefix.len(), |(_, tail)| tail.len())
        + 1;
    (line, column)
}

fn format_diagnostics<T: std::fmt::Debug>(diagnostics: &[T]) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| format!("{diagnostic:?}"))
        .collect::<Vec<_>>()
        .join("; ")
}

fn normalize_path(path: &Path) -> Result<PathBuf> {
    fs::canonicalize(path).with_context(|| format!("Failed to resolve path {}", path.display()))
}

/// The nearest ancestor with a project marker anchors display names and
/// `/`-prefixed local imports; the entry's directory otherwise.
fn find_project_root(entry: &Path) -> PathBuf {
    let fallback = entry
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let mut current = entry.parent();
    while let Some(dir) = current {
        for marker in ["package.json", "lxapp.json", "lingxia.yaml"] {
            if dir.join(marker).exists() {
                return dir.to_path_buf();
            }
        }
        current = dir.parent();
    }
    fallback
}

fn relative_display(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .map(|relative| relative.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        dir
    }

    fn write(dir: &tempfile::TempDir, name: &str, source: &str) -> PathBuf {
        let path = dir.path().join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, source).unwrap();
        path
    }

    #[test]
    fn bundles_imports_exports_and_types() {
        let dir = project();
        write(
            &dir,
            "lib/helpers.ts",
            "export interface Opts { x: number }\nexport const helper = (x: number): number => x + 1;\nexport default function base(): number { return 41; }\n",
        );
        write(
            &dir,
            "lib/extra.ts",
            "export * from './helpers';\nexport { helper as aliased } from './helpers';\n",
        );
        let entry = write(
            &dir,
            "flow.ts",
            "import base, { helper } from './lib/helpers';\nimport * as extra from './lib/extra';\nimport type { Opts } from './lib/helpers';\nconst opts: Opts = { x: base() };\nif (helper(opts.x) !== 42 || extra.aliased(1) !== 2) {\n  throw new Error('math failed');\n}\nconsole.log('ok');\n",
        );

        let bundle = bundle_test_entry(&entry).expect("bundle");
        assert!(bundle.code.starts_with("(async () => {"));
        assert!(bundle.code.contains("__lx_mod_0"));
        assert!(
            bundle
                .code
                .contains("//# sourceMappingURL=data:application/json")
        );
        assert!(bundle.code.contains("//# sourceURL=lxdev-test://flow.ts"));
        assert_eq!(bundle.bundle_name, "lxdev-test://flow.ts");
        // TS types are erased.
        assert!(!bundle.code.contains("interface"));
        assert!(!bundle.code.contains(": number"));
        // The map carries original names and contents.
        let sources: Vec<&str> = bundle.map.get_sources().collect();
        assert!(sources.contains(&"flow.ts"));
        assert!(sources.iter().any(|source| source.contains("helpers.ts")));
        assert!(
            bundle
                .map
                .get_source_contents()
                .all(|content| content.is_some())
        );
    }

    #[test]
    fn rejects_jsx_and_tsx_modules_before_transforming() {
        let dir = project();
        for extension in ["jsx", "tsx"] {
            let entry = write(
                &dir,
                &format!("flow.{extension}"),
                "export const view = <div>unsupported</div>;\n",
            );
            let error = bundle_test_entry(&entry)
                .err()
                .expect("JSX entry must be rejected")
                .to_string();
            assert!(error.contains("JSX/TSX is not available"), "{error}");
            assert!(error.contains(".js, .ts, .mjs, or .mts"), "{error}");
        }

        write(
            &dir,
            "component.tsx",
            "export const view = <div>unsupported</div>;\n",
        );
        let entry = write(&dir, "imports-tsx.ts", "import './component.tsx';\n");
        let error = bundle_test_entry(&entry)
            .err()
            .expect("imported TSX module must be rejected")
            .to_string();
        assert!(error.contains("component.tsx"), "{error}");
        assert!(error.contains("JSX/TSX is not available"), "{error}");
    }

    #[test]
    fn side_effect_imports_execute() {
        let dir = project();
        write(&dir, "setup.ts", "(globalThis as any).__ready = true;\n");
        let entry = write(&dir, "flow.ts", "import './setup';\nconsole.log('ok');\n");
        let bundle = bundle_test_entry(&entry).expect("bundle");
        assert!(bundle.code.contains("void __lx_mod_0;"));
    }

    #[test]
    fn dependencies_follow_source_order_not_path_order() {
        let dir = project();
        write(&dir, "z-first.ts", "globalThis.__order.push('first');\n");
        write(&dir, "a-second.ts", "globalThis.__order.push('second');\n");
        let entry = write(
            &dir,
            "flow.ts",
            "globalThis.__order = [];\nimport './z-first';\nexport * from './a-second';\n",
        );

        let bundle = bundle_test_entry(&entry).expect("bundle");
        assert!(
            bundle.code.find("__order.push(\"first\")").unwrap()
                < bundle.code.find("__order.push(\"second\")").unwrap()
        );
    }

    #[test]
    fn bundles_rong_test_package_from_node_modules() {
        let dir = project();
        write(
            &dir,
            "node_modules/@rongjs/test/package.json",
            r#"{
              "name": "@rongjs/test",
              "type": "module",
              "exports": { ".": { "import": "./src/index.js" } }
            }"#,
        );
        write(
            &dir,
            "node_modules/@rongjs/test/src/runtime.js",
            "globalThis.__RONG_TEST__ = { run: async () => ({ total: 0, passed: 0, failed: 0, skipped: 0, duration_ms: 0, cases: [] }) };\n",
        );
        write(
            &dir,
            "node_modules/@rongjs/test/src/index.js",
            "import './runtime.js';\nexport const test = (name, run) => void [name, run];\n",
        );
        let entry = write(
            &dir,
            "flow.ts",
            "import { test } from '@rongjs/test';\ntest('works', async () => {});\n",
        );

        let bundle = bundle_test_entry(&entry).expect("bundle @rongjs/test");
        assert!(bundle.code.contains("__RONG_TEST__"));
        assert!(bundle.code.contains("const { test }"));
        assert!(
            bundle.code.find("__RONG_TEST_HOST__").unwrap()
                < bundle.code.find("globalThis.__RONG_TEST__ =").unwrap()
        );
        assert!(
            bundle
                .code
                .contains("return await __lx_test_framework.run();")
        );
        assert!(
            bundle
                .map
                .get_sources()
                .any(|source| source.ends_with("@rongjs/test/src/runtime.js"))
        );
    }

    #[test]
    fn empty_specifier_import_still_executes_for_side_effects() {
        let dir = project();
        write(&dir, "setup.ts", "(globalThis as any).__ready = true;\n");
        // `import {} from './setup'` must run the module, like a bare import.
        let entry = write(
            &dir,
            "flow.ts",
            "import {} from './setup';\nconsole.log('ok');\n",
        );
        let bundle = bundle_test_entry(&entry).expect("bundle");
        assert!(
            bundle.code.contains("__lx_mod_0"),
            "empty-specifier module was dropped: {}",
            bundle.code
        );
    }

    #[test]
    fn resolves_js_specifier_to_ts_file() {
        let dir = project();
        // NodeNext convention: the file is helper.ts, the import writes .js.
        write(&dir, "helper.ts", "export const value = 7;\n");
        let entry = write(
            &dir,
            "flow.ts",
            "import { value } from './helper.js';\nif (value !== 7) throw new Error('x');\n",
        );
        let bundle = bundle_test_entry(&entry).expect("bundle must resolve .js -> .ts");
        assert!(bundle.map.get_sources().any(|s| s.contains("helper.ts")));
    }

    #[test]
    fn exports_runtime_enum() {
        let dir = project();
        write(
            &dir,
            "colors.ts",
            "export enum Color { Red, Green }\nexport const enum Erased { A }\n",
        );
        let entry = write(
            &dir,
            "flow.ts",
            "import { Color } from './colors';\nif (Color.Green !== 1) throw new Error('x');\n",
        );
        let bundle = bundle_test_entry(&entry).expect("bundle");
        // The runtime enum reaches the export table; the const enum is erased.
        assert!(
            bundle.code.contains("__lx_module_exports[\"Color\"]"),
            "runtime enum missing from exports: {}",
            bundle.code
        );
        assert!(!bundle.code.contains("\"Erased\""));
    }

    #[test]
    fn remaps_generated_positions_to_original_lines() {
        let dir = project();
        let entry = write(
            &dir,
            "flow.ts",
            "const pad: number = 1;\nconst again: number = pad + 1;\nthrow new Error('boom-' + again);\n",
        );
        let bundle = bundle_test_entry(&entry).expect("bundle");

        let throw_line_zero_based = bundle
            .code
            .lines()
            .position(|line| line.contains("boom-"))
            .expect("throw line in bundle");
        let column = bundle
            .code
            .lines()
            .nth(throw_line_zero_based)
            .unwrap()
            .find("throw")
            .unwrap_or(0);
        let stack = format!(
            "Error: boom\n    at {}:{}:{}",
            bundle.bundle_name,
            throw_line_zero_based + 1,
            column + 1
        );
        let (mapped, primary) = bundle.remap_stack(&stack);
        let primary = primary.expect("mapped frame");
        assert_eq!(primary.source, "flow.ts");
        assert_eq!(primary.line, 3, "mapped stack: {mapped}");
        assert!(mapped.contains("flow.ts:3:"));
        assert!(!mapped.contains("lxdev-test://"));
    }

    #[test]
    fn rejects_dynamic_import() {
        let dir = project();
        let entry = write(&dir, "flow.ts", "const x = 1;\nawait import('./other');\n");
        let error = bundle_test_entry(&entry)
            .err()
            .expect("expected error")
            .to_string();
        assert!(error.contains("Dynamic import()"), "{error}");
        assert!(error.contains("flow.ts:2:"), "{error}");
    }

    #[test]
    fn rejects_node_builtins() {
        let dir = project();
        let entry = write(&dir, "flow.ts", "import { readFileSync } from 'node:fs';\n");
        let error = bundle_test_entry(&entry)
            .err()
            .expect("expected error")
            .to_string();
        assert!(error.contains("built-in"), "{error}");

        let entry = write(&dir, "flow2.ts", "import * as path from 'path';\n");
        let error = bundle_test_entry(&entry)
            .err()
            .expect("expected error")
            .to_string();
        assert!(error.contains("built-in"), "{error}");
    }

    #[test]
    fn rejects_circular_imports() {
        let dir = project();
        write(&dir, "a.ts", "import './b';\nexport const a = 1;\n");
        write(&dir, "b.ts", "import './a';\nexport const b = 2;\n");
        let entry = write(&dir, "flow.ts", "import './a';\n");
        let error = bundle_test_entry(&entry)
            .err()
            .expect("expected error")
            .to_string();
        assert!(error.contains("Circular"), "{error}");
    }

    #[test]
    fn rejects_oversized_bundles() {
        let dir = project();
        let big = format!("const s = \"{}\";\n", "x".repeat(MAX_BUNDLE_BYTES));
        let entry = write(&dir, "flow.ts", &big);
        let error = bundle_test_entry(&entry)
            .err()
            .expect("expected error")
            .to_string();
        assert!(error.contains("limit"), "{error}");
    }

    #[test]
    fn line_counts_survive_multiline_imports() {
        let dir = project();
        write(
            &dir,
            "dep.ts",
            "export const one = 1;\nexport const two = 2;\n",
        );
        let entry = write(
            &dir,
            "flow.ts",
            "import {\n  one,\n  two,\n} from './dep';\nthrow new Error('after-imports:' + one + two);\n",
        );
        let bundle = bundle_test_entry(&entry).expect("bundle");
        let throw_line_zero_based = bundle
            .code
            .lines()
            .position(|line| line.contains("after-imports"))
            .expect("throw line");
        let stack = format!("at {}:{}:1", bundle.bundle_name, throw_line_zero_based + 1);
        let (_, primary) = bundle.remap_stack(&stack);
        // The multi-line import collapsed to one stub line + padding, so the
        // throw on original line 5 must still map to line 5.
        assert_eq!(primary.expect("mapped").line, 5);
    }
}
