//! Window grouping: attachments, group panels, attached layout
//! computation, panel resize, and host panel input plumbing.

use super::*;

mod host_panel;
mod layout;
mod resize;

pub(crate) use host_panel::*;
pub(crate) use layout::*;
pub(crate) use resize::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WindowAttachment {
    pub(crate) group_key: String,
    pub(crate) kind: WindowAttachmentKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WindowAttachmentKind {
    MainHost,
    MainChild,
    Panel {
        panel_id: String,
        position: WindowsPanelPosition,
    },
    /// A floating card layered over the group's main content (an
    /// `overlay`-kind surface). Unlike a `MainChild` it does not displace the
    /// active main — the main content stays visible behind it — and it is sized
    /// to a sub-rect of the content area (see [`OVERLAY_PLACEMENTS`]) instead of
    /// filling it. Being a child of the host window, it follows the host on
    /// move/resize and cannot leave the content area.
    Overlay,
}

/// Where an overlay card is anchored within the content area. Mirrors the
/// `SurfacePosition` discriminants the logic layer sends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OverlayAnchor {
    Center,
    Bottom,
    Left,
    Right,
    Top,
}

impl OverlayAnchor {
    pub(crate) fn from_position(position: u8) -> Self {
        match position {
            1 => OverlayAnchor::Bottom,
            2 => OverlayAnchor::Left,
            3 => OverlayAnchor::Right,
            4 => OverlayAnchor::Top,
            _ => OverlayAnchor::Center,
        }
    }
}

/// Requested overlay-card geometry within the content area. `width`/`height`
/// are logical pixels (0 = derive from the ratio or a default); `width_ratio`/
/// `height_ratio` are fractions of the content area (0 = none).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct OverlayCardPlacement {
    pub width: f64,
    pub height: f64,
    pub width_ratio: f64,
    pub height_ratio: f64,
    pub anchor: OverlayAnchor,
}

impl Default for OverlayCardPlacement {
    fn default() -> Self {
        OverlayCardPlacement {
            width: 0.0,
            height: 0.0,
            width_ratio: 0.0,
            height_ratio: 0.0,
            anchor: OverlayAnchor::Center,
        }
    }
}

pub(crate) static WINDOW_LAYOUTS: OnceLock<Mutex<HashMap<String, WindowsWindowLayout>>> =
    OnceLock::new();

pub(crate) static WINDOW_GROUP_LAYOUTS: OnceLock<Mutex<HashMap<String, WindowsWindowLayout>>> =
    OnceLock::new();

pub(crate) static WINDOW_GROUP_HOSTS: OnceLock<Mutex<HashMap<String, isize>>> = OnceLock::new();

pub(crate) static WINDOW_HANDLES: OnceLock<Mutex<HashMap<String, isize>>> = OnceLock::new();

pub(crate) static WINDOW_ATTACHMENTS: OnceLock<Mutex<HashMap<String, WindowAttachment>>> =
    OnceLock::new();

pub(crate) static WINDOW_GROUP_ACTIVE_MAIN: OnceLock<Mutex<HashMap<String, String>>> =
    OnceLock::new();

/// A webview presented over a group's main content card via
/// `WindowsWebViewHandler::present_in_active_group`, plus the main webview
/// it displaced (restored when the presentation ends).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PresentedGroupMain {
    pub(crate) presented_key: String,
    pub(crate) previous_main_key: Option<String>,
}

pub(crate) static WINDOW_GROUP_PRESENTED_MAIN: OnceLock<
    Mutex<HashMap<String, PresentedGroupMain>>,
> = OnceLock::new();

pub(crate) static WINDOW_ACTIVE_GROUP: OnceLock<Mutex<Option<String>>> = OnceLock::new();

pub(crate) static WINDOW_GROUP_PANEL_SIZES: OnceLock<Mutex<HashMap<String, i32>>> = OnceLock::new();

