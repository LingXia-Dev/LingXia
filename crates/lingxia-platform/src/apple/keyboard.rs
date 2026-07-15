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

        let _: () = msg_send![window, makeKeyAndOrderFront: std::ptr::null_mut::<AnyObject>()];
        dispatch_action(window, action)?;

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
        // A keyDown carrying `characters` drives `insertText:` on the focused
        // control; keyCode 0 is fine since ASCII (URLs) route by characters.
        AppKeyboardAction::Type { text } => {
            post_key(window, 0, text, NSEventModifierFlags::empty(), true)?;
            post_key(window, 0, text, NSEventModifierFlags::empty(), false)
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
