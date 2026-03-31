mod vite;

use crate::lxapp::framework::{PageAction, PageActionMode, ProjectFramework};
use crate::lxapp::options::BuildOptions;
use crate::lxapp::project::Project;
use anyhow::{Result, anyhow, bail};
use indicatif::ProgressBar;
use oxc_allocator::Allocator;
use oxc_ast::ast::{Expression, ObjectPropertyKind, PropertyKey, Statement};
use oxc_parser::Parser;
use oxc_span::SourceType;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

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
) -> Result<ViewBuildReport> {
    vite::build(project, options, progress)
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
            .get("navigationBarTitleText")
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

pub(crate) fn extract_page_actions(logic_path: Option<&Path>) -> Result<Vec<PageAction>> {
    let Some(logic_path) = logic_path else {
        return Ok(Vec::new());
    };
    let source = fs::read_to_string(logic_path)?;
    let source_type = SourceType::from_path(logic_path)
        .map_err(|_| anyhow!("Unsupported page logic file {}", logic_path.display()))?;
    let allocator = Allocator::default();
    let parse_result = Parser::new(&allocator, &source, source_type).parse();
    if !parse_result.errors.is_empty() {
        bail!("Failed to parse page logic {}", logic_path.display());
    }

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
            if name == "data" || name.starts_with('_') || is_page_lifecycle(&name) {
                continue;
            }
            actions.push(PageAction {
                name,
                mode: infer_property_mode(property),
            });
        }
        return Ok(actions);
    }

    Ok(Vec::new())
}

pub(crate) fn render_page_bridge_runtime_module() -> String {
    String::from(
        "// Auto-generated by lingxia-cli. Do not edit.\n\
export function __lx_filter_payload(name, args) {\n\
  const clean = [];\n\
  for (let i = 0; i < args.length; i += 1) {\n\
    const value = args[i];\n\
    if (value instanceof Event) continue;\n\
    if (value && typeof value === \"object\" && typeof value.stopPropagation === \"function\") continue;\n\
    clean.push(value);\n\
  }\n\
  if (clean.length > 1) {\n\
    throw new Error(`Page action '${name}' accepts at most one payload argument`);\n\
  }\n\
  return clean.length === 0 ? undefined : clean[0];\n\
}\n\
\n\
export function __lx_define_page_bridge(name, mode) {\n\
  function fn(...args) {\n\
    const payload = __lx_filter_payload(name, args);\n\
    if (mode === 'stream') {\n\
      return window.LingXiaBridge.callStream(name, payload);\n\
    }\n\
    if (mode === 'call') {\n\
      return window.LingXiaBridge.call(name, payload);\n\
    }\n\
    window.LingXiaBridge.notify(name, payload);\n\
  }\n\
  fn.__logicFunc = true;\n\
  fn.__funcName = name;\n\
  fn.__bridgeMode = mode;\n\
  return fn;\n\
}\n",
    )
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
    format!(
        "window.__pageBridge = {{ __names: {} }};",
        serde_json::to_string(&names).unwrap_or_else(|_| "[]".to_string())
    )
}

fn infer_property_mode(property: &oxc_ast::ast::ObjectProperty<'_>) -> PageActionMode {
    if property.method {
        if let Expression::FunctionExpression(function) = unwrap_expression(&property.value) {
            return function_mode(function.r#async, function.generator);
        }
        return PageActionMode::Notify;
    }

    match unwrap_expression(&property.value) {
        Expression::FunctionExpression(function) => {
            function_mode(function.r#async, function.generator)
        }
        Expression::ArrowFunctionExpression(function) => function_mode(function.r#async, false),
        _ => PageActionMode::Notify,
    }
}

fn function_mode(is_async: bool, is_generator: bool) -> PageActionMode {
    if is_generator {
        PageActionMode::Stream
    } else if is_async {
        PageActionMode::Call
    } else {
        PageActionMode::Notify
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

fn is_page_lifecycle(name: &str) -> bool {
    matches!(
        name,
        "onLoad"
            | "onShow"
            | "onReady"
            | "onHide"
            | "onUnload"
            | "onPullDownRefresh"
            | "onReachBottom"
            | "onShareAppMessage"
            | "onPageScroll"
            | "onResize"
            | "onTabItemTap"
    )
}