/// Per-webview overlay-card placement, keyed by webtag. Set when an overlay is
/// presented and read by the layout to size/position the card; cleared on close.
pub(crate) static OVERLAY_PLACEMENTS: OnceLock<Mutex<HashMap<String, OverlayCardPlacement>>> =
    OnceLock::new();

pub(crate) fn set_overlay_placement(webtag_key: &str, placement: OverlayCardPlacement) {
    let map = OVERLAY_PLACEMENTS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut map) = map.lock() {
        map.insert(webtag_key.to_string(), placement);
    }
}

pub(crate) fn overlay_placement_for(webtag_key: &str) -> Option<OverlayCardPlacement> {
    OVERLAY_PLACEMENTS
        .get()
        .and_then(|map| map.lock().ok())
        .and_then(|map| map.get(webtag_key).copied())
}

pub(crate) fn clear_overlay_placement(webtag_key: &str) {
    if let Some(map) = OVERLAY_PLACEMENTS.get()
        && let Ok(mut map) = map.lock()
    {
        map.remove(webtag_key);
    }
}

pub(crate) fn current_window_layout(webtag_key: &str) -> WindowsWindowLayout {
    let exact = WINDOW_LAYOUTS
        .get()
        .and_then(|layouts| layouts.lock().ok())
        .and_then(|layouts| layouts.get(webtag_key).cloned());
    let group = layout_group_key_for_webtag(webtag_key);
    let group_layout = WINDOW_GROUP_LAYOUTS
        .get()
        .and_then(|layouts| layouts.lock().ok())
        .and_then(|layouts| layouts.get(&group).cloned());

    if window_attachment(webtag_key)
        .is_some_and(|attachment| matches!(attachment.kind, WindowAttachmentKind::MainHost))
    {
        return group_layout.or(exact).unwrap_or_default();
    }

    exact.or(group_layout).unwrap_or_default()
}

pub(crate) fn set_window_layout_for_key(webtag_key: &str, layout: WindowsWindowLayout) {
    let layouts = WINDOW_LAYOUTS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut layouts) = layouts.lock() {
        layouts.insert(webtag_key.to_string(), layout);
    }

    if !window_attachment(webtag_key)
        .is_some_and(|attachment| matches!(attachment.kind, WindowAttachmentKind::Panel { .. }))
    {
        let group_key = layout_group_key_for_webtag(webtag_key);
        let layouts = WINDOW_GROUP_LAYOUTS.get_or_init(|| Mutex::new(HashMap::new()));
        if let Ok(mut layouts) = layouts.lock() {
            layouts.insert(
                group_key,
                current_exact_window_layout(webtag_key).unwrap_or_default(),
            );
        }
    }
}

pub(crate) fn remove_window_layout(webtag_key: &str) {
    if let Some(layouts) = WINDOW_LAYOUTS.get()
        && let Ok(mut layouts) = layouts.lock()
    {
        layouts.remove(webtag_key);
    }
}

pub(crate) fn remove_group_layout(group_key: &str) {
    if let Some(layouts) = WINDOW_GROUP_LAYOUTS.get()
        && let Ok(mut layouts) = layouts.lock()
    {
        layouts.remove(group_key);
    }
}

/// Per-webview group-key overrides. By default a webview's group is
/// `appid#session` (all pages of one app/session share one host window). A
/// surface presented as its own window registers an override here so it
/// becomes its own group's `MainHost` — a standalone top-level window.
pub(crate) static WEBTAG_GROUP_OVERRIDES: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

pub(crate) fn set_group_override(webtag_key: &str, group_key: &str) {
    let map = WEBTAG_GROUP_OVERRIDES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut map) = map.lock() {
        map.insert(webtag_key.to_string(), group_key.to_string());
    }
}

pub(crate) fn clear_group_override(webtag_key: &str) {
    if let Some(map) = WEBTAG_GROUP_OVERRIDES.get()
        && let Ok(mut map) = map.lock()
    {
        map.remove(webtag_key);
    }
}

