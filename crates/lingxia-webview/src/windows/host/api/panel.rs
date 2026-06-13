//! Public host panel API surface.

use super::*;

/// Handler for structured key input targeted at a host panel.
///
/// Returns `true` when the event was consumed (the window message is then
/// swallowed); `false` lets default window handling proceed.
pub type WindowsHostPanelInputHandler = Arc<dyn Fn(WindowsHostPanelKeyEvent) -> bool + Send + Sync>;

/// Structured key event forwarded to a host panel input handler.
///
/// `lingxia-webview` does not interpret keys (e.g. text-control escape
/// sequences); it only reports what the window received. `character` is set
/// for translated character input (`WM_CHAR`); for raw key-down input
/// (`WM_KEYDOWN`) it is `None` and `vk` carries the virtual-key code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WindowsHostPanelKeyEvent {
    /// Virtual-key code for key-down events; `0` for character events.
    pub vk: u32,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    /// Translated character for character events.
    pub character: Option<char>,
}

pub fn is_panel_visible(panel_id: &str) -> bool {
    active_group_key()
        .map(|group_key| {
            group_panels(&group_key)
                .into_iter()
                .any(|panel| panel.panel_id == panel_id)
        })
        .unwrap_or(false)
}

pub fn show_interactive_host_panel(
    panel_id: &str,
    title: &str,
    body: &str,
    position: WindowsPanelPosition,
) -> StdResult<()> {
    let group_key = active_group_key()
        .ok_or_else(|| WebViewError::WebView("no active Windows host group".to_string()))?;
    let Some(_host) = host_handle_for_group(&group_key) else {
        return Err(WebViewError::WebView(format!(
            "active Windows host group has no host: {group_key}"
        )));
    };

    register_group_panel(
        &group_key,
        GroupPanel {
            webtag_key: host_panel_key(panel_id),
            panel_id: panel_id.to_string(),
            position,
            host_title: Some(title.to_string()),
            host_body: Some(body.to_string()),
            host_tabs: Vec::new(),
            maximized: false,
        },
    );
    set_active_host_panel(Some(panel_id.to_string()));
    layout_group_windows(&group_key);
    request_group_chrome_refresh(&group_key);
    Ok(())
}

pub fn update_host_panel_body(panel_id: &str, body: &str) -> StdResult<()> {
    let Some(group_key) = update_group_panel_body(panel_id, body.to_string()) else {
        return Ok(());
    };
    request_group_chrome_refresh(&group_key);
    Ok(())
}

/// Replaces the header tab strip of a host panel and repaints the host
/// chrome. The strip is generic data: ids, titles, and the active flag are
/// owned by the host integration. Returns `false` when no group currently
/// hosts the panel.
pub fn set_host_panel_tabs(panel_id: &str, tabs: Vec<WindowsHostPanelTab>) -> bool {
    let Some(group_key) = update_group_panel_tabs(panel_id, tabs) else {
        return false;
    };
    request_group_chrome_refresh(&group_key);
    true
}

/// Sets whether a host panel is maximized over the whole content area
/// (the main card collapses while maximized) and re-runs the group layout.
/// Pure rect mechanics; the toggle policy lives in the host integration.
/// Returns `false` when no group currently hosts the panel.
pub fn set_host_panel_maximized(panel_id: &str, maximized: bool) -> bool {
    let Some(group_key) = update_group_panel_maximized(panel_id, maximized) else {
        return false;
    };
    layout_group_windows(&group_key);
    request_group_chrome_refresh(&group_key);
    true
}

/// Repaints the host-window region covering `panel_id` without changing the
/// panel's registered body text (for panels whose content is drawn by the
/// chrome renderer from external state). Returns `false` when no group
/// currently hosts the panel.
pub fn invalidate_host_panel(panel_id: &str) -> bool {
    let Some(group_key) = group_key_for_panel(panel_id) else {
        return false;
    };
    // Content-only updates repaint just the panel rect; a full-window
    // invalidation here would repaint unrelated host chrome every frame.
    request_group_panel_repaint(&group_key, panel_id);
    true
}

#[inline]
pub fn hide_host_panel(panel_id: &str) -> StdResult<()> {
    let group_key = active_group_key()
        .ok_or_else(|| WebViewError::WebView("no active Windows host group".to_string()))?;
    remove_group_panel_by_panel_id(&group_key, panel_id);
    layout_group_windows(&group_key);
    request_group_chrome_refresh(&group_key);
    Ok(())
}
