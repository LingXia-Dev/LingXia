use super::vite_pipeline::strip_ext;
use crate::lxapp::project::{Project, ProjectKind};
use anyhow::{Context, Result, anyhow, bail};
use oxc_allocator::Allocator;
use oxc_ast::ast::{Expression, ObjectPropertyKind, PropertyKey, Statement};
use oxc_parser::Parser;
use oxc_span::SourceType;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

#[derive(Debug, Default, Clone)]
struct LxAppBuildConfig {
    static_dirs: Vec<String>,
    optional_static_dirs: BTreeSet<String>,
}

pub(super) fn write_root_manifest(project: &Project) -> Result<()> {
    match project.kind {
        ProjectKind::LxApp => {
            let source = project.root.join("lxapp.json");
            let mut value: Value = serde_json::from_str(&fs::read_to_string(&source)?)?;
            let page_map = project
                .pages
                .iter()
                .map(|page| (strip_ext(page), page.clone()))
                .collect::<std::collections::HashMap<_, _>>();

            if let Some(pages) = value.get_mut("pages").and_then(Value::as_array_mut) {
                for page in pages.iter_mut() {
                    if let Some(raw) = page.as_str()
                        && let Some(resolved) = page_map.get(&strip_ext(raw))
                    {
                        *page = Value::String(resolved.clone());
                    }
                }
            }

            if let Some(list) = value
                .get_mut("tabBar")
                .and_then(|value| value.get_mut("list"))
                .and_then(Value::as_array_mut)
            {
                for item in list.iter_mut() {
                    if let Some(page_path) = item.get("pagePath").and_then(Value::as_str)
                        && let Some(resolved) = page_map.get(&strip_ext(page_path))
                        && let Some(object) = item.as_object_mut()
                    {
                        object.insert("pagePath".to_string(), Value::String(resolved.clone()));
                    }
                }
            }

            fs::write(
                project.output_dir.join("lxapp.json"),
                serde_json::to_string_pretty(&value)?,
            )?;
        }
        ProjectKind::LxPlugin => {
            let source = project.root.join("lxplugin.json");
            fs::copy(source, project.output_dir.join("lxplugin.json"))?;
        }
    }
    Ok(())
}

pub(super) fn copy_static_assets(project: &Project) -> Result<()> {
    let build_config = load_lxapp_build_config(project.root.as_path())?;
    for relative_dir in build_config.static_dirs {
        let source_dir = project.root.join(&relative_dir);
        if !source_dir.exists() {
            if build_config.optional_static_dirs.contains(&relative_dir) {
                continue;
            }
            bail!(
                "Configured staticDirs entry does not exist: {}",
                source_dir.display()
            );
        }
        if !source_dir.is_dir() {
            bail!(
                "Configured staticDirs entry is not a directory: {}",
                source_dir.display()
            );
        }
        copy_dir_recursive(&source_dir, &project.output_dir.join(&relative_dir))?;
    }

    let pages_dir = project.root.join("pages");
    if pages_dir.exists() {
        for page_dir in fs::read_dir(&pages_dir)? {
            let page_dir = page_dir?;
            if !page_dir.file_type()?.is_dir() {
                continue;
            }
            let images_dir = page_dir.path().join("images");
            if images_dir.exists() {
                copy_dir_recursive(
                    &images_dir,
                    &project
                        .output_dir
                        .join("pages")
                        .join(page_dir.file_name())
                        .join("images"),
                )?;
            }
        }
    }
    Ok(())
}

