use super::app::Platform;
use crate::traits::screenshot::{AppScreenshot, WindowInfo};
use async_trait::async_trait;

use lingxia_error::Error;
#[cfg(any(target_os = "ios", target_os = "macos"))]
use std::sync::{Arc, Mutex};
#[cfg(any(target_os = "ios", target_os = "macos"))]
use std::time::Duration;
#[cfg(any(target_os = "ios", target_os = "macos"))]
use tokio::sync::oneshot;
#[cfg(any(target_os = "ios", target_os = "macos"))]
use tokio::time::timeout;

#[async_trait]
impl AppScreenshot for Platform {
    async fn list_app_windows(&self) -> Result<Vec<WindowInfo>, Error> {
        #[cfg(target_os = "macos")]
        {
            list_app_windows_macos().await
        }
        #[cfg(target_os = "ios")]
        {
            list_app_windows_ios().await
        }
        #[cfg(not(any(target_os = "ios", target_os = "macos")))]
        {
            Err(Error::not_supported(
                "list_app_windows is not implemented on this Apple target".to_string(),
            ))
        }
    }

    async fn bring_app_window_to_front(
        &self,
        window_id: Option<&str>,
        focus: bool,
    ) -> Result<WindowInfo, Error> {
        #[cfg(target_os = "macos")]
        {
            let window = self.resolve_app_window(window_id).await?;
            bring_to_front_macos(&window.id, focus, false).await?;
            list_app_windows_macos()
                .await?
                .into_iter()
                .find(|listed| listed.id == window.id)
                .ok_or_else(|| Error::internal(format!("window {} closed", window.id)))
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (window_id, focus);
            Err(Error::not_supported(
                "raising an app window is macOS-only on Apple platforms".to_string(),
            ))
        }
    }

    async fn raise_app_window_if_hidden(&self) -> Result<Option<&'static str>, Error> {
        #[cfg(target_os = "macos")]
        {
            let window = self.resolve_app_window(None).await?;
            bring_to_front_macos(&window.id, false, true).await
        }
        #[cfg(not(target_os = "macos"))]
        {
            Err(Error::not_supported(
                "raising an app window is macOS-only on Apple platforms".to_string(),
            ))
        }
    }

    async fn take_app_screenshot(&self, window_id: Option<&str>) -> Result<Vec<u8>, Error> {
        #[cfg(target_os = "ios")]
        {
            // iOS apps are effectively single-window; ignore selector.
            let _ = window_id;
            take_app_screenshot_ios().await
        }
        #[cfg(target_os = "macos")]
        {
            take_app_screenshot_macos(window_id).await
        }
        #[cfg(not(any(target_os = "ios", target_os = "macos")))]
        {
            let _ = window_id;
            Err(Error::not_supported(
                "app screenshot is not yet implemented on this Apple target".to_string(),
            ))
        }
    }
}

#[cfg(target_os = "ios")]
unsafe extern "C" {
    fn lingxia_ios_capture_app_png(out_len: *mut usize) -> *mut u8;
    fn lingxia_ios_capture_app_png_free(ptr: *mut u8, len: usize);
}

/// Capture every window in the foreground scene, including `AVPlayerLayer`
/// frames. The Swift helper (`lingxia_ios_capture_app_png`) does the work:
/// `CALayer.render(in:)` skips hardware video layers, which is why a playing
/// preview used to come back as a black PNG.
#[cfg(target_os = "ios")]
async fn take_app_screenshot_ios() -> Result<Vec<u8>, Error> {
    use dispatch2::DispatchQueue;

    const SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(8);
    let (tx, rx) = oneshot::channel::<Result<Vec<u8>, String>>();
    let tx_state = Arc::new(Mutex::new(Some(tx)));
    let tx_state_for_block = Arc::clone(&tx_state);

    DispatchQueue::main().exec_async(move || {
        let sender = tx_state_for_block
            .lock()
            .ok()
            .and_then(|mut guard| guard.take());
        let Some(sender) = sender else { return };

        let mut len: usize = 0;
        let ptr = unsafe { lingxia_ios_capture_app_png(&mut len) };
        if ptr.is_null() || len == 0 {
            if !ptr.is_null() {
                unsafe { lingxia_ios_capture_app_png_free(ptr, len) };
            }
            let _ = sender.send(Err("iOS app screenshot returned no pixels".to_string()));
            return;
        }
        let bytes = unsafe { std::slice::from_raw_parts(ptr, len).to_vec() };
        unsafe { lingxia_ios_capture_app_png_free(ptr, len) };
        let _ = sender.send(Ok(bytes));
    });

    match timeout(SNAPSHOT_TIMEOUT, rx).await {
        Ok(Ok(Ok(bytes))) => Ok(bytes),
        Ok(Ok(Err(err))) => Err(Error::internal(err)),
        Ok(Err(_)) => Err(Error::internal(
            "app screenshot request was canceled".to_string(),
        )),
        Err(_) => Err(Error::internal("app screenshot timed out".to_string())),
    }
}

