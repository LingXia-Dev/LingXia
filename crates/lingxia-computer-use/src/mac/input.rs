//! Synthetic pointer/keyboard input via `CGEvent`. Requires the Accessibility
//! permission; a denied post surfaces as `Error::Permission`. Coordinates are
//! global display points with a top-left origin, matching `windows()`.
//!
//! Every entry point takes a `target: Option<u32>` pid. When `Some`, events are
//! delivered straight to that process with `CGEventPostToPid` — this drives an
//! app **in the background** without bringing it to the foreground. When `None`,
//! events go to the active session (foreground app), like a physical device.

use super::keymap::keycode;
use crate::error::{Error, Result};
use crate::model::{Ack, Modifier, MouseButton};
use objc2_core_foundation::{CFRetained, CGPoint};
use objc2_core_graphics::{
    CGEvent, CGEventField, CGEventFlags, CGEventSource, CGEventSourceStateID, CGEventTapLocation,
    CGEventType, CGMouseButton, CGPreflightPostEventAccess, CGScrollEventUnit,
    CGWarpMouseCursorPosition,
};

fn point(x: i32, y: i32) -> CGPoint {
    CGPoint::new(x as f64, y as f64)
}

/// A synthetic HID event source. Events created without a source are rejected by
/// some apps (Calculator, secure fields); a real source makes them behave like
/// physical input.
fn source() -> Option<CFRetained<CGEventSource>> {
    CGEventSource::new(CGEventSourceStateID::HIDSystemState)
}

/// Fail early with a permission error if we cannot post events.
fn ensure_can_post() -> Result<()> {
    if CGPreflightPostEventAccess() {
        Ok(())
    } else {
        Err(Error::Permission(
            "input denied: grant Accessibility to this terminal in System Settings › Privacy & Security".into(),
        ))
    }
}

/// Deliver an event: to a specific process when `target` is given (background),
/// otherwise to the HID tap (foreground/active app).
fn post(event: &CGEvent, target: Option<u32>) {
    match target {
        Some(pid) => CGEvent::post_to_pid(pid as i32, Some(event)),
        None => CGEvent::post(CGEventTapLocation::HIDEventTap, Some(event)),
    }
}

/// Keyboard delivery. With an explicit `target` it goes straight to that pid;
/// otherwise it targets the frontmost app's process directly (more reliable for
/// a short-lived injector than tap routing), falling back to the HID tap.
fn post_key(event: &CGEvent, target: Option<u32>) {
    let pid = target.map(|p| p as i32).or_else(super::frontmost_pid);
    match pid {
        Some(pid) if pid > 0 => CGEvent::post_to_pid(pid, Some(event)),
        _ => CGEvent::post(CGEventTapLocation::HIDEventTap, Some(event)),
    }
}

/// Keep the process (and the CGEventSource) alive briefly so the WindowServer
/// drains injected events before `lxdev` exits — otherwise a one-shot injector
/// can post a key and die before the event is delivered.
fn flush() {
    std::thread::sleep(std::time::Duration::from_millis(40));
}

fn cg_button(button: MouseButton) -> CGMouseButton {
    match button {
        MouseButton::Left => CGMouseButton::Left,
        MouseButton::Right => CGMouseButton::Right,
        MouseButton::Middle => CGMouseButton::Center,
    }
}

fn down_type(button: MouseButton) -> CGEventType {
    match button {
        MouseButton::Left => CGEventType::LeftMouseDown,
        MouseButton::Right => CGEventType::RightMouseDown,
        MouseButton::Middle => CGEventType::OtherMouseDown,
    }
}

fn up_type(button: MouseButton) -> CGEventType {
    match button {
        MouseButton::Left => CGEventType::LeftMouseUp,
        MouseButton::Right => CGEventType::RightMouseUp,
        MouseButton::Middle => CGEventType::OtherMouseUp,
    }
}

fn drag_type(button: MouseButton) -> CGEventType {
    match button {
        MouseButton::Left => CGEventType::LeftMouseDragged,
        MouseButton::Right => CGEventType::RightMouseDragged,
        MouseButton::Middle => CGEventType::OtherMouseDragged,
    }
}

fn mouse_event(
    kind: CGEventType,
    x: i32,
    y: i32,
    button: MouseButton,
    target: Option<u32>,
) -> Result<()> {
    let event = CGEvent::new_mouse_event(source().as_deref(), kind, point(x, y), cg_button(button))
        .ok_or_else(|| Error::Failed("could not create mouse event".into()))?;
    post(&event, target);
    Ok(())
}

pub fn pointer_move(x: i32, y: i32, target: Option<u32>) -> Result<Ack> {
    ensure_can_post()?;
    // Warping the OS cursor only makes sense for foreground input; a background
    // target should not hijack the visible cursor.
    if target.is_none() {
        let _ = CGWarpMouseCursorPosition(point(x, y));
    }
    mouse_event(CGEventType::MouseMoved, x, y, MouseButton::Left, target)?;
    flush();
    Ok(Ack::new("pointer.move"))
}

pub fn pointer_down(x: i32, y: i32, button: MouseButton, target: Option<u32>) -> Result<Ack> {
    ensure_can_post()?;
    mouse_event(down_type(button), x, y, button, target)?;
    flush();
    Ok(Ack::new("pointer.down"))
}

pub fn pointer_up(x: i32, y: i32, button: MouseButton, target: Option<u32>) -> Result<Ack> {
    ensure_can_post()?;
    mouse_event(up_type(button), x, y, button, target)?;
    flush();
    Ok(Ack::new("pointer.up"))
}

