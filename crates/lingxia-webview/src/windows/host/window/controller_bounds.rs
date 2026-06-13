//! WebView2 controller bounds and window snapshots.

use super::*;

/// Drops the per-window layout caches and corner caps of a window that is
/// going away.
pub(crate) fn forget_window_layout_state(hwnd: HWND) {
    destroy_corner_caps(hwnd);
    forget_live_layout_rect(hwnd);
    let key = hwnd_handle(hwnd);
    if let Some(bounds) = LAST_CONTROLLER_BOUNDS.get()
        && let Ok(mut bounds) = bounds.lock()
    {
        bounds.remove(&key);
    }
}

pub(crate) fn sync_controller_bounds(state: &UiState) -> StdResult<()> {
    sync_controller_bounds_for(state.hwnd, &state.webtag_key, &state.controller)
}

/// Last bounds applied to each window's WebView2 controller. The controller
/// resize is the expensive part of a layout pass, and the interactive
/// move/size paths re-enter the layout far more often than the bounds
/// actually change, so unchanged `SetBounds` calls are skipped.
static LAST_CONTROLLER_BOUNDS: OnceLock<Mutex<HashMap<isize, (i32, i32, i32, i32)>>> =
    OnceLock::new();

pub(crate) fn sync_controller_bounds_for(
    hwnd: HWND,
    webtag_key: &str,
    controller: &ICoreWebView2Controller,
) -> StdResult<()> {
    let mut rect = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetClientRect(hwnd, &mut rect);
    }
    if rect.right <= rect.left || rect.bottom <= rect.top {
        rect = RECT {
            left: 0,
            top: 0,
            right: 1024,
            bottom: 768,
        };
    }
    let rect = controller_bounds_for_window(hwnd, webtag_key, rect);

    let bounds = (rect.left, rect.top, rect.right, rect.bottom);
    let cache = LAST_CONTROLLER_BOUNDS.get_or_init(|| Mutex::new(HashMap::new()));
    let unchanged = cache
        .lock()
        .map(|cache| cache.get(&hwnd_handle(hwnd)) == Some(&bounds))
        .unwrap_or(false);
    if !unchanged {
        unsafe {
            controller
                .SetBounds(rect)
                .map_err(|err| WebViewError::WebView(format!("SetBounds failed: {err}")))?;
        }
        if let Ok(mut cache) = cache.lock() {
            cache.insert(hwnd_handle(hwnd), bounds);
        }
    }
    // A group host's own main card is not an attached child window; its
    // corner caps are managed here, where its card rect is known. Attached
    // cards get theirs from `set_attached_window_rect`.
    if matches!(
        window_attachment(webtag_key).map(|attachment| attachment.kind),
        Some(WindowAttachmentKind::MainHost)
    ) {
        update_corner_caps(hwnd, rect);
    }
    Ok(())
}

pub(crate) fn controller_bounds_for_window(hwnd: HWND, webtag_key: &str, client: RECT) -> RECT {
    match window_attachment(webtag_key) {
        Some(WindowAttachment {
            kind: WindowAttachmentKind::MainChild | WindowAttachmentKind::Panel { .. },
            ..
        }) => normalize_rect(client),
        Some(WindowAttachment {
            group_key,
            kind: WindowAttachmentKind::MainHost,
        }) => {
            let content = renderer_content_rect(client, &current_window_layout(webtag_key));
            attached_group_rects(&group_key, hwnd)
                .map(|rects| rects.main)
                .unwrap_or(content)
        }
        None => renderer_content_rect(client, &current_window_layout(webtag_key)),
    }
}

pub(crate) fn window_snapshot(state: &UiState) -> StdResult<WindowsWebViewWindowSnapshot> {
    let mut window_rect = RECT::default();
    let mut client_rect = RECT::default();
    let mut client_origin = POINT { x: 0, y: 0 };

    let window_id = if let Some(attachment) = window_attachment(&state.webtag_key) {
        if matches!(
            attachment.kind,
            WindowAttachmentKind::MainChild | WindowAttachmentKind::Panel { .. }
        ) {
            let host = host_handle_for_group(&attachment.group_key).unwrap_or(state.hwnd);
            unsafe {
                WindowsAndMessaging::GetWindowRect(host, &mut window_rect)
                    .map_err(|err| WebViewError::WebView(format!("GetWindowRect failed: {err}")))?;
            }
            hwnd_handle(host) as usize
        } else {
            unsafe {
                WindowsAndMessaging::GetWindowRect(state.hwnd, &mut window_rect)
                    .map_err(|err| WebViewError::WebView(format!("GetWindowRect failed: {err}")))?;
            }
            hwnd_handle(state.hwnd) as usize
        }
    } else {
        unsafe {
            WindowsAndMessaging::GetWindowRect(state.hwnd, &mut window_rect)
                .map_err(|err| WebViewError::WebView(format!("GetWindowRect failed: {err}")))?;
        }
        hwnd_handle(state.hwnd) as usize
    };

    unsafe {
        WindowsAndMessaging::GetClientRect(state.hwnd, &mut client_rect)
            .map_err(|err| WebViewError::WebView(format!("GetClientRect failed: {err}")))?;
        if !ClientToScreen(state.hwnd, &mut client_origin).as_bool() {
            return Err(WebViewError::WebView("ClientToScreen failed".to_string()));
        }
    }

    let content = controller_bounds_for_window(state.hwnd, &state.webtag_key, client_rect);
    let content_left = client_origin.x - window_rect.left + content.left;
    let content_top = client_origin.y - window_rect.top + content.top;
    let content_width = rect_width(&content) as u32;
    let content_height = rect_height(&content) as u32;

    Ok(WindowsWebViewWindowSnapshot {
        window_id,
        webtag_key: state.webtag_key.clone(),
        visible: state.window_visible
            && unsafe { WindowsAndMessaging::IsWindowVisible(state.hwnd).as_bool() },
        content_left,
        content_top,
        content_width,
        content_height,
    })
}
