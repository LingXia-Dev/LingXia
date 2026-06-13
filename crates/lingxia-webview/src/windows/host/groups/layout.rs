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

    // Overlays must end up above the main content, so position them in a
    // second pass — set_attached_window_rect raises each window to HWND_TOP,
    // and the main/panels are laid out first.
    let mut overlays = Vec::new();
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
            WindowAttachmentKind::Overlay => overlays.push((webtag_key, hwnd)),
        }
    }

    if !overlays.is_empty() {
        let dpi = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(host) };
        for (webtag_key, hwnd) in overlays {
            let placement = overlay_placement_for(&webtag_key).unwrap_or_default();
            let card = overlay_card_rect(rects.main, &placement, dpi);
            set_attached_window_rect(hwnd, card, true);
            // The card layers over arbitrary page content, so the corner-cap
            // decorator (which paints corners in the window-background color)
            // cannot round it; clip the card window to a rounded rect so the
            // content behind shows through at the corners.
            apply_overlay_corner_region(hwnd, rect_width(&card), rect_height(&card), dpi);
        }
    }

    unsafe {
        let _ = InvalidateRect(Some(host), None, false);
    }
}

/// Logical (DIP) corner radius of an overlay card, matching the macOS card.
const OVERLAY_CORNER_RADIUS_DIP: i32 = 12;

/// Clips an overlay card window to a rounded rectangle so its corners reveal
/// the page content layered behind it. The system owns the region after
/// `SetWindowRgn` succeeds, so it must not be deleted here.
fn apply_overlay_corner_region(hwnd: HWND, width: i32, height: i32, dpi: u32) {
    if width <= 0 || height <= 0 {
        return;
    }
    let scale = if dpi == 0 { 1.0 } else { dpi as f64 / 96.0 };
    let diameter = (((OVERLAY_CORNER_RADIUS_DIP as f64) * scale).round() as i32 * 2).max(2);
    unsafe {
        let region = windows::Win32::Graphics::Gdi::CreateRoundRectRgn(
            0,
            0,
            width + 1,
            height + 1,
            diameter,
            diameter,
        );
        let _ = windows::Win32::Graphics::Gdi::SetWindowRgn(hwnd, Some(region), true);
    }
}

/// Resolves an overlay card's rect within the content area: logical
/// width/height (DPI-scaled), else a ratio of the content area, else a
/// generous default, anchored per the requested position (e.g. `Bottom` sits
/// the card flush against the content area's bottom edge).
fn overlay_card_rect(content: RECT, placement: &OverlayCardPlacement, dpi: u32) -> RECT {
    let area_w = (content.right - content.left).max(1);
    let area_h = (content.bottom - content.top).max(1);
    let scale = if dpi == 0 { 1.0 } else { dpi as f64 / 96.0 };

    let mut width = if placement.width > 0.0 {
        (placement.width * scale).round() as i32
    } else if placement.width_ratio > 0.0 {
        (area_w as f64 * placement.width_ratio.min(1.0)).round() as i32
    } else {
        (area_w as f64 * 0.9).round() as i32
    };
    let mut height = if placement.height > 0.0 {
        (placement.height * scale).round() as i32
    } else if placement.height_ratio > 0.0 {
        (area_h as f64 * placement.height_ratio.min(1.0)).round() as i32
    } else {
        (area_h as f64 * 0.55).round() as i32
    };
    width = width.clamp(160.min(area_w), area_w);
    height = height.clamp(120.min(area_h), area_h);

    let center_x = content.left + (area_w - width) / 2;
    let center_y = content.top + (area_h - height) / 2;
    let (left, top) = match placement.anchor {
        OverlayAnchor::Center => (center_x, center_y),
        OverlayAnchor::Bottom => (center_x, content.bottom - height),
        OverlayAnchor::Top => (center_x, content.top),
        OverlayAnchor::Left => (content.left, center_y),
        OverlayAnchor::Right => (content.right - width, center_y),
    };
    RECT {
        left,
        top,
        right: left + width,
        bottom: top + height,
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
