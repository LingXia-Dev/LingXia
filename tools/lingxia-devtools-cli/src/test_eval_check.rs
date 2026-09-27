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

use anyhow::{Result, bail};
use oxc_ast::AstKind;
use oxc_ast::ast::{
    Argument, BindingPattern, CallExpression, Expression, IdentifierReference, Program,
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
    };
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

    /// Whether `object.eval(…)` is a fixture Logic or View eval: `….logic`,
    /// `….view`, `….view.page(name)`, or a `const` holding one of those.
    fn classify(&self, object: &Expression<'a>, depth: u8) -> Option<Target> {
        if depth > 8 {
            return None;
        }
        match object.get_inner_expression() {
            Expression::StaticMemberExpression(member) => match member.property.name.as_str() {
                "logic" => Some(Target::Logic),
                "view" => Some(Target::View),
                _ => None,
            },
            Expression::CallExpression(call) => {
                let Expression::StaticMemberExpression(member) = call.callee.get_inner_expression()
                else {
                    return None;
                };
                (member.property.name == "page"
                    && self.classify(&member.object, depth + 1) == Some(Target::View))
                .then_some(Target::View)
            }
            Expression::Identifier(ident) => {
                if let Some(symbol) = self.symbol_of(ident)
                    && let AstKind::VariableDeclarator(declarator) =
                        self.semantic.symbol_declaration(symbol).kind()
                    && matches!(declarator.id, BindingPattern::BindingIdentifier(_))
                    && let Some(init) = &declarator.init
                {
                    return self.classify(init, depth + 1);
                }
                // A destructured `const { logic, view } = t.app`.
                match ident.name.as_str() {
                    "logic" => Some(Target::Logic),
                    "view" => Some(Target::View),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn symbol_of(&self, ident: &IdentifierReference<'a>) -> Option<oxc_semantic::SymbolId> {
        let reference = ident.reference_id.get()?;
        self.semantic.scoping().get_reference(reference).symbol_id()
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

    fn check(source: &str) -> Result<()> {
        let allocator = Allocator::default();
        let program = Parser::new(&allocator, source, SourceType::ts())
            .parse()
            .program;
        check_eval_functions(&program, source, "tests/a.test.ts")
    }

    fn error(source: &str) -> String {
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
            "const label = 'x';\nfunction read({ document }: any) { return document.title === label; }\nspec('x', async (t) => {\n  const todo = t.app.view.page('todo');\n  await todo.eval(read);\n  const { logic } = t.app;\n  await logic.eval(() => label);\n});\n",
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
}
