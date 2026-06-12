//! Custom chrome hit testing and frame-button interaction.

use super::*;

pub(crate) fn handle_window_frame_hit_test(hwnd: HWND, webtag_key: &str, lparam: LPARAM) -> u32 {
    if !window_draws_shell_chrome(webtag_key) {
        return WindowsAndMessaging::HTCLIENT;
    }

    let mut client = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetClientRect(hwnd, &mut client);
    }
    let point = lparam_screen_to_client(hwnd, lparam);
    if let Some(hit) = window_resize_hit_test(hwnd, client, point) {
        return hit;
    }

    let Some(renderer) = windows_chrome_renderer() else {
        return WindowsAndMessaging::HTCLIENT;
    };
    let state = chrome_state_for_window(hwnd, webtag_key);
    match renderer.hit_test(&state, point) {
        Some(WindowsChromeHit::Caption) => WindowsAndMessaging::HTCAPTION,
        // Win11 Snap Layouts: the flyout only appears when WM_NCHITTEST
        // reports HTMAXBUTTON over the maximize button. DefWindowProc does
        // not click client-drawn snap buttons, so the click itself is
        // performed in WM_NCLBUTTONDOWN/WM_NCLBUTTONUP. Minimize and close
        // stay HTCLIENT and keep their client-message click handling.
        Some(WindowsChromeHit::FrameButton(WindowsFrameButton::Maximize)) => {
            WindowsAndMessaging::HTMAXBUTTON
        }
        _ => WindowsAndMessaging::HTCLIENT,
    }
}

/// Hover/pressed frame-button state of a window, surfaced to the chrome
/// renderer through [`WindowsChromeState`].
pub(crate) fn frame_button_visual_state(
    hwnd: HWND,
) -> (Option<WindowsFrameButton>, Option<WindowsFrameButton>) {
    with_window_user_data(hwnd, |data| {
        (
            data.hovered_frame_button.get(),
            data.pressed_frame_button.get(),
        )
    })
    .unwrap_or((None, None))
}

/// Invalidates just the rect of one frame button (no full-window flicker on
/// hover changes). Falls back to a full invalidation when the renderer does
/// not expose button rects.
fn invalidate_frame_button(hwnd: HWND, button: WindowsFrameButton) {
    let Some(renderer) = windows_chrome_renderer() else {
        return;
    };
    let Some(webtag_key) = window_webtag_key(hwnd) else {
        return;
    };
    let state = chrome_state_for_window(hwnd, &webtag_key);
    match renderer.frame_button_rect(&state, button) {
        Some(rect) => unsafe {
            let _ = InvalidateRect(Some(hwnd), Some(&rect), false);
        },
        None => unsafe {
            let _ = InvalidateRect(Some(hwnd), None, false);
        },
    }
}

fn set_frame_button_hover(hwnd: HWND, hovered: Option<WindowsFrameButton>) {
    let previous = with_window_user_data(hwnd, |data| data.hovered_frame_button.replace(hovered));
    let Some(previous) = previous else {
        return;
    };
    if previous == hovered {
        return;
    }
    if let Some(button) = previous {
        invalidate_frame_button(hwnd, button);
    }
    if let Some(button) = hovered {
        invalidate_frame_button(hwnd, button);
    }
}

fn set_frame_button_pressed(hwnd: HWND, pressed: Option<WindowsFrameButton>) {
    let previous = with_window_user_data(hwnd, |data| data.pressed_frame_button.replace(pressed));
    let Some(previous) = previous else {
        return;
    };
    if previous == pressed {
        return;
    }
    if let Some(button) = previous {
        invalidate_frame_button(hwnd, button);
    }
    if let Some(button) = pressed {
        invalidate_frame_button(hwnd, button);
    }
}

/// Arms `TrackMouseEvent` so the window receives WM_MOUSELEAVE (client) or
/// WM_NCMOUSELEAVE (non-client) once, deduplicated per window via the
/// tracking flags in [`WindowUserData`].
fn begin_mouse_tracking(hwnd: HWND, nonclient: bool) {
    let already = with_window_user_data(hwnd, |data| {
        if nonclient {
            data.tracking_nc_mouse.replace(true)
        } else {
            data.tracking_client_mouse.replace(true)
        }
    })
    .unwrap_or(true);
    if already {
        return;
    }
    let mut track = TRACKMOUSEEVENT {
        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
        dwFlags: if nonclient {
            TME_LEAVE | TME_NONCLIENT
        } else {
            TME_LEAVE
        },
        hwndTrack: hwnd,
        dwHoverTime: 0,
    };
    unsafe {
        let _ = TrackMouseEvent(&mut track);
    }
}

