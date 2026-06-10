//! Window grouping: attachments, group panels, attached layout
//! computation, panel resize, and native panel input plumbing.

use super::*;

pub(crate) const ATTACHED_PANEL_WIDTH: i32 = 380;

pub(crate) const ATTACHED_PANEL_BOTTOM_HEIGHT: i32 = 280;

pub(crate) const ATTACHED_PANEL_MIN_SIZE: i32 = 160;

pub(crate) const ATTACHED_PANEL_MAX_SIZE: i32 = 700;

pub(crate) const ATTACHED_PANEL_HANDLE_SIZE: i32 = 5;

pub(crate) const ATTACHED_MAIN_MIN_WIDTH: i32 = 320;

pub(crate) const ATTACHED_MAIN_MIN_HEIGHT: i32 = 240;

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GroupPanel {
    pub(crate) webtag_key: String,
    pub(crate) panel_id: String,
    pub(crate) position: WindowsPanelPosition,
    pub(crate) native_kind: NativePanelKind,
    pub(crate) native_title: Option<String>,
    pub(crate) native_body: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NativePanelKind {
    Text,
    Terminal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PanelResizeDrag {
    group_key: String,
    panel_id: String,
    position: WindowsPanelPosition,
    start_point: (i32, i32),
    start_size: i32,
}

pub(crate) static WINDOW_ACTIVE_NATIVE_PANEL: OnceLock<Mutex<Option<String>>> = OnceLock::new();

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

pub(crate) static WINDOW_ACTIVE_GROUP: OnceLock<Mutex<Option<String>>> = OnceLock::new();

pub(crate) static WINDOW_GROUP_PANELS: OnceLock<Mutex<HashMap<String, Vec<GroupPanel>>>> =
    OnceLock::new();

pub(crate) static WINDOW_GROUP_PANEL_SIZES: OnceLock<Mutex<HashMap<String, i32>>> = OnceLock::new();

pub(crate) static WINDOW_PANEL_RESIZE_DRAG: OnceLock<Mutex<Option<PanelResizeDrag>>> =
    OnceLock::new();

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

pub(crate) fn webtag_group_key(webtag_key: &str) -> String {
    WebTag::from(webtag_key).group_key()
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

pub(crate) fn panel_position_for_group(group_key: &str, panel_id: &str) -> WindowsPanelPosition {
    WINDOW_GROUP_LAYOUTS
        .get()
        .and_then(|layouts| layouts.lock().ok())
        .and_then(|layouts| layouts.get(group_key).cloned())
        .and_then(|layout| {
            layout
                .panel_activators
                .into_iter()
                .find(|activator| activator.id == panel_id)
                .map(|activator| activator.position)
        })
        .unwrap_or_default()
}

pub(crate) fn native_panel_key(panel_id: &str) -> String {
    format!("native-panel:{panel_id}")
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
            panel.native_body = Some(body);
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
        if let Some(active) = active_native_panel() {
            removed_active = group_panels
                .iter()
                .any(|panel| panel.webtag_key == webtag_key && panel.panel_id == active);
        }
        group_panels.retain(|panel| panel.webtag_key != webtag_key);
    }
    if removed_active {
        set_active_native_panel(None);
    }
}

pub(crate) fn remove_group_panel_by_panel_id(group_key: &str, panel_id: &str) {
    if let Some(panels) = WINDOW_GROUP_PANELS.get()
        && let Ok(mut panels) = panels.lock()
        && let Some(group_panels) = panels.get_mut(group_key)
    {
        group_panels.retain(|panel| panel.panel_id != panel_id);
    }
    if active_native_panel().as_deref() == Some(panel_id) {
        set_active_native_panel(None);
    }
}

pub(crate) fn group_panels(group_key: &str) -> Vec<GroupPanel> {
    WINDOW_GROUP_PANELS
        .get()
        .and_then(|panels| panels.lock().ok())
        .and_then(|panels| panels.get(group_key).cloned())
        .unwrap_or_default()
}

pub(crate) fn active_native_panel() -> Option<String> {
    WINDOW_ACTIVE_NATIVE_PANEL
        .get()
        .and_then(|active| active.lock().ok())
        .and_then(|active| active.clone())
}

pub(crate) fn set_active_native_panel(panel_id: Option<String>) {
    let active = WINDOW_ACTIVE_NATIVE_PANEL.get_or_init(|| Mutex::new(None));
    if let Ok(mut active) = active.lock() {
        *active = panel_id;
    }
}

#[derive(Debug, Clone)]
pub(crate) struct AttachedGroupRects {
    pub(crate) main: RECT,
    pub(crate) panels: HashMap<String, RECT>,
    pub(crate) resize_handles: HashMap<String, RECT>,
}

pub(crate) fn attached_group_rects(group_key: &str, host: HWND) -> Option<AttachedGroupRects> {
    let mut client = RECT::default();
    unsafe {
        if WindowsAndMessaging::GetClientRect(host, &mut client).is_err() {
            return None;
        }
    }
    let layout = WINDOW_GROUP_LAYOUTS
        .get()
        .and_then(|layouts| layouts.lock().ok())
        .and_then(|layouts| layouts.get(group_key).cloned())
        .unwrap_or_default();
    Some(attached_group_rects_from_layout(
        group_key,
        client,
        &layout,
        group_panels(group_key),
    ))
}

pub(crate) fn attached_group_rects_from_layout(
    group_key: &str,
    client: RECT,
    layout: &WindowsWindowLayout,
    panels: Vec<GroupPanel>,
) -> AttachedGroupRects {
    let panel_gap = renderer_panel_gap();
    let mut main = renderer_content_rect(client, layout);
    let mut panel_rects = HashMap::new();
    let mut resize_handles = HashMap::new();
    let (side_panels, bottom_panels): (Vec<_>, Vec<_>) = panels
        .into_iter()
        .partition(|panel| panel.position != WindowsPanelPosition::Bottom);

    for panel in side_panels.into_iter().chain(bottom_panels) {
        let rect = match panel.position {
            WindowsPanelPosition::Left => {
                let width = attached_panel_width(group_key, &panel, main);
                let rect = RECT {
                    left: main.left,
                    top: main.top,
                    right: (main.left + width).min(main.right),
                    bottom: main.bottom,
                };
                let handle_width = panel_gap.max(ATTACHED_PANEL_HANDLE_SIZE);
                let handle = normalize_rect(RECT {
                    left: rect.right,
                    top: rect.top,
                    right: (rect.right + handle_width).min(main.right),
                    bottom: rect.bottom,
                });
                main.left = handle.right.min(main.right);
                resize_handles.insert(panel.panel_id.clone(), handle);
                rect
            }
            WindowsPanelPosition::Right => {
                let width = attached_panel_width(group_key, &panel, main);
                let rect = RECT {
                    left: (main.right - width).max(main.left),
                    top: main.top,
                    right: main.right,
                    bottom: main.bottom,
                };
                let handle_width = panel_gap.max(ATTACHED_PANEL_HANDLE_SIZE);
                let handle = normalize_rect(RECT {
                    left: (rect.left - handle_width).max(main.left),
                    top: rect.top,
                    right: rect.left,
                    bottom: rect.bottom,
                });
                main.right = handle.left.max(main.left);
                resize_handles.insert(panel.panel_id.clone(), handle);
                rect
            }
            WindowsPanelPosition::Bottom => {
                let height = attached_panel_bottom_height(group_key, &panel, main);
                let rect = RECT {
                    left: main.left,
                    top: (main.bottom - height).max(main.top),
                    right: main.right,
                    bottom: main.bottom,
                };
                let handle_height = panel_gap.max(ATTACHED_PANEL_HANDLE_SIZE);
                let handle = normalize_rect(RECT {
                    left: rect.left,
                    top: (rect.top - handle_height).max(main.top),
                    right: rect.right,
                    bottom: rect.top,
                });
                main.bottom = handle.top.max(main.top);
                resize_handles.insert(panel.panel_id.clone(), handle);
                rect
            }
        };
        panel_rects.insert(panel.webtag_key, normalize_rect(rect));
    }

    AttachedGroupRects {
        main: normalize_rect(main),
        panels: panel_rects,
        resize_handles,
    }
}

pub(crate) fn attached_panel_width(group_key: &str, panel: &GroupPanel, content: RECT) -> i32 {
    let requested = remembered_panel_size(group_key, &panel.panel_id)
        .unwrap_or(ATTACHED_PANEL_WIDTH)
        .max(1);
    clamp_attached_panel_size(panel.position, requested, content)
}

pub(crate) fn attached_panel_bottom_height(
    group_key: &str,
    panel: &GroupPanel,
    content: RECT,
) -> i32 {
    let requested = remembered_panel_size(group_key, &panel.panel_id)
        .unwrap_or(ATTACHED_PANEL_BOTTOM_HEIGHT)
        .max(1);
    clamp_attached_panel_size(panel.position, requested, content)
}

pub(crate) fn clamp_attached_panel_size(
    position: WindowsPanelPosition,
    requested: i32,
    content: RECT,
) -> i32 {
    let available = match position {
        WindowsPanelPosition::Bottom => rect_height(&content),
        WindowsPanelPosition::Left | WindowsPanelPosition::Right => rect_width(&content),
    };
    if available <= 0 {
        return 0;
    }

    let panel_gap = renderer_panel_gap();
    let max_with_main = match position {
        WindowsPanelPosition::Bottom => available - panel_gap - ATTACHED_MAIN_MIN_HEIGHT,
        WindowsPanelPosition::Left | WindowsPanelPosition::Right => {
            available - panel_gap - ATTACHED_MAIN_MIN_WIDTH
        }
    };
    let max_size = if max_with_main > 0 {
        max_with_main
    } else {
        available / 2
    }
    .min(ATTACHED_PANEL_MAX_SIZE)
    .min(available)
    .max(1);
    let min_size = ATTACHED_PANEL_MIN_SIZE.min(max_size);
    requested.clamp(min_size, max_size)
}

pub(crate) fn panel_size_key(group_key: &str, panel_id: &str) -> String {
    format!("{group_key}::{panel_id}")
}

pub(crate) fn remembered_panel_size(group_key: &str, panel_id: &str) -> Option<i32> {
    WINDOW_GROUP_PANEL_SIZES
        .get()
        .and_then(|sizes| sizes.lock().ok())
        .and_then(|sizes| sizes.get(&panel_size_key(group_key, panel_id)).copied())
}

pub(crate) fn set_remembered_panel_size(group_key: &str, panel_id: &str, size: i32) {
    let sizes = WINDOW_GROUP_PANEL_SIZES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut sizes) = sizes.lock() {
        sizes.insert(panel_size_key(group_key, panel_id), size.max(1));
    }
}

pub(crate) fn layout_group_windows(group_key: &str) {
    let Some(host) = host_handle_for_group(group_key) else {
        return;
    };
    let Some(rects) = attached_group_rects(group_key, host) else {
        return;
    };
    let active_main = group_active_main(group_key);
    let attachments = WINDOW_ATTACHMENTS
        .get()
        .and_then(|attachments| attachments.lock().ok())
        .map(|attachments| {
            attachments
                .iter()
                .filter(|(_, attachment)| attachment.group_key == group_key)
                .map(|(webtag_key, attachment)| (webtag_key.clone(), attachment.clone()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    for (webtag_key, attachment) in attachments {
        let Some(hwnd) = window_handle_for_key(&webtag_key) else {
            continue;
        };
        match attachment.kind {
            WindowAttachmentKind::MainHost => {}
            WindowAttachmentKind::MainChild => {
                let visible = active_main.as_deref() == Some(webtag_key.as_str());
                set_attached_window_rect(hwnd, rects.main, visible);
            }
            WindowAttachmentKind::Panel { .. } => {
                let Some(rect) = rects.panels.get(&webtag_key).copied() else {
                    hide_attached_window(hwnd);
                    continue;
                };
                set_attached_window_rect(hwnd, rect, true);
            }
        }
    }

    unsafe {
        let _ = InvalidateRect(Some(host), None, false);
    }
}

pub(crate) fn layout_group_for_main_host(webtag_key: &str) {
    if !matches!(
        window_attachment(webtag_key).map(|attachment| attachment.kind),
        Some(WindowAttachmentKind::MainHost)
    ) {
        return;
    }
    layout_group_windows(&layout_group_key_for_webtag(webtag_key));
}

pub(crate) fn request_group_shell_refresh(group_key: &str) {
    let Some(host) = host_handle_for_group(group_key) else {
        return;
    };
    unsafe {
        let _ = WindowsAndMessaging::PostMessageW(
            Some(host),
            WM_LINGXIA_LAYOUT,
            WPARAM::default(),
            LPARAM::default(),
        );
        let _ = InvalidateRect(Some(host), None, false);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PanelResizeHit {
    group_key: String,
    panel_id: String,
    position: WindowsPanelPosition,
    current_size: i32,
}

pub(crate) fn panel_resize_hit_test(
    webtag_key: &str,
    client: RECT,
    layout: &WindowsWindowLayout,
    point: (i32, i32),
) -> Option<PanelResizeHit> {
    if !window_attachment(webtag_key)
        .is_some_and(|attachment| matches!(attachment.kind, WindowAttachmentKind::MainHost))
    {
        return None;
    }

    let group_key = layout_group_key_for_webtag(webtag_key);
    let panels = group_panels(&group_key);
    if panels.is_empty() {
        return None;
    }
    let rects = attached_group_rects_from_layout(&group_key, client, layout, panels.clone());
    panels.into_iter().find_map(|panel| {
        let handle = rects.resize_handles.get(&panel.panel_id).copied()?;
        if !rect_contains(&handle, point) {
            return None;
        }
        let panel_rect = rects.panels.get(&panel.webtag_key).copied()?;
        let current_size = match panel.position {
            WindowsPanelPosition::Bottom => rect_height(&panel_rect),
            WindowsPanelPosition::Left | WindowsPanelPosition::Right => rect_width(&panel_rect),
        };
        Some(PanelResizeHit {
            group_key: group_key.clone(),
            panel_id: panel.panel_id,
            position: panel.position,
            current_size,
        })
    })
}

pub(crate) fn handle_window_chrome_mouse_down(
    hwnd: HWND,
    webtag_key: &str,
    point: (i32, i32),
) -> bool {
    if !window_draws_shell_chrome(webtag_key) {
        return false;
    }
    let layout = current_window_layout(webtag_key);
    let mut client = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetClientRect(hwnd, &mut client);
    }
    let Some(hit) = panel_resize_hit_test(webtag_key, client, &layout, point) else {
        return false;
    };
    let drag = PanelResizeDrag {
        group_key: hit.group_key,
        panel_id: hit.panel_id,
        position: hit.position,
        start_point: point,
        start_size: hit.current_size,
    };
    let state = WINDOW_PANEL_RESIZE_DRAG.get_or_init(|| Mutex::new(None));
    if let Ok(mut state) = state.lock() {
        *state = Some(drag);
    }
    unsafe {
        let _ = SetCapture(hwnd);
    }
    true
}

pub(crate) fn handle_window_chrome_mouse_move(_hwnd: HWND, point: (i32, i32)) -> bool {
    let drag = WINDOW_PANEL_RESIZE_DRAG
        .get()
        .and_then(|state| state.lock().ok())
        .and_then(|state| state.clone());
    let Some(drag) = drag else {
        return false;
    };

    let delta = match drag.position {
        WindowsPanelPosition::Left => point.0 - drag.start_point.0,
        WindowsPanelPosition::Right => drag.start_point.0 - point.0,
        WindowsPanelPosition::Bottom => drag.start_point.1 - point.1,
    };
    let requested = (drag.start_size + delta).max(1);
    let Some(host) = host_handle_for_group(&drag.group_key) else {
        return true;
    };
    let mut client = RECT::default();
    unsafe {
        if WindowsAndMessaging::GetClientRect(host, &mut client).is_err() {
            return true;
        }
    }
    let content = WINDOW_GROUP_LAYOUTS
        .get()
        .and_then(|layouts| layouts.lock().ok())
        .and_then(|layouts| layouts.get(&drag.group_key).cloned())
        .map(|layout| renderer_content_rect(client, &layout))
        .unwrap_or(client);
    let clamped = clamp_attached_panel_size(drag.position, requested, content);
    set_remembered_panel_size(&drag.group_key, &drag.panel_id, clamped);
    layout_group_windows(&drag.group_key);
    request_group_shell_refresh(&drag.group_key);
    true
}

pub(crate) fn handle_window_chrome_mouse_up(hwnd: HWND) -> bool {
    let state = WINDOW_PANEL_RESIZE_DRAG.get_or_init(|| Mutex::new(None));
    let had_drag = state
        .lock()
        .map(|mut state| state.take().is_some())
        .unwrap_or(false);
    if had_drag {
        unsafe {
            let _ = ReleaseCapture();
            let _ = InvalidateRect(Some(hwnd), None, false);
        }
    }
    had_drag
}

/// Builds the chrome renderer's view of a host window: client rect, layout,
/// and (for group hosts with panels) the attached-group geometry.
pub(crate) fn chrome_state_for_window(hwnd: HWND, webtag_key: &str) -> WindowsChromeState {
    let mut client = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetClientRect(hwnd, &mut client);
    }
    let layout = current_window_layout(webtag_key);

    let attached = window_attachment(webtag_key)
        .filter(|attachment| matches!(attachment.kind, WindowAttachmentKind::MainHost))
        .and_then(|_| {
            let group_key = layout_group_key_for_webtag(webtag_key);
            let panels = group_panels(&group_key);
            if panels.is_empty() {
                return None;
            }
            let rects =
                attached_group_rects_from_layout(&group_key, client, &layout, panels.clone());
            let panels = panels
                .into_iter()
                .filter_map(|panel| {
                    let rect = rects.panels.get(&panel.webtag_key).copied()?;
                    let native = panel
                        .native_title
                        .is_some()
                        .then(|| WindowsNativePanelContent {
                            kind: match panel.native_kind {
                                NativePanelKind::Text => WindowsNativePanelKind::Text,
                                NativePanelKind::Terminal => WindowsNativePanelKind::Terminal,
                            },
                            title: panel.native_title.clone(),
                            body: panel.native_body.clone(),
                        });
                    Some(WindowsChromePanel {
                        panel_id: panel.panel_id,
                        rect,
                        native,
                    })
                })
                .collect();
            Some(WindowsChromeAttachedState {
                main: rects.main,
                panels,
            })
        });

    let (frame_button_hover, frame_button_pressed) = frame_button_visual_state(hwnd);
    WindowsChromeState {
        hwnd,
        client,
        layout,
        attached,
        frame_button_hover,
        frame_button_pressed,
    }
}

pub(crate) fn handle_native_panel_char(wparam: WPARAM) -> bool {
    let Some(character) = char::from_u32(wparam.0 as u32) else {
        return false;
    };
    let (ctrl, shift, alt) = keyboard_modifiers();
    invoke_native_panel_input(WindowsPanelKeyEvent {
        vk: 0,
        ctrl,
        shift,
        alt,
        character: Some(character),
    })
}

pub(crate) fn handle_native_panel_keydown(wparam: WPARAM) -> bool {
    let (ctrl, shift, alt) = keyboard_modifiers();
    invoke_native_panel_input(WindowsPanelKeyEvent {
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

pub(crate) fn invoke_native_panel_input(event: WindowsPanelKeyEvent) -> bool {
    let Some(panel_id) = active_native_panel() else {
        return false;
    };
    let Some(handler) = WINDOW_NATIVE_PANEL_INPUT_HANDLERS
        .get()
        .and_then(|handlers| handlers.lock().ok())
        .and_then(|handlers| handlers.get(&panel_id).cloned())
    else {
        return false;
    };
    handler(event)
}
