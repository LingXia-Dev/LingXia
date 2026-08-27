use super::{
    ViewUsageAudit, analyze_script_bindings, downstream_action_usage,
    ensure_forwarded_actions_cover_downstream, ensure_no_direct_lx_usage,
    ensure_used_actions_exist,
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

    // What the entry itself reads off `actions` is what it can forward. Keep it
    // before the downstream set is folded in, so the two can be compared.
    let forwarded = used_actions.clone();

    // The entry can only judge what it reads itself; once the object is handed
    // to a child view, the child files decide what is wired. Scan downstream
    // either way: when the whole `actions` object escapes the children can
    // reach anything, but when the entry re-wraps it into a literal the
    // children can only reach what that literal names — and an omission there
    // is invisible to every other check.
    let (downstream, complete) = downstream_action_usage(project, &source_path);
    let mut unused_reportable = true;
    if analyzer.actions_escaped {
        unused_reportable = complete;
    } else {
        ensure_forwarded_actions_cover_downstream(page_path, &forwarded, &downstream, complete)?;
    }
    used_actions.extend(downstream);
    Ok(ViewUsageAudit {
        used_actions,
        unused_reportable,
    })
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