/// Frame-button element under a client-space point, or `None`.
fn frame_button_at_point(
    hwnd: HWND,
    webtag_key: &str,
    point: (i32, i32),
) -> Option<WindowsFrameButton> {
    if !window_draws_shell_chrome(webtag_key) {
        return None;
    }
    let renderer = windows_chrome_renderer()?;
    let state = chrome_state_for_window(hwnd, webtag_key);
    match renderer.hit_test(&state, point) {
        Some(WindowsChromeHit::FrameButton(button)) => Some(button),
        _ => None,
    }
}

/// WM_MOUSEMOVE path: tracks hover for the client-handled frame buttons
/// (minimize/close; the maximize button lives in non-client space).
pub(crate) fn handle_frame_button_client_mouse_move(hwnd: HWND, point: (i32, i32)) {
    if windows_chrome_renderer().is_none() {
        return;
    }
    let Some(webtag_key) = window_webtag_key(hwnd) else {
        return;
    };
    let hovered = frame_button_at_point(hwnd, &webtag_key, point);
    set_frame_button_hover(hwnd, hovered);
    if hovered.is_some() {
        begin_mouse_tracking(hwnd, false);
    }
}

/// WM_NCMOUSEMOVE path: the maximize button reports HTMAXBUTTON from
/// WM_NCHITTEST, so its hover updates arrive as non-client mouse moves.
pub(crate) fn handle_frame_button_nc_mouse_move(hwnd: HWND, hit_code: u32) {
    if windows_chrome_renderer().is_none() {
        return;
    }
    if hit_code == WindowsAndMessaging::HTMAXBUTTON {
        set_frame_button_hover(hwnd, Some(WindowsFrameButton::Maximize));
        begin_mouse_tracking(hwnd, true);
    } else if frame_button_visual_state(hwnd).0 == Some(WindowsFrameButton::Maximize) {
        set_frame_button_hover(hwnd, None);
    }
}

/// WM_MOUSELEAVE: clears hover for client-tracked buttons only; the maximize
/// button is cleared by WM_NCMOUSELEAVE (the cursor moving from a client
/// button onto the maximize button produces WM_MOUSELEAVE after the
/// non-client move already set the new hover).
pub(crate) fn handle_frame_button_client_mouse_leave(hwnd: HWND) {
    with_window_user_data(hwnd, |data| data.tracking_client_mouse.set(false));
    if frame_button_visual_state(hwnd).0 != Some(WindowsFrameButton::Maximize) {
        set_frame_button_hover(hwnd, None);
    }
}

/// WM_NCMOUSELEAVE: clears maximize-button hover/pressed state.
pub(crate) fn handle_frame_button_nc_mouse_leave(hwnd: HWND) {
    with_window_user_data(hwnd, |data| data.tracking_nc_mouse.set(false));
    let (hovered, pressed) = frame_button_visual_state(hwnd);
    if hovered == Some(WindowsFrameButton::Maximize) {
        set_frame_button_hover(hwnd, None);
    }
    if pressed == Some(WindowsFrameButton::Maximize) {
        set_frame_button_pressed(hwnd, None);
    }
}

/// WM_LBUTTONDOWN on a client-handled frame button: records the pressed
/// state for painting and captures the mouse so the release is seen even
/// when it happens outside the button.
pub(crate) fn handle_frame_button_mouse_down(
    hwnd: HWND,
    webtag_key: &str,
    point: (i32, i32),
) -> bool {
    let Some(button) = frame_button_at_point(hwnd, webtag_key, point) else {
        return false;
    };
    set_frame_button_pressed(hwnd, Some(button));
    unsafe {
        let _ = SetCapture(hwnd);
    }
    true
}

