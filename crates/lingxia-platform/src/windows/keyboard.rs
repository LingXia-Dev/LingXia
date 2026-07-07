//! App-window keyboard input for devtools (`app.keyboard`).
//!
//! Symmetric to the mouse backend: input is injected with `PostMessage` to the
//! target window's focused control (so keys reach WebView2 content and shell
//! chrome alike) rather than `SendInput`, so it works without an interactive
//! desktop or real foreground focus.
//!
//! Limitation: modifier state is delivered as bare key-down/up messages, which
//! the target's `GetKeyState` does not observe, so chorded shortcuts
//! (e.g. Ctrl+A) are best-effort. Plain keys and text typing are reliable.

use async_trait::async_trait;

use super::Platform;
use super::screenshot::resolve_screenshot_window;
use crate::error::PlatformError;
use crate::traits::keyboard::{
    AppKeyboard, AppKeyboardAction, AppKeyboardModifier, AppKeyboardRequest, AppKeyboardResult,
};

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    MAPVK_VK_TO_VSC, MapVirtualKeyW, VIRTUAL_KEY, VK_BACK, VK_CONTROL, VK_DELETE, VK_DOWN, VK_END,
    VK_ESCAPE, VK_HOME, VK_LEFT, VK_MENU, VK_NEXT, VK_PRIOR, VK_RETURN, VK_RIGHT, VK_SHIFT,
    VK_SPACE, VK_TAB, VK_UP, VkKeyScanW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    self, GUITHREADINFO, GetGUIThreadInfo, GetWindowThreadProcessId, PostMessageW,
};

#[async_trait]
impl AppKeyboard for Platform {
    async fn perform_app_keyboard(
        &self,
        request: AppKeyboardRequest,
    ) -> Result<AppKeyboardResult, PlatformError> {
        let hwnd = resolve_screenshot_window(request.window_id.as_deref())?;
        // HWND is not Send; carry the raw handle (no awaits below need it back).
        let window = hwnd.0 as isize;
        let window_id = window.to_string();
        let action_kind = request.action.kind().to_string();
        dispatch_key(window, &request.action)?;
        Ok(AppKeyboardResult {
            window_id,
            action: action_kind,
        })
    }
}

fn dispatch_key(window: isize, action: &AppKeyboardAction) -> Result<(), PlatformError> {
    let target = focus_target(window);
    match action {
        AppKeyboardAction::Type { text } => {
            for unit in text.encode_utf16() {
                post(target, WindowsAndMessaging::WM_CHAR, unit as usize, 1)?;
            }
            Ok(())
        }
        AppKeyboardAction::Press { key, modifiers } => {
            let (vk, ch) = resolve_key(key)
                .ok_or_else(|| PlatformError::Platform(format!("unknown key '{key}'")))?;
            let mods: Vec<u16> = modifiers.iter().map(|m| modifier_vk(*m)).collect();
            // Modifier downs, key down (+ char), key up, modifier ups (reverse).
            for &m in &mods {
                key_event(target, m, true)?;
            }
            key_event(target, vk, true)?;
            if let Some(ch) = ch {
                post(target, WindowsAndMessaging::WM_CHAR, ch as usize, 1)?;
            }
            key_event(target, vk, false)?;
            for &m in mods.iter().rev() {
                key_event(target, m, false)?;
            }
            Ok(())
        }
    }
}

/// The focused control of the target window's GUI thread, falling back to the
/// top-level window when there is no distinct focus.
fn focus_target(window: isize) -> HWND {
    let top = HWND(window as *mut core::ffi::c_void);
    let tid = unsafe { GetWindowThreadProcessId(top, None) };
    if tid != 0 {
        let mut gti = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        unsafe {
            if GetGUIThreadInfo(tid, &mut gti).is_ok() && !gti.hwndFocus.is_invalid() {
                return gti.hwndFocus;
            }
        }
    }
    top
}

/// Post a WM_KEYDOWN/WM_KEYUP for a virtual-key, with a realistic lParam
/// (scan code + transition flags) so key-aware controls accept it.
fn key_event(target: HWND, vk: u16, down: bool) -> Result<(), PlatformError> {
    let scan = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) } & 0xFF;
    let mut lparam = (scan << 16) | 1;
    let msg = if down {
        WindowsAndMessaging::WM_KEYDOWN
    } else {
        // Key-up sets the previous-state (30) and transition (31) bits.
        lparam |= (1 << 30) | (1 << 31);
        WindowsAndMessaging::WM_KEYUP
    };
    post(target, msg, vk as usize, lparam as isize)
}

fn post(target: HWND, msg: u32, wparam: usize, lparam: isize) -> Result<(), PlatformError> {
    unsafe {
        PostMessageW(Some(target), msg, WPARAM(wparam), LPARAM(lparam))
            .map_err(|err| PlatformError::Platform(format!("PostMessageW failed: {err}")))
    }
}

fn modifier_vk(m: AppKeyboardModifier) -> u16 {
    match m {
        // Command has no Windows analogue; map it to Ctrl like the shell does.
        AppKeyboardModifier::Command | AppKeyboardModifier::Control => VK_CONTROL.0,
        AppKeyboardModifier::Shift => VK_SHIFT.0,
        AppKeyboardModifier::Option => VK_MENU.0,
    }
}

/// Resolve a key name to `(virtual_key, optional_char_to_also_emit)`. Named
/// keys map to their VK; a single character maps via the keyboard layout.
fn resolve_key(key: &str) -> Option<(u16, Option<u16>)> {
    let named: Option<(VIRTUAL_KEY, Option<u16>)> = match key.to_ascii_lowercase().as_str() {
        "return" | "enter" => Some((VK_RETURN, Some(b'\r' as u16))),
        "tab" => Some((VK_TAB, Some(b'\t' as u16))),
        "escape" | "esc" => Some((VK_ESCAPE, Some(0x1B))),
        "space" => Some((VK_SPACE, Some(b' ' as u16))),
        "backspace" => Some((VK_BACK, Some(0x08))),
        "delete" | "del" => Some((VK_DELETE, None)),
        "up" => Some((VK_UP, None)),
        "down" => Some((VK_DOWN, None)),
        "left" => Some((VK_LEFT, None)),
        "right" => Some((VK_RIGHT, None)),
        "home" => Some((VK_HOME, None)),
        "end" => Some((VK_END, None)),
        "pageup" => Some((VK_PRIOR, None)),
        "pagedown" => Some((VK_NEXT, None)),
        _ => None,
    };
    if let Some((vk, ch)) = named {
        return Some((vk.0, ch));
    }
    // Single printable character: derive the VK from the active layout and also
    // emit the character so text fields receive it.
    let mut chars = key.chars();
    let ch = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    let scan = unsafe { VkKeyScanW(ch as u16) };
    if scan == -1 {
        return None;
    }
    let vk = (scan & 0xFF) as u16;
    Some((vk, Some(ch as u16)))
}
