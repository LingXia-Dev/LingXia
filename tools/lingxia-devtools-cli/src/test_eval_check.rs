//! Bundle-time check of the functions a spec hands to `logic.eval` and
//! `view.eval`.
//!
//! The fixture sends such a function as source text and runs it in the app's
//! Logic runtime or a page WebView, so a name it reads from the spec file (a
//! `const`, an import, the fixture `t`) does not exist there. At run time that
//! is a `ReferenceError` at best; at worst the target has a global of the same
//! name (`name`, `status`, `location` in a WebView) and the function silently
//! reads that instead. Semantic analysis tells the two apart before the run:
//! a reference that resolves to a binding declared outside the function is a
//! capture. Unresolved references are globals and are left to the target.

use std::collections::HashMap;

use anyhow::{Result, bail};
use oxc_ast::AstKind;
use oxc_ast::ast::{
    Argument, BindingPattern, CallExpression, Expression, FormalParameters, IdentifierReference,
    ImportDeclarationSpecifier, Program, Statement, TSType, TSTypeName,
};
use oxc_ast_visit::{Visit, walk};
use oxc_semantic::{Semantic, SemanticBuilder};
use oxc_span::{GetSpan, Span};

use crate::test_bundle::line_column;

/// Where an eval function runs, as its callee names it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
    Logic,
    View,
}

impl Target {
    fn place(self) -> &'static str {
        match self {
            Target::Logic => "the app's Logic runtime",
            Target::View => "the page WebView",
        }
    }

    fn scope(self) -> &'static str {
        match self {
            Target::Logic => "{ lx }",
            Target::View => "{ document }",
        }
    }
}

struct Capture {
    offset: u32,
    callee: String,
    name: String,
    target: Target,
}

/// Fail when an eval function in `program` closes over a spec-side binding.
pub(crate) fn check_eval_functions(
    program: &Program<'_>,
    source: &str,
    display: &str,
) -> Result<()> {
    let semantic = SemanticBuilder::new()
        .with_build_nodes(true)
        .build(program)
        .semantic;
    let mut finder = EvalFinder {
        semantic: &semantic,
        source,
        captures: Vec::new(),
        roots: HashMap::new(),
    };
    finder.find_roots(program);
    finder.visit_program(program);
    if finder.captures.is_empty() {
        return Ok(());
    }
    let mut out = Vec::new();
    for capture in &finder.captures {
        let (line, column) = line_column(source, capture.offset as usize);
        out.push(format!(
            "{display}:{line}:{column}  {callee}(fn) closes over `{name}`, which does not exist in {place}.\n  \
Pass it as an argument: {callee}(({scope}, {name}) => …, {name})",
            callee = capture.callee,
            name = capture.name,
            place = capture.target.place(),
            scope = capture.target.scope(),
        ));
    }
    bail!(
        "{}\nAn eval function is sent as source text and runs in the app, not in the spec.",
        out.join("\n")
    )
}

struct EvalFinder<'s, 'a> {
    semantic: &'s Semantic<'a>,
    source: &'s str,
    captures: Vec<Capture>,
    roots: HashMap<oxc_semantic::SymbolId, Origin>,
}

impl<'a> Visit<'a> for EvalFinder<'_, 'a> {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        self.check_call(call);
        walk::walk_call_expression(self, call);
    }
}