/// WM_LBUTTONUP with a pressed frame button: executes the button only when
/// the release still lands on it (standard button-cancel semantics).
pub(crate) fn handle_frame_button_mouse_up(
    hwnd: HWND,
    webtag_key: &str,
    point: (i32, i32),
) -> bool {
    let (_, pressed) = frame_button_visual_state(hwnd);
    let Some(button) = pressed else {
        return false;
    };
    set_frame_button_pressed(hwnd, None);
    unsafe {
        let _ = ReleaseCapture();
    }
    if frame_button_at_point(hwnd, webtag_key, point) == Some(button) {
        handle_window_frame_button(hwnd, button);
    }
    true
}

/// WM_NCLBUTTONDOWN/WM_NCLBUTTONUP for HTMAXBUTTON: DefWindowProc does not
/// click client-drawn snap buttons, so the maximize/restore click is
/// performed here. Returns `true` when the message was consumed.
pub(crate) fn handle_frame_button_nc_button(hwnd: HWND, msg: u32, hit_code: u32) -> bool {
    if windows_chrome_renderer().is_none() {
        return false;
    }
    match msg {
        WindowsAndMessaging::WM_NCLBUTTONDOWN | WindowsAndMessaging::WM_NCLBUTTONDBLCLK => {
            if hit_code != WindowsAndMessaging::HTMAXBUTTON {
                return false;
            }
            set_frame_button_pressed(hwnd, Some(WindowsFrameButton::Maximize));
            true
        }
        WindowsAndMessaging::WM_NCLBUTTONUP => {
            if frame_button_visual_state(hwnd).1 != Some(WindowsFrameButton::Maximize) {
                return false;
            }
            set_frame_button_pressed(hwnd, None);
            if hit_code == WindowsAndMessaging::HTMAXBUTTON {
                handle_window_frame_button(hwnd, WindowsFrameButton::Maximize);
            }
            true
        }
        _ => false,
    }
}

pub(crate) fn window_resize_hit_test(hwnd: HWND, client: RECT, point: (i32, i32)) -> Option<u32> {
    if unsafe { WindowsAndMessaging::IsZoomed(hwnd).as_bool() } {
        return None;
    }
    let border = resize_border_thickness();
    let left = point.0 >= client.left && point.0 < client.left + border;
    let right = point.0 < client.right && point.0 >= client.right - border;
    let top = point.1 >= client.top && point.1 < client.top + border;
    let bottom = point.1 < client.bottom && point.1 >= client.bottom - border;

    match (left, right, top, bottom) {
        (true, _, true, _) => Some(WindowsAndMessaging::HTTOPLEFT),
        (_, true, true, _) => Some(WindowsAndMessaging::HTTOPRIGHT),
        (true, _, _, true) => Some(WindowsAndMessaging::HTBOTTOMLEFT),
        (_, true, _, true) => Some(WindowsAndMessaging::HTBOTTOMRIGHT),
        (_, _, true, _) => Some(WindowsAndMessaging::HTTOP),
        (_, _, _, true) => Some(WindowsAndMessaging::HTBOTTOM),
        (true, _, _, _) => Some(WindowsAndMessaging::HTLEFT),
        (_, true, _, _) => Some(WindowsAndMessaging::HTRIGHT),
        _ => None,
    }
}

pub(crate) fn resize_border_thickness() -> i32 {
    unsafe {
        let frame = WindowsAndMessaging::GetSystemMetrics(WindowsAndMessaging::SM_CXFRAME);
        let padded = WindowsAndMessaging::GetSystemMetrics(WindowsAndMessaging::SM_CXPADDEDBORDER);
        (frame + padded).max(6)
    }
}

pub(crate) fn window_draws_shell_chrome(webtag_key: &str) -> bool {
    !matches!(
        window_attachment(webtag_key).map(|attachment| attachment.kind),
        Some(WindowAttachmentKind::MainChild | WindowAttachmentKind::Panel { .. })
    )
}

pub(crate) fn window_webtag_key(hwnd: HWND) -> Option<String> {
    let raw =
        unsafe { WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA) }
            as *mut WindowUserData;
    if raw.is_null() {
        None
    } else {
        Some(unsafe { (*raw).webtag_key.clone() })
    }
}

