//! Native accessibility via UI Automation. Provides a bounded tree dump, a
//! flat query, and atomic match-and-invoke. Node ids are path-based
//! (`ax:0/2/1`) — stable within one process run, which is what the atomic
//! query→act flow needs.

use super::parse_hwnd;
use super::rect_to;
use crate::error::{Error, Result};
use crate::model::{Ack, AxNode, AxQuery};
use std::sync::Once;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationInvokePattern,
    IUIAutomationTreeWalker, IUIAutomationValuePattern, UIA_ButtonControlTypeId,
    UIA_CheckBoxControlTypeId, UIA_ComboBoxControlTypeId, UIA_DocumentControlTypeId,
    UIA_EditControlTypeId, UIA_GroupControlTypeId, UIA_HyperlinkControlTypeId,
    UIA_ImageControlTypeId, UIA_InvokePatternId, UIA_ListControlTypeId, UIA_ListItemControlTypeId,
    UIA_MenuItemControlTypeId, UIA_PaneControlTypeId, UIA_TextControlTypeId, UIA_ValuePatternId,
    UIA_WindowControlTypeId,
};
use windows::core::BSTR;

const DEFAULT_MAX_NODES: usize = 2000;

fn bstr_to_string(b: &BSTR) -> String {
    b.display().to_string()
}

fn automation() -> Result<IUIAutomation> {
    super::ensure_dpi_aware();
    static INIT: Once = Once::new();
    INIT.call_once(|| unsafe {
        // Ignore the HRESULT: RPC_E_CHANGED_MODE just means COM is already up.
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    });
    unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }
        .map_err(|e| Error::Unavailable(format!("UI Automation unavailable: {e}")))
}

fn root(uia: &IUIAutomation, window_id: &str) -> Result<IUIAutomationElement> {
    let hwnd = parse_hwnd(window_id)?;
    unsafe { uia.ElementFromHandle(hwnd) }
        .map_err(|e| Error::Stale(format!("window {window_id} has no AX root: {e}")))
}

fn control_type_name(id: i32) -> String {
    let name = if id == UIA_ButtonControlTypeId.0 {
        "button"
    } else if id == UIA_EditControlTypeId.0 {
        "edit"
    } else if id == UIA_TextControlTypeId.0 {
        "text"
    } else if id == UIA_MenuItemControlTypeId.0 {
        "menuitem"
    } else if id == UIA_CheckBoxControlTypeId.0 {
        "checkbox"
    } else if id == UIA_ComboBoxControlTypeId.0 {
        "combobox"
    } else if id == UIA_ListControlTypeId.0 {
        "list"
    } else if id == UIA_ListItemControlTypeId.0 {
        "listitem"
    } else if id == UIA_HyperlinkControlTypeId.0 {
        "hyperlink"
    } else if id == UIA_ImageControlTypeId.0 {
        "image"
    } else if id == UIA_WindowControlTypeId.0 {
        "window"
    } else if id == UIA_PaneControlTypeId.0 {
        "pane"
    } else if id == UIA_GroupControlTypeId.0 {
        "group"
    } else if id == UIA_DocumentControlTypeId.0 {
        "document"
    } else {
        return format!("control-{id}");
    };
    name.to_string()
}

fn node_data(el: &IUIAutomationElement, id: String) -> AxNode {
    unsafe {
        let name = el
            .CurrentName()
            .map(|b| bstr_to_string(&b))
            .unwrap_or_default();
        let role = control_type_name(el.CurrentControlType().map(|c| c.0).unwrap_or(0));
        let enabled = el.CurrentIsEnabled().map(|b| b.as_bool()).unwrap_or(false);
        let focused = el
            .CurrentHasKeyboardFocus()
            .map(|b| b.as_bool())
            .unwrap_or(false);
        let rect = el
            .CurrentBoundingRectangle()
            .map(rect_to)
            .unwrap_or(crate::model::Rect {
                x: 0,
                y: 0,
                w: 0,
                h: 0,
            });
        let value = el
            .GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)
            .ok()
            .and_then(|p| p.CurrentValue().ok())
            .map(|b| bstr_to_string(&b))
            .filter(|s| !s.is_empty());
        AxNode {
            id,
            role,
            name,
            value,
            enabled,
            focused,
            rect,
            children: Vec::new(),
        }
    }
}

