//! Attached group geometry, panel resize, and chrome state assembly.

use super::*;

pub(crate) const ATTACHED_PANEL_WIDTH: i32 = 380;

pub(crate) const ATTACHED_PANEL_BOTTOM_HEIGHT: i32 = 280;

pub(crate) const ATTACHED_PANEL_MIN_SIZE: i32 = 160;

pub(crate) const ATTACHED_PANEL_MAX_SIZE: i32 = 700;

pub(crate) const ATTACHED_PANEL_HANDLE_SIZE: i32 = 5;

pub(crate) const ATTACHED_MAIN_MIN_WIDTH: i32 = 320;

pub(crate) const ATTACHED_MAIN_MIN_HEIGHT: i32 = 240;

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

    // A maximized docked panel takes over the whole app area the renderer
    // grants it (typically everything below the caption strip, covering the
    // sidebar); the main card collapses and every other panel hides.
    if let Some(maximized) = panels
        .iter()
        .find(|panel| panel.docked() && panel.maximized)
    {
        panel_rects.insert(
            maximized.webtag_key.clone(),
            renderer_maximized_panel_rect(client, layout),
        );
        main.bottom = main.top;
        return AttachedGroupRects {
            main,
            panels: panel_rects,
            resize_handles,
        };
    }

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
            WindowsPanelPosition::Bottom if panel.docked() => {
                // Compact dock: the panel sits flush under the main card
                // (gap 0); the top strip of the panel itself doubles as the
                // thin draggable divider (the renderer draws it; the strip
                // is host-owned chrome, so the host receives the drag's
                // mouse messages). The maximized case returns early above.
                let height = attached_panel_bottom_height(group_key, &panel, main);
                let rect = RECT {
                    left: main.left,
                    top: (main.bottom - height).max(main.top),
                    right: main.right,
                    bottom: main.bottom,
                };
                let handle = normalize_rect(RECT {
                    left: rect.left,
                    top: rect.top,
                    right: rect.right,
                    bottom: (rect.top + ATTACHED_PANEL_HANDLE_SIZE).min(rect.bottom),
                });
                main.bottom = rect.top.max(main.top);
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

/// Repaints only the host-window region of one panel, identified by its
/// panel id. Content-only updates (e.g. terminal output frames) use this
/// instead of [`request_group_shell_refresh`] so the rest of the chrome
/// (sidebar, top bar) is not repainted dozens of times per second.
pub(crate) fn request_group_panel_repaint(group_key: &str, panel_id: &str) {
    let Some(host) = host_handle_for_group(group_key) else {
        return;
    };
    let panel_rect = group_panels(group_key)
        .into_iter()
        .find(|panel| panel.panel_id == panel_id)
        .and_then(|panel| {
            attached_group_rects(group_key, host)
                .and_then(|rects| rects.panels.get(&panel.webtag_key).copied())
        });
    match panel_rect {
        Some(rect) => unsafe {
            let _ = InvalidateRect(Some(host), Some(&rect), false);
        },
        // Unknown rect (panel not laid out yet): fall back to a full refresh.
        None => request_group_shell_refresh(group_key),
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
                    let docked = panel.docked();
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
                            tabs: panel.native_tabs.clone(),
                            maximized: panel.maximized,
                        });
                    Some(WindowsChromePanel {
                        panel_id: panel.panel_id,
                        rect,
                        native,
                        docked,
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