impl<'a> EvalFinder<'_, 'a> {
    fn check_call(&mut self, call: &CallExpression<'a>) {
        let Expression::StaticMemberExpression(member) = call.callee.get_inner_expression() else {
            return;
        };
        if member.property.name != "eval" {
            return;
        }
        let Some(target) = self.classify(&member.object, 0) else {
            return;
        };
        // `eval(fn, ...args)` or `eval(options, fn, ...args)`.
        let mut arguments = call.arguments.iter().filter_map(Argument::as_expression);
        let first = arguments.next().map(Expression::get_inner_expression);
        let function = match first {
            Some(Expression::ObjectExpression(_)) => {
                arguments.next().map(Expression::get_inner_expression)
            }
            other => other,
        };
        let Some(function) = function.and_then(|expr| self.function_span(expr)) else {
            return;
        };
        let callee = self
            .source
            .get(call.callee.span().start as usize..call.callee.span().end as usize)
            .unwrap_or("eval")
            .split_whitespace()
            .collect::<String>();
        let scoping = self.semantic.scoping();
        let mut seen = Vec::<String>::new();
        for (ident_span, name, reference_id) in references_within(self.semantic, function) {
            let reference = scoping.get_reference(reference_id);
            let flags = reference.flags();
            // Type positions (`as Todo[]`, `typeof x` in a type) are erased.
            if !flags.is_read() && !flags.is_write() || flags.is_value_as_type() {
                continue;
            }
            let Some(symbol) = reference.symbol_id() else {
                continue; // A global: the target has its own.
            };
            let declared = scoping.symbol_span(symbol);
            if function.start <= declared.start && declared.end <= function.end {
                continue;
            }
            if seen.contains(&name) {
                continue;
            }
            seen.push(name.clone());
            self.captures.push(Capture {
                offset: ident_span.start,
                callee: callee.clone(),
                name,
                target,
            });
        }
    }

    /// The span of the function an argument passes: an inline function, or
    /// one the file declares and passes by name.
    fn function_span(&self, expr: &Expression<'a>) -> Option<Span> {
        match expr {
            Expression::ArrowFunctionExpression(arrow) => Some(arrow.span),
            Expression::FunctionExpression(function) => Some(function.span),
            Expression::Identifier(ident) => {
                let symbol = self.symbol_of(ident)?;
                match self.semantic.symbol_declaration(symbol).kind() {
                    AstKind::Function(function) => Some(function.span),
                    AstKind::VariableDeclarator(declarator) => match declarator
                        .init
                        .as_ref()
                        .map(Expression::get_inner_expression)
                    {
                        Some(Expression::ArrowFunctionExpression(arrow)) => Some(arrow.span),
                        Some(Expression::FunctionExpression(function)) => Some(function.span),
                        _ => None,
                    },
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn find_roots(&mut self, program: &Program<'a>) {
        let mut types = HashMap::new();
        for statement in &program.body {
            let Statement::ImportDeclaration(import) = statement else {
                continue;
            };
            if import.source.value != "@lingxia/test" {
                continue;
            }
            for specifier in import.specifiers.iter().flatten() {
                if let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier
                    && let Some(name) = crate::test_bundle::module_export_name(&specifier.imported)
                    && let Some(origin) = Origin::from_type(&name)
                    && let Some(symbol) = specifier.local.symbol_id.get()
                {
                    types.insert(symbol, origin);
                }
                let (local, origin) = match specifier {
                    ImportDeclarationSpecifier::ImportSpecifier(specifier)
                        if crate::test_bundle::module_export_name(&specifier.imported)
                            .as_deref()
                            == Some("spec") =>
                    {
                        (&specifier.local, Origin::Spec)
                    }
                    ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                        (&specifier.local, Origin::Module)
                    }
                    _ => continue,
                };
                if let Some(symbol) = local.symbol_id.get() {
                    self.roots.insert(symbol, origin);
                }
            }
        }
        // Typed helpers are fixture entrypoints too; resolve the imported type's
        // symbol so a same-named application type never makes this check fire.
        for node in self.semantic.nodes().iter() {
            let AstKind::FormalParameter(parameter) = node.kind() else {
                continue;
            };
            let Some(annotation) = &parameter.type_annotation else {
                continue;
            };
            let TSType::TSTypeReference(reference) = &annotation.type_annotation else {
                continue;
            };
            let origin = match &reference.type_name {
                TSTypeName::IdentifierReference(ident) => self
                    .symbol_of(ident)
                    .and_then(|symbol| types.get(&symbol).copied()),
                TSTypeName::QualifiedName(name) => match &name.left {
                    TSTypeName::IdentifierReference(ident)
                        if self
                            .symbol_of(ident)
                            .and_then(|symbol| self.roots.get(&symbol))
                            == Some(&Origin::Module) =>
                    {
                        Origin::from_type(name.right.name.as_str())
                    }
                    _ => None,
                },
                _ => None,
            };
            if let Some(origin) = origin {
                collect_bindings(&parameter.pattern, origin, &mut self.roots);
            }
        }
        // Discover callback bindings before checking bodies, including named
        // callbacks declared earlier in the file. Names alone prove nothing.
        for node in self.semantic.nodes().iter() {
            let AstKind::CallExpression(call) = node.kind() else {
                continue;
            };
            if self.origin(&call.callee, 0) != Some(Origin::Spec) {
                continue;
            }
            let Some(parameters) = call
                .arguments
                .iter()
                .rev()
                .filter_map(Argument::as_expression)
                .find_map(|expr| self.parameters(expr, 0))
            else {
                continue;
            };
            if let Some(parameter) = parameters.items.first() {
                let mut bindings = HashMap::new();
                collect_bindings(&parameter.pattern, Origin::Fixture, &mut bindings);
                self.roots.extend(bindings);
            }
        }
    }

    fn parameters<'e>(
        &'e self,
        expr: &'e Expression<'a>,
        depth: u8,
    ) -> Option<&'e FormalParameters<'a>> {
        if depth > 16 {
            return None;
        }
        match expr.get_inner_expression() {
            Expression::ArrowFunctionExpression(function) => Some(&function.params),
            Expression::FunctionExpression(function) => Some(&function.params),
            Expression::Identifier(ident) => match self
                .semantic
                .symbol_declaration(self.symbol_of(ident)?)
                .kind()
            {
                AstKind::Function(function) => Some(&function.params),
                AstKind::VariableDeclarator(declaration) => {
                    self.parameters(declaration.init.as_ref()?, depth + 1)
                }
                _ => None,
            },
            _ => None,
        }
    }

    fn classify(&self, object: &Expression<'a>, depth: u8) -> Option<Target> {
        match self.origin(object, depth)? {
            Origin::Logic => Some(Target::Logic),
            Origin::View => Some(Target::View),
            _ => None,
        }
    }

    fn origin(&self, expression: &Expression<'a>, depth: u8) -> Option<Origin> {
        if depth > 16 {
            return None;
        }
        match expression.get_inner_expression() {
            Expression::AwaitExpression(value) => self.origin(&value.argument, depth + 1),
            Expression::StaticMemberExpression(member) => self
                .origin(&member.object, depth + 1)?
                .member(member.property.name.as_str()),
            Expression::CallExpression(call) => {
                let Expression::StaticMemberExpression(member) = call.callee.get_inner_expression()
                else {
                    return None;
                };
                match (
                    self.origin(&member.object, depth + 1)?,
                    member.property.name.as_str(),
                ) {
                    (Origin::App, "page") => Some(Origin::Page),
                    (Origin::Automation, "lxapp") => Some(Origin::App),
                    _ => None,
                }
            }
            Expression::Identifier(ident) => {
                let symbol = self.symbol_of(ident)?;
                // A reassigned alias no longer proves where an eval runs.
                if self
                    .semantic
                    .scoping()
                    .get_resolved_references(symbol)
                    .any(|reference| reference.is_write())
                {
                    return None;
                }
                if let Some(origin) = self.roots.get(&symbol) {
                    return Some(*origin);
                }
                let AstKind::VariableDeclarator(declaration) =
                    self.semantic.symbol_declaration(symbol).kind()
                else {
                    return None;
                };
                let origin = self.origin(declaration.init.as_ref()?, depth + 1)?;
                let mut bindings = HashMap::new();
                collect_bindings(&declaration.id, origin, &mut bindings);
                bindings.get(&symbol).copied()
            }
            _ => None,
        }
    }

    fn symbol_of(&self, ident: &IdentifierReference<'a>) -> Option<oxc_semantic::SymbolId> {
        let reference = ident.reference_id.get()?;
        self.semantic.scoping().get_reference(reference).symbol_id()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Origin {
    Module,
    Spec,
    Fixture,
    Automation,
    App,
    Page,
    Logic,
    View,
}

impl Origin {
    fn from_type(name: &str) -> Option<Self> {
        match name {
            "Fixture" => Some(Self::Fixture),
            "TestAutomation" => Some(Self::Automation),
            "TestApp" => Some(Self::App),
            "TestPage" => Some(Self::Page),
            "TestLogic" => Some(Self::Logic),
            "TestView" | "BoundTestView" => Some(Self::View),
            _ => None,
        }
    }

    fn member(self, name: &str) -> Option<Self> {
        match (self, name) {
            (Self::Module, "spec") => Some(Self::Spec),
            (
                Self::Spec,
                "only" | "skip" | "fail" | "fixme" | "beforeEach" | "afterEach" | "reset",
            ) => Some(Self::Spec),
            (Self::Fixture, "app") => Some(Self::App),
            (Self::Fixture, "automation") => Some(Self::Automation),
            (Self::App, "logic") => Some(Self::Logic),
            (Self::App | Self::Page, "view") => Some(Self::View),
            _ => None,
        }
    }
}

fn collect_bindings(
    pattern: &BindingPattern<'_>,
    origin: Origin,
    out: &mut HashMap<oxc_semantic::SymbolId, Origin>,
) {
    match pattern {
        BindingPattern::BindingIdentifier(ident) => {
            if let Some(symbol) = ident.symbol_id.get() {
                out.insert(symbol, origin);
            }
        }
        BindingPattern::ObjectPattern(object) => {
            for property in &object.properties {
                if property.computed
                    && !matches!(property.key, oxc_ast::ast::PropertyKey::StringLiteral(_))
                {
                    continue;
                }
                if let Some(name) = property.key.static_name()
                    && let Some(child) = origin.member(&name)
                {
                    collect_bindings(&property.value, child, out);
                }
            }
        }
        _ => {}
    }
}

/// Every identifier reference inside `span`, in source order.
fn references_within(
    semantic: &Semantic<'_>,
    span: Span,
) -> Vec<(Span, String, oxc_semantic::ReferenceId)> {
    semantic
        .nodes()
        .iter()
        .filter_map(|node| match node.kind() {
            AstKind::IdentifierReference(ident)
                if span.start <= ident.span.start && ident.span.end <= span.end =>
            {
                Some((
                    ident.span,
                    ident.name.to_string(),
                    ident.reference_id.get()?,
                ))
            }
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxc_allocator::Allocator;
    use oxc_parser::Parser;
    use oxc_span::SourceType;

    pub(super) fn check(source: &str) -> Result<()> {
        let source = format!("import {{ spec }} from '@lingxia/test';{source}");
        let source = source.as_str();
        let allocator = Allocator::default();
        let program = Parser::new(&allocator, source, SourceType::ts())
            .parse()
            .program;
        check_eval_functions(&program, source, "tests/a.test.ts")
    }

    pub(super) fn error(source: &str) -> String {
        format!("{:#}", check(source).expect_err("the capture is refused"))
    }

    #[test]
    fn a_captured_const_fails_with_its_position_and_name() {
        let message = error(
            "const name = 'Ada';\nspec('x', async (t) => {\n  await t.app.view.eval(({ document }) => document.title === name);\n});\n",
        );
        assert!(message.contains("tests/a.test.ts:3:"), "{message}");
        assert!(
            message.contains("t.app.view.eval(fn) closes over `name`"),
            "{message}"
        );
        assert!(message.contains("the page WebView"), "{message}");
        assert!(
            message.contains("t.app.view.eval(({ document }, name) => …, name)"),
            "{message}"
        );
    }

    #[test]
    fn imports_the_fixture_and_helpers_are_captures_too() {
        let message = error(
            "import { expect } from '@lingxia/test';\nimport { helper } from './helper';\nspec('x', async (t) => {\n  await t.app.logic.eval(() => { expect(1); helper(); return t; });\n});\n",
        );
        for name in ["expect", "helper", "t"] {
            assert!(
                message.contains(&format!("closes over `{name}`")),
                "{message}"
            );
        }
        assert!(message.contains("the app's Logic runtime"), "{message}");
    }

    #[test]
    fn arguments_locals_globals_and_types_pass() {
        check(
            "type Todo = { id: string };\nconst id = 'x';\nspec('x', async (t) => {\n  await t.app.logic.eval(({ getCurrentPages }, wanted: string) => {\n    const pages = getCurrentPages() as unknown as Todo[];\n    const found = pages.filter((page) => page.id === wanted);\n    return JSON.stringify(found) + Math.max(1, 2) + String(setTimeout);\n  }, id);\n  await t.app.view.eval({ page: 'home', timeout: 10 }, ({ document }) => document.title);\n});\n",
        )
        .unwrap();
    }

    #[test]
    fn page_views_saved_views_and_named_functions_are_checked() {
        let message = error(
            "const label = 'x';\nfunction read({ document }: any) { return document.title === label; }\nspec('x', async (t) => {\n  const todo = (await t.app.page({ name: 'todo' })).view;\n  await todo.eval(read);\n  const { logic } = t.app;\n  await logic.eval(() => label);\n});\n",
        );
        assert!(
            message.contains("todo.eval(fn) closes over `label`"),
            "{message}"
        );
        assert!(
            message.contains("logic.eval(fn) closes over `label`"),
            "{message}"
        );
    }

    #[test]
    fn other_evals_are_not_fixture_evals() {
        check(
            "const script = '1';\nspec('x', async (t) => {\n  await rawAutomation().lxapp().eval({ script });\n  await t.automation.browser.eval({ js: script });\n  const other = { eval: (fn: () => string) => fn() };\n  other.eval(() => script);\n});\n",
        )
        .unwrap();
    }
    #[test]
    fn unrelated_logic_view_and_shadowed_fixtures_are_not_remote() {
        check("const captured = 1; const logic = { eval: f => f() }; logic.eval(() => captured); const other = { view: logic }; other.view.eval(() => captured); function helper(spec) { spec('local', t => t.app.logic.eval(() => captured)); }").unwrap();
    }

    #[test]
    fn renamed_nested_bindings_and_named_callbacks_are_remote() {
        let message = error(
            "const captured = 1; function body({ app: { view: screen } }) { const alias = screen; alias.eval(() => captured); } spec.only('remote', body);",
        );
        assert!(
            message.contains("alias.eval(fn) closes over `captured`"),
            "{message}"
        );
    }

    #[test]
    fn import_aliases_and_hooks_retain_the_fixture_origin() {
        let message = error(
            "import * as tests from '@lingxia/test'; const captured = 1; const check = tests.spec; check.beforeEach(({ app }) => { app.logic.eval(() => captured); });",
        );
        assert!(
            message.contains("app.logic.eval(fn) closes over `captured`"),
            "{message}"
        );
    }

    #[test]
    fn a_computed_business_key_does_not_prove_a_fixture_member() {
        check("const key = 'logic'; const captured = 1; spec('x', t => { const { [key]: logic } = t.app; logic.eval(() => captured); });").unwrap();
    }
    #[test]
    fn automation_selectors_and_reset_hooks_retain_the_fixture_origin() {
        let message = error(
            "const captured = 1; spec('other', async t => { await t.automation.lxapp('other').view.eval(() => captured); const { automation } = t; const app = automation.lxapp(); const { logic: runtime } = app; await runtime.eval(() => captured); }); spec.reset(async ({ app }) => { await app.logic.eval(() => captured); });",
        );
        for callee in [
            "t.automation.lxapp('other').view.eval",
            "runtime.eval",
            "app.logic.eval",
        ] {
            assert!(
                message.contains(&format!("{callee}(fn) closes over `captured`")),
                "{message}"
            );
        }
    }

    #[test]
    fn fixed_page_handles_keep_remote_eval_provenance() {
        let message = error(
            "const captured = 1; spec('page', async t => { const page = await t.app.page({ name: 'home' }); page.view.eval(() => captured); });",
        );
        assert!(
            message.contains("page.view.eval(fn) closes over `captured`"),
            "{message}"
        );
    }

    #[test]
    fn business_selectors_and_raw_automation_are_not_fixture_origins() {
        check("const captured = 1; const other = { automation: { lxapp: () => ({ logic: { eval: fn => fn() } }) } }; other.automation.lxapp().logic.eval(() => captured); spec('raw', t => { t.automation.browser.eval({ js: String(captured) }); });").unwrap();
    }
}

#[cfg(test)]
mod helper_tests {
    use super::tests::{check, error};

    #[test]
    fn typed_helpers_keep_the_eval_boundary() {
        for (import, parameter, call) in [
            (
                "import type { TestApp } from '@lingxia/test';",
                "app: TestApp",
                "app.logic",
            ),
            (
                "import type { TestApp as App } from '@lingxia/test';",
                "app: App",
                "app.view",
            ),
            (
                "import type * as test from '@lingxia/test';",
                "app: test.TestPage",
                "app.view",
            ),
            (
                "import type { TestApp } from '@lingxia/test';",
                "{ logic }: TestApp",
                "logic",
            ),
        ] {
            let source = format!(
                "{import} const captured = 42; async function helper({parameter}) {{ await {call}.eval(() => captured); }}"
            );
            assert!(error(&source).contains("closes over `captured`"));
        }
        check("interface TestApp { logic: any } const captured = 42; function helper(app: TestApp) { app.logic.eval(() => captured); }").unwrap();
        check("import type { TestApp } from '@lingxia/test'; const captured = 42; function helper(app: TestApp) { app.logic.eval((_, value) => value, captured); }").unwrap();
    }
}