pub(crate) fn handle_window_frame_button(hwnd: HWND, button: WindowsFrameButton) {
    unsafe {
        match button {
            WindowsFrameButton::Minimize => {
                let _ = WindowsAndMessaging::ShowWindow(hwnd, WindowsAndMessaging::SW_MINIMIZE);
            }
            WindowsFrameButton::Maximize => {
                let cmd = if WindowsAndMessaging::IsZoomed(hwnd).as_bool() {
                    WindowsAndMessaging::SW_RESTORE
                } else {
                    WindowsAndMessaging::SW_MAXIMIZE
                };
                let _ = WindowsAndMessaging::ShowWindow(hwnd, cmd);
            }
            WindowsFrameButton::Close => {
                let _ = WindowsAndMessaging::SendMessageW(
                    hwnd,
                    WindowsAndMessaging::WM_CLOSE,
                    None,
                    None,
                );
            }
        }
    }
}

pub(crate) fn handle_window_chrome_click(hwnd: HWND, webtag_key: &str, point: (i32, i32)) -> bool {
    if !window_draws_shell_chrome(webtag_key) {
        return false;
    }
    let Some(renderer) = windows_chrome_renderer() else {
        return false;
    };
    let state = chrome_state_for_window(hwnd, webtag_key);

    match renderer.hit_test(&state, point) {
        Some(WindowsChromeHit::FrameButton(button)) => {
            handle_window_frame_button(hwnd, button);
            true
        }
        Some(WindowsChromeHit::NativePanel { panel_id }) => {
            set_active_native_panel(Some(panel_id));
            unsafe {
                let _ = SetFocus(Some(hwnd));
            }
            true
        }
        Some(WindowsChromeHit::NavigationBack) => {
            invoke_chrome_event_handler(webtag_key, WindowsChromeEvent::NavigationBack)
        }
        Some(WindowsChromeHit::NavigationHome) => {
            invoke_chrome_event_handler(webtag_key, WindowsChromeEvent::NavigationHome)
        }
        Some(WindowsChromeHit::PanelActivator { panel_id }) => invoke_chrome_event_handler(
            webtag_key,
            WindowsChromeEvent::PanelActivatorClick { panel_id },
        ),
        Some(WindowsChromeHit::TabBarItem { index }) => {
            invoke_chrome_event_handler(webtag_key, WindowsChromeEvent::TabBarClick { index })
        }
        Some(WindowsChromeHit::BrowserNewTab) => {
            invoke_chrome_event_handler(webtag_key, WindowsChromeEvent::BrowserNewTabClick)
        }
        Some(WindowsChromeHit::BrowserTab { tab_id }) => {
            invoke_chrome_event_handler(webtag_key, WindowsChromeEvent::BrowserTabClick { tab_id })
        }
        Some(WindowsChromeHit::BrowserTabClose { tab_id }) => invoke_chrome_event_handler(
            webtag_key,
            WindowsChromeEvent::BrowserTabCloseClick { tab_id },
        ),
        Some(WindowsChromeHit::NativePanelTab { panel_id, tab_id }) => {
            // Switching tabs keeps keyboard input flowing into the panel.
            set_active_native_panel(Some(panel_id.clone()));
            unsafe {
                let _ = SetFocus(Some(hwnd));
            }
            invoke_chrome_event_handler(
                webtag_key,
                WindowsChromeEvent::NativePanelTabClick { panel_id, tab_id },
            )
        }
        Some(WindowsChromeHit::NativePanelTabClose { panel_id, tab_id }) => {
            invoke_chrome_event_handler(
                webtag_key,
                WindowsChromeEvent::NativePanelTabCloseClick { panel_id, tab_id },
            )
        }
        Some(WindowsChromeHit::NativePanelNewTab { panel_id }) => invoke_chrome_event_handler(
            webtag_key,
            WindowsChromeEvent::NativePanelNewTabClick { panel_id },
        ),
        Some(WindowsChromeHit::NativePanelMaximize { panel_id }) => invoke_chrome_event_handler(
            webtag_key,
            WindowsChromeEvent::NativePanelMaximizeClick { panel_id },
        ),
        Some(WindowsChromeHit::BrowserNavBack) => {
            invoke_chrome_event_handler(webtag_key, WindowsChromeEvent::BrowserNavBackClick)
        }
        Some(WindowsChromeHit::BrowserNavForward) => {
            invoke_chrome_event_handler(webtag_key, WindowsChromeEvent::BrowserNavForwardClick)
        }
        Some(WindowsChromeHit::BrowserNavReload) => {
            invoke_chrome_event_handler(webtag_key, WindowsChromeEvent::BrowserNavReloadClick)
        }
        Some(WindowsChromeHit::BrowserAddressBar) => {
            invoke_chrome_event_handler(webtag_key, WindowsChromeEvent::BrowserAddressBarClick)
        }
        Some(WindowsChromeHit::SidebarToggle) => {
            invoke_chrome_event_handler(webtag_key, WindowsChromeEvent::SidebarToggleClick)
        }
        Some(WindowsChromeHit::SidebarGroupToggle { group }) => invoke_chrome_event_handler(
            webtag_key,
            WindowsChromeEvent::SidebarGroupToggleClick { group },
        ),
        Some(WindowsChromeHit::SidebarAction { action_id }) => invoke_chrome_event_handler(
            webtag_key,
            WindowsChromeEvent::SidebarActionClick { action_id },
        ),
        Some(WindowsChromeHit::Chrome) => true,
        // Caption points never arrive as client clicks (WM_NCHITTEST maps
        // them to HTCAPTION first); treat defensively as unhandled.
        Some(WindowsChromeHit::Caption) | None => false,
    }
}

