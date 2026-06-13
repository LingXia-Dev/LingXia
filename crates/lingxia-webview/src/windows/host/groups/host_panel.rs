//! Host panel registry, focus, and keyboard input dispatch.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GroupPanel {
    pub(crate) webtag_key: String,
    pub(crate) panel_id: String,
    pub(crate) position: WindowsPanelPosition,
    pub(crate) host_title: Option<String>,
    pub(crate) host_body: Option<String>,
    /// Header tab strip of a host panel (generic ids/titles supplied by
    /// the host integration); empty for webview-backed panels.
    pub(crate) host_tabs: Vec<WindowsHostPanelTab>,
    /// Whether the panel currently covers the whole content area (the
    /// main card collapses while maximized).
    pub(crate) maximized: bool,
}

impl GroupPanel {
    /// Whether the panel lays out as a compact dock flush against the main
    /// card (zero gap, thin divider): bottom-positioned interactive panels.
    pub(crate) fn docked(&self) -> bool {
        self.position == WindowsPanelPosition::Bottom && self.host_title.is_some()
    }
}

pub(crate) static WINDOW_GROUP_PANELS: OnceLock<Mutex<HashMap<String, Vec<GroupPanel>>>> =
    OnceLock::new();

pub(crate) static WINDOW_ACTIVE_HOST_PANEL: OnceLock<Mutex<Option<String>>> = OnceLock::new();

pub(crate) fn active_host_panel() -> Option<String> {
    WINDOW_ACTIVE_HOST_PANEL
        .get()
        .and_then(|active| active.lock().ok())
        .and_then(|active| active.clone())
}

pub(crate) fn set_active_host_panel(panel_id: Option<String>) {
    let active = WINDOW_ACTIVE_HOST_PANEL.get_or_init(|| Mutex::new(None));
    if let Ok(mut active) = active.lock() {
        *active = panel_id;
    }
}

pub(crate) fn handle_host_panel_char(wparam: WPARAM) -> bool {
    let Some(character) = char::from_u32(wparam.0 as u32) else {
        return false;
    };
    let (ctrl, shift, alt) = keyboard_modifiers();
    invoke_host_panel_input(WindowsHostPanelKeyEvent {
        vk: 0,
        ctrl,
        shift,
        alt,
        character: Some(character),
    })
}

pub(crate) fn handle_host_panel_keydown(wparam: WPARAM) -> bool {
    let (ctrl, shift, alt) = keyboard_modifiers();
    invoke_host_panel_input(WindowsHostPanelKeyEvent {
        vk: wparam.0 as u32,
        ctrl,
        shift,
        alt,
        character: None,
    })
}

pub(crate) fn keyboard_modifiers() -> (bool, bool, bool) {
    unsafe {
        (
            (GetKeyState(VK_CONTROL.0 as i32) as u16 & 0x8000) != 0,
            (GetKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000) != 0,
            (GetKeyState(VK_MENU.0 as i32) as u16 & 0x8000) != 0,
        )
    }
}

pub(crate) fn invoke_host_panel_input(event: WindowsHostPanelKeyEvent) -> bool {
    let Some(panel_id) = active_host_panel() else {
        return false;
    };
    let Some(handler) = WINDOW_HOST_PANEL_INPUT_HANDLERS
        .get()
        .and_then(|handlers| handlers.lock().ok())
        .and_then(|handlers| handlers.get(&panel_id).cloned())
    else {
        return false;
    };
    handler(event)
}

pub(crate) fn panel_position_for_group(group_key: &str, panel_id: &str) -> WindowsPanelPosition {
    group_panels(group_key)
        .into_iter()
        .find(|panel| panel.panel_id == panel_id)
        .map(|panel| panel.position)
        .unwrap_or_default()
}

pub(crate) fn host_panel_key(panel_id: &str) -> String {
    format!("host-panel:{panel_id}")
}

pub(crate) fn register_group_panel(group_key: &str, panel: GroupPanel) {
    let panels = WINDOW_GROUP_PANELS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut panels) = panels.lock() {
        let group_panels = panels.entry(group_key.to_string()).or_default();
        group_panels.retain(|item| item.panel_id != panel.panel_id);
        group_panels.push(panel);
    }
}