/// Recursively build the tree up to `depth` (None = unbounded) and `max` nodes.
fn build_tree(
    walker: &IUIAutomationTreeWalker,
    el: &IUIAutomationElement,
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
    unsafe {
        let mut child = walker.GetFirstChildElement(el).ok();
        let mut idx = 0usize;
        while let Some(c) = child {
            if *count >= max {
                break;
            }
            node.children.push(build_tree(
                walker,
                &c,
                format!("{id}/{idx}"),
                depth.map(|d| d - 1),
                count,
                max,
            ));
            idx += 1;
            child = walker.GetNextSiblingElement(&c).ok();
        }
    }
    node
}

/// Walk the whole subtree flat, keeping live elements for actions.
fn collect_flat(
    walker: &IUIAutomationTreeWalker,
    el: &IUIAutomationElement,
    id: String,
    count: &mut usize,
    max: usize,
    out: &mut Vec<(AxNode, IUIAutomationElement)>,
) {
    if *count >= max {
        return;
    }
    let node = node_data(el, id.clone());
    *count += 1;
    out.push((node, el.clone()));
    unsafe {
        let mut child = walker.GetFirstChildElement(el).ok();
        let mut idx = 0usize;
        while let Some(c) = child {
            if *count >= max {
                break;
            }
            collect_flat(walker, &c, format!("{id}/{idx}"), count, max, out);
            idx += 1;
            child = walker.GetNextSiblingElement(&c).ok();
        }
    }
}

pub fn tree(window_id: &str, depth: Option<u32>, max_nodes: Option<usize>) -> Result<AxNode> {
    let uia = automation()?;
    let root = root(&uia, window_id)?;
    let walker = unsafe { uia.ControlViewWalker() }
        .map_err(|e| Error::Failed(format!("no tree walker: {e}")))?;
    let mut count = 0;
    Ok(build_tree(
        &walker,
        &root,
        "ax:0".to_string(),
        depth,
        &mut count,
        max_nodes.unwrap_or(DEFAULT_MAX_NODES),
    ))
}

fn flat(window_id: &str) -> Result<Vec<(AxNode, IUIAutomationElement)>> {
    let uia = automation()?;
    let root = root(&uia, window_id)?;
    let walker = unsafe { uia.ControlViewWalker() }
        .map_err(|e| Error::Failed(format!("no tree walker: {e}")))?;
    let mut out = Vec::new();
    let mut count = 0;
    collect_flat(
        &walker,
        &root,
        "ax:0".to_string(),
        &mut count,
        DEFAULT_MAX_NODES,
        &mut out,
    );
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

/// Atomic match-and-invoke: resolve exactly one node, then Invoke it.
pub fn invoke(window_id: &str, q: &AxQuery) -> Result<Ack> {
    let mut matches: Vec<(AxNode, IUIAutomationElement)> = flat(window_id)?
        .into_iter()
        .filter(|(n, _)| q.is_empty() || q.matches(n))
        .collect();
    let (_, el) = match matches.len() {
        0 => return Err(Error::NotFound("no accessibility node matched".into())),
        1 => matches.remove(0),
        n => {
            return Err(Error::Ambiguous(format!(
                "{n} nodes matched; narrow the query"
            )));
        }
    };
    let pattern =
        unsafe { el.GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId) }
            .map_err(|_| Error::Unsupported("target does not support the invoke pattern".into()))?;
    unsafe { pattern.Invoke() }.map_err(|e| Error::Failed(format!("invoke failed: {e}")))?;
    Ok(Ack::new("ax.invoke"))
}
