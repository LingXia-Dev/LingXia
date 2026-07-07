//! Native accessibility via `AXUIElement`: a bounded tree dump, a flat query,
//! and atomic match-and-act. Node ids are path-based (`ax:0/2/1`), stable within
//! one process run — which is what the query→act flow needs. Rects are in global
//! points (top-left origin), matching the rest of the backend.

use super::axui::{require_trusted, AxEl};
use super::window_ops::ax_window_for_id;
use crate::error::{Error, Result};
use crate::model::{Ack, AxNode, AxQuery, Rect};

const DEFAULT_MAX_NODES: usize = 2000;

/// Normalize an AX role ("AXButton") to the contract's lowercase form ("button").
fn role_name(role: Option<String>) -> String {
    role.map(|r| r.strip_prefix("AX").unwrap_or(&r).to_lowercase())
        .unwrap_or_default()
}

fn node_rect(el: &AxEl) -> Rect {
    use objc2_core_foundation::{CGPoint, CGSize};
    let pos = el.attr_point("AXPosition").unwrap_or(CGPoint::new(0.0, 0.0));
    let size = el.attr_size("AXSize").unwrap_or(CGSize::new(0.0, 0.0));
    Rect {
        x: pos.x.round() as i32,
        y: pos.y.round() as i32,
        w: size.width.round() as i32,
        h: size.height.round() as i32,
    }
}

fn node_data(el: &AxEl, id: String) -> AxNode {
    let role = role_name(el.attr_string("AXRole"));
    let value = el.attr_string("AXValue").filter(|s| !s.is_empty());
    // Prefer the title, then a description, then a role description as the name.
    let name = el
        .attr_string("AXTitle")
        .filter(|s| !s.is_empty())
        .or_else(|| el.attr_string("AXDescription").filter(|s| !s.is_empty()))
        .or_else(|| el.attr_string("AXRoleDescription").filter(|s| !s.is_empty()))
        .unwrap_or_default();
    AxNode {
        id,
        role,
        name,
        value,
        enabled: el.attr_bool("AXEnabled").unwrap_or(true),
        focused: el.attr_bool("AXFocused").unwrap_or(false),
        rect: node_rect(el),
        children: Vec::new(),
    }
}

fn build_tree(
    el: &AxEl,
    id: String,
    depth: Option<u32>,
    count: &mut usize,
    max: usize,
) -> AxNode {
    let mut node = node_data(el, id.clone());
    *count += 1;
    if depth == Some(0) || *count >= max {
        return node;
    }
    for (idx, child) in el.children().into_iter().enumerate() {
        if *count >= max {
            break;
        }
        node.children.push(build_tree(
            &child,
            format!("{id}/{idx}"),
            depth.map(|d| d - 1),
            count,
            max,
        ));
    }
    node
}

fn collect_flat(el: &AxEl, id: String, count: &mut usize, max: usize, out: &mut Vec<(AxNode, AxEl)>) {
    if *count >= max {
        return;
    }
    let node = node_data(el, id.clone());
    *count += 1;
    let children = el.children();
    out.push((node, el.clone_ref()));
    for (idx, child) in children.into_iter().enumerate() {
        if *count >= max {
            break;
        }
        collect_flat(&child, format!("{id}/{idx}"), count, max, out);
    }
}

pub fn tree(window_id: &str, depth: Option<u32>, max_nodes: Option<usize>) -> Result<AxNode> {
    require_trusted()?;
    let root = ax_window_for_id(window_id)?;
    let mut count = 0;
    Ok(build_tree(
        &root,
        "ax:0".to_string(),
        depth,
        &mut count,
        max_nodes.unwrap_or(DEFAULT_MAX_NODES),
    ))
}

pub fn hit_test(x: i32, y: i32) -> Result<AxNode> {
    require_trusted()?;
    let sys = AxEl::system_wide()?;
    let el = sys.element_at(x as f32, y as f32)?;
    Ok(node_data(&el, format!("ax:@{x},{y}")))
}

fn flat(window_id: &str) -> Result<Vec<(AxNode, AxEl)>> {
    require_trusted()?;
    let root = ax_window_for_id(window_id)?;
    let mut out = Vec::new();
    let mut count = 0;
    collect_flat(&root, "ax:0".to_string(), &mut count, DEFAULT_MAX_NODES, &mut out);
    Ok(out)
}