/// Enumerate this app's top-level NSWindows. No privacy permission needed
/// since `NSApplication.windows` only sees the calling app's own windows.
#[cfg(target_os = "macos")]
async fn list_app_windows_macos() -> Result<Vec<WindowInfo>, Error> {
    use dispatch2::DispatchQueue;
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};
    use objc2_core_foundation::CGRect;
    use objc2_foundation::NSString;

    const ENUMERATE_TIMEOUT: Duration = Duration::from_secs(2);
    let (tx, rx) = oneshot::channel::<Result<Vec<WindowInfo>, String>>();
    let tx_state = Arc::new(Mutex::new(Some(tx)));
    let tx_state_for_block = Arc::clone(&tx_state);

    DispatchQueue::main().exec_async(move || unsafe {
        let sender = tx_state_for_block
            .lock()
            .ok()
            .and_then(|mut guard| guard.take());
        let Some(sender) = sender else { return };

        let app_class = class!(NSApplication);
        let app: *mut AnyObject = msg_send![app_class, sharedApplication];
        if app.is_null() {
            let _ = sender.send(Err("NSApplication.sharedApplication is null".to_string()));
            return;
        }
        let windows: *mut AnyObject = msg_send![app, windows];
        if windows.is_null() {
            let _ = sender.send(Ok(Vec::new()));
            return;
        }
        let count: usize = msg_send![windows, count];
        let key_window: *mut AnyObject = msg_send![app, keyWindow];
        let main_window: *mut AnyObject = msg_send![app, mainWindow];

        let mut out = Vec::with_capacity(count);
        for index in 0..count {
            let window: *mut AnyObject = msg_send![windows, objectAtIndex: index];
            if window.is_null() {
                continue;
            }
            let window_number: i64 = msg_send![window, windowNumber];
            let title_ptr: *mut NSString = msg_send![window, title];
            let title = if title_ptr.is_null() {
                String::new()
            } else {
                (&*title_ptr).to_string()
            };
            // `isVisible` is still YES for minimized windows on macOS, so
            // combine with `isMiniaturized` to match user expectation that
            // a Dock-iconified window is not "visible" for screenshot.
            let is_visible: bool = msg_send![window, isVisible];
            let is_miniaturized: bool = msg_send![window, isMiniaturized];
            let content_view: *mut AnyObject = msg_send![window, contentView];
            let (content_width, content_height) = if content_view.is_null() {
                (0, 0)
            } else {
                let content_bounds: CGRect = msg_send![content_view, bounds];
                (
                    content_bounds.size.width.max(0.0) as u32,
                    content_bounds.size.height.max(0.0) as u32,
                )
            };
            out.push(WindowInfo {
                id: window_number.to_string(),
                title,
                focused: window == key_window,
                main: window == main_window,
                visible: is_visible && !is_miniaturized,
                width: content_width,
                height: content_height,
            });
        }
        let _ = sender.send(Ok(out));
    });

    match timeout(ENUMERATE_TIMEOUT, rx).await {
        Ok(Ok(Ok(list))) => Ok(list),
        Ok(Ok(Err(err))) => Err(Error::internal(err)),
        Ok(Err(_)) => Err(Error::internal(
            "list_app_windows request was canceled".to_string(),
        )),
        Err(_) => Err(Error::internal("list_app_windows timed out".to_string())),
    }
}

