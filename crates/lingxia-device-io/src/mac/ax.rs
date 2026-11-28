//! Native accessibility via `AXUIElement`: a bounded tree dump, a flat query,
//! and atomic match-and-act. Node ids are path-based (`ax:0/2/1`), stable within
//! one process run — which is what the query→act flow needs. Rects are in global
//! points (top-left origin), matching the rest of the backend.

use super::axui::{AxEl, SendEl, on_caller, require_trusted};
use super::window_ops::ax_window_for_id;
use super::{parse_window_id, window_record};
use crate::error::{Error, Result};
use crate::model::{Ack, AxNode, AxQuery, Rect};

const DEFAULT_MAX_NODES: usize = 2000;

/// How long a main-thread read waits on an element before leaving it to the caller.
const MAIN_THREAD_PATIENCE_SECS: f32 = 0.25;

/// Whether reads of this window must hop to the main thread: it is this process's own,
/// and the caller is not already on the main thread.
fn hops_to_main(window_id: &str) -> Result<bool> {
    let own =
        window_record(parse_window_id(window_id)?).is_some_and(|w| w.pid == std::process::id());
    Ok(own && unsafe { libc::pthread_main_np() } == 0)
}

/// Whether this process shows windows, so a system-wide hit test may land in it. One that
/// does off the main thread aborts in AppKit; a hop for another app's window costs nothing,
/// and a windowless CLI has no main queue to hop to.
fn has_own_windows() -> bool {
    let pid = std::process::id();
    super::windows(&Default::default()).is_ok_and(|windows| windows.iter().any(|w| w.pid == pid))
}

/// Runs `work` on the main thread when the window is this process's own: AX answers an
/// in-process query on the calling thread, and AppKit and WebKit allow only the main one.
fn on_owner_thread<T: Send>(window_id: &str, work: impl FnOnce() -> Result<T> + Send) -> Result<T> {
    on_main_if(hops_to_main(window_id)?, work)
}

fn on_main_if<T: Send>(hop: bool, work: impl FnOnce() -> Result<T> + Send) -> Result<T> {
    if !hop {
        return work();
    }
    let mut work = Some(work);
    let mut out = None;
    dispatch2::DispatchQueue::main().exec_sync(|| {
        if let Some(work) = work.take() {
            out = Some(work());
        }
    });
    out.unwrap_or_else(|| {
        Err(Error::Failed(
            "the main thread did not run the accessibility read".into(),
        ))
    })
}

/// Normalize an AX role ("AXButton") to the contract's lowercase form ("button").
fn role_name(role: Option<String>) -> String {
    role.map(|r| r.strip_prefix("AX").unwrap_or(&r).to_lowercase())
        .unwrap_or_default()
}

/// Normalize an AX action like a role ("AXShowMenu" → "showmenu"); a custom action's
/// entry ("Name:Delete\nTarget:…") becomes its name.
fn action_name(raw: &str) -> String {
    if let Some(custom) = raw.strip_prefix("Name:") {
        return custom.lines().next().unwrap_or_default().to_string();
    }
    raw.strip_prefix("AX").unwrap_or(raw).to_lowercase()
}

fn node_rect(el: &AxEl) -> Rect {
    use objc2_core_foundation::{CGPoint, CGSize};
    let pos = el
        .attr_point("AXPosition")
        .unwrap_or(CGPoint::new(0.0, 0.0));
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
        .or_else(|| {
            el.attr_string("AXRoleDescription")
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_default();
    AxNode {
        id,
        role,
        name,
        value,
        enabled: el.attr_bool("AXEnabled").unwrap_or(true),
        focused: el.attr_bool("AXFocused").unwrap_or(false),
        rect: node_rect(el),
        actions: el
            .action_names()
            .iter()
            .map(|raw| action_name(raw))
            .collect(),
        children: Vec::new(),
    }
}

/// A subtree the main-thread read left for the caller: `at` is where it belongs in a flat
/// read, `depth` what remains of a tree read's depth.
struct Elsewhere {
    id: String,
    at: usize,
    depth: Option<u32>,
    el: SendEl,
}

/// Whether to leave `el` for the caller; `elsewhere` is set only for a main-thread read.
fn leave_for_caller(
    el: &AxEl,
    id: &str,
    at: usize,
    depth: Option<u32>,
    elsewhere: &mut Option<&mut Vec<Elsewhere>>,
) -> bool {
    let Some(list) = elsewhere.as_deref_mut() else {
        return false;
    };
    if el.answers_within(MAIN_THREAD_PATIENCE_SECS) {
        return false;
    }
    list.push(Elsewhere {
        id: id.to_string(),
        at,
        depth,
        el: SendEl(el.clone_ref()),
    });
    true
}

fn placeholder(id: String) -> AxNode {
    AxNode {
        id,
        role: String::new(),
        name: String::new(),
        value: None,
        enabled: false,
        focused: false,
        rect: Rect {
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        },
        actions: Vec::new(),
        children: Vec::new(),
    }
}

fn build_tree(
    el: &AxEl,
    id: String,
    depth: Option<u32>,
    count: &mut usize,
    max: usize,
    elsewhere: &mut Option<&mut Vec<Elsewhere>>,
) -> AxNode {
    if leave_for_caller(el, &id, 0, depth, elsewhere) {
        return placeholder(id);
    }
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
            elsewhere,
        ));
    }
    node
}

