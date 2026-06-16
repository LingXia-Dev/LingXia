//! App-window mouse input for devtools (`app.mouse`).
//!
//! Input is injected with `PostMessage` to the deepest child window under
//! the target point (so clicks land in WebView2 content as well as the
//! shell chrome), not with `SendInput`: message injection works without an
//! interactive desktop or a real cursor, matching how the macOS backend
//! injects CGEvents directly into the app.

use async_trait::async_trait;

use super::Platform;
use super::screenshot::resolve_screenshot_window;
use crate::error::PlatformError;
use crate::traits::mouse::{
    AppMouse, AppMouseAction, AppMouseButton, AppMouseRequest, AppMouseResult,
};

use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    self, CWP_SKIPDISABLED, CWP_SKIPINVISIBLE, CWP_SKIPTRANSPARENT, ChildWindowFromPointEx,
    WHEEL_DELTA,
};

#[async_trait]
impl AppMouse for Platform {
    async fn perform_app_mouse(
        &self,
        request: AppMouseRequest,
    ) -> Result<AppMouseResult, PlatformError> {
        let hwnd = resolve_screenshot_window(request.window_id.as_deref())?;
        // HWND is not Send; carry the raw handle across awaits instead.
        let window = hwnd.0 as isize;
        let window_id = window.to_string();
        let action_kind = request.action.kind().to_string();
        dispatch_action(window, &request.action).await?;
        Ok(AppMouseResult {
            window_id,
            action: action_kind,
        })
    }
}

async fn dispatch_action(window: isize, action: &AppMouseAction) -> Result<(), PlatformError> {
    match *action {
        AppMouseAction::Move { x, y } => {
            post_mouse_message(window, (x, y), WindowsAndMessaging::WM_MOUSEMOVE, 0)
        }
        AppMouseAction::Down { x, y, button } => post_mouse_message(
            window,
            (x, y),
            button_message(button, true),
            button_mk(button),
        ),
        AppMouseAction::Up { x, y, button } => {
            post_mouse_message(window, (x, y), button_message(button, false), 0)
        }
        AppMouseAction::Click {
            x,
            y,
            button,
            click_count,
        } => {
            let point = (x, y);
            post_mouse_message(window, point, WindowsAndMessaging::WM_MOUSEMOVE, 0)?;
            let presses = click_count.max(1);
            for repeat in 0..presses {
                // The second press of a double click arrives as a DBLCLK
                // message on CS_DBLCLKS window classes (the shell's hosts).
                let down = if repeat % 2 == 1 {
                    button_dblclk_message(button)
                } else {
                    button_message(button, true)
                };
                post_mouse_message(window, point, down, button_mk(button))?;
                tokio::time::sleep(std::time::Duration::from_millis(30)).await;
                post_mouse_message(window, point, button_message(button, false), 0)?;
                if repeat + 1 < presses {
                    tokio::time::sleep(std::time::Duration::from_millis(60)).await;
                }
            }
            Ok(())
        }
        AppMouseAction::Drag {
            from_x,
            from_y,
            to_x,
            to_y,
            button,
        } => {
            post_mouse_message(
                window,
                (from_x, from_y),
                button_message(button, true),
                button_mk(button),
            )?;
            const STEPS: i32 = 8;
            for step in 1..=STEPS {
                let t = f64::from(step) / f64::from(STEPS);
                let x = from_x + (to_x - from_x) * t;
                let y = from_y + (to_y - from_y) * t;
                post_mouse_message(
                    window,
                    (x, y),
                    WindowsAndMessaging::WM_MOUSEMOVE,
                    button_mk(button),
                )?;
                tokio::time::sleep(std::time::Duration::from_millis(15)).await;
            }
            post_mouse_message(window, (to_x, to_y), button_message(button, false), 0)
        }
        AppMouseAction::Scroll { x, y, dx, dy } => {
            if dy.abs() > f64::EPSILON {
                post_wheel_message(window, (x, y), WindowsAndMessaging::WM_MOUSEWHEEL, dy)?;
            }
            if dx.abs() > f64::EPSILON {
                post_wheel_message(window, (x, y), WindowsAndMessaging::WM_MOUSEHWHEEL, dx)?;
            }
            Ok(())
        }
    }
}