/// Order this app's window `id` in front of every other app's windows at its
/// level, on the main queue. Public AppKit acting on the app's own window, so
/// it needs no Accessibility permission. `orderFrontRegardless` works while
/// the app is inactive; activation is cooperative on current macOS, so with
/// `focus` the window may be raised without becoming key if the system
/// declines to switch apps.
#[cfg(target_os = "macos")]
/// Raises window `id`. With `only_if_hidden` it acts only on a minimized window or one nothing of
/// which is visible, and says why it raised; it never activates the app then.
async fn bring_to_front_macos(
    id: &str,
    focus: bool,
    only_if_hidden: bool,
) -> Result<Option<&'static str>, Error> {
    use dispatch2::DispatchQueue;
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};

    const RAISE_TIMEOUT: Duration = Duration::from_secs(2);
    let number: i64 = id
        .parse()
        .map_err(|_| Error::invalid_arg(format!("not a window id: {id}")))?;
    let (tx, rx) = oneshot::channel::<Result<Option<&'static str>, String>>();
    let tx_state = Arc::new(Mutex::new(Some(tx)));
    let tx_state_for_block = Arc::clone(&tx_state);

    DispatchQueue::main().exec_async(move || unsafe {
        let sender = tx_state_for_block
            .lock()
            .ok()
            .and_then(|mut guard| guard.take());
        let Some(sender) = sender else { return };

        let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        if app.is_null() {
            let _ = sender.send(Err("NSApplication.sharedApplication is null".to_string()));
            return;
        }
        let windows: *mut AnyObject = msg_send![app, windows];
        let count: usize = if windows.is_null() {
            0
        } else {
            msg_send![windows, count]
        };
        for index in 0..count {
            let window: *mut AnyObject = msg_send![windows, objectAtIndex: index];
            if window.is_null() {
                continue;
            }
            let window_number: i64 = msg_send![window, windowNumber];
            if window_number != number {
                continue;
            }
            let miniaturized: bool = msg_send![window, isMiniaturized];
            // NSWindowOcclusionStateVisible
            let occlusion: usize = msg_send![window, occlusionState];
            let reason = if miniaturized {
                "minimized"
            } else if occlusion & (1 << 1) == 0 {
                "covered by other windows"
            } else {
                "requested"
            };
            if only_if_hidden && reason == "requested" {
                let _ = sender.send(Ok(None));
                return;
            }
            if miniaturized {
                let _: () = msg_send![window, deminiaturize: std::ptr::null_mut::<AnyObject>()];
            }
            if focus {
                let _: () = msg_send![app, activateIgnoringOtherApps: true];
                let _: () =
                    msg_send![window, makeKeyAndOrderFront: std::ptr::null_mut::<AnyObject>()];
            }
            let _: () = msg_send![window, orderFrontRegardless];
            let _ = sender.send(Ok(Some(reason)));
            return;
        }
        let _ = sender.send(Err(format!("window {number} no longer exists")));
    });

    match timeout(RAISE_TIMEOUT, rx).await {
        Ok(Ok(result)) => result.map_err(Error::internal),
        Ok(Err(_)) => Err(Error::internal(
            "raising the window was canceled".to_string(),
        )),
        Err(_) => Err(Error::internal("raising the window timed out".to_string())),
    }
}

/// iOS apps have a single key window in practice. Return a single entry
/// derived from the key UIWindow so the API surface is consistent.
#[cfg(target_os = "ios")]
async fn list_app_windows_ios() -> Result<Vec<WindowInfo>, Error> {
    // The iOS app-screenshot path captures the key window unconditionally;
    // surface a single-element list here so `lxdev host windows` has a
    // useful answer on iOS too.
    Ok(vec![WindowInfo {
        id: "main".to_string(),
        title: String::new(),
        focused: true,
        main: true,
        visible: true,
        width: 0,
        height: 0,
    }])
}

/// The window server's composite of `window` and its child windows (the feedback layer), cut to
/// the content view's `frame`: what is on screen, Metal layers and see-through windows included.
/// An app may capture its own windows without the Screen Recording permission.
#[cfg(target_os = "macos")]
#[allow(deprecated)]
unsafe fn window_server_image(
    window: *mut objc2::runtime::AnyObject,
    frame: objc2_core_foundation::CGRect,
) -> Result<objc2_core_foundation::CFRetained<objc2_core_graphics::CGImage>, String> {
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};
    use objc2_core_foundation::{CFArray, CGPoint, CGRect};
    use objc2_core_graphics::{CGWindowImageOption, CGWindowListCreateImageFromArray};

    // Front to back: the children (above the window) first.
    let mut ids: Vec<*const std::ffi::c_void> = Vec::new();
    let children: *mut AnyObject = msg_send![window, childWindows];
    if !children.is_null() {
        let count: usize = msg_send![children, count];
        for index in (0..count).rev() {
            let child: *mut AnyObject = msg_send![children, objectAtIndex: index];
            let visible: bool = msg_send![child, isVisible];
            let n: isize = msg_send![child, windowNumber];
            if visible && n > 0 {
                ids.push(n as usize as *const std::ffi::c_void);
            }
        }
    }
    let n: isize = msg_send![window, windowNumber];
    if n <= 0 {
        return Err("window is not on screen".to_string());
    }
    ids.push(n as usize as *const std::ffi::c_void);
    let array =
        unsafe { CFArray::new(None, ids.as_mut_ptr(), ids.len() as isize, std::ptr::null()) }
            .ok_or("CFArrayCreate failed")?;

    // Cocoa screen space (origin bottom-left of the primary screen) to Quartz (top-left).
    let on_screen: CGRect = msg_send![window, convertRectToScreen: frame];
    let screens: *mut AnyObject = msg_send![class!(NSScreen), screens];
    let primary: *mut AnyObject = msg_send![screens, objectAtIndex: 0usize];
    let primary_frame: CGRect = msg_send![primary, frame];
    let bounds = CGRect::new(
        CGPoint::new(
            on_screen.origin.x,
            primary_frame.size.height - on_screen.origin.y - on_screen.size.height,
        ),
        on_screen.size,
    );
    unsafe { CGWindowListCreateImageFromArray(bounds, &array, CGWindowImageOption::BestResolution) }
        .ok_or_else(|| "the window server returned no image of the window".to_string())
}

