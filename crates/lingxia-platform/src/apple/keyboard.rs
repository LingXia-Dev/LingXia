use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::keyboard::{AppKeyboard, AppKeyboardRequest, AppKeyboardResult};
// Keyboard synthesis is macOS-only; iOS has no hardware keyboard to drive.
#[cfg(target_os = "macos")]
use crate::traits::keyboard::{AppKeyboardAction, AppKeyboardModifier};
use async_trait::async_trait;

#[cfg(target_os = "macos")]
use std::time::Duration;
#[cfg(target_os = "macos")]
use tokio::sync::oneshot;
#[cfg(target_os = "macos")]
use tokio::time::timeout;

#[async_trait]
impl AppKeyboard for Platform {
    async fn perform_app_keyboard(
        &self,
        request: AppKeyboardRequest,
    ) -> Result<AppKeyboardResult, PlatformError> {
        #[cfg(target_os = "macos")]
        {
            perform_app_keyboard_macos(request).await
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = request;
            Err(PlatformError::NotSupported(
                "app keyboard input is not implemented on this Apple target".to_string(),
            ))
        }
    }
}

#[cfg(target_os = "macos")]
async fn perform_app_keyboard_macos(
    request: AppKeyboardRequest,
) -> Result<AppKeyboardResult, PlatformError> {
    use super::mouse::parse_window_id;
    use dispatch2::DispatchQueue;

    const KEYBOARD_TIMEOUT: Duration = Duration::from_secs(2);

    let target_window_number = parse_window_id(request.window_id.as_deref())?;
    let action_kind = request.action.kind();
    let modifier_reliability = match &request.action {
        AppKeyboardAction::Press { modifiers, .. } if !modifiers.is_empty() => {
            Some("native".to_string())
        }
        _ => None,
    };
    let action = request.action;

    let (tx, rx) = oneshot::channel::<Result<String, String>>();

    DispatchQueue::main().exec_async(move || {
        let _ = tx.send(perform_app_keyboard_on_main(target_window_number, &action));
    });

    let window_id = match timeout(KEYBOARD_TIMEOUT, rx).await {
        Ok(Ok(Ok(id))) => id,
        Ok(Ok(Err(err))) => return Err(PlatformError::Platform(err)),
        Ok(Err(_)) => {
            return Err(PlatformError::Platform(
                "app keyboard request was canceled".to_string(),
            ));
        }
        Err(_) => {
            return Err(PlatformError::Platform(
                "app keyboard timed out".to_string(),
            ));
        }
    };

    Ok(AppKeyboardResult {
        window_id,
        action: action_kind.to_string(),
        modifier_reliability,
    })
}

#[cfg(target_os = "macos")]
fn perform_app_keyboard_on_main(
    target_window_number: Option<i64>,
    action: &AppKeyboardAction,
) -> Result<String, String> {
    use super::mouse::resolve_window;
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};

    unsafe {
        let app_class = class!(NSApplication);
        let app: *mut AnyObject = msg_send![app_class, sharedApplication];
        if app.is_null() {
            return Err("NSApplication.sharedApplication is null".to_string());
        }

        let Some(window) = resolve_window(app, target_window_number) else {
            return match target_window_number {
                Some(id) => Err(format!("no NSWindow with windowNumber={} in this app", id)),
                None => Err("no NSWindow available for keyboard input".to_string()),
            };
        };
        let window_number: isize = msg_send![window, windowNumber];

        // Key events need a key window; hand activation back to the app the user was in.
        let workspace: *mut AnyObject = msg_send![class!(NSWorkspace), sharedWorkspace];
        let previous: *mut AnyObject = msg_send![workspace, frontmostApplication];
        let _: () = msg_send![window, makeKeyAndOrderFront: std::ptr::null_mut::<AnyObject>()];
        let result = dispatch_action(window, action);
        let current: *mut AnyObject = msg_send![workspace, frontmostApplication];
        if !previous.is_null() && current != previous {
            let _: bool = msg_send![previous, activateWithOptions: 0usize];
        }
        result?;

        Ok(window_number.to_string())
    }
}

