//! Bundle-time check that every spec is registered by a direct `spec` call.
//!
//! `FILE:LINE`, offline listing and `Rerun:` lines read spec calls from the
//! source. That only works when `spec` is `@lingxia/test`'s binding, under its
//! own name, and only ever called: `spec(…)` or `spec.skip(…)`. An alias
//! (`const s = cond ? spec.skip : spec`), a rename, a re-export or a namespace
//! import hides the registration, so it is refused here instead of producing
//! a listing that silently misses specs.

use std::collections::HashSet;

use anyhow::{Result, bail};
use oxc_ast::ast::{
    CallExpression, ExportAllDeclaration, ExportFromDeclaration, Expression, IdentifierReference,
    ImportDeclarationSpecifier, ImportOrExportKind, ModuleExportName, Program, Statement,
    TSTypeQuery,
};
use oxc_ast_visit::{Visit, walk};
use oxc_semantic::{SemanticBuilder, SymbolId};
use oxc_span::{GetSpan, Span};

use crate::test_bundle::{line_column, module_export_name};

const PACKAGE: &str = "@lingxia/test";
const VALUE_USE: &str = "`spec` is used as a value, not called: spec(…) or spec.skip(…)";

/// Fail when `program` reaches `spec` other than by calling it.
pub(crate) fn check_spec_usage(program: &Program<'_>, source: &str, display: &str) -> Result<()> {
    // Fills the bindings' symbol ids the import loop reads.
    let semantic = SemanticBuilder::new().build(program).semantic;
    let mut problems: Vec<(u32, String)> = Vec::new();
    let mut symbols = HashSet::new();
    for statement in &program.body {
        match statement {
            Statement::ImportDeclaration(import)
                if import.source.value == PACKAGE
                    && import.import_kind == ImportOrExportKind::Value =>
            {
                for specifier in import.specifiers.iter().flatten() {
                    match specifier {
                        ImportDeclarationSpecifier::ImportSpecifier(specifier)
                            if specifier.import_kind == ImportOrExportKind::Value
                                && module_export_name(&specifier.imported).as_deref()
                                    == Some("spec") =>
                        {
                            let local = specifier.local.name.as_str();
                            if local != "spec" {
                                problems.push((
                                    specifier.span.start,
                                    format!(
                                        "`spec` is imported as `{local}`. Import it by its own \
                                         name: import {{ spec }} from '{PACKAGE}'"
                                    ),
                                ));
                            } else if let Some(symbol) = specifier.local.symbol_id.get() {
                                symbols.insert(symbol);
                            }
                        }
                        ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                            problems.push((
                                specifier.span.start,
                                format!(
                                    "`import * as {}` from '{PACKAGE}'. Import the names you \
                                     use: import {{ spec, expect }} from '{PACKAGE}'",
                                    specifier.local.name
                                ),
                            ));
                        }
                        _ => {}
                    }
                }
            }
            Statement::ExportAllDeclaration(export) => check_export_all(export, &mut problems),
            Statement::ExportFromDeclaration(export) => check_export_from(export, &mut problems),
            _ => {}
        }
    }
    if !symbols.is_empty() {
        let mut finder = Uses {
            scoping: semantic.scoping(),
            symbols: &symbols,
            called: HashSet::new(),
            values: Vec::new(),
        };
        finder.visit_program(program);
        for span in finder.values {
            if !finder.called.contains(&span.start) {
                problems.push((span.start, VALUE_USE.to_string()));
            }
        }
    }
    if problems.is_empty() {
        return Ok(());
    }
    problems.sort_by_key(|(offset, _)| *offset);
    let out: Vec<String> = problems
        .iter()
        .map(|(offset, message)| {
            let (line, column) = line_column(source, *offset as usize);
            format!("{display}:{line}:{column}  {message}")
        })
        .collect();
    let hint = if problems.iter().any(|(_, message)| message == VALUE_USE) {
        "\nCall `spec` directly so every spec is found without running the file. To skip \
         on a condition known at load time: spec(title, { skip: cond ? 'why' : undefined }, \
         body), or spec.configure({ skip: … }) for the whole file."
    } else {
        ""
    };
    bail!("{}{hint}", out.join("\n"))
}

fn check_export_all(export: &ExportAllDeclaration<'_>, problems: &mut Vec<(u32, String)>) {
    if export.source.value == PACKAGE && export.export_kind == ImportOrExportKind::Value {
        problems.push((
            export.span.start,
            format!(
                "`export * from '{PACKAGE}'` re-exports `spec`. Spec files import it from \
                 '{PACKAGE}' themselves"
            ),
        ));
    }
}

