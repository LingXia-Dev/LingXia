use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::mouse::{AppMouse, AppMouseRequest, AppMouseResult};
// Mouse synthesis is macOS-only; iOS has no pointer to drive.
#[cfg(target_os = "macos")]
use crate::traits::mouse::{AppMouseAction, AppMouseButton};
use async_trait::async_trait;

#[cfg(target_os = "macos")]
use std::time::Duration;
#[cfg(target_os = "macos")]
use tokio::sync::oneshot;
#[cfg(target_os = "macos")]
use tokio::time::timeout;

#[async_trait]
impl AppMouse for Platform {
    async fn perform_app_mouse(
        &self,
        request: AppMouseRequest,
    ) -> Result<AppMouseResult, PlatformError> {
        #[cfg(target_os = "macos")]
        {
            perform_app_mouse_macos(request).await
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = request;
            Err(PlatformError::NotSupported(
                "app mouse input is not implemented on this Apple target".to_string(),
            ))
        }
    }
}

#[cfg(target_os = "macos")]
async fn perform_app_mouse_macos(
    request: AppMouseRequest,
) -> Result<AppMouseResult, PlatformError> {
    use dispatch2::DispatchQueue;

    const MOUSE_TIMEOUT: Duration = Duration::from_secs(2);

    validate_action(&request.action)?;
    let target_window_number = parse_window_id(request.window_id.as_deref())?;
    let action_kind = request.action.kind();
    let action = request.action;

    let (tx, rx) = oneshot::channel::<Result<String, String>>();

    DispatchQueue::main().exec_async(move || {
        let _ = tx.send(perform_app_mouse_on_main(target_window_number, &action));
    });

    let window_id = match timeout(MOUSE_TIMEOUT, rx).await {
        Ok(Ok(Ok(id))) => id,
        Ok(Ok(Err(err))) => return Err(PlatformError::Platform(err)),
        Ok(Err(_)) => {
            return Err(PlatformError::Platform(
                "app mouse request was canceled".to_string(),
            ));
        }
        Err(_) => return Err(PlatformError::Platform("app mouse timed out".to_string())),
    };

    Ok(AppMouseResult {
        window_id,
        action: action_kind.to_string(),
    })
}

#[cfg(target_os = "macos")]
pub(super) fn parse_window_id(raw: Option<&str>) -> Result<Option<i64>, PlatformError> {
    match raw {
        Some(value) => value.parse::<i64>().map(Some).map_err(|_| {
            PlatformError::InvalidParameter(format!(
                "window id must be a numeric NSWindow.windowNumber, got: {value}"
            ))
        }),
        None => Ok(None),
    }
}

