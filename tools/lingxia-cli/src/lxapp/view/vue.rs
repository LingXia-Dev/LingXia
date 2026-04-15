use super::{
    ViewUsageAudit, analyze_script_bindings, ensure_no_direct_lx_usage, ensure_used_actions_exist,
    line_number_for_offset,
};
use crate::lxapp::framework::PageAction;
use crate::lxapp::project::Project;
use anyhow::{Context, Result, bail};
use oxc_span::SourceType;
use std::collections::HashSet;
use std::fs;

#[derive(Debug, Clone)]
struct VueTemplateExpression {
    raw: String,
    source: String,
    line: usize,
}

#[derive(Debug, Clone)]
struct VueSfcSections {
    script: String,
    script_source_type: SourceType,
    template: String,
}

pub(super) fn validate_vue_bindings(
    project: &Project,
    page_path: &str,
    actions: &[PageAction],
) -> Result<ViewUsageAudit> {
    let source_path = project.root.join(page_path);
    let source = fs::read_to_string(&source_path)
        .with_context(|| format!("Failed to read {}", source_path.display()))?;
    let sections = extract_vue_sfc_sections(&source);
    let defined_actions = actions
        .iter()
        .map(|action| action.name.as_str())
        .collect::<HashSet<_>>();
    let script_analyzer =
        analyze_script_bindings(&sections.script, sections.script_source_type, None)
            .with_context(|| format!("Failed to analyze {}", source_path.display()))?;
    ensure_no_direct_lx_usage(
        page_path,
        &sections.script,
        &script_analyzer.direct_lx_uses,
        "script",
    )?;

    let mut used_actions = script_analyzer.used_actions.clone();
    mark_channel_topic_actions(&sections.script, actions, &mut used_actions);
    for expression in extract_vue_template_expressions(&sections.template) {
        let raw = expression.raw.trim();
        if is_identifier_name(raw) && defined_actions.contains(raw) {
            used_actions.insert(raw.to_string());
            continue;
        }
        if let Some(action_name) = script_analyzer.local_action_aliases.get(raw).cloned() {
            used_actions.insert(action_name);
            continue;
        }
        let template_analyzer = analyze_script_bindings(
            &expression.source,
            SourceType::ts(),
            Some((
                script_analyzer.action_object_aliases.clone(),
                script_analyzer.local_action_aliases.clone(),
            )),
        )
        .with_context(|| format!("Failed to analyze {}", source_path.display()))?;

        if !template_analyzer.direct_lx_uses.is_empty() {
            let members = template_analyzer
                .direct_lx_uses
                .iter()
                .map(|(_, member)| format!("lx.{member}"))
                .collect::<Vec<_>>()
                .join(", ");
            bail!(
                "View {page_path} must not call lx.* directly in template expressions. Move it into Page(...) logic actions. Approx line {}: {}",
                expression.line,
                members
            );
        }
        used_actions.extend(template_analyzer.used_actions);
    }

    ensure_used_actions_exist(page_path, actions, &used_actions)?;
    Ok(ViewUsageAudit { used_actions })
}

fn mark_channel_topic_actions(
    script: &str,
    actions: &[PageAction],
    used_actions: &mut std::collections::BTreeSet<String>,
) {
    if !script.contains("channel.open") {
        return;
    }
    for action in actions {
        let quoted_single = format!("'{}'", action.name);
        let quoted_double = format!("\"{}\"", action.name);
        if script.contains(&quoted_single) || script.contains(&quoted_double) {
            used_actions.insert(action.name.clone());
        }
    }
}

fn extract_vue_sfc_sections(source: &str) -> VueSfcSections {
    let mut script = String::new();
    let mut script_source_type = SourceType::ts();
    let mut cursor = 0;
    while let Some(tag_start_rel) = source[cursor..].find("<script") {
        let tag_start = cursor + tag_start_rel;
        let tag_end = match source[tag_start..].find('>') {
            Some(index) => tag_start + index,
            None => break,
        };
        let attrs = &source[tag_start..=tag_end];
        let close_tag = match source[tag_end + 1..].find("</script>") {
            Some(index) => tag_end + 1 + index,
            None => break,
        };
        let content = &source[tag_end + 1..close_tag];
        if !content.trim().is_empty() {
            if !script.is_empty() {
                script.push('\n');
            }
            script.push_str(content);
        }
        script_source_type = vue_script_source_type(attrs);
        cursor = close_tag + "</script>".len();
    }

    let template = extract_tag_content(source, "template").unwrap_or_default();
    VueSfcSections {
        script,
        script_source_type,
        template,
    }
}

