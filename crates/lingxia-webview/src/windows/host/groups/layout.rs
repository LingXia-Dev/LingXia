//! Attached group geometry application and chrome state assembly.

use super::*;

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
    let inputs = panels
        .iter()
        .map(|panel| WindowsChromePanelLayoutInput {
            panel_id: panel.panel_id.clone(),
            webtag_key: panel.webtag_key.clone(),
            position: panel.position,
            requested_size: remembered_panel_size(group_key, &panel.panel_id),
            docked: panel.docked(),
            maximized: panel.maximized,
        })
        .collect::<Vec<_>>();

    let Some(attached) = renderer_attached_layout(client, layout, &inputs) else {
        return AttachedGroupRects {
            main: renderer_content_rect(client, layout),
            panels: HashMap::new(),
            resize_handles: HashMap::new(),
        };
    };

    let mut panel_rects = HashMap::new();
    let mut resize_handles = HashMap::new();
    for panel in attached.panels {
        panel_rects.insert(panel.webtag_key, normalize_rect(panel.rect));
        if let Some(handle) = panel.resize_handle {
            resize_handles.insert(panel.panel_id, normalize_rect(handle));
        }
    }

    AttachedGroupRects {
        main: normalize_rect(attached.main),
        panels: panel_rects,
        resize_handles,
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

pub(crate) fn request_group_chrome_refresh(group_key: &str) {
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
/// panel id. Content-only updates for host-drawn panels use this
/// instead of [`request_group_chrome_refresh`] so the rest of the chrome
/// (rail, top bar) is not repainted dozens of times per second.
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
        None => request_group_chrome_refresh(group_key),
    }
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
                    let host_content =
                        panel.host_title.is_some().then(|| WindowsHostPanelContent {
                            title: panel.host_title.clone(),
                            body: panel.host_body.clone(),
                            tabs: panel.host_tabs.clone(),
                            maximized: panel.maximized,
                        });
                    Some(WindowsChromePanel {
                        panel_id: panel.panel_id,
                        rect,
                        host_content,
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
