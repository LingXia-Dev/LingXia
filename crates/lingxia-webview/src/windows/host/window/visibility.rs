//! Host window show/hide and layout visibility flows.

use super::*;

pub(crate) fn show_group_host(group_key: &str, host: HWND, title: &str, activate: bool) {
    let host_visible = unsafe { WindowsAndMessaging::IsWindowVisible(host).as_bool() };
    let host_zoomed = unsafe { WindowsAndMessaging::IsZoomed(host).as_bool() };
    if !host_visible
        && !host_zoomed
        && let Some(placement) = current_group_window_placement_for_group(group_key)
    {
        unsafe {
            let _ = WindowsAndMessaging::SetWindowPos(
                host,
                None,
                placement.left,
                placement.top,
                placement.width,
                placement.height,
                WindowsAndMessaging::SWP_NOZORDER | WindowsAndMessaging::SWP_NOACTIVATE,
            );
        }
    }

    fit_window_to_work_area(host);

    // With custom chrome the renderer paints the title area itself; plain
    // OS-frame windows keep the real title and title-bar icon.
    let custom_chrome = windows_chrome_renderer().is_some();
    let title = to_wide(if custom_chrome { "" } else { title });
    unsafe {
        let _ = WindowsAndMessaging::SetWindowTextW(host, PCWSTR(title.as_ptr()));
        let mut flags = WindowsAndMessaging::SWP_NOMOVE | WindowsAndMessaging::SWP_NOSIZE;
        if !activate {
            flags |= WindowsAndMessaging::SWP_NOACTIVATE;
        }
        let _ = WindowsAndMessaging::SetWindowPos(
            host,
            None,
            0,
            0,
            0,
            0,
            flags | WindowsAndMessaging::SWP_SHOWWINDOW,
        );
        if activate {
            let _ = WindowsAndMessaging::BringWindowToTop(host);
            let _ = WindowsAndMessaging::SetForegroundWindow(host);
        }
    }
}

pub(crate) fn show_native_window(
    state: &mut UiState,
    title: &str,
    activate: bool,
    role: WindowsWindowRole,
) -> StdResult<()> {
    match role {
        WindowsWindowRole::Main => show_native_main_window(state, title, activate),
        WindowsWindowRole::Panel { panel_id } => show_host_panel_window(state, &panel_id),
    }
}

pub(crate) fn show_native_main_window(
    state: &mut UiState,
    title: &str,
    activate: bool,
) -> StdResult<()> {
    let (group_key, host, is_host) = ensure_main_attachment(state);
    set_active_group(&group_key);
    set_group_active_main(&group_key, &state.webtag_key);
    // A regular main webview taking over the main surface ends any
    // in-flight presentation (there is nothing left to restore).
    clear_presented_main_for_new_main(&group_key, &state.webtag_key);

    if is_host {
        show_group_host(&group_key, host, title, activate);
        sync_controller_bounds(state)?;
        layout_group_windows(&group_key);
        set_controller_visible(state, true)?;
    } else {
        attach_child_window_to_host(state.hwnd, host);
        show_group_host(&group_key, host, title, activate);
        layout_group_windows(&group_key);
        sync_controller_bounds(state)?;
        set_controller_visible(state, true)?;
    }

    request_group_chrome_refresh(&group_key);
    state.window_visible = true;
    store_current_window_placement(state);
    Ok(())
}

/// Presents this window as the main-content child of `group_key`'s host:
/// reparents it into the host (same SetParent/child-style machinery as
/// attached main children), makes it the group's active main surface over
/// the main card rect, and remembers the displaced main webview for
/// `restore_presented_group_main`.
pub(crate) fn present_native_window_as_group_main(
    state: &mut UiState,
    group_key: &str,
) -> StdResult<()> {
    let Some(host) = host_handle_for_group(group_key) else {
        return Err(WebViewError::WebView(format!(
            "no host window for Windows host group {group_key}"
        )));
    };
    if hwnd_handle(host) == hwnd_handle(state.hwnd) {
        return Err(WebViewError::WebView(
            "cannot present a group host window as its own main child".to_string(),
        ));
    }

    register_window_handle(&state.webtag_key, state.hwnd);
    let previous_main = group_active_main(group_key)
        .filter(|previous| previous.as_str() != state.webtag_key.as_str());
    if previous_main.is_some() || group_active_main(group_key).is_none() {
        remember_presented_main(group_key, &state.webtag_key, previous_main);
    }

    attach_child_window_to_host(state.hwnd, host);
    set_window_attachment(
        &state.webtag_key,
        WindowAttachment {
            group_key: group_key.to_string(),
            kind: WindowAttachmentKind::MainChild,
        },
    );
    set_group_active_main(group_key, &state.webtag_key);
    layout_group_windows(group_key);
    sync_controller_bounds(state)?;
    set_controller_visible(state, true)?;
    request_group_chrome_refresh(group_key);
    state.window_visible = true;
    Ok(())
}