#[cfg(target_os = "macos")]
fn dispatch_action(
    window: *mut objc2::runtime::AnyObject,
    action: &AppKeyboardAction,
) -> Result<(), String> {
    use objc2_app_kit::NSEventModifierFlags;

    match action {
        AppKeyboardAction::Type { text, raw: true } => {
            post_key(window, 0, text, NSEventModifierFlags::empty(), true)?;
            post_key(window, 0, text, NSEventModifierFlags::empty(), false)
        }
        AppKeyboardAction::Type { text, raw: false } => {
            let layout = layout::keys();
            let mut buffer = [0u8; 4];
            for ch in text.chars() {
                match layout.get(&ch) {
                    Some(&(code, shift)) => {
                        post_layout_key(window, code, ch, shift, true)?;
                        post_layout_key(window, code, ch, shift, false)?;
                    }
                    // Not on the keyboard (CJK, emoji): delivered as its text.
                    None => {
                        let text = ch.encode_utf8(&mut buffer);
                        post_key(window, 0, text, NSEventModifierFlags::empty(), true)?;
                        post_key(window, 0, text, NSEventModifierFlags::empty(), false)?;
                    }
                }
            }
            Ok(())
        }
        AppKeyboardAction::Press { key, modifiers } => {
            let (key_code, characters) = resolve_key(key)?;
            let flags = modifier_flags(modifiers);
            post_key(window, key_code, characters, flags, true)?;
            post_key(window, key_code, characters, flags, false)
        }
    }
}

/// Maps a key name to its macOS virtual keycode and the character it emits
/// (so `insertText:`/key bindings on the focused control fire correctly).
#[cfg(target_os = "macos")]
fn resolve_key(name: &str) -> Result<(u16, &'static str), String> {
    Ok(match name.to_ascii_lowercase().as_str() {
        "return" | "enter" => (36, "\r"),
        "tab" => (48, "\t"),
        "escape" | "esc" => (53, "\u{1b}"),
        "delete" | "backspace" => (51, "\u{7f}"),
        "space" => (49, " "),
        "left" => (123, "\u{f702}"),
        "right" => (124, "\u{f703}"),
        "down" => (125, "\u{f701}"),
        "up" => (126, "\u{f700}"),
        other => return Err(format!("unknown key name: {other}")),
    })
}

#[cfg(target_os = "macos")]
fn modifier_flags(modifiers: &[AppKeyboardModifier]) -> objc2_app_kit::NSEventModifierFlags {
    use objc2_app_kit::NSEventModifierFlags;

    let mut flags = NSEventModifierFlags::empty();
    for modifier in modifiers {
        flags |= match modifier {
            AppKeyboardModifier::Command => NSEventModifierFlags::Command,
            AppKeyboardModifier::Shift => NSEventModifierFlags::Shift,
            AppKeyboardModifier::Option => NSEventModifierFlags::Option,
            AppKeyboardModifier::Control => NSEventModifierFlags::Control,
        };
    }
    flags
}

#[cfg(target_os = "macos")]
fn post_key(
    window: *mut objc2::runtime::AnyObject,
    key_code: u16,
    characters: &str,
    modifiers: objc2_app_kit::NSEventModifierFlags,
    is_down: bool,
) -> Result<(), String> {
    use super::mouse::send_event_to_window;
    use objc2::msg_send;
    use objc2_app_kit::{NSEvent, NSEventType};
    use objc2_foundation::{NSPoint, NSString};

    unsafe {
        let window_number: isize = msg_send![window, windowNumber];
        let chars = NSString::from_str(characters);
        let event_type = if is_down {
            NSEventType::KeyDown
        } else {
            NSEventType::KeyUp
        };
        let event = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
            event_type,
            NSPoint::new(0.0, 0.0),
            modifiers,
            0.0,
            window_number,
            None,
            &chars,
            &chars,
            false,
            key_code,
        )
        .ok_or_else(|| "failed to create key event".to_string())?;

        send_event_to_window(window, event.as_ref());
    }

    Ok(())
}

/// A real key: its virtual key code and, as a keyboard would, its characters,
/// so the focused view's input context (an IME) handles it.
#[cfg(target_os = "macos")]
fn post_layout_key(
    window: *mut objc2::runtime::AnyObject,
    code: u16,
    ch: char,
    shift: bool,
    is_down: bool,
) -> Result<(), String> {
    use objc2_app_kit::NSEvent;
    use objc2_core_graphics::{CGEvent, CGEventFlags};

    let event =
        CGEvent::new_keyboard_event(None, code, is_down).ok_or("failed to create key event")?;
    let flags = if shift {
        CGEventFlags::MaskShift
    } else {
        CGEventFlags::empty()
    };
    CGEvent::set_flags(Some(&event), flags);
    let mut units = [0u16; 2];
    let units = ch.encode_utf16(&mut units);
    unsafe {
        CGEvent::keyboard_set_unicode_string(Some(&event), units.len() as _, units.as_ptr());
        let ns = NSEvent::eventWithCGEvent(&event).ok_or("failed to convert key event")?;
        super::mouse::send_event_to_window(window, ns.as_ref());
    }
    Ok(())
}