fn collect_flat(
    el: &AxEl,
    id: String,
    count: &mut usize,
    max: usize,
    out: &mut Vec<(AxNode, AxEl)>,
    elsewhere: &mut Option<&mut Vec<Elsewhere>>,
) {
    if *count >= max || leave_for_caller(el, &id, out.len(), None, elsewhere) {
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
        collect_flat(&child, format!("{id}/{idx}"), count, max, out, elsewhere);
    }
}

fn find_mut<'a>(node: &'a mut AxNode, id: &str) -> Option<&'a mut AxNode> {
    if node.id == id {
        return Some(node);
    }
    node.children
        .iter_mut()
        .find_map(|child| find_mut(child, id))
}

pub fn tree(window_id: &str, depth: Option<u32>, max_nodes: Option<usize>) -> Result<AxNode> {
    let max = max_nodes.unwrap_or(DEFAULT_MAX_NODES);
    let split = hops_to_main(window_id)?;
    let (mut root, left, mut count) = on_owner_thread(window_id, || {
        require_trusted()?;
        let window = ax_window_for_id(window_id)?;
        let mut left = Vec::new();
        let mut elsewhere = split.then_some(&mut left);
        let mut count = 0;
        let root = build_tree(
            &window,
            "ax:0".into(),
            depth,
            &mut count,
            max,
            &mut elsewhere,
        );
        Ok((root, left, count))
    })?;
    for part in left {
        let subtree = build_tree(
            &part.el.0,
            part.id.clone(),
            part.depth,
            &mut count,
            max,
            &mut None,
        );
        if let Some(slot) = find_mut(&mut root, &part.id) {
            *slot = subtree;
        }
    }
    Ok(root)
}

pub fn hit_test(x: i32, y: i32) -> Result<AxNode> {
    let id = format!("ax:@{x},{y}");
    let hop = has_own_windows() && unsafe { libc::pthread_main_np() } == 0;
    let (node, left) = on_main_if(hop, || {
        require_trusted()?;
        let el = AxEl::system_wide()?.element_at(x as f32, y as f32)?;
        if hop && !el.answers_within(MAIN_THREAD_PATIENCE_SECS) {
            return Ok((None, Some(SendEl(el))));
        }
        Ok((Some(node_data(&el, id.clone())), None))
    })?;
    match (node, left) {
        (Some(node), _) => Ok(node),
        (None, Some(el)) => Ok(node_data(&el.0, id)),
        (None, None) => unreachable!("the read returns a node or an element"),
    }
}

/// The window's flat tree with live elements, `true` beside each one another process
/// serves. This process's own elements are read on the main thread; any that keep it
/// waiting are read afterwards on the calling thread and spliced back in tree order.
fn flat(window_id: &str) -> Result<Vec<(AxNode, SendEl, bool)>> {
    let split = hops_to_main(window_id)?;
    let (local, left, mut count) = on_owner_thread(window_id, || {
        require_trusted()?;
        let root = ax_window_for_id(window_id)?;
        let mut out = Vec::new();
        let mut left = Vec::new();
        let mut elsewhere = split.then_some(&mut left);
        let mut count = 0;
        collect_flat(
            &root,
            "ax:0".into(),
            &mut count,
            DEFAULT_MAX_NODES,
            &mut out,
            &mut elsewhere,
        );
        let out: Vec<(AxNode, SendEl)> = out.into_iter().map(|(n, el)| (n, SendEl(el))).collect();
        Ok((out, left, count))
    })?;
    let mut all: Vec<(AxNode, SendEl, bool)> =
        local.into_iter().map(|(n, el)| (n, el, false)).collect();
    // Back to front, so each recorded position still holds when its subtree goes in.
    for part in left.into_iter().rev() {
        let mut sub = Vec::new();
        collect_flat(
            &part.el.0,
            part.id,
            &mut count,
            DEFAULT_MAX_NODES,
            &mut sub,
            &mut None,
        );
        all.splice(
            part.at..part.at,
            sub.into_iter().map(|(n, el)| (n, SendEl(el), true)),
        );
    }
    Ok(all)
}

