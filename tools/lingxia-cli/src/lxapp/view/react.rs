use super::{
    ViewUsageAudit, analyze_script_bindings, ensure_no_direct_lx_usage, ensure_used_actions_exist,
};
use crate::lxapp::framework::PageAction;
use crate::lxapp::project::Project;
use anyhow::{Context, Result, anyhow};
use oxc_span::SourceType;
use std::fs;

pub(super) fn validate_react_bindings(
    project: &Project,
    page_path: &str,
    actions: &[PageAction],
) -> Result<ViewUsageAudit> {
    let source_path = project.root.join(page_path);
    let source = fs::read_to_string(&source_path)
        .with_context(|| format!("Failed to read {}", source_path.display()))?;
    let source_type = SourceType::from_path(&source_path)
        .map_err(|_| anyhow!("Unsupported view file {}", source_path.display()))?;
    let analyzer = analyze_script_bindings(&source, source_type, None)
        .with_context(|| format!("Failed to analyze {}", source_path.display()))?;
    ensure_no_direct_lx_usage(page_path, &source, &analyzer.direct_lx_uses, "script")?;
    let mut used_actions = analyzer.used_actions;
    mark_channel_topic_actions(&source, actions, &mut used_actions);
    ensure_used_actions_exist(page_path, actions, &used_actions)?;
    Ok(ViewUsageAudit { used_actions })
}

fn mark_channel_topic_actions(
    source: &str,
    actions: &[PageAction],
    used_actions: &mut std::collections::BTreeSet<String>,
) {
    if !source.contains("channel.open") {
        return;
    }
    for action in actions {
        let quoted_single = format!("'{}'", action.name);
        let quoted_double = format!("\"{}\"", action.name);
        if source.contains(&quoted_single) || source.contains(&quoted_double) {
            used_actions.insert(action.name.clone());
        }
    }
}