/// The current keyboard layout, reversed: which key (and shift) types a character.
/// An input method's layout is its ASCII-capable one (ABC under Pinyin).
#[cfg(target_os = "macos")]
mod layout {
    use std::collections::HashMap;
    use std::ffi::c_void;

    type CopySource = unsafe extern "C" fn() -> *mut c_void;
    type GetProperty = unsafe extern "C" fn(*mut c_void, *const c_void) -> *const c_void;
    type KbdType = unsafe extern "C" fn() -> u8;
    type Translate = unsafe extern "C" fn(
        *const c_void,
        u16,
        u16,
        u32,
        u32,
        u32,
        *mut u32,
        usize,
        *mut usize,
        *mut u16,
    ) -> i32;

    // Text Input Sources live in Carbon, which the host app's link does not name;
    // AppKit has already loaded it, so the symbols resolve at run time.
    unsafe extern "C" {
        fn dlsym(handle: *mut c_void, symbol: *const std::ffi::c_char) -> *mut c_void;
        fn CFDataGetBytePtr(data: *const c_void) -> *const u8;
        fn CFRelease(object: *const c_void);
    }
    const RTLD_DEFAULT: *mut c_void = -2isize as *mut c_void;

    unsafe fn symbol(name: &std::ffi::CStr) -> Option<*mut c_void> {
        let found = unsafe { dlsym(RTLD_DEFAULT, name.as_ptr()) };
        (!found.is_null()).then_some(found)
    }

    const KEY_DOWN: u16 = 0;
    const NO_DEAD_KEYS: u32 = 1;
    /// `shiftKey >> 8`, as UCKeyTranslate takes modifier state.
    const SHIFT: u32 = 0x02;

    pub(super) fn keys() -> HashMap<char, (u16, bool)> {
        let mut keys = HashMap::new();
        unsafe {
            let (Some(copy), Some(get), Some(kbd), Some(translate), Some(property)) = (
                symbol(c"TISCopyCurrentKeyboardLayoutInputSource"),
                symbol(c"TISGetInputSourceProperty"),
                symbol(c"LMGetKbdType"),
                symbol(c"UCKeyTranslate"),
                symbol(c"kTISPropertyUnicodeKeyLayoutData"),
            ) else {
                return keys;
            };
            let copy: CopySource = std::mem::transmute(copy);
            let get: GetProperty = std::mem::transmute(get);
            let kbd: KbdType = std::mem::transmute(kbd);
            let translate: Translate = std::mem::transmute(translate);
            let property = *(property as *const *const c_void);
            let source = copy();
            if source.is_null() {
                return keys;
            }
            let data = get(source, property);
            if !data.is_null() {
                let layout = CFDataGetBytePtr(data) as *const c_void;
                let keyboard = u32::from(kbd());
                for shift in [false, true] {
                    for code in 0u16..128 {
                        let mut dead = 0u32;
                        let mut len = 0usize;
                        let mut out = [0u16; 4];
                        let status = translate(
                            layout,
                            code,
                            KEY_DOWN,
                            if shift { SHIFT } else { 0 },
                            keyboard,
                            NO_DEAD_KEYS,
                            &mut dead,
                            out.len(),
                            &mut len,
                            out.as_mut_ptr(),
                        );
                        if status != 0 || len != 1 {
                            continue;
                        }
                        if let Some(Ok(ch)) = char::decode_utf16([out[0]]).next()
                            && !ch.is_control()
                        {
                            keys.entry(ch).or_insert((code, shift));
                        }
                    }
                }
            }
            CFRelease(source);
        }
        keys
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    // The app gets Carbon through AppKit; a test binary has to ask for it.
    #[link(name = "Carbon", kind = "framework")]
    unsafe extern "C" {}

    #[test]
    fn the_current_layout_types_letters_and_their_capitals() {
        let keys = super::layout::keys();
        for ch in 'a'..='z' {
            let (code, shift) = keys[&ch];
            assert!(!shift, "{ch}");
            assert_eq!(keys[&ch.to_ascii_uppercase()], (code, true), "{ch}");
        }
    }
}