pub(crate) fn update_group_panel_body(panel_id: &str, body: String) -> Option<String> {
    let panels = WINDOW_GROUP_PANELS.get()?;
    let mut panels = panels.lock().ok()?;
    for (group_key, group_panels) in panels.iter_mut() {
        if let Some(panel) = group_panels
            .iter_mut()
            .find(|panel| panel.panel_id == panel_id)
        {
            panel.host_body = Some(body);
            return Some(group_key.clone());
        }
    }
    None
}

/// Replaces a host panel's header tab strip; returns the owning group.
pub(crate) fn update_group_panel_tabs(
    panel_id: &str,
    tabs: Vec<WindowsHostPanelTab>,
) -> Option<String> {
    let panels = WINDOW_GROUP_PANELS.get()?;
    let mut panels = panels.lock().ok()?;
    for (group_key, group_panels) in panels.iter_mut() {
        if let Some(panel) = group_panels
            .iter_mut()
            .find(|panel| panel.panel_id == panel_id)
        {
            panel.host_tabs = tabs;
            return Some(group_key.clone());
        }
    }
    None
}

/// Sets a host panel's maximized flag; returns the owning group so the
/// caller can re-run its layout.
pub(crate) fn update_group_panel_maximized(panel_id: &str, maximized: bool) -> Option<String> {
    let panels = WINDOW_GROUP_PANELS.get()?;
    let mut panels = panels.lock().ok()?;
    for (group_key, group_panels) in panels.iter_mut() {
        if let Some(panel) = group_panels
            .iter_mut()
            .find(|panel| panel.panel_id == panel_id)
        {
            panel.maximized = maximized;
            return Some(group_key.clone());
        }
    }
    None
}

pub(crate) fn group_key_for_panel(panel_id: &str) -> Option<String> {
    let panels = WINDOW_GROUP_PANELS.get()?;
    let panels = panels.lock().ok()?;
    panels.iter().find_map(|(group_key, group_panels)| {
        group_panels
            .iter()
            .any(|panel| panel.panel_id == panel_id)
            .then(|| group_key.clone())
    })
}

pub(crate) fn remove_group_panel(group_key: &str, webtag_key: &str) {
    let mut removed_active = false;
    if let Some(panels) = WINDOW_GROUP_PANELS.get()
        && let Ok(mut panels) = panels.lock()
        && let Some(group_panels) = panels.get_mut(group_key)
    {
        if let Some(active) = active_host_panel() {
            removed_active = group_panels
                .iter()
                .any(|panel| panel.webtag_key == webtag_key && panel.panel_id == active);
        }
        group_panels.retain(|panel| panel.webtag_key != webtag_key);
    }
    if removed_active {
        set_active_host_panel(None);
    }
}

pub(crate) fn remove_group_panel_by_panel_id(group_key: &str, panel_id: &str) {
    if let Some(panels) = WINDOW_GROUP_PANELS.get()
        && let Ok(mut panels) = panels.lock()
        && let Some(group_panels) = panels.get_mut(group_key)
    {
        group_panels.retain(|panel| panel.panel_id != panel_id);
    }
    if active_host_panel().as_deref() == Some(panel_id) {
        set_active_host_panel(None);
    }
}

pub(crate) fn group_panels(group_key: &str) -> Vec<GroupPanel> {
    WINDOW_GROUP_PANELS
        .get()
        .and_then(|panels| panels.lock().ok())
        .and_then(|panels| panels.get(group_key).cloned())
        .unwrap_or_default()
}

/// Whether `webtag_key`'s window covers the group's main-card surface and
/// that surface sits flush above a docked bottom panel. The card's bottom
/// corners then stay square (its corner caps would otherwise notch the
/// shared edge with the dock).
pub(crate) fn main_surface_has_docked_bottom_panel(webtag_key: &str) -> bool {
    let Some(attachment) = window_attachment(webtag_key) else {
        return false;
    };
    if matches!(attachment.kind, WindowAttachmentKind::Panel { .. }) {
        return false;
    }
    group_panels(&attachment.group_key)
        .iter()
        .any(GroupPanel::docked)
}
