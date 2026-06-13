//! Attached host-panel resize state and hit testing.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PanelResizeDrag {
    group_key: String,
    panel_id: String,
    position: WindowsPanelPosition,
    start_point: (i32, i32),
    start_size: i32,
}

pub(crate) static WINDOW_PANEL_RESIZE_DRAG: OnceLock<Mutex<Option<PanelResizeDrag>>> =
    OnceLock::new();

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
    if !window_draws_host_chrome(webtag_key) {
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
    set_remembered_panel_size(&drag.group_key, &drag.panel_id, requested);
    layout_group_windows(&drag.group_key);
    request_group_chrome_refresh(&drag.group_key);
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