/// Whether the window server shows windows now: a locked screen or a sleeping display yields a
/// blank composite.
#[cfg(target_os = "macos")]
unsafe fn screen_shows_windows() -> bool {
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};
    unsafe extern "C" {
        fn CGSessionCopyCurrentDictionary() -> *mut AnyObject;
        fn CGMainDisplayID() -> u32;
        fn CGDisplayIsAsleep(display: u32) -> u32;
        fn CFRelease(cf: *const std::ffi::c_void);
    }
    if unsafe { CGDisplayIsAsleep(CGMainDisplayID()) } != 0 {
        return false;
    }
    let session = unsafe { CGSessionCopyCurrentDictionary() };
    if session.is_null() {
        return false;
    }
    let key: *mut AnyObject = unsafe {
        msg_send![class!(NSString), stringWithUTF8String: c"CGSSessionScreenIsLocked".as_ptr()]
    };
    let locked: *mut AnyObject = unsafe { msg_send![session, objectForKey: key] };
    let locked = !locked.is_null() && unsafe { msg_send![locked, boolValue] };
    unsafe { CFRelease(session as *const std::ffi::c_void) };
    !locked
}

/// macOS implementation: the window server's composite of the window (see
/// `window_server_image`), so a capture shows exactly what the user sees.
///
/// When `window_id` is `Some`, looks up the NSWindow with that
/// `windowNumber`. When `None`, falls back to key → main → first window.
#[cfg(target_os = "macos")]
async fn take_app_screenshot_macos(window_id: Option<&str>) -> Result<Vec<u8>, Error> {
    use dispatch2::DispatchQueue;
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};
    use objc2_core_foundation::CGRect;

    // NSBitmapImageFileType.PNG raw value.
    const NS_BITMAP_PNG: u64 = 4;
    const SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(5);

    // Parse the requested window id on the caller's thread so we don't have
    // to ferry the string into the dispatch block.
    let target_window_number: Option<i64> = match window_id {
        Some(raw) => match raw.parse::<i64>() {
            Ok(n) => Some(n),
            Err(_) => {
                return Err(Error::invalid_arg(format!(
                    "window id must be a numeric NSWindow.windowNumber, got: {raw}"
                )));
            }
        },
        None => None,
    };

    let (tx, rx) = oneshot::channel::<Result<Vec<u8>, String>>();
    let tx_state = Arc::new(Mutex::new(Some(tx)));
    let tx_state_for_block = Arc::clone(&tx_state);

    DispatchQueue::main().exec_async(move || unsafe {
        let sender = tx_state_for_block
            .lock()
            .ok()
            .and_then(|mut guard| guard.take());
        let Some(sender) = sender else { return };

        let app_class = class!(NSApplication);
        let app: *mut AnyObject = msg_send![app_class, sharedApplication];
        if app.is_null() {
            let _ = sender.send(Err("NSApplication.sharedApplication is null".to_string()));
            return;
        }

        // Window resolution:
        //   * with explicit id → look it up by windowNumber
        //   * without id → keyWindow → mainWindow → first window
        let mut window: *mut AnyObject = std::ptr::null_mut();
        if let Some(requested) = target_window_number {
            let windows: *mut AnyObject = msg_send![app, windows];
            if !windows.is_null() {
                let count: usize = msg_send![windows, count];
                for index in 0..count {
                    let candidate: *mut AnyObject = msg_send![windows, objectAtIndex: index];
                    if candidate.is_null() {
                        continue;
                    }
                    let n: i64 = msg_send![candidate, windowNumber];
                    if n == requested {
                        window = candidate;
                        break;
                    }
                }
            }
            if window.is_null() {
                let _ = sender.send(Err(format!(
                    "no NSWindow with windowNumber={} in this app",
                    requested
                )));
                return;
            }
        } else {
            window = msg_send![app, keyWindow];
            if window.is_null() {
                window = msg_send![app, mainWindow];
            }
            if window.is_null() {
                let windows: *mut AnyObject = msg_send![app, windows];
                if !windows.is_null() {
                    let count: usize = msg_send![windows, count];
                    if count > 0 {
                        window = msg_send![windows, objectAtIndex: 0usize];
                    }
                }
            }
        }
        if window.is_null() {
            let _ = sender.send(Err("no NSWindow available to snapshot".to_string()));
            return;
        }

        let content_view: *mut AnyObject = msg_send![window, contentView];
        if content_view.is_null() {
            let _ = sender.send(Err("NSWindow has no contentView".to_string()));
            return;
        }

        let frame: CGRect = msg_send![content_view, frame];
        if frame.size.width <= 0.0 || frame.size.height <= 0.0 {
            let _ = sender.send(Err(format!(
                "contentView has empty bounds {}x{}",
                frame.size.width, frame.size.height
            )));
            return;
        }
        // The window server's composite is what the user sees, and an app may read its own
        // windows without Screen Recording. A window it cannot image (off screen, locked screen,
        // sleeping display) is rebuilt from the view tree and each GPU page's last presented frame.
        let composite = screen_shows_windows()
            .then(|| window_server_image(window, frame).ok())
            .flatten();
        let rep: Option<objc2::rc::Retained<AnyObject>> = if let Some(image) = composite {
            let allocated: objc2::rc::Allocated<AnyObject> =
                msg_send![class!(NSBitmapImageRep), alloc];
            msg_send![allocated, initWithCGImage: &*image]
        } else {
            let bounds: CGRect = msg_send![content_view, bounds];
            let cached: Option<objc2::rc::Retained<AnyObject>> =
                msg_send![content_view, bitmapImageRepForCachingDisplayInRect: bounds];
            if let Some(cached) = &cached {
                let _: () = msg_send![
                    content_view,
                    cacheDisplayInRect: bounds,
                    toBitmapImageRep: &**cached
                ];
                // GPU pages present through Metal drawables, which caching cannot see; the
                // SDK draws their last presented frames over the bitmap when it is linked.
                if let Some(capture) = objc2::runtime::AnyClass::get(c"LingXiaGpuCapture") {
                    let _: () =
                        msg_send![capture, overlayContentView: content_view, into: &**cached];
                }
            }
            cached
        };
        let Some(rep) = rep else {
            let _ = sender.send(Err("could not create the window's bitmap".to_string()));
            return;
        };
        let bitmap_rep = objc2::rc::Retained::as_ptr(&rep) as *mut AnyObject;

        let empty_props_class = class!(NSDictionary);
        let empty_props: *mut AnyObject = msg_send![empty_props_class, dictionary];
        let png_data: *mut AnyObject = msg_send![
            bitmap_rep,
            representationUsingType: NS_BITMAP_PNG,
            properties: empty_props
        ];
        if png_data.is_null() {
            let _ = sender.send(Err(
                "NSBitmapImageRep failed to produce PNG data".to_string()
            ));
            return;
        }
        let length: usize = msg_send![png_data, length];
        let bytes_ptr: *const u8 = {
            let sel = objc2::sel!(bytes);
            let func: unsafe extern "C" fn(
                *mut AnyObject,
                objc2::runtime::Sel,
            ) -> *const core::ffi::c_void =
                core::mem::transmute(objc2::ffi::objc_msgSend as *const ());
            func(png_data, sel).cast()
        };
        if bytes_ptr.is_null() || length == 0 {
            let _ = sender.send(Err("PNG data was empty".to_string()));
            return;
        }
        let bytes = std::slice::from_raw_parts(bytes_ptr, length).to_vec();
        let _ = sender.send(Ok(bytes));
    });

    match timeout(SNAPSHOT_TIMEOUT, rx).await {
        Ok(Ok(Ok(bytes))) => Ok(bytes),
        Ok(Ok(Err(err))) => Err(Error::internal(err)),
        Ok(Err(_)) => Err(Error::internal(
            "app screenshot request was canceled".to_string(),
        )),
        Err(_) => Err(Error::internal("app screenshot timed out".to_string())),
    }
}