#[cfg(target_os = "macos")]
fn validate_point(x: f64, y: f64) -> Result<(), PlatformError> {
    if !x.is_finite() || !y.is_finite() {
        return Err(PlatformError::InvalidParameter(
            "mouse coordinates must be finite numbers".to_string(),
        ));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn validate_delta(dx: f64, dy: f64) -> Result<(), PlatformError> {
    if !dx.is_finite() || !dy.is_finite() {
        return Err(PlatformError::InvalidParameter(
            "mouse scroll deltas must be finite numbers".to_string(),
        ));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn validate_action(action: &AppMouseAction) -> Result<(), PlatformError> {
    match action {
        AppMouseAction::Move { x, y }
        | AppMouseAction::Down { x, y, .. }
        | AppMouseAction::Up { x, y, .. } => validate_point(*x, *y),
        AppMouseAction::Click {
            x, y, click_count, ..
        } => {
            validate_point(*x, *y)?;
            if *click_count == 0 {
                return Err(PlatformError::InvalidParameter(
                    "click_count must be greater than zero".to_string(),
                ));
            }
            Ok(())
        }
        AppMouseAction::Drag {
            from_x,
            from_y,
            to_x,
            to_y,
            ..
        } => {
            validate_point(*from_x, *from_y)?;
            validate_point(*to_x, *to_y)
        }
        AppMouseAction::Scroll { x, y, dx, dy } => {
            validate_point(*x, *y)?;
            validate_delta(*dx, *dy)
        }
    }
}

#[cfg(target_os = "macos")]
fn perform_app_mouse_on_main(
    target_window_number: Option<i64>,
    action: &AppMouseAction,
) -> Result<String, String> {
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
                None => Err("no NSWindow available for mouse input".to_string()),
            };
        };
        let window_number: isize = msg_send![window, windowNumber];

        let content_view: *mut AnyObject = msg_send![window, contentView];
        if content_view.is_null() {
            return Err("NSWindow has no contentView".to_string());
        }

        let _: () = msg_send![window, makeKeyAndOrderFront: std::ptr::null_mut::<AnyObject>()];
        dispatch_action(window, content_view, action)?;

        Ok(window_number.to_string())
    }
}

#[cfg(target_os = "macos")]
pub(super) fn resolve_window(
    app: *mut objc2::runtime::AnyObject,
    target_window_number: Option<i64>,
) -> Option<*mut objc2::runtime::AnyObject> {
    use objc2::msg_send;

    unsafe {
        if let Some(requested) = target_window_number {
            let windows: *mut objc2::runtime::AnyObject = msg_send![app, windows];
            if windows.is_null() {
                return None;
            }
            let count: usize = msg_send![windows, count];
            for index in 0..count {
                let candidate: *mut objc2::runtime::AnyObject =
                    msg_send![windows, objectAtIndex: index];
                if candidate.is_null() {
                    continue;
                }
                let number: isize = msg_send![candidate, windowNumber];
                if number as i64 == requested {
                    return Some(candidate);
                }
            }
            return None;
        }

        let mut window: *mut objc2::runtime::AnyObject = msg_send![app, keyWindow];
        if window.is_null() {
            window = msg_send![app, mainWindow];
        }
        if window.is_null() {
            let windows: *mut objc2::runtime::AnyObject = msg_send![app, windows];
            if !windows.is_null() {
                let count: usize = msg_send![windows, count];
                if count > 0 {
                    window = msg_send![windows, objectAtIndex: 0usize];
                }
            }
        }
        (!window.is_null()).then_some(window)
    }
}

#[cfg(target_os = "macos")]
fn dispatch_action(
    window: *mut objc2::runtime::AnyObject,
    content_view: *mut objc2::runtime::AnyObject,
    action: &AppMouseAction,
) -> Result<(), String> {
    match action {
        AppMouseAction::Move { x, y } => post_mouse(window, content_view, *x, *y, MousePhase::Move),
        AppMouseAction::Down { x, y, button } => {
            post_mouse(window, content_view, *x, *y, MousePhase::Down(*button, 1))
        }
        AppMouseAction::Up { x, y, button } => {
            post_mouse(window, content_view, *x, *y, MousePhase::Up(*button, 1))
        }
        AppMouseAction::Click {
            x,
            y,
            button,
            click_count,
        } => {
            // click_count >= 1 is enforced by validate_action
            let count = isize::from(*click_count);
            // Deliver down+up through the app event queue (NSApp dispatch)
            // rather than window.sendEvent: a control hit by the down runs a
            // nested tracking loop that needs the up already queued (else the
            // main thread deadlocks) and reads NSApp.currentEvent, which only
            // the queue dispatch path maintains.
            queue_mouse(
                window,
                content_view,
                *x,
                *y,
                MousePhase::Down(*button, count),
            )?;
            queue_mouse(window, content_view, *x, *y, MousePhase::Up(*button, count))
        }
        AppMouseAction::Drag {
            from_x,
            from_y,
            to_x,
            to_y,
            button,
        } => {
            // Fully queued for the same reasons as Click.
            queue_mouse(
                window,
                content_view,
                *from_x,
                *from_y,
                MousePhase::Down(*button, 1),
            )?;
            queue_mouse(
                window,
                content_view,
                *to_x,
                *to_y,
                MousePhase::Drag(*button),
            )?;
            queue_mouse(
                window,
                content_view,
                *to_x,
                *to_y,
                MousePhase::Up(*button, 1),
            )
        }
        AppMouseAction::Scroll { x, y, dx, dy } => {
            post_scroll(window, content_view, *x, *y, *dx, *dy)
        }
    }
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy)]
enum MousePhase {
    Move,
    Down(AppMouseButton, isize),
    Up(AppMouseButton, isize),
    Drag(AppMouseButton),
}

