//! Synthetic pointer/keyboard input via SendInput. The cursor is positioned in
//! physical pixels with SetCursorPos (the process is DPI aware), then button /
//! wheel / key events are injected at that position.
//!
//! The `_target` pid parameter (background, app-directed input) is accepted for
//! contract parity but ignored here: SendInput is always foreground/active. A
//! Windows background path would post window messages instead — not yet built.

use crate::error::{Error, Result};
use crate::model::{Ack, Modifier, MouseButton};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, MOUSE_EVENT_FLAGS, MOUSEEVENTF_HWHEEL, MOUSEEVENTF_LEFTDOWN,
    MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_RIGHTDOWN,
    MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL, MOUSEINPUT, SendInput, VIRTUAL_KEY, VK_BACK,
    VK_CAPITAL, VK_CONTROL, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_F1, VK_F2, VK_F3, VK_F4,
    VK_F5, VK_F6, VK_F7, VK_F8, VK_F9, VK_F10, VK_F11, VK_F12, VK_HOME, VK_INSERT, VK_LEFT,
    VK_LWIN, VK_MENU, VK_NEXT, VK_PRIOR, VK_RETURN, VK_RIGHT, VK_SHIFT, VK_SPACE, VK_TAB, VK_UP,
    VkKeyScanW,
};
use windows::Win32::UI::WindowsAndMessaging::SetCursorPos;

const WHEEL_DELTA: i32 = 120;