pub fn query(window_id: &str, q: &AxQuery, all: bool, index: Option<usize>) -> Result<Vec<AxNode>> {
    let nodes: Vec<AxNode> = flat_nodes(window_id)?
        .into_iter()
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

fn flat_nodes(window_id: &str) -> Result<Vec<AxNode>> {
    Ok(flat(window_id)?.into_iter().map(|(n, ..)| n).collect())
}

/// Match exactly one live element and act on it (atomic match-and-act). An element this
/// process serves is changed on the main thread; one another process serves, here.
fn act(window_id: &str, q: &AxQuery, action: impl FnOnce(AxEl) -> Result<()>) -> Result<()> {
    let mut matches: Vec<(SendEl, bool)> = flat(window_id)?
        .into_iter()
        .filter(|(n, ..)| q.is_empty() || q.matches(n))
        .map(|(_, el, elsewhere)| (el, elsewhere))
        .collect();
    match matches.len() {
        0 => Err(Error::NotFound("no accessibility node matched".into())),
        1 => {
            let (el, elsewhere) = matches.remove(0);
            if elsewhere {
                on_caller(|| action(el.0))
            } else {
                action(el.0)
            }
        }
        n => Err(Error::Ambiguous(format!(
            "{n} nodes matched; narrow the query"
        ))),
    }
}

pub fn invoke(window_id: &str, q: &AxQuery) -> Result<Ack> {
    act(window_id, q, |el| el.perform("AXPress"))?;
    Ok(Ack::new("ax.invoke"))
}

pub fn focus(window_id: &str, q: &AxQuery) -> Result<Ack> {
    act(window_id, q, |el| el.set_bool("AXFocused", true))?;
    Ok(Ack::new("ax.focus"))
}

pub fn set_value(window_id: &str, q: &AxQuery, value: &str) -> Result<Ack> {
    act(window_id, q, |el| el.set_string("AXValue", value))?;
    Ok(Ack::new("ax.set-value"))
}

pub fn select(window_id: &str, q: &AxQuery) -> Result<Ack> {
    act(window_id, q, |el| el.set_bool("AXSelected", true))?;
    Ok(Ack::new("ax.select"))
}

pub fn expand(window_id: &str, q: &AxQuery) -> Result<Ack> {
    act(window_id, q, |el| el.set_bool("AXDisclosing", true))?;
    Ok(Ack::new("ax.expand"))
}

pub fn collapse(window_id: &str, q: &AxQuery) -> Result<Ack> {
    act(window_id, q, |el| el.set_bool("AXDisclosing", false))?;
    Ok(Ack::new("ax.collapse"))
}

pub fn scroll_into_view(window_id: &str, q: &AxQuery) -> Result<Ack> {
    act(window_id, q, |el| el.perform("AXScrollToVisible"))?;
    Ok(Ack::new("ax.scroll-into-view"))
}

/// Poll the tree until a node matching `q` reaches the requested state, or time
/// out (exit 5). States: exists (default) / gone / enabled / focused.
pub fn wait(window_id: &str, q: &AxQuery, state: &str, timeout_ms: u64) -> Result<Ack> {
    if !matches!(state, "exists" | "gone" | "enabled" | "focused") {
        return Err(Error::Usage(format!(
            "unknown ax wait state '{state}'; expected exists, gone, enabled, or focused"
        )));
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
    loop {
        let nodes: Vec<AxNode> = flat_nodes(window_id)?
            .into_iter()
            .filter(|n| q.matches(n))
            .collect();
        let satisfied = match state {
            "gone" => nodes.is_empty(),
            "enabled" => nodes.iter().any(|n| n.enabled),
            "focused" => nodes.iter().any(|n| n.focused),
            "exists" => !nodes.is_empty(),
            _ => unreachable!("state was validated above"),
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

#[cfg(test)]
mod tests {
    use super::action_name;

    #[test]
    fn action_names_read_like_roles() {
        assert_eq!(action_name("AXPress"), "press");
        assert_eq!(action_name("AXShowMenu"), "showmenu");
        assert_eq!(
            action_name("Name:Delete row\nTarget:0x0\nSelector:(null)"),
            "Delete row"
        );
        assert_eq!(action_name("custom"), "custom");
    }
}
