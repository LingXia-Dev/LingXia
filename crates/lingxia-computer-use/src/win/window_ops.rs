//! Window management: focus, raise, move, resize, min/max/restore,
//! always-on-top, close, status, and activate-by-match.

use super::{display_id_for_rect, parse_hwnd, process_name, rect_to};
use crate::error::{Error, Result};
use crate::model::{Window, WindowQuery, WindowTarget};
use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GetForegroundWindow, GetWindowLongW, GetWindowRect, GetWindowTextW,
    GetWindowThreadProcessId, HWND_NOTOPMOST, HWND_TOP, HWND_TOPMOST, IsIconic, IsWindow,
    IsWindowVisible, IsZoomed, PostMessageW, SW_MINIMIZE, SW_RESTORE, SW_SHOWMAXIMIZED,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SetForegroundWindow,
    SetWindowPos, ShowWindow, WM_CLOSE, WS_EX_TOPMOST,
};

/// Resolve a target to a live HWND.
fn resolve(target: &WindowTarget) -> Result<HWND> {
    super::ensure_dpi_aware();
    match target {
        WindowTarget::Id(id) => {
            let hwnd = parse_hwnd(id)?;
            if !unsafe { IsWindow(Some(hwnd)).as_bool() } {
                return Err(Error::Stale(format!("window {id} no longer exists")));
            }
            Ok(hwnd)
        }
        WindowTarget::Match(query) => {
            let mut wins = super::windows(query)?;
            match wins.len() {
                0 => Err(Error::NotFound("no window matched the query".into())),
                1 => parse_hwnd(&wins.remove(0).id),
                n => Err(Error::Ambiguous(format!(
                    "{n} windows matched; refine --match or use --window <id>"
                ))),
            }
        }
    }
}

/// Build a `Window` record for a single live HWND.
pub(crate) fn window_info(hwnd: HWND) -> Result<Window> {
    unsafe {
        if !IsWindow(Some(hwnd)).as_bool() {
            return Err(Error::Stale("window no longer exists".into()));
        }
        let mut rect = RECT::default();
        let _ = GetWindowRect(hwnd, &mut rect);

        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));

        let mut title_buf = [0u16; 512];
        let len = GetWindowTextW(hwnd, &mut title_buf);
        let title = String::from_utf16_lossy(&title_buf[..len.max(0) as usize]);

        let ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        let displays = super::displays().unwrap_or_default();
        let (display_id, dpi, scale) = display_id_for_rect(&displays, &rect);

        Ok(Window {
            id: format!("0x{:X}", hwnd.0 as isize),
            title,
            process: process_name(pid),
            pid,
            bounds: rect_to(rect),
            display_id,
            scale,
            dpi,
            visible: IsWindowVisible(hwnd).as_bool(),
            focused: hwnd == GetForegroundWindow(),
            minimized: IsIconic(hwnd).as_bool(),
            maximized: IsZoomed(hwnd).as_bool(),
            always_on_top: ex & WS_EX_TOPMOST.0 != 0,
            z: 0,
        })
    }
}

pub fn status(target: &WindowTarget) -> Result<Window> {
    window_info(resolve(target)?)
}

pub fn focus(target: &WindowTarget) -> Result<Window> {
    let hwnd = resolve(target)?;
    unsafe {
        let ok = SetForegroundWindow(hwnd).as_bool();
        // Windows may refuse foreground changes (foreground lock / integrity).
        // Report failure instead of a false success.
        if !ok && GetForegroundWindow() != hwnd {
            return Err(Error::Failed(
                "could not bring the window to the foreground (foreground lock or integrity level)"
                    .into(),
            ));
        }
    }
    window_info(hwnd)
}

pub fn activate(query: WindowQuery) -> Result<Window> {
    focus(&WindowTarget::Match(query))
}

pub fn raise(target: &WindowTarget) -> Result<Window> {
    let hwnd = resolve(target)?;
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOP),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
        );
    }
    window_info(hwnd)
}

pub fn move_to(target: &WindowTarget, x: i32, y: i32) -> Result<Window> {
    let hwnd = resolve(target)?;
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            None,
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
    window_info(hwnd)
}

pub fn move_to_display(target: &WindowTarget, display_id: &str) -> Result<Window> {
    let hwnd = resolve(target)?;
    let displays = super::displays()?;
    let display = displays
        .iter()
        .find(|d| d.id == display_id)
        .ok_or_else(|| Error::NotFound(format!("no display {display_id}")))?;
    move_to(
        &WindowTarget::Id(format!("0x{:X}", hwnd.0 as isize)),
        display.work_area.x,
        display.work_area.y,
    )
}

pub fn resize(target: &WindowTarget, w: i32, h: i32) -> Result<Window> {
    if w <= 0 || h <= 0 {
        return Err(Error::Usage("width/height must be positive".into()));
    }
    let hwnd = resolve(target)?;
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            w,
            h,
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
    window_info(hwnd)
}

pub fn minimize(target: &WindowTarget) -> Result<Window> {
    let hwnd = resolve(target)?;
    unsafe {
        let _ = ShowWindow(hwnd, SW_MINIMIZE);
    }
    window_info(hwnd)
}

pub fn maximize(target: &WindowTarget) -> Result<Window> {
    let hwnd = resolve(target)?;
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOWMAXIMIZED);
    }
    window_info(hwnd)
}

pub fn restore(target: &WindowTarget) -> Result<Window> {
    let hwnd = resolve(target)?;
    unsafe {
        let _ = ShowWindow(hwnd, SW_RESTORE);
    }
    window_info(hwnd)
}

pub fn set_always_on_top(target: &WindowTarget, on: bool) -> Result<Window> {
    let hwnd = resolve(target)?;
    let after = if on { HWND_TOPMOST } else { HWND_NOTOPMOST };
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            Some(after),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
    window_info(hwnd)
}

/// Ask a window to close (WM_CLOSE). Destructive.
pub fn close(target: &WindowTarget) -> Result<Window> {
    let hwnd = resolve(target)?;
    // Snapshot before closing so the result identifies what was closed.
    let info = window_info(hwnd)?;
    unsafe {
        PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0))
            .map_err(|e| Error::Failed(format!("WM_CLOSE failed: {e}")))?;
    }
    Ok(info)
}