#[cfg(target_os = "macos")]
fn event_type(phase: MousePhase) -> objc2_app_kit::NSEventType {
    use objc2_app_kit::NSEventType;
    match phase {
        MousePhase::Move => NSEventType::MouseMoved,
        MousePhase::Down(AppMouseButton::Left, _) => NSEventType::LeftMouseDown,
        MousePhase::Down(AppMouseButton::Right, _) => NSEventType::RightMouseDown,
        MousePhase::Down(AppMouseButton::Middle, _) => NSEventType::OtherMouseDown,
        MousePhase::Up(AppMouseButton::Left, _) => NSEventType::LeftMouseUp,
        MousePhase::Up(AppMouseButton::Right, _) => NSEventType::RightMouseUp,
        MousePhase::Up(AppMouseButton::Middle, _) => NSEventType::OtherMouseUp,
        MousePhase::Drag(AppMouseButton::Left) => NSEventType::LeftMouseDragged,
        MousePhase::Drag(AppMouseButton::Right) => NSEventType::RightMouseDragged,
        MousePhase::Drag(AppMouseButton::Middle) => NSEventType::OtherMouseDragged,
    }
}

#[cfg(target_os = "macos")]
fn click_count(phase: MousePhase) -> isize {
    match phase {
        MousePhase::Down(_, count) | MousePhase::Up(_, count) => count,
        MousePhase::Move | MousePhase::Drag(_) => 0,
    }
}

#[cfg(target_os = "macos")]
fn content_point_to_window_point(
    content_view: *mut objc2::runtime::AnyObject,
    x: f64,
    y: f64,
) -> Result<objc2_foundation::NSPoint, String> {
    use objc2_app_kit::NSView;
    use objc2_foundation::NSPoint;

    unsafe {
        let view = &*(content_view as *mut NSView);
        let bounds = view.bounds();
        if bounds.size.width <= 0.0 || bounds.size.height <= 0.0 {
            return Err(format!(
                "contentView has empty bounds {}x{}",
                bounds.size.width, bounds.size.height
            ));
        }
        let local_y = if view.isFlipped() {
            y
        } else {
            bounds.size.height - y
        };
        Ok(view.convertPoint_toView(NSPoint::new(x, local_y), None))
    }
}

/// Rebase the event onto the target window via the private
/// `_eventRelativeToWindow:` (falling back to the original event when it
/// returns nil), then deliver it through `NSWindow.sendEvent:`.
#[cfg(target_os = "macos")]
pub(super) unsafe fn send_event_to_window(
    window: *mut objc2::runtime::AnyObject,
    event: &objc2_app_kit::NSEvent,
) {
    use objc2::msg_send;
    use objc2_app_kit::NSEvent;

    let event_ptr = event as *const NSEvent as *mut objc2::runtime::AnyObject;
    let relative_event: *mut objc2::runtime::AnyObject =
        msg_send![event_ptr, _eventRelativeToWindow: window];
    let event_ptr = if relative_event.is_null() {
        event_ptr
    } else {
        relative_event
    };
    let _: () = msg_send![window, sendEvent: event_ptr];
}