/// Posts a client-space mouse message to the deepest enabled, visible child
/// under `point` (falling back to the top-level window), translating the
/// point into that child's client coordinates.
fn post_mouse_message(
    window: isize,
    point: (f64, f64),
    message: u32,
    extra_mk: usize,
) -> Result<(), PlatformError> {
    let (target, client) = deepest_child_at(window, point);
    let lparam = LPARAM(((client.y as isize) << 16) | (client.x as isize & 0xFFFF));
    unsafe {
        WindowsAndMessaging::PostMessageW(Some(target), message, WPARAM(extra_mk), lparam)
            .map_err(|err| PlatformError::Platform(format!("PostMessageW failed: {err}")))
    }
}

/// Wheel messages carry SCREEN coordinates; deliver to the deepest child
/// under the point like the system would.
fn post_wheel_message(
    window: isize,
    point: (f64, f64),
    message: u32,
    amount: f64,
) -> Result<(), PlatformError> {
    use windows::Win32::Graphics::Gdi::ClientToScreen;

    let (target, client) = deepest_child_at(window, point);
    let mut screen = client;
    unsafe {
        let _ = ClientToScreen(target, &mut screen);
    }
    let delta = (amount * f64::from(WHEEL_DELTA)) as i16;
    let wparam = WPARAM((delta as u16 as usize) << 16);
    let lparam = LPARAM(((screen.y as isize) << 16) | (screen.x as isize & 0xFFFF));
    unsafe {
        WindowsAndMessaging::PostMessageW(Some(target), message, wparam, lparam)
            .map_err(|err| PlatformError::Platform(format!("PostMessageW failed: {err}")))
    }
}

/// Walks `ChildWindowFromPointEx` down from `window` to the deepest child
/// containing `point`; returns that window and the point translated into
/// its client coordinates.
fn deepest_child_at(window: isize, point: (f64, f64)) -> (HWND, POINT) {
    use windows::Win32::Graphics::Gdi::{ClientToScreen, ScreenToClient};

    let mut target = HWND(window as *mut core::ffi::c_void);
    let mut client = POINT {
        x: point.0.round() as i32,
        y: point.1.round() as i32,
    };
    loop {
        let child = unsafe {
            ChildWindowFromPointEx(
                target,
                client,
                CWP_SKIPINVISIBLE | CWP_SKIPDISABLED | CWP_SKIPTRANSPARENT,
            )
        };
        if child.is_invalid() || child == target {
            break;
        }
        let mut translated = client;
        unsafe {
            let _ = ClientToScreen(target, &mut translated);
            let _ = ScreenToClient(child, &mut translated);
        }
        target = child;
        client = translated;
    }
    (target, client)
}

fn button_message(button: AppMouseButton, down: bool) -> u32 {
    match (button, down) {
        (AppMouseButton::Left, true) => WindowsAndMessaging::WM_LBUTTONDOWN,
        (AppMouseButton::Left, false) => WindowsAndMessaging::WM_LBUTTONUP,
        (AppMouseButton::Right, true) => WindowsAndMessaging::WM_RBUTTONDOWN,
        (AppMouseButton::Right, false) => WindowsAndMessaging::WM_RBUTTONUP,
        (AppMouseButton::Middle, true) => WindowsAndMessaging::WM_MBUTTONDOWN,
        (AppMouseButton::Middle, false) => WindowsAndMessaging::WM_MBUTTONUP,
    }
}

fn button_dblclk_message(button: AppMouseButton) -> u32 {
    match button {
        AppMouseButton::Left => WindowsAndMessaging::WM_LBUTTONDBLCLK,
        AppMouseButton::Right => WindowsAndMessaging::WM_RBUTTONDBLCLK,
        AppMouseButton::Middle => WindowsAndMessaging::WM_MBUTTONDBLCLK,
    }
}

fn button_mk(button: AppMouseButton) -> usize {
    match button {
        AppMouseButton::Left => 0x0001,   // MK_LBUTTON
        AppMouseButton::Right => 0x0002,  // MK_RBUTTON
        AppMouseButton::Middle => 0x0010, // MK_MBUTTON
    }
}
