use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::screenshot::{AppScreenshot, WindowInfo};
use async_trait::async_trait;

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
    async fn list_app_windows(&self) -> Result<Vec<WindowInfo>, PlatformError> {
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
            Err(PlatformError::NotSupported(
                "list_app_windows is not implemented on this Apple target".to_string(),
            ))
        }
    }

    async fn take_app_screenshot(&self, window_id: Option<&str>) -> Result<Vec<u8>, PlatformError> {
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
            Err(PlatformError::NotSupported(
                "app screenshot is not yet implemented on this Apple target".to_string(),
            ))
        }
    }
}

#[cfg(target_os = "ios")]
async fn take_app_screenshot_ios() -> Result<Vec<u8>, PlatformError> {
    use dispatch2::DispatchQueue;
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};
    use objc2_core_foundation::{CGRect, CGSize};

    // Free function declared instead of the (Swift-only) `-[UIImage pngData]`
    // selector; sending that selector via msg_send crashes the app.
    #[link(name = "UIKit", kind = "framework")]
    unsafe extern "C" {
        fn UIImagePNGRepresentation(image: *mut AnyObject) -> *mut AnyObject;
        fn UIGraphicsBeginImageContextWithOptions(size: CGSize, opaque: bool, scale: f64);
        fn UIGraphicsGetImageFromCurrentImageContext() -> *mut AnyObject;
        fn UIGraphicsEndImageContext();
        fn UIGraphicsGetCurrentContext() -> *mut core::ffi::c_void;
    }

    const SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(5);
    let (tx, rx) = oneshot::channel::<Result<Vec<u8>, String>>();
    let tx_state = Arc::new(Mutex::new(Some(tx)));
    let tx_state_for_block = Arc::clone(&tx_state);

    DispatchQueue::main().exec_async(move || unsafe {
        let sender = tx_state_for_block
            .lock()
            .ok()
            .and_then(|mut guard| guard.take());
        let Some(sender) = sender else { return };

        // Window resolution chain (iOS 13+ aware):
        //   * iOS 15+: scan UIApplication.connectedScenes, pick a
        //     foreground-active UIWindowScene, send `keyWindow`.
        //   * iOS 13/14: `-[UIScene keyWindow]` does not exist and would
        //     crash with "unrecognized selector". Skip the scene path and
        //     fall through to `UIApplication.windows` (deprecated since
        //     iOS 15 but still functional) and pick the `isKeyWindow` one.
        //   * Last resort: `windows[0]`.
        let app_class = class!(UIApplication);
        let app: *mut AnyObject = msg_send![app_class, sharedApplication];
        if app.is_null() {
            let _ = sender.send(Err("UIApplication.sharedApplication is null".to_string()));
            return;
        }

        let key_window_sel = objc2::sel!(keyWindow);
        let mut window: *mut AnyObject = std::ptr::null_mut();

        // Step 1: iterate connectedScenes, pick foreground-active
        // UIWindowScene and send keyWindow only if responds_to_selector.
        let scenes: *mut AnyObject = msg_send![app, connectedScenes];
        if !scenes.is_null() {
            // NSSet → NSArray via allObjects so we can index.
            let scene_array: *mut AnyObject = msg_send![scenes, allObjects];
            if !scene_array.is_null() {
                let count: usize = msg_send![scene_array, count];
                for index in 0..count {
                    let scene: *mut AnyObject = msg_send![scene_array, objectAtIndex: index];
                    if scene.is_null() {
                        continue;
                    }
                    // UISceneActivationState raw values from UISceneDefinitions.h:
                    //   .unattached         = -1
                    //   .foregroundActive   =  0   ← what we want
                    //   .foregroundInactive =  1
                    //   .background         =  2
                    const UI_SCENE_ACTIVATION_FOREGROUND_ACTIVE: i64 = 0;
                    let activation_state: i64 = msg_send![scene, activationState];
                    if activation_state != UI_SCENE_ACTIVATION_FOREGROUND_ACTIVE {
                        continue;
                    }
                    let responds: bool = msg_send![scene, respondsToSelector: key_window_sel];
                    if !responds {
                        continue;
                    }
                    let candidate: *mut AnyObject = msg_send![scene, keyWindow];
                    if !candidate.is_null() {
                        window = candidate;
                        break;
                    }
                }
            }
        }

        // Step 2: fall back to deprecated -[UIApplication windows] and pick
        // the isKeyWindow one (works on iOS 13/14 and as a backstop on 15+).
        if window.is_null() {
            let windows: *mut AnyObject = msg_send![app, windows];
            if !windows.is_null() {
                let count: usize = msg_send![windows, count];
                for index in 0..count {
                    let candidate: *mut AnyObject = msg_send![windows, objectAtIndex: index];
                    if candidate.is_null() {
                        continue;
                    }
                    let is_key: bool = msg_send![candidate, isKeyWindow];
                    if is_key {
                        window = candidate;
                        break;
                    }
                }
                // Step 3: still nothing, take the first window.
                if window.is_null() && count > 0 {
                    window = msg_send![windows, objectAtIndex: 0usize];
                }
            }
        }

        if window.is_null() {
            let _ = sender.send(Err("no key window for current scene".to_string()));
            return;
        }

        // Read window.bounds.size to size the renderer.
        let bounds: CGRect = msg_send![window, bounds];
        if bounds.size.width <= 0.0 || bounds.size.height <= 0.0 {
            let _ = sender.send(Err(format!(
                "window has empty bounds {}x{}",
                bounds.size.width, bounds.size.height
            )));
            return;
        }

        // Render layer through a fresh image context. `scale: 0.0` defers to
        // the device's native scale (Retina-aware).
        UIGraphicsBeginImageContextWithOptions(bounds.size, false, 0.0);
        let ctx = UIGraphicsGetCurrentContext();
        if ctx.is_null() {
            UIGraphicsEndImageContext();
            let _ = sender.send(Err("UIGraphicsGetCurrentContext returned null".to_string()));
            return;
        }
        let layer: *mut AnyObject = msg_send![window, layer];
        // Bypass msg_send! type encoding check: `ctx` here is *mut c_void (since we
        // don't depend on objc2-core-graphics for a typed CGContextRef), but
        // -[CALayer renderInContext:] declares `^{CGContext=}`. msg_send! verifies the
        // encoding and panics on mismatch, so call objc_msgSend directly.
        {
            let sel = objc2::sel!(renderInContext:);
            let func: unsafe extern "C" fn(
                *mut AnyObject,
                objc2::runtime::Sel,
                *mut core::ffi::c_void,
            ) = core::mem::transmute(objc2::ffi::objc_msgSend as *const ());
            func(layer, sel, ctx);
        }
        let image: *mut AnyObject = UIGraphicsGetImageFromCurrentImageContext();
        UIGraphicsEndImageContext();
        if image.is_null() {
            let _ = sender.send(Err(
                "UIGraphicsGetImageFromCurrentImageContext returned null".to_string(),
            ));
            return;
        }

        let png_data = UIImagePNGRepresentation(image);
        if png_data.is_null() {
            let _ = sender.send(Err("UIImagePNGRepresentation returned null".to_string()));
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
        Ok(Ok(Err(err))) => Err(PlatformError::Platform(err)),
        Ok(Err(_)) => Err(PlatformError::Platform(
            "app screenshot request was canceled".to_string(),
        )),
        Err(_) => Err(PlatformError::Platform(
            "app screenshot timed out".to_string(),
        )),
    }
}

/// Enumerate this app's top-level NSWindows. No privacy permission needed
/// since `NSApplication.windows` only sees the calling app's own windows.
#[cfg(target_os = "macos")]
async fn list_app_windows_macos() -> Result<Vec<WindowInfo>, PlatformError> {
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
        Ok(Ok(Err(err))) => Err(PlatformError::Platform(err)),
        Ok(Err(_)) => Err(PlatformError::Platform(
            "list_app_windows request was canceled".to_string(),
        )),
        Err(_) => Err(PlatformError::Platform(
            "list_app_windows timed out".to_string(),
        )),
    }
}