#[cfg(target_os = "macos")]
fn make_mouse_event(
    window: *mut objc2::runtime::AnyObject,
    content_view: *mut objc2::runtime::AnyObject,
    x: f64,
    y: f64,
    phase: MousePhase,
) -> Result<objc2::rc::Retained<objc2_app_kit::NSEvent>, String> {
    use objc2::msg_send;
    use objc2_app_kit::{NSEvent, NSEventModifierFlags};

    let location = content_point_to_window_point(content_view, x, y)?;

    unsafe {
        let window_number: isize = msg_send![window, windowNumber];
        NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
            event_type(phase),
            location,
            NSEventModifierFlags::empty(),
            0.0,
            window_number,
            None,
            0,
            click_count(phase),
            1.0,
        )
        .ok_or_else(|| "failed to create mouse event".to_string())
    }
}

#[cfg(target_os = "macos")]
fn post_mouse(
    window: *mut objc2::runtime::AnyObject,
    content_view: *mut objc2::runtime::AnyObject,
    x: f64,
    y: f64,
    phase: MousePhase,
) -> Result<(), String> {
    let event = make_mouse_event(window, content_view, x, y, phase)?;
    unsafe {
        send_event_to_window(window, event.as_ref());
    }
    Ok(())
}

/// Enqueue the event in the app event queue (`postEvent:atStart:NO`) instead
/// of delivering it synchronously, so a nested mouse-tracking loop started by
/// an earlier down can consume it.
#[cfg(target_os = "macos")]
fn queue_mouse(
    window: *mut objc2::runtime::AnyObject,
    content_view: *mut objc2::runtime::AnyObject,
    x: f64,
    y: f64,
    phase: MousePhase,
) -> Result<(), String> {
    use objc2::msg_send;
    use objc2_app_kit::NSEvent;

    let event = make_mouse_event(window, content_view, x, y, phase)?;
    unsafe {
        let event_ptr = event.as_ref() as *const NSEvent as *mut objc2::runtime::AnyObject;
        let relative_event: *mut objc2::runtime::AnyObject =
            msg_send![event_ptr, _eventRelativeToWindow: window];
        let event_ptr = if relative_event.is_null() {
            event_ptr
        } else {
            relative_event
        };
        let app: *mut objc2::runtime::AnyObject =
            msg_send![objc2::class!(NSApplication), sharedApplication];
        let _: () = msg_send![app, postEvent: event_ptr, atStart: false];
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn post_scroll(
    window: *mut objc2::runtime::AnyObject,
    content_view: *mut objc2::runtime::AnyObject,
    x: f64,
    y: f64,
    dx: f64,
    dy: f64,
) -> Result<(), String> {
    use objc2_app_kit::{NSEvent, NSView};
    use objc2_core_foundation::CGPoint;
    use objc2_core_graphics::{CGDisplayBounds, CGEvent, CGMainDisplayID, CGScrollEventUnit};

    let window_point = content_point_to_window_point(content_view, x, y)?;

    unsafe {
        let view = &*(content_view as *mut NSView);
        let typed_window = view
            .window()
            .ok_or_else(|| "contentView is not attached to a window".to_string())?;
        let screen_point = typed_window.convertPointToScreen(window_point);
        let main_display_height = CGDisplayBounds(CGMainDisplayID()).size.height;
        let cg_point = CGPoint {
            x: screen_point.x,
            y: main_display_height - screen_point.y,
        };

        let event = CGEvent::new_scroll_wheel_event2(
            None,
            CGScrollEventUnit::Pixel,
            2,
            (-dy).round() as i32,
            (-dx).round() as i32,
            0,
        )
        .ok_or_else(|| "failed to create scroll event".to_string())?;
        CGEvent::set_location(Some(event.as_ref()), cg_point);

        let ns_event = NSEvent::eventWithCGEvent(event.as_ref())
            .ok_or_else(|| "failed to convert scroll event".to_string())?;
        send_event_to_window(window, ns_event.as_ref());
    }

    Ok(())
}