/// WM_LBUTTONDBLCLK on chrome: double-clicking the ACTIVE tab of a native
/// panel requests an inline rename; an inactive tab is treated as a plain
/// tab click. Returns `false` for all other chrome (the caller then runs
/// the regular button-down path).
pub(crate) fn handle_window_chrome_double_click(
    hwnd: HWND,
    webtag_key: &str,
    point: (i32, i32),
) -> bool {
    if !window_draws_shell_chrome(webtag_key) {
        return false;
    }
    let Some(renderer) = windows_chrome_renderer() else {
        return false;
    };
    let state = chrome_state_for_window(hwnd, webtag_key);
    let Some(WindowsChromeHit::NativePanelTab { panel_id, tab_id }) =
        renderer.hit_test(&state, point)
    else {
        return false;
    };

    let is_active_tab = state
        .attached
        .as_ref()
        .and_then(|attached| {
            attached
                .panels
                .iter()
                .find(|panel| panel.panel_id == panel_id)
        })
        .and_then(|panel| panel.native.as_ref())
        .is_some_and(|native| native.tabs.iter().any(|tab| tab.id == tab_id && tab.active));
    let event = if is_active_tab {
        WindowsChromeEvent::NativePanelTabRenameRequest { panel_id, tab_id }
    } else {
        WindowsChromeEvent::NativePanelTabClick { panel_id, tab_id }
    };
    invoke_chrome_event_handler(webtag_key, event)
}

/// WM_RBUTTONDOWN on chrome: a right-click on a native panel's content area
/// is dispatched to the product layer with the screen-space click point
/// (products typically show a context menu there). Returns `false` for all
/// other chrome so the message falls through.
pub(crate) fn handle_window_chrome_right_click(
    hwnd: HWND,
    webtag_key: &str,
    point: (i32, i32),
) -> bool {
    if !window_draws_shell_chrome(webtag_key) {
        return false;
    }
    let Some(renderer) = windows_chrome_renderer() else {
        return false;
    };
    let state = chrome_state_for_window(hwnd, webtag_key);
    let Some(WindowsChromeHit::NativePanel { panel_id }) = renderer.hit_test(&state, point) else {
        return false;
    };

    // Keep keyboard input flowing into the panel the user right-clicked.
    set_active_native_panel(Some(panel_id.clone()));
    let mut screen = POINT {
        x: point.0,
        y: point.1,
    };
    unsafe {
        let _ = SetFocus(Some(hwnd));
        let _ = ClientToScreen(hwnd, &mut screen);
    }
    invoke_chrome_event_handler(
        webtag_key,
        WindowsChromeEvent::NativePanelRightClick {
            panel_id,
            screen_x: screen.x,
            screen_y: screen.y,
        },
    )
}