fn group_override_for(webtag_key: &str) -> Option<String> {
    WEBTAG_GROUP_OVERRIDES
        .get()
        .and_then(|map| map.lock().ok())
        .and_then(|map| map.get(webtag_key).cloned())
}

pub(crate) fn webtag_group_key(webtag_key: &str) -> String {
    group_override_for(webtag_key).unwrap_or_else(|| WebTag::from(webtag_key).group_key())
}

pub(crate) fn current_exact_window_layout(webtag_key: &str) -> Option<WindowsWindowLayout> {
    WINDOW_LAYOUTS
        .get()
        .and_then(|layouts| layouts.lock().ok())
        .and_then(|layouts| layouts.get(webtag_key).cloned())
}

pub(crate) fn window_attachment(webtag_key: &str) -> Option<WindowAttachment> {
    WINDOW_ATTACHMENTS
        .get()
        .and_then(|attachments| attachments.lock().ok())
        .and_then(|attachments| attachments.get(webtag_key).cloned())
}

pub(crate) fn layout_group_key_for_webtag(webtag_key: &str) -> String {
    window_attachment(webtag_key)
        .map(|attachment| attachment.group_key)
        .unwrap_or_else(|| webtag_group_key(webtag_key))
}

pub(crate) fn register_window_handle(webtag_key: &str, hwnd: HWND) {
    let handles = WINDOW_HANDLES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut handles) = handles.lock() {
        handles.insert(webtag_key.to_string(), hwnd_handle(hwnd));
    }
}

pub(crate) fn window_handle_for_key(webtag_key: &str) -> Option<HWND> {
    WINDOW_HANDLES
        .get()
        .and_then(|handles| handles.lock().ok())
        .and_then(|handles| handles.get(webtag_key).copied())
        .filter(|handle| is_window_handle_valid(*handle))
        .map(hwnd_from_handle)
}

pub(crate) fn host_handle_for_group(group_key: &str) -> Option<HWND> {
    WINDOW_GROUP_HOSTS
        .get()
        .and_then(|hosts| hosts.lock().ok())
        .and_then(|hosts| hosts.get(group_key).copied())
        .filter(|handle| is_window_handle_valid(*handle))
        .map(hwnd_from_handle)
}

pub(crate) fn set_window_attachment(webtag_key: &str, attachment: WindowAttachment) {
    let attachments = WINDOW_ATTACHMENTS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut attachments) = attachments.lock() {
        attachments.insert(webtag_key.to_string(), attachment);
    }
}

pub(crate) fn remove_window_attachment(webtag_key: &str) -> Option<WindowAttachment> {
    WINDOW_ATTACHMENTS
        .get()
        .and_then(|attachments| attachments.lock().ok())
        .and_then(|mut attachments| attachments.remove(webtag_key))
}

pub(crate) fn remove_window_handle(webtag_key: &str) {
    if let Some(handles) = WINDOW_HANDLES.get()
        && let Ok(mut handles) = handles.lock()
    {
        handles.remove(webtag_key);
    }
}

pub(crate) fn ensure_main_attachment(state: &UiState) -> (String, HWND, bool) {
    register_window_handle(&state.webtag_key, state.hwnd);
    let group_key = webtag_group_key(&state.webtag_key);
    let host_handle = {
        let hosts = WINDOW_GROUP_HOSTS.get_or_init(|| Mutex::new(HashMap::new()));
        let Ok(mut hosts) = hosts.lock() else {
            return (group_key, state.hwnd, true);
        };
        let existing = hosts
            .get(&group_key)
            .copied()
            .filter(|handle| is_window_handle_valid(*handle));
        let host_handle = existing.unwrap_or_else(|| hwnd_handle(state.hwnd));
        hosts.insert(group_key.clone(), host_handle);
        host_handle
    };
    let is_host = host_handle == hwnd_handle(state.hwnd);
    let kind = if is_host {
        WindowAttachmentKind::MainHost
    } else {
        WindowAttachmentKind::MainChild
    };
    set_window_attachment(
        &state.webtag_key,
        WindowAttachment {
            group_key: group_key.clone(),
            kind,
        },
    );

    let host = hwnd_from_handle(host_handle);
    if !is_host {
        attach_child_window_to_host(state.hwnd, host);
    }
    (group_key, host, is_host)
}