pub(super) fn copy_dir_recursive(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&source_path, &destination_path)?;
        } else {
            if let Some(parent) = destination_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&source_path, &destination_path).with_context(|| {
                format!(
                    "Failed to copy {} -> {}",
                    source_path.display(),
                    destination_path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn load_lxapp_build_config(project_root: &Path) -> Result<LxAppBuildConfig> {
    let mut static_dirs = BTreeSet::new();
    let mut optional_static_dirs = BTreeSet::new();
    if project_root.join("public").is_dir() {
        static_dirs.insert("public".to_string());
    } else {
        optional_static_dirs.insert("public".to_string());
    }

    let config_path = project_root.join("lxapp.config.ts");
    if !config_path.exists() {
        return Ok(LxAppBuildConfig {
            static_dirs: static_dirs.into_iter().collect(),
            optional_static_dirs,
        });
    }

    let source = fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read {}", config_path.display()))?;
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(&config_path)
        .map_err(|_| anyhow!("Unsupported lxapp build config {}", config_path.display()))?;
    let parse_result = Parser::new(&allocator, &source, source_type).parse();
    if !parse_result.errors.is_empty() {
        bail!(
            "Failed to parse {}: {}",
            config_path.display(),
            parse_result
                .errors
                .iter()
                .map(|error| format!("{error:?}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    let mut object_expr = None;
    for statement in &parse_result.program.body {
        let Statement::ExportDefaultDeclaration(export_default) = statement else {
            continue;
        };
        object_expr = extract_config_object_expression(export_default.declaration.to_expression());
        if object_expr.is_some() {
            break;
        }
    }

    let Some(config_object) = object_expr else {
        return Ok(LxAppBuildConfig {
            static_dirs: static_dirs.into_iter().collect(),
            optional_static_dirs,
        });
    };

    for property in &config_object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            continue;
        };
        let Some(name) = property_name(&property.key) else {
            continue;
        };
        if name != "staticDirs" {
            continue;
        }
        let Expression::ArrayExpression(array) = unwrap_expression(&property.value) else {
            bail!("lxapp.config.ts staticDirs must be an array of root-relative directory strings");
        };
        for element in &array.elements {
            let expression = match element {
                oxc_ast::ast::ArrayExpressionElement::SpreadElement(_)
                | oxc_ast::ast::ArrayExpressionElement::Elision(_) => {
                    bail!(
                        "lxapp.config.ts staticDirs must contain only root-relative directory strings"
                    );
                }
                _ => unwrap_expression(element.to_expression()),
            };
            let Expression::StringLiteral(value) = expression else {
                bail!(
                    "lxapp.config.ts staticDirs must contain only root-relative directory strings"
                );
            };
            let normalized = normalize_static_dir_entry(value.value.as_str()).ok_or_else(|| {
                anyhow!(
                    "Invalid staticDirs entry in {}: {:?}",
                    config_path.display(),
                    value.value
                )
            })?;
            optional_static_dirs.remove(&normalized);
            static_dirs.insert(normalized);
        }
    }

    Ok(LxAppBuildConfig {
        static_dirs: static_dirs.into_iter().collect(),
        optional_static_dirs,
    })
}

fn extract_config_object_expression<'a>(
    expression: &'a Expression<'a>,
) -> Option<&'a oxc_ast::ast::ObjectExpression<'a>> {
    match unwrap_expression(expression) {
        Expression::ObjectExpression(object) => Some(object),
        Expression::CallExpression(call) => call
            .arguments
            .first()
            .map(|arg| arg.to_expression())
            .and_then(extract_config_object_expression),
        _ => None,
    }
}

fn normalize_static_dir_entry(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed == "."
        || trimmed == ".."
        || trimmed.starts_with("../")
        || trimmed.contains("/../")
        || trimmed.starts_with("//")
        || Path::new(trimmed).is_absolute()
    {
        return None;
    }
    let normalized = trimmed
        .trim_start_matches("./")
        .trim_start_matches('/')
        .trim_end_matches('/')
        .to_string();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lxapp::framework::ProjectFramework;
    use tempfile::tempdir;

    fn write_file(root: &Path, relative: &str, content: &str) {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    fn make_project(root: &Path) -> Project {
        Project {
            root: root.to_path_buf(),
            kind: ProjectKind::LxApp,
            framework: ProjectFramework::Html,
            output_dir: root.join("dist"),
            pages: vec!["pages/home/index.html".to_string()],
            logic_entry: Some("logic.js".to_string()),
            plugin_id: None,
            package_name: Some("demo".to_string()),
            version: "1.0.0".to_string(),
        }
    }

    #[test]
    fn copy_static_assets_preserves_public_dir_in_dist() {
        let temp = tempdir().unwrap();
        let project = make_project(temp.path());
        write_file(
            temp.path(),
            "public/runtime-extra.js",
            "console.log('extra');",
        );

        copy_static_assets(&project).unwrap();

        assert!(project.output_dir.join("public/runtime-extra.js").exists());
        assert!(!project.output_dir.join("runtime-extra.js").exists());
    }

    #[test]
    fn copy_static_assets_respects_configured_static_dirs() {
        let temp = tempdir().unwrap();
        let project = make_project(temp.path());
        write_file(
            temp.path(),
            "lxapp.config.ts",
            r#"export default {
  staticDirs: ['view', 'assets']
};"#,
        );
        write_file(temp.path(), "public/home.png", "home");
        write_file(temp.path(), "view/info-panel.js", "console.log('info');");
        write_file(temp.path(), "assets/logo.svg", "<svg />");

        copy_static_assets(&project).unwrap();

        assert!(project.output_dir.join("public/home.png").exists());
        assert!(project.output_dir.join("view/info-panel.js").exists());
        assert!(project.output_dir.join("assets/logo.svg").exists());
    }

    #[test]
    fn copy_static_assets_errors_when_configured_dir_is_missing() {
        let temp = tempdir().unwrap();
        let project = make_project(temp.path());
        write_file(
            temp.path(),
            "lxapp.config.ts",
            r#"export default {
  staticDirs: ['assets']
};"#,
        );

        let error = copy_static_assets(&project).unwrap_err().to_string();
        assert!(error.contains("Configured staticDirs entry does not exist"));
        assert!(error.contains("assets"));
    }
}