pub fn pointer_click(
    x: i32,
    y: i32,
    button: MouseButton,
    count: u32,
    target: Option<u32>,
) -> Result<Ack> {
    if count == 0 {
        return Err(Error::Usage("count must be greater than zero".into()));
    }
    ensure_can_post()?;
    // Each successive down/up carries the running click count so the target
    // recognizes double/triple clicks.
    for i in 1..=count {
        let down = CGEvent::new_mouse_event(
            source().as_deref(),
            down_type(button),
            point(x, y),
            cg_button(button),
        )
        .ok_or_else(|| Error::Failed("could not create mouse event".into()))?;
        CGEvent::set_integer_value_field(Some(&down), CGEventField::MouseEventClickState, i as i64);
        post(&down, target);
        let up = CGEvent::new_mouse_event(
            source().as_deref(),
            up_type(button),
            point(x, y),
            cg_button(button),
        )
        .ok_or_else(|| Error::Failed("could not create mouse event".into()))?;
        CGEvent::set_integer_value_field(Some(&up), CGEventField::MouseEventClickState, i as i64);
        post(&up, target);
    }
    flush();
    Ok(Ack::new("pointer.click"))
}

pub fn pointer_scroll(x: i32, y: i32, dx: i32, dy: i32, target: Option<u32>) -> Result<Ack> {
    ensure_can_post()?;
    if target.is_none() {
        let _ = CGWarpMouseCursorPosition(point(x, y));
    }
    // Positive dy scrolls content up on macOS; `dy` means "wheel up".
    let event = CGEvent::new_scroll_wheel_event2(
        source().as_deref(),
        CGScrollEventUnit::Line,
        2,
        dy,
        dx,
        0,
    )
    .ok_or_else(|| Error::Failed("could not create scroll event".into()))?;
    CGEvent::set_location(Some(&event), point(x, y));
    post(&event, target);
    flush();
    Ok(Ack::new("pointer.scroll"))
}

pub fn pointer_drag(
    fx: i32,
    fy: i32,
    tx: i32,
    ty: i32,
    button: MouseButton,
    target: Option<u32>,
) -> Result<Ack> {
    ensure_can_post()?;
    mouse_event(down_type(button), fx, fy, button, target)?;
    // Once the button is down, always release it — even on a mid-drag failure —
    // so a stuck button can never strand the pointer.
    let steps = (|| {
        for i in 1..=4 {
            let x = fx + (tx - fx) * i / 4;
            let y = fy + (ty - fy) * i / 4;
            mouse_event(drag_type(button), x, y, button, target)?;
        }
        Ok(())
    })();
    let up = mouse_event(up_type(button), tx, ty, button, target);
    steps?;
    up?;
    flush();
    Ok(Ack::new("pointer.drag"))
}

// ------------------------------- keyboard -------------------------------

pub fn key_type(text: &str, target: Option<u32>) -> Result<Ack> {
    ensure_can_post()?;
    // Type the literal characters via Unicode-string events (keycode 0). This
    // bypasses the keyboard layout and the IME, so CJK/emoji/symbols all work.
    for ch in text.chars() {
        let mut utf16 = [0u16; 2];
        let units = ch.encode_utf16(&mut utf16);
        for &down in &[true, false] {
            let event = CGEvent::new_keyboard_event(source().as_deref(), 0, down)
                .ok_or_else(|| Error::Failed("could not create keyboard event".into()))?;
            unsafe {
                CGEvent::keyboard_set_unicode_string(
                    Some(&event),
                    units.len() as _,
                    units.as_ptr(),
                );
            }
            post_key(&event, target);
        }
    }
    flush();
    Ok(Ack::new("key.type"))
}

fn modifier_flag(m: Modifier) -> CGEventFlags {
    match m {
        Modifier::Ctrl => CGEventFlags::MaskControl,
        Modifier::Shift => CGEventFlags::MaskShift,
        Modifier::Alt => CGEventFlags::MaskAlternate,
        Modifier::Meta => CGEventFlags::MaskCommand,
    }
}

fn resolve_key(name: &str) -> Result<u16> {
    keycode(name).ok_or_else(|| Error::Usage(format!("unknown key '{name}'")))
}

pub fn key_down(name: &str, target: Option<u32>) -> Result<Ack> {
    ensure_can_post()?;
    let code = resolve_key(name)?;
    let event = CGEvent::new_keyboard_event(source().as_deref(), code, true)
        .ok_or_else(|| Error::Failed("could not create keyboard event".into()))?;
    post_key(&event, target);
    flush();
    Ok(Ack::new("key.down"))
}

pub fn key_up(name: &str, target: Option<u32>) -> Result<Ack> {
    ensure_can_post()?;
    let code = resolve_key(name)?;
    let event = CGEvent::new_keyboard_event(source().as_deref(), code, false)
        .ok_or_else(|| Error::Failed("could not create keyboard event".into()))?;
    post_key(&event, target);
    flush();
    Ok(Ack::new("key.up"))
}

pub fn key_press(name: &str, mods: &[Modifier], target: Option<u32>) -> Result<Ack> {
    ensure_can_post()?;
    let code = resolve_key(name)?;
    let mut flags = CGEventFlags::empty();
    for m in mods {
        flags |= modifier_flag(*m);
    }
    let down = CGEvent::new_keyboard_event(source().as_deref(), code, true)
        .ok_or_else(|| Error::Failed("could not create keyboard event".into()))?;
    if !flags.is_empty() {
        CGEvent::set_flags(Some(&down), flags);
    }
    post_key(&down, target);
    let up = CGEvent::new_keyboard_event(source().as_deref(), code, false)
        .ok_or_else(|| Error::Failed("could not create keyboard event".into()))?;
    if !flags.is_empty() {
        CGEvent::set_flags(Some(&up), flags);
    }
    post_key(&up, target);
    flush();
    Ok(Ack::new("key.press"))
}
