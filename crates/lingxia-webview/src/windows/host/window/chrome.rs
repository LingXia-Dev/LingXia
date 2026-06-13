//! Custom chrome hit testing and frame-button interaction.

use super::*;

pub(crate) fn handle_window_frame_hit_test(hwnd: HWND, webtag_key: &str, lparam: LPARAM) -> u32 {
    if !window_draws_host_chrome(webtag_key) {
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
    if !window_draws_host_chrome(webtag_key) {
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

/// Webviews whose host window keeps the standard OS frame (native title bar +
/// minimize/maximize/close) instead of custom chrome. A standalone
/// `window`-kind surface registers here so its own top-level window has real
/// window controls.
pub(crate) static WINDOW_OS_FRAME: OnceLock<Mutex<std::collections::HashSet<String>>> =
    OnceLock::new();

pub(crate) fn set_os_frame(webtag_key: &str) {
    let set = WINDOW_OS_FRAME.get_or_init(|| Mutex::new(std::collections::HashSet::new()));
    if let Ok(mut set) = set.lock() {
        set.insert(webtag_key.to_string());
    }
}

pub(crate) fn clear_os_frame(webtag_key: &str) {
    if let Some(set) = WINDOW_OS_FRAME.get()
        && let Ok(mut set) = set.lock()
    {
        set.remove(webtag_key);
    }
}

pub(crate) fn webtag_uses_os_frame(webtag_key: &str) -> bool {
    WINDOW_OS_FRAME
        .get()
        .and_then(|set| set.lock().ok())
        .is_some_and(|set| set.contains(webtag_key))
}

pub(crate) fn window_uses_os_frame(hwnd: HWND) -> bool {
    window_webtag_key(hwnd).is_some_and(|key| webtag_uses_os_frame(&key))
}

pub(crate) fn window_draws_host_chrome(webtag_key: &str) -> bool {
    if webtag_uses_os_frame(webtag_key) {
        return false;
    }
    !matches!(
        window_attachment(webtag_key).map(|attachment| attachment.kind),
        Some(
            WindowAttachmentKind::MainChild
                | WindowAttachmentKind::Panel { .. }
                | WindowAttachmentKind::Overlay
        )
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
    if !window_draws_host_chrome(webtag_key) {
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
        Some(WindowsChromeHit::Focusable { id, .. }) => {
            focus_chrome_surface(hwnd, id);
            true
        }
        Some(WindowsChromeHit::Command(command)) => {
            dispatch_chrome_command(hwnd, webtag_key, command)
        }
        Some(WindowsChromeHit::Chrome) => true,
        // Caption points never arrive as client clicks (WM_NCHITTEST maps
        // them to HTCAPTION first); treat defensively as unhandled.
        Some(WindowsChromeHit::Caption) | None => false,
    }
}

/// WM_LBUTTONDBLCLK on chrome: renderer-defined hits can provide a
/// dedicated double-click command; otherwise the regular button-down path
/// handles the interaction.
pub(crate) fn handle_window_chrome_double_click(
    hwnd: HWND,
    webtag_key: &str,
    point: (i32, i32),
) -> bool {
    if !window_draws_host_chrome(webtag_key) {
        return false;
    }
    let Some(renderer) = windows_chrome_renderer() else {
        return false;
    };
    let state = chrome_state_for_window(hwnd, webtag_key);
    let Some(WindowsChromeHit::Command(command)) = renderer.hit_test(&state, point) else {
        return false;
    };
    let command = command.double_click.as_deref().cloned().unwrap_or(command);
    dispatch_chrome_command(hwnd, webtag_key, command)
}

/// WM_RBUTTONDOWN on chrome: renderer-defined context commands receive the
/// screen-space click point. Returns `false` for all other chrome so the
/// message falls through.
pub(crate) fn handle_window_chrome_right_click(
    hwnd: HWND,
    webtag_key: &str,
    point: (i32, i32),
) -> bool {
    if !window_draws_host_chrome(webtag_key) {
        return false;
    }
    let Some(renderer) = windows_chrome_renderer() else {
        return false;
    };
    let state = chrome_state_for_window(hwnd, webtag_key);
    let (focus, command) = match renderer.hit_test(&state, point) {
        Some(WindowsChromeHit::Command(command)) if command.include_screen_position => {
            (command.focus.clone(), command)
        }
        Some(WindowsChromeHit::Focusable {
            id,
            context_menu: Some(command),
        }) if command.include_screen_position => (Some(id), command),
        _ => return false,
    };
    let mut screen = POINT {
        x: point.0,
        y: point.1,
    };
    unsafe {
        let _ = SetFocus(Some(hwnd));
        let _ = ClientToScreen(hwnd, &mut screen);
    }
    if let Some(focus) = focus {
        focus_chrome_surface(hwnd, focus);
    }
    let payload = payload_with_screen_position(command.payload.clone(), screen.x, screen.y);
    invoke_chrome_event_handler(webtag_key, command.with_payload(payload))
}

fn dispatch_chrome_command(hwnd: HWND, webtag_key: &str, command: WindowsChromeCommand) -> bool {
    if let Some(focus) = command.focus.clone() {
        focus_chrome_surface(hwnd, focus);
    }
    invoke_chrome_event_handler(webtag_key, command)
}

fn focus_chrome_surface(hwnd: HWND, id: String) {
    set_active_host_panel(Some(id));
    unsafe {
        let _ = SetFocus(Some(hwnd));
    }
}

fn payload_with_screen_position(
    mut payload: serde_json::Value,
    screen_x: i32,
    screen_y: i32,
) -> serde_json::Value {
    if !payload.is_object() {
        payload = serde_json::json!({});
    }
    if let Some(object) = payload.as_object_mut() {
        object.insert("screen_x".to_string(), serde_json::json!(screen_x));
        object.insert("screen_y".to_string(), serde_json::json!(screen_y));
    }
    payload
}