pub fn query(window_id: &str, q: &AxQuery, all: bool, index: Option<usize>) -> Result<Vec<AxNode>> {
    let nodes: Vec<AxNode> = flat(window_id)?
        .into_iter()
        .map(|(n, _)| n)
        .filter(|n| q.is_empty() || q.matches(n))
        .collect();
    if all {
        return Ok(nodes);
    }
    if let Some(i) = index {
        return nodes
            .into_iter()
            .nth(i)
            .map(|n| vec![n])
            .ok_or_else(|| Error::NotFound(format!("no node at --index {i}")));
    }
    match nodes.len() {
        0 => Err(Error::NotFound("no accessibility node matched".into())),
        1 => Ok(nodes),
        n => Err(Error::Ambiguous(format!(
            "{n} nodes matched; pass --all, --index, or a narrower query"
        ))),
    }
}

/// Resolve exactly one live element for an action (atomic match-and-act).
fn resolve_one(window_id: &str, q: &AxQuery) -> Result<AxEl> {
    let mut matches: Vec<AxEl> = flat(window_id)?
        .into_iter()
        .filter(|(n, _)| q.is_empty() || q.matches(n))
        .map(|(_, el)| el)
        .collect();
    match matches.len() {
        0 => Err(Error::NotFound("no accessibility node matched".into())),
        1 => Ok(matches.remove(0)),
        n => Err(Error::Ambiguous(format!("{n} nodes matched; narrow the query"))),
    }
}

pub fn invoke(window_id: &str, q: &AxQuery) -> Result<Ack> {
    resolve_one(window_id, q)?.perform("AXPress")?;
    Ok(Ack::new("ax.invoke"))
}

pub fn focus(window_id: &str, q: &AxQuery) -> Result<Ack> {
    resolve_one(window_id, q)?.set_bool("AXFocused", true)?;
    Ok(Ack::new("ax.focus"))
}

pub fn set_value(window_id: &str, q: &AxQuery, value: &str) -> Result<Ack> {
    resolve_one(window_id, q)?.set_string("AXValue", value)?;
    Ok(Ack::new("ax.set-value"))
}

pub fn select(window_id: &str, q: &AxQuery) -> Result<Ack> {
    resolve_one(window_id, q)?.set_bool("AXSelected", true)?;
    Ok(Ack::new("ax.select"))
}

pub fn expand(window_id: &str, q: &AxQuery) -> Result<Ack> {
    resolve_one(window_id, q)?.set_bool("AXDisclosing", true)?;
    Ok(Ack::new("ax.expand"))
}

pub fn collapse(window_id: &str, q: &AxQuery) -> Result<Ack> {
    resolve_one(window_id, q)?.set_bool("AXDisclosing", false)?;
    Ok(Ack::new("ax.collapse"))
}

pub fn scroll_into_view(window_id: &str, q: &AxQuery) -> Result<Ack> {
    resolve_one(window_id, q)?.perform("AXScrollToVisible")?;
    Ok(Ack::new("ax.scroll-into-view"))
}

/// Poll the tree until a node matching `q` reaches the requested state, or time
/// out (exit 5). States: exists (default) / gone / enabled / focused.
pub fn wait(window_id: &str, q: &AxQuery, state: &str, timeout_ms: u64) -> Result<Ack> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
    loop {
        let nodes: Vec<AxNode> = flat(window_id)
            .unwrap_or_default()
            .into_iter()
            .map(|(n, _)| n)
            .filter(|n| q.matches(n))
            .collect();
        let satisfied = match state {
            "gone" => nodes.is_empty(),
            "enabled" => nodes.iter().any(|n| n.enabled),
            "focused" => nodes.iter().any(|n| n.focused),
            _ => !nodes.is_empty(),
        };
        if satisfied {
            return Ok(Ack::new(format!("ax.wait:{state}")));
        }
        if std::time::Instant::now() >= deadline {
            return Err(Error::Timeout(format!(
                "timed out waiting for ax {state} on '{}'",
                describe(q)
            )));
        }
        std::thread::sleep(std::time::Duration::from_millis(120));
    }
}

fn describe(q: &AxQuery) -> String {
    q.name
        .clone()
        .or_else(|| q.role.clone())
        .or_else(|| q.text.clone())
        .or_else(|| q.id.clone())
        .unwrap_or_default()
}