fn extract_vue_template_expressions(template: &str) -> Vec<VueTemplateExpression> {
    let mut expressions = Vec::new();
    let mut cursor = 0;

    while let Some(start_rel) = template[cursor..].find("{{") {
        let start = cursor + start_rel;
        let expr_start = start + 2;
        if let Some(end_rel) = template[expr_start..].find("}}") {
            let end = expr_start + end_rel;
            let expr = template[expr_start..end].trim();
            if !expr.is_empty() {
                expressions.push(VueTemplateExpression {
                    raw: expr.to_string(),
                    source: wrap_template_expression(expr),
                    line: line_number_for_offset(template, expr_start),
                });
            }
            cursor = end + 2;
        } else {
            break;
        }
    }

    cursor = 0;
    let bytes = template.as_bytes();
    let mut in_tag = false;
    let mut quoted_attr: Option<u8> = None;
    while cursor < bytes.len() {
        if let Some(quote) = quoted_attr {
            if bytes[cursor] == quote {
                quoted_attr = None;
            }
            cursor += 1;
            continue;
        }

        match bytes[cursor] {
            b'<' => {
                in_tag = true;
                cursor += 1;
                continue;
            }
            b'>' => {
                in_tag = false;
                cursor += 1;
                continue;
            }
            b'"' | b'\'' => {
                quoted_attr = Some(bytes[cursor]);
                cursor += 1;
                continue;
            }
            _ => {}
        }

        if !in_tag {
            cursor += 1;
            continue;
        }

        let ch = bytes[cursor] as char;
        let at_attribute_boundary = cursor == 0
            || matches!(
                bytes.get(cursor.wrapping_sub(1)).copied(),
                Some(b'<') | Some(b' ') | Some(b'\n') | Some(b'\r') | Some(b'\t')
            );
        let is_directive = ch == '@'
            || ch == ':'
            || (ch == 'v' && matches!(bytes.get(cursor + 1).copied(), Some(b'-')));
        if !is_directive || !at_attribute_boundary {
            cursor += 1;
            continue;
        }

        let name_start = cursor;
        while cursor < bytes.len() {
            let ch = bytes[cursor] as char;
            if ch == '=' || ch.is_whitespace() || ch == '>' {
                break;
            }
            cursor += 1;
        }
        if name_start == cursor {
            cursor += 1;
            continue;
        }
        let name = &template[name_start..cursor];

        while cursor < bytes.len() && (bytes[cursor] as char).is_whitespace() {
            cursor += 1;
        }
        if cursor >= bytes.len() || bytes[cursor] as char != '=' {
            continue;
        }
        cursor += 1;
        while cursor < bytes.len() && (bytes[cursor] as char).is_whitespace() {
            cursor += 1;
        }
        if cursor >= bytes.len() {
            break;
        }
        let quote = bytes[cursor] as char;
        if quote != '"' && quote != '\'' {
            continue;
        }
        let value_start = cursor + 1;
        cursor += 1;
        while cursor < bytes.len() && bytes[cursor] as char != quote {
            cursor += 1;
        }
        if cursor >= bytes.len() {
            break;
        }
        let expr = template[value_start..cursor].trim();
        if !expr.is_empty() {
            let source = if name.starts_with('@') {
                wrap_template_event_expression(expr)
            } else {
                wrap_template_expression(expr)
            };
            expressions.push(VueTemplateExpression {
                raw: expr.to_string(),
                source,
                line: line_number_for_offset(template, value_start),
            });
        }
        cursor += 1;
    }

    expressions
}

fn wrap_template_expression(expression: &str) -> String {
    format!("const __lx_expr__ = ({expression});")
}

fn wrap_template_event_expression(expression: &str) -> String {
    format!("function __lx_event__() {{ {expression}; }}")
}

fn is_identifier_name(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first == '_' || first == '$' || first.is_ascii_alphabetic()) {
        return false;
    }
    chars.all(|ch| ch == '_' || ch == '$' || ch.is_ascii_alphanumeric())
}

fn extract_tag_content(source: &str, tag_name: &str) -> Option<String> {
    let open_tag = format!("<{tag_name}");
    let close_tag = format!("</{tag_name}>");
    let start = source.find(&open_tag)?;
    let tag_end = start + source[start..].find('>')?;
    let content_start = tag_end + 1;
    let end = source.rfind(&close_tag)?;
    if end <= content_start {
        return None;
    }
    Some(source[content_start..end].to_string())
}

fn vue_script_source_type(attrs: &str) -> SourceType {
    let lower = attrs.to_ascii_lowercase();
    let is_ts = lower.contains("lang=\"ts\"")
        || lower.contains("lang='ts'")
        || lower.contains("lang=\"tsx\"")
        || lower.contains("lang='tsx'");
    let is_jsx = lower.contains("lang=\"tsx\"") || lower.contains("lang='tsx'");
    SourceType::mjs().with_typescript(is_ts).with_jsx(is_jsx)
}