fn check_export_from(export: &ExportFromDeclaration<'_>, problems: &mut Vec<(u32, String)>) {
    if export.source.value != PACKAGE || export.export_kind != ImportOrExportKind::Value {
        return;
    }
    for specifier in &export.specifiers {
        if specifier.export_kind == ImportOrExportKind::Value
            && module_export_name(&specifier.local).as_deref() == Some("spec")
        {
            problems.push((
                specifier.span.start,
                format!("re-exports `spec`. Spec files import it from '{PACKAGE}' themselves"),
            ));
        }
    }
}

/// References to the imported `spec`, and the ones that are a call's callee
/// or the object of one (`spec.skip(…)`).
struct Uses<'s> {
    scoping: &'s oxc_semantic::Scoping,
    symbols: &'s HashSet<SymbolId>,
    called: HashSet<u32>,
    values: Vec<Span>,
}

impl Uses<'_> {
    fn is_spec(&self, ident: &IdentifierReference<'_>) -> bool {
        ident
            .reference_id
            .get()
            .and_then(|reference| self.scoping.get_reference(reference).symbol_id())
            .is_some_and(|symbol| self.symbols.contains(&symbol))
    }
}

impl<'a> Visit<'a> for Uses<'_> {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        let callee = match call.callee.get_inner_expression() {
            Expression::Identifier(ident) => Some(ident),
            Expression::StaticMemberExpression(member) => {
                match member.object.get_inner_expression() {
                    Expression::Identifier(ident) => Some(ident),
                    _ => None,
                }
            }
            _ => None,
        };
        if let Some(ident) = callee {
            self.called.insert(ident.span.start);
        }
        walk::walk_call_expression(self, call);
    }

    fn visit_identifier_reference(&mut self, ident: &IdentifierReference<'a>) {
        if self.is_spec(ident) {
            self.values.push(ident.span());
        }
    }

    fn visit_module_export_name(&mut self, name: &ModuleExportName<'a>) {
        if let ModuleExportName::IdentifierReference(ident) = name
            && self.is_spec(ident)
        {
            // `export { spec }`: never called, always a value.
            self.values.push(ident.span());
        }
    }

    // `typeof spec` names a type; nothing registers there.
    fn visit_ts_type_query(&mut self, _: &TSTypeQuery<'a>) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxc_allocator::Allocator;
    use oxc_parser::Parser;
    use oxc_span::SourceType;

    fn check(source: &str) -> Result<()> {
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, source, SourceType::ts()).parse();
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        check_spec_usage(&parsed.program, source, "a.test.ts")
    }

    #[test]
    fn direct_calls_pass() {
        check(
            "import { spec, type Fixture } from '@lingxia/test';\n\
             spec.configure({ tags: ['x'] });\n\
             spec('a', async () => {});\n\
             spec.skip('b', { skip: 'why' }, async () => {});\n\
             (spec.only)('c', async () => {});\n\
             type S = typeof spec;\n\
             const specs = [1].map((spec) => spec + 1);\n",
        )
        .unwrap();
        // Another module's `spec` is not this package's.
        check("import { spec } from './other';\nconst s = spec;\n").unwrap();
    }

    #[test]
    fn aliases_are_refused_with_the_fix() {
        let err = check(
            "import { spec } from '@lingxia/test';\n\
             const gated = process ? spec.skip : spec;\n\
             gated('a', async () => {});\n",
        )
        .unwrap_err()
        .to_string();
        assert!(
            err.starts_with("a.test.ts:2:25  `spec` is used as a value"),
            "{err}"
        );
        assert!(err.contains("\na.test.ts:2:37  "), "{err}");
        assert!(err.contains("{ skip: cond ? 'why' : undefined }"), "{err}");

        for (source, expected) in [
            (
                "import { spec as it } from '@lingxia/test';\nit('a', async () => {});\n",
                "a.test.ts:1:10  `spec` is imported as `it`",
            ),
            (
                "import * as lt from '@lingxia/test';\nlt.spec('a', async () => {});\n",
                "a.test.ts:1:8  `import * as lt`",
            ),
            (
                "import { spec } from '@lingxia/test';\nexport { spec };\n",
                "2:10",
            ),
            (
                "export { spec } from '@lingxia/test';\n",
                "re-exports `spec`",
            ),
            ("export * from '@lingxia/test';\n", "re-exports `spec`"),
            (
                "import { spec } from '@lingxia/test';\nregister(spec);\n",
                "2:10  `spec` is used as a value",
            ),
        ] {
            let err = check(source).unwrap_err().to_string();
            assert!(err.contains(expected), "{source}\n{err}");
        }
        // Types only: nothing to register.
        check("import type * as lt from '@lingxia/test';\nexport type { Fixture } from '@lingxia/test';\n")
            .unwrap();
    }
}