pub(crate) fn show_host_panel_window(state: &mut UiState, panel_id: &str) -> StdResult<()> {
    register_window_handle(&state.webtag_key, state.hwnd);
    let group_key = active_group_key().unwrap_or_else(|| webtag_group_key(&state.webtag_key));
    let Some(host) = host_handle_for_group(&group_key) else {
        return show_native_main_window(state, "", true);
    };
    let position = panel_position_for_group(&group_key, panel_id);
    attach_child_window_to_host(state.hwnd, host);
    set_window_attachment(
        &state.webtag_key,
        WindowAttachment {
            group_key: group_key.clone(),
            kind: WindowAttachmentKind::Panel {
                panel_id: panel_id.to_string(),
                position,
            },
        },
    );
    register_group_panel(
        &group_key,
        GroupPanel {
            webtag_key: state.webtag_key.clone(),
            panel_id: panel_id.to_string(),
            position,
            host_title: None,
            host_body: None,
            host_tabs: Vec::new(),
            maximized: false,
        },
    );
    set_controller_visible(state, true)?;
    layout_group_windows(&group_key);
    request_group_chrome_refresh(&group_key);
    state.window_visible = true;
    Ok(())
}

pub(crate) fn hide_native_window(state: &mut UiState) -> StdResult<()> {
    store_current_window_placement(state);
    match window_attachment(&state.webtag_key).map(|attachment| attachment.kind) {
        Some(WindowAttachmentKind::MainHost) => hide_native_main_host_window(state),
        Some(WindowAttachmentKind::MainChild) => {
            set_controller_visible(state, false)?;
            hide_attached_window(state.hwnd);
            state.window_visible = false;
            Ok(())
        }
        Some(WindowAttachmentKind::Panel { .. }) => {
            let group_key = layout_group_key_for_webtag(&state.webtag_key);
            set_controller_visible(state, false)?;
            hide_attached_window(state.hwnd);
            remove_group_panel(&group_key, &state.webtag_key);
            layout_group_windows(&group_key);
            request_group_chrome_refresh(&group_key);
            state.window_visible = false;
            Ok(())
        }
        None => hide_detached_window(state),
    }
}

pub(crate) fn hide_native_main_host_window(state: &mut UiState) -> StdResult<()> {
    let group_key = layout_group_key_for_webtag(&state.webtag_key);
    if group_active_main(&group_key).as_deref() != Some(state.webtag_key.as_str()) {
        set_controller_visible(state, false)?;
        state.window_visible = false;
        return Ok(());
    }
    hide_detached_window(state)
}

pub(crate) fn hide_detached_window(state: &mut UiState) -> StdResult<()> {
    set_controller_visible(state, false)?;
    // A hidden group host drops its main-card corner caps; they are
    // recreated by the next bounds sync when the window shows again.
    destroy_corner_caps(state.hwnd);
    unsafe {
        let _ = WindowsAndMessaging::SetWindowPos(
            state.hwnd,
            None,
            0,
            0,
            0,
            0,
            WindowsAndMessaging::SWP_NOMOVE
                | WindowsAndMessaging::SWP_NOSIZE
                | WindowsAndMessaging::SWP_NOZORDER
                | WindowsAndMessaging::SWP_NOACTIVATE
                | WindowsAndMessaging::SWP_HIDEWINDOW,
        );
    }
    state.window_visible = false;
    Ok(())
}

pub(crate) fn set_controller_visible(state: &UiState, visible: bool) -> StdResult<()> {
    unsafe {
        state
            .controller
            .SetIsVisible(visible)
            .map_err(|err| WebViewError::WebView(format!("SetIsVisible failed: {err}")))?;
    }
    if visible {
        // WebView2 may reorder its child-window chain while it becomes
        // visible; keep the corner caps above the webview surface.
        raise_corner_caps(state.hwnd);
    }
    Ok(())
}

pub(crate) fn set_native_window_layout(
    state: &UiState,
    layout: WindowsWindowLayout,
) -> StdResult<()> {
    // Layout payloads are opaque to lingxia-webview. Avoid comparing or
    // interpreting host chrome state here; the renderer owns that model.
    set_window_layout_for_key(&state.webtag_key, layout);
    sync_controller_bounds(state)?;
    if let Some(attachment) = window_attachment(&state.webtag_key)
        && !matches!(attachment.kind, WindowAttachmentKind::Panel { .. })
    {
        layout_group_windows(&attachment.group_key);
        request_group_chrome_refresh(&attachment.group_key);
    }
    unsafe {
        let _ = InvalidateRect(Some(state.hwnd), None, false);
    }
    Ok(())
}