pub(crate) fn active_group_key() -> Option<String> {
    WINDOW_ACTIVE_GROUP
        .get()
        .and_then(|active| active.lock().ok())
        .and_then(|active| active.clone())
}

pub(crate) fn set_active_group(group_key: &str) {
    let active = WINDOW_ACTIVE_GROUP.get_or_init(|| Mutex::new(None));
    if let Ok(mut active) = active.lock() {
        *active = Some(group_key.to_string());
    }
}

pub(crate) fn group_active_main(group_key: &str) -> Option<String> {
    WINDOW_GROUP_ACTIVE_MAIN
        .get()
        .and_then(|active| active.lock().ok())
        .and_then(|active| active.get(group_key).cloned())
}

pub(crate) fn set_group_active_main(group_key: &str, webtag_key: &str) {
    let active = WINDOW_GROUP_ACTIVE_MAIN.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut active) = active.lock() {
        active.insert(group_key.to_string(), webtag_key.to_string());
    }
}

/// Removes the group's active-main entry when it currently points at
/// `webtag_key`.
pub(crate) fn remove_group_active_main(group_key: &str, webtag_key: &str) {
    if let Some(active) = WINDOW_GROUP_ACTIVE_MAIN.get()
        && let Ok(mut active) = active.lock()
        && active
            .get(group_key)
            .is_some_and(|key| key.as_str() == webtag_key)
    {
        active.remove(group_key);
    }
}

/// Records `presented_key` as the group's presented main surface. When a
/// presentation is already in flight (switching directly between presented
/// webviews), the originally displaced main webview is kept so the eventual
/// restore returns to it, not to an intermediate presented webview.
pub(crate) fn remember_presented_main(
    group_key: &str,
    presented_key: &str,
    previous_main: Option<String>,
) {
    let map = WINDOW_GROUP_PRESENTED_MAIN.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut map) = map.lock() {
        let previous_main_key = map
            .get(group_key)
            .map(|existing| existing.previous_main_key.clone())
            .unwrap_or(previous_main);
        map.insert(
            group_key.to_string(),
            PresentedGroupMain {
                presented_key: presented_key.to_string(),
                previous_main_key,
            },
        );
    }
}

pub(crate) fn take_presented_main(group_key: &str) -> Option<PresentedGroupMain> {
    WINDOW_GROUP_PRESENTED_MAIN
        .get()?
        .lock()
        .ok()?
        .remove(group_key)
}

/// Takes the group's presented-main entry only when it is `presented_key`.
pub(crate) fn take_presented_main_if(
    group_key: &str,
    presented_key: &str,
) -> Option<PresentedGroupMain> {
    let map = WINDOW_GROUP_PRESENTED_MAIN.get()?;
    let mut map = map.lock().ok()?;
    if map
        .get(group_key)
        .is_some_and(|entry| entry.presented_key == presented_key)
    {
        map.remove(group_key)
    } else {
        None
    }
}

/// Drops the group's presented-main entry when a different webview became
/// the group's main surface through the regular show flow (the
/// presentation is over; there is nothing left to restore).
pub(crate) fn clear_presented_main_for_new_main(group_key: &str, new_main_key: &str) {
    if let Some(map) = WINDOW_GROUP_PRESENTED_MAIN.get()
        && let Ok(mut map) = map.lock()
        && map
            .get(group_key)
            .is_some_and(|entry| entry.presented_key != new_main_key)
    {
        map.remove(group_key);
    }
}