/// iOS apps have a single key window in practice. Return a single entry
/// derived from the key UIWindow so the API surface is consistent.
#[cfg(target_os = "ios")]
async fn list_app_windows_ios() -> Result<Vec<WindowInfo>, PlatformError> {
    // The iOS app-screenshot path captures the key window unconditionally;
    // surface a single-element list here so `lxdev app windows` has a
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

/// macOS implementation. Uses `NSView`'s bitmap-caching display API rather
/// than `CGWindowListCreateImage` — the latter requires Screen Recording
/// privacy permission on macOS 10.15+ since it can also capture other apps'
/// windows. The caching path renders only our own window into a private
/// bitmap, needs no permission, and returns the same visual content the
/// user sees.
///
/// When `window_id` is `Some`, looks up the NSWindow with that
/// `windowNumber`. When `None`, falls back to key → main → first window.
#[cfg(target_os = "macos")]
async fn take_app_screenshot_macos(window_id: Option<&str>) -> Result<Vec<u8>, PlatformError> {
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
                return Err(PlatformError::InvalidParameter(format!(
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

        // `bounds` is the view's local coordinate space — perfect for the
        // cache-display call below.
        let bounds: CGRect = msg_send![content_view, bounds];
        if bounds.size.width <= 0.0 || bounds.size.height <= 0.0 {
            let _ = sender.send(Err(format!(
                "contentView has empty bounds {}x{}",
                bounds.size.width, bounds.size.height
            )));
            return;
        }

        // Ask the view for a bitmap rep that matches its current backing
        // scale (Retina), then have it render into the rep. Together this
        // captures all subviews (including embedded WKWebViews) with
        // proper compositing.
        let bitmap_rep: *mut AnyObject =
            msg_send![content_view, bitmapImageRepForCachingDisplayInRect: bounds];
        if bitmap_rep.is_null() {
            let _ = sender.send(Err(
                "bitmapImageRepForCachingDisplayInRect returned null".to_string()
            ));
            return;
        }
        let _: () = msg_send![
            content_view,
            cacheDisplayInRect: bounds,
            toBitmapImageRep: bitmap_rep
        ];

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
        Ok(Ok(Err(err))) => Err(PlatformError::Platform(err)),
        Ok(Err(_)) => Err(PlatformError::Platform(
            "app screenshot request was canceled".to_string(),
        )),
        Err(_) => Err(PlatformError::Platform(
            "app screenshot timed out".to_string(),
        )),
    }
}