fn send(inputs: &[INPUT]) -> Result<()> {
    let sent = unsafe { SendInput(inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent as usize == inputs.len() {
        Ok(())
    } else {
        Err(Error::Failed(
            "SendInput was blocked or partially delivered".into(),
        ))
    }
}

fn set_cursor(x: i32, y: i32) -> Result<()> {
    super::ensure_dpi_aware();
    unsafe { SetCursorPos(x, y) }.map_err(|e| Error::Failed(format!("SetCursorPos failed: {e}")))
}

fn mouse_event(flags: MOUSE_EVENT_FLAGS, data: i32) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: data as u32,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn button_flags(button: MouseButton) -> (MOUSE_EVENT_FLAGS, MOUSE_EVENT_FLAGS) {
    match button {
        MouseButton::Left => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
        MouseButton::Right => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
        MouseButton::Middle => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP),
    }
}

pub fn pointer_move(x: i32, y: i32, _target: Option<u32>) -> Result<Ack> {
    set_cursor(x, y)?;
    Ok(Ack::new("pointer.move"))
}

pub fn pointer_down(x: i32, y: i32, button: MouseButton, _target: Option<u32>) -> Result<Ack> {
    set_cursor(x, y)?;
    let (down, _) = button_flags(button);
    send(&[mouse_event(down, 0)])?;
    Ok(Ack::new("pointer.down"))
}

pub fn pointer_up(x: i32, y: i32, button: MouseButton, _target: Option<u32>) -> Result<Ack> {
    set_cursor(x, y)?;
    let (_, up) = button_flags(button);
    send(&[mouse_event(up, 0)])?;
    Ok(Ack::new("pointer.up"))
}

pub fn pointer_click(
    x: i32,
    y: i32,
    button: MouseButton,
    count: u32,
    _target: Option<u32>,
) -> Result<Ack> {
    if count == 0 {
        return Err(Error::Usage("count must be greater than zero".into()));
    }
    set_cursor(x, y)?;
    let (down, up) = button_flags(button);
    for _ in 0..count {
        send(&[mouse_event(down, 0), mouse_event(up, 0)])?;
    }
    Ok(Ack::new("pointer.click"))
}

pub fn pointer_scroll(x: i32, y: i32, dx: i32, dy: i32, _target: Option<u32>) -> Result<Ack> {
    set_cursor(x, y)?;
    if dy != 0 {
        send(&[mouse_event(MOUSEEVENTF_WHEEL, dy * WHEEL_DELTA)])?;
    }
    if dx != 0 {
        send(&[mouse_event(MOUSEEVENTF_HWHEEL, dx * WHEEL_DELTA)])?;
    }
    Ok(Ack::new("pointer.scroll"))
}

pub fn pointer_drag(
    fx: i32,
    fy: i32,
    tx: i32,
    ty: i32,
    button: MouseButton,
    _target: Option<u32>,
) -> Result<Ack> {
    let (down, up) = button_flags(button);
    set_cursor(fx, fy)?;
    send(&[mouse_event(down, 0)])?;
    // Once the button is down, always release it — even if a step fails — so a
    // failure can't leave the mouse button stuck down.
    let result = (|| {
        for i in 1..=4 {
            let x = fx + (tx - fx) * i / 4;
            let y = fy + (ty - fy) * i / 4;
            set_cursor(x, y)?;
        }
        Ok(())
    })();
    let up_result = send(&[mouse_event(up, 0)]);
    result?;
    up_result?;
    Ok(Ack::new("pointer.drag"))
}

// ------------------------------- keyboard -------------------------------

fn key_input(vk: VIRTUAL_KEY, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn unicode_input(unit: u16, up: bool) -> INPUT {
    let flags = if up {
        KEYEVENTF_UNICODE | KEYEVENTF_KEYUP
    } else {
        KEYEVENTF_UNICODE
    };
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: unit,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

pub fn key_type(text: &str, _target: Option<u32>) -> Result<Ack> {
    let mut inputs = Vec::new();
    for unit in text.encode_utf16() {
        inputs.push(unicode_input(unit, false));
        inputs.push(unicode_input(unit, true));
    }
    if !inputs.is_empty() {
        send(&inputs)?;
    }
    Ok(Ack::new("key.type"))
}

fn modifier_vk(m: Modifier) -> VIRTUAL_KEY {
    match m {
        Modifier::Ctrl => VK_CONTROL,
        Modifier::Shift => VK_SHIFT,
        Modifier::Alt => VK_MENU,
        Modifier::Meta => VK_LWIN,
    }
}

/// Resolve a key name to a virtual key. Named keys first, then single printable
/// characters via VkKeyScanW.
fn key_vk(name: &str) -> Result<VIRTUAL_KEY> {
    let lower = name.to_lowercase();
    let vk = match lower.as_str() {
        "return" | "enter" => VK_RETURN,
        "tab" => VK_TAB,
        "escape" | "esc" => VK_ESCAPE,
        "space" => VK_SPACE,
        "delete" | "del" => VK_DELETE,
        "backspace" => VK_BACK,
        // Modifier keys, so `key down/up` can hold them for chords.
        "shift" => VK_SHIFT,
        "ctrl" | "control" => VK_CONTROL,
        "alt" | "option" => VK_MENU,
        "win" | "meta" | "cmd" | "command" => VK_LWIN,
        "capslock" => VK_CAPITAL,
        "insert" | "ins" => VK_INSERT,
        "left" => VK_LEFT,
        "right" => VK_RIGHT,
        "up" => VK_UP,
        "down" => VK_DOWN,
        "home" => VK_HOME,
        "end" => VK_END,
        "pageup" => VK_PRIOR,
        "pagedown" => VK_NEXT,
        "f1" => VK_F1,
        "f2" => VK_F2,
        "f3" => VK_F3,
        "f4" => VK_F4,
        "f5" => VK_F5,
        "f6" => VK_F6,
        "f7" => VK_F7,
        "f8" => VK_F8,
        "f9" => VK_F9,
        "f10" => VK_F10,
        "f11" => VK_F11,
        "f12" => VK_F12,
        _ => {
            let mut chars = name.chars();
            let (Some(ch), None) = (chars.next(), chars.next()) else {
                return Err(Error::Usage(format!("unknown key '{name}'")));
            };
            let scan = unsafe { VkKeyScanW(ch as u16) };
            if scan == -1 {
                return Err(Error::Usage(format!("unmappable key '{name}'")));
            }
            VIRTUAL_KEY((scan & 0xff) as u16)
        }
    };
    Ok(vk)
}

pub fn key_down(name: &str, _target: Option<u32>) -> Result<Ack> {
    send(&[key_input(key_vk(name)?, KEYBD_EVENT_FLAGS(0))])?;
    Ok(Ack::new("key.down"))
}

pub fn key_up(name: &str, _target: Option<u32>) -> Result<Ack> {
    send(&[key_input(key_vk(name)?, KEYEVENTF_KEYUP)])?;
    Ok(Ack::new("key.up"))
}

pub fn key_press(name: &str, mods: &[Modifier], _target: Option<u32>) -> Result<Ack> {
    let vk = key_vk(name)?;
    let mut downs = Vec::new();
    for m in mods {
        downs.push(key_input(modifier_vk(*m), KEYBD_EVENT_FLAGS(0)));
    }
    downs.push(key_input(vk, KEYBD_EVENT_FLAGS(0)));

    // Releases in reverse; always flush these so a partial/blocked SendInput can
    // never strand ctrl/shift/etc held down system-wide.
    let mut ups = vec![key_input(vk, KEYEVENTF_KEYUP)];
    for m in mods.iter().rev() {
        ups.push(key_input(modifier_vk(*m), KEYEVENTF_KEYUP));
    }

    let result = send(&downs).and_then(|_| send(&ups));
    if result.is_err() {
        let _ = send(&ups);
    }
    result?;
    Ok(Ack::new("key.press"))
}
