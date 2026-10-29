use super::app::Platform;
use crate::error::PlatformError;
#[cfg(target_os = "macos")]
use crate::traits::gesture::PullRefreshIndicator;
use crate::traits::gesture::{AppGesture, AppGestureRequest, AppGestureResult};
use async_trait::async_trait;

#[async_trait]
impl AppGesture for Platform {
    #[cfg(target_os = "macos")]
    fn gesture_inputs(&self) -> Vec<crate::traits::gesture::GestureInput> {
        use crate::traits::gesture::GestureInput;
        vec![GestureInput::Mouse, GestureInput::Trackpad]
    }

    async fn perform_app_gesture(
        &self,
        request: AppGestureRequest,
    ) -> Result<AppGestureResult, PlatformError> {
        #[cfg(target_os = "macos")]
        {
            mac::perform(request).await
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = request;
            Err(PlatformError::NotSupported(
                "iOS has no public touch injection; gestures need a desktop or Android host"
                    .to_string(),
            ))
        }
    }

    #[cfg(target_os = "macos")]
    async fn gesture_viewport(
        &self,
        target: crate::traits::gesture::GestureTarget,
    ) -> Result<[f64; 4], PlatformError> {
        mac::viewport(target).await
    }

    #[cfg(target_os = "macos")]
    async fn orient_window(&self, landscape: bool) -> Result<[f64; 2], PlatformError> {
        mac::orient_window(landscape).await
    }

    #[cfg(target_os = "macos")]
    async fn chrome_elements(
        &self,
        scope: Option<String>,
    ) -> Result<Vec<crate::traits::gesture::ChromeElement>, PlatformError> {
        mac::chrome_elements(scope).await
    }

    #[cfg(target_os = "macos")]
    async fn pull_refresh_indicator(
        &self,
        app_id: &str,
        webtag: &str,
    ) -> Result<Option<PullRefreshIndicator>, PlatformError> {
        mac::pull_refresh_indicator(app_id, webtag).await
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use super::super::mouse::{content_point_to_window_point, parse_window_id, resolve_window};
    use crate::error::PlatformError;
    use crate::traits::gesture::{
        AppGestureRequest, AppGestureResult, GestureDelivery, GestureEnd, GestureInput,
        GesturePoint, GestureShape, GestureTarget, PullRefreshIndicator,
    };
    use dispatch2::DispatchQueue;
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, Instant};
    use tokio::sync::oneshot;

    /// A pointer left down by a `Hold` end, continued by a later request.
    #[derive(Clone, Copy)]
    struct Held {
        input: GestureInput,
        window: i64,
        origin: (f64, f64),
        last: (f64, f64),
    }

    type Point = (f64, f64);

    static HELD: Mutex<Option<HashMap<u64, Held>>> = Mutex::new(None);
    static NEXT: AtomicU64 = AtomicU64::new(1);

    fn held() -> std::sync::MutexGuard<'static, Option<HashMap<u64, Held>>> {
        HELD.lock().unwrap_or_else(|e| e.into_inner())
    }

    async fn on_main<T: Send + 'static>(
        work: impl FnOnce() -> Result<T, String> + Send + 'static,
    ) -> Result<T, PlatformError> {
        let (tx, rx) = oneshot::channel();
        DispatchQueue::main().exec_async(move || {
            let _ = tx.send(work());
        });
        match tokio::time::timeout(Duration::from_secs(2), rx).await {
            Ok(Ok(result)) => result.map_err(PlatformError::Platform),
            Ok(Err(_)) => Err(PlatformError::Platform("gesture event was dropped".into())),
            Err(_) => Err(PlatformError::Platform("main thread did not answer".into())),
        }
    }

    fn content_view_of(window: i64) -> Result<(*mut AnyObject, *mut AnyObject), String> {
        unsafe {
            let app: *mut AnyObject = msg_send![objc2::class!(NSApplication), sharedApplication];
            let window = resolve_window(app, Some(window))
                .ok_or_else(|| format!("no NSWindow with windowNumber={window}"))?;
            let content: *mut AnyObject = msg_send![window, contentView];
            if content.is_null() {
                return Err("NSWindow has no contentView".into());
            }
            Ok((window, content))
        }
    }

    pub(super) async fn viewport(target: GestureTarget) -> Result<[f64; 4], PlatformError> {
        on_main(move || {
            let (_, _, size) = resolve(&target)?;
            Ok([0.0, 0.0, size.0, size.1])
        })
        .await
    }

    fn resolve_target(target: &GestureTarget) -> Result<(i64, (f64, f64)), String> {
        resolve(target).map(|(window, origin, _)| (window, origin))
    }

    /// Window number, the content-view (top-left) origin and the size of the target.
    fn resolve(target: &GestureTarget) -> Result<(i64, Point, Point), String> {
        use objc2_foundation::NSRect;
        unsafe {
            let app: *mut AnyObject = msg_send![objc2::class!(NSApplication), sharedApplication];
            let (window, view) = match target {
                GestureTarget::Window { window_id } => {
                    let number =
                        parse_window_id(window_id.as_deref()).map_err(|e| e.to_string())?;
                    let window = resolve_window(app, number)
                        .ok_or_else(|| "no NSWindow available for gesture input".to_string())?;
                    (window, None)
                }
                GestureTarget::Page { view, .. } => {
                    let view = match view {
                        Some(ptr) if *ptr != 0 => *ptr as *mut AnyObject,
                        _ => find_gpu_page_view(app)?,
                    };
                    let window: *mut AnyObject = msg_send![view, window];
                    if window.is_null() {
                        return Err("the page is not in a window".into());
                    }
                    let hidden: bool = msg_send![view, isHiddenOrHasHiddenAncestor];
                    if hidden {
                        return Err("the page is hidden".into());
                    }
                    (window, Some(view))
                }
            };
            let number: isize = msg_send![window, windowNumber];
            let content: *mut AnyObject = msg_send![window, contentView];
            if content.is_null() {
                return Err("NSWindow has no contentView".into());
            }
            let content_size = {
                let bounds: NSRect = msg_send![content, bounds];
                (bounds.size.width, bounds.size.height)
            };
            let (origin, size) = match view {
                None => ((0.0, 0.0), content_size),
                Some(view) => {
                    let bounds: NSRect = msg_send![view, bounds];
                    let rect: NSRect = msg_send![view, convertRect: bounds, toView: content];
                    let flipped: bool = msg_send![content, isFlipped];
                    let content_bounds: NSRect = msg_send![content, bounds];
                    let top = if flipped {
                        rect.origin.y
                    } else {
                        content_bounds.size.height - (rect.origin.y + rect.size.height)
                    };
                    ((rect.origin.x, top), (rect.size.width, rect.size.height))
                }
            };
            Ok((number as i64, origin, size))
        }
    }

    /// The front-most visible GPU page view of the key window.
    unsafe fn find_gpu_page_view(app: *mut AnyObject) -> Result<*mut AnyObject, String> {
        unsafe fn walk(view: *mut AnyObject, found: &mut Option<*mut AnyObject>) {
            unsafe {
                let class: *const AnyObject = msg_send![view, className];
                let name = objc2_foundation::NSString::from_raw(class);
                let hidden: bool = msg_send![view, isHiddenOrHasHiddenAncestor];
                if !hidden && name.ends_with("GpuPageView") {
                    *found = Some(view);
                }
                let subviews: *mut AnyObject = msg_send![view, subviews];
                let count: usize = msg_send![subviews, count];
                for index in 0..count {
                    let child: *mut AnyObject = msg_send![subviews, objectAtIndex: index];
                    walk(child, found);
                }
            }
        }
        unsafe {
            let window = resolve_window(app, None).ok_or("no NSWindow available")?;
            let content: *mut AnyObject = msg_send![window, contentView];
            let mut found = None;
            if !content.is_null() {
                walk(content, &mut found);
            }
            found.ok_or_else(|| "no visible GPU page view in the key window".to_string())
        }
    }

    trait FromRaw {
        unsafe fn from_raw(raw: *const AnyObject) -> String;
    }
    impl FromRaw for objc2_foundation::NSString {
        unsafe fn from_raw(raw: *const AnyObject) -> String {
            if raw.is_null() {
                return String::new();
            }
            unsafe { (*(raw as *const objc2_foundation::NSString)).to_string() }
        }
    }

    fn uptime() -> f64 {
        objc2_foundation::NSProcessInfo::processInfo().systemUptime()
    }

    #[derive(Clone, Copy)]
    enum Mouse {
        Down,
        Dragged,
        Up,
    }

    fn post_mouse(window: i64, x: f64, y: f64, phase: Mouse) -> Result<(), String> {
        use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSEventType};
        let (window, content) = content_view_of(window)?;
        let location = content_point_to_window_point(content, x, y)?;
        let kind = match phase {
            Mouse::Down => NSEventType::LeftMouseDown,
            Mouse::Dragged => NSEventType::LeftMouseDragged,
            Mouse::Up => NSEventType::LeftMouseUp,
        };
        unsafe {
            let number: isize = msg_send![window, windowNumber];
            let event = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
                kind,
                location,
                NSEventModifierFlags::empty(),
                uptime(),
                number,
                None,
                0,
                if matches!(phase, Mouse::Dragged) { 0 } else { 1 },
                if matches!(phase, Mouse::Up) { 0.0 } else { 1.0 },
            )
            .ok_or("failed to create mouse event")?;
            // Queued, not sent: a control's tracking loop started by the down
            // pulls the later events from the queue.
            let app: *mut AnyObject = msg_send![objc2::class!(NSApplication), sharedApplication];
            let ptr = event.as_ref() as *const NSEvent as *mut AnyObject;
            let _: () = msg_send![app, postEvent: ptr, atStart: false];
        }
        Ok(())
    }

    #[derive(Clone, Copy)]
    enum Scroll {
        Phase(u32),
        Momentum(u32),
    }

    /// A continuous (trackpad) scroll event; `dx`/`dy` are finger travel.
    fn post_scroll(
        window: i64,
        x: f64,
        y: f64,
        dx: f64,
        dy: f64,
        phase: Scroll,
    ) -> Result<(), String> {
        use objc2_app_kit::NSEvent;
        use objc2_core_foundation::CGPoint;
        use objc2_core_graphics::{
            CGDisplayBounds, CGEvent, CGEventField, CGMainDisplayID, CGScrollEventUnit,
        };
        let (window, content) = content_view_of(window)?;
        let point = content_point_to_window_point(content, x, y)?;
        unsafe {
            let height = CGDisplayBounds(CGMainDisplayID()).size.height;
            let event = CGEvent::new_scroll_wheel_event2(
                None,
                CGScrollEventUnit::Pixel,
                2,
                dy.round() as i32,
                dx.round() as i32,
                0,
            )
            .ok_or("failed to create scroll event")?;
            let field = |n: u32| CGEventField(n);
            // Continuous, with point deltas, as a trackpad reports them.
            CGEvent::set_integer_value_field(Some(&event), field(88), 1);
            CGEvent::set_double_value_field(Some(&event), field(96), dy);
            CGEvent::set_double_value_field(Some(&event), field(97), dx);
            let (scroll, momentum) = match phase {
                Scroll::Phase(p) => (p, 0),
                Scroll::Momentum(p) => (0, p),
            };
            CGEvent::set_integer_value_field(Some(&event), field(99), i64::from(scroll));
            CGEvent::set_integer_value_field(Some(&event), field(123), i64::from(momentum));
            CGEvent::set_location(
                Some(&event),
                CGPoint {
                    x: point.x,
                    y: height - point.y,
                },
            );
            let ns = NSEvent::eventWithCGEvent(&event).ok_or("failed to convert scroll event")?;
            if let Scroll::Momentum(_) = phase {
                // NSWindow.sendEvent drops synthesized momentum-phase scrolls, so they
                // go to the view under the point, as the window routed the gesture.
                let frame: *mut AnyObject = msg_send![content, superview];
                let host = if frame.is_null() { content } else { frame };
                let hit: *mut AnyObject = msg_send![host, hitTest: point];
                let view = if hit.is_null() { content } else { hit };
                let ptr = ns.as_ref() as *const NSEvent as *mut AnyObject;
                let _: () = msg_send![view, scrollWheel: ptr];
            } else {
                super::super::mouse::send_event_to_window(window, ns.as_ref());
            }
        }
        Ok(())
    }

    const BEGAN: u32 = 1;
    const CHANGED: u32 = 2;
    const ENDED: u32 = 4;
    const CANCELLED: u32 = 8;

    pub(super) async fn perform(
        request: AppGestureRequest,
    ) -> Result<AppGestureResult, PlatformError> {
        let points = request
            .shape
            .points()
            .map_err(PlatformError::InvalidParameter)?;
        let fling = matches!(request.shape, GestureShape::Fling { .. });
        let continued = match request.finger {
            Some(id) => Some(
                held()
                    .as_mut()
                    .and_then(|fingers| fingers.remove(&id))
                    .ok_or_else(|| {
                        PlatformError::InvalidParameter(format!("finger {id} is not held"))
                    })?,
            ),
            None => None,
        };
        let input = continued
            .map(|held| held.input)
            .or(request.input)
            .unwrap_or(GestureInput::Trackpad);
        if input == GestureInput::Touch {
            return Err(PlatformError::NotSupported(
                "macOS has no touch screen; use mouse or trackpad".into(),
            ));
        }
        if input == GestureInput::Mouse && request.end == GestureEnd::Cancel {
            return Err(PlatformError::InvalidParameter(
                "a mouse gesture cannot be cancelled; end it with up".into(),
            ));
        }
        let (window, origin) = match continued {
            Some(held) => (held.window, held.origin),
            None => {
                let target = request.target.clone();
                on_main(move || resolve_target(&target)).await?
            }
        };
        let started = Instant::now();
        let mut events = 0u32;
        let mut last = continued.map(|held| held.last);
        for GesturePoint { x, y, at_ms } in points {
            let due = started + Duration::from_secs_f64(at_ms / 1000.0);
            tokio::time::sleep_until(due.into()).await;
            let (x, y) = (origin.0 + x, origin.1 + y);
            let previous = last.replace((x, y));
            let sent = on_main(move || match (input, previous) {
                (GestureInput::Mouse, None) => post_mouse(window, x, y, Mouse::Down).map(|_| true),
                (GestureInput::Mouse, Some(_)) => {
                    post_mouse(window, x, y, Mouse::Dragged).map(|_| true)
                }
                // A trackpad sends nothing for fingers that rest or have not moved.
                (_, None) => Ok(false),
                (_, Some((px, py))) if (px, py) == (x, y) => Ok(false),
                (_, Some((px, py))) => {
                    let phase = if continued.is_none() && events == 0 {
                        BEGAN
                    } else {
                        CHANGED
                    };
                    post_scroll(window, x, y, x - px, y - py, Scroll::Phase(phase)).map(|_| true)
                }
            })
            .await?;
            if sent {
                events += 1;
            }
        }
        let (x, y) = last.unwrap_or(origin);
        let finger = match request.end {
            GestureEnd::Hold => {
                let id = NEXT.fetch_add(1, Ordering::Relaxed);
                held().get_or_insert_with(HashMap::new).insert(
                    id,
                    Held {
                        input,
                        window,
                        origin,
                        last: (x, y),
                    },
                );
                Some(id)
            }
            end => {
                let velocity = fling_velocity(&request.shape);
                on_main(move || match input {
                    GestureInput::Mouse => post_mouse(window, x, y, Mouse::Up),
                    _ => {
                        let phase = if end == GestureEnd::Cancel {
                            CANCELLED
                        } else {
                            ENDED
                        };
                        post_scroll(window, x, y, 0.0, 0.0, Scroll::Phase(phase))
                    }
                })
                .await?;
                events += 1;
                if fling && input == GestureInput::Trackpad && end == GestureEnd::Up {
                    events += momentum(window, (x, y), velocity).await?;
                }
                None
            }
        };
        Ok(AppGestureResult {
            input,
            delivered: GestureDelivery::Window,
            events,
            duration_ms: started.elapsed().as_secs_f64() * 1000.0,
            finger,
        })
    }

    fn fling_velocity(shape: &GestureShape) -> (f64, f64) {
        let Ok(points) = shape.points() else {
            return (0.0, 0.0);
        };
        let [.., a, b] = points.as_slice() else {
            return (0.0, 0.0);
        };
        let dt = (b.at_ms - a.at_ms).max(1.0) / 1000.0;
        ((b.x - a.x) / dt, (b.y - a.y) / dt)
    }

    /// The momentum a trackpad reports after a flick: per-frame deltas
    /// decaying from the release velocity.
    async fn momentum(
        window: i64,
        at: (f64, f64),
        velocity: (f64, f64),
    ) -> Result<u32, PlatformError> {
        const FRAME: f64 = 1.0 / 60.0;
        let (mut vx, mut vy) = velocity;
        let mut events = 0;
        let mut phase = 1;
        while (vx * FRAME).hypot(vy * FRAME) >= 0.5 && events < 240 {
            let (dx, dy) = (vx * FRAME, vy * FRAME);
            let current = phase;
            on_main(move || post_scroll(window, at.0, at.1, dx, dy, Scroll::Momentum(current)))
                .await?;
            phase = 2;
            events += 1;
            vx *= 0.95;
            vy *= 0.95;
            tokio::time::sleep(Duration::from_secs_f64(FRAME)).await;
        }
        on_main(move || post_scroll(window, at.0, at.1, 0.0, 0.0, Scroll::Momentum(3))).await?;
        Ok(events + 1)
    }

    pub(super) async fn orient_window(landscape: bool) -> Result<[f64; 2], PlatformError> {
        use objc2_foundation::{NSRect, NSSize};
        on_main(move || unsafe {
            let app: *mut AnyObject = msg_send![objc2::class!(NSApplication), sharedApplication];
            let window = resolve_window(app, None).ok_or("no NSWindow available")?;
            let content: *mut AnyObject = msg_send![window, contentView];
            let bounds: NSRect = msg_send![content, bounds];
            let (w, h) = (bounds.size.width, bounds.size.height);
            if (w > h) != landscape {
                let size = NSSize::new(h, w);
                let _: () = msg_send![window, setContentSize: size];
            }
            let bounds: NSRect = msg_send![content, bounds];
            Ok([bounds.size.width, bounds.size.height])
        })
        .await
    }

    /// Pressable shell chrome of the key window: controls and sidebar rows,
    /// labelled by their accessibility label, else their title or text.
    /// Page views (WebKit, GPU) are not searched.
    pub(super) async fn chrome_elements(
        scope: Option<String>,
    ) -> Result<Vec<crate::traits::gesture::ChromeElement>, PlatformError> {
        use crate::traits::gesture::ChromeElement;
        use objc2_foundation::NSRect;

        unsafe fn class_name(view: *mut AnyObject) -> String {
            unsafe {
                let class: *const AnyObject = msg_send![view, className];
                <objc2_foundation::NSString as FromRaw>::from_raw(class)
            }
        }
        unsafe fn string(value: *const AnyObject) -> String {
            unsafe { <objc2_foundation::NSString as FromRaw>::from_raw(value) }
        }
        unsafe fn texts(view: *mut AnyObject) -> String {
            unsafe {
                let hidden: bool = msg_send![view, isHiddenOrHasHiddenAncestor];
                if hidden {
                    return String::new();
                }
                if class_name(view) == "NSTextField" {
                    let value: *const AnyObject = msg_send![view, stringValue];
                    return string(value);
                }
                let subviews: *mut AnyObject = msg_send![view, subviews];
                let count: usize = msg_send![subviews, count];
                (0..count)
                    .map(|i| texts(msg_send![subviews, objectAtIndex: i]))
                    .filter(|t| !t.trim().is_empty())
                    .collect::<Vec<_>>()
                    .join(" ")
            }
        }
        #[allow(clippy::too_many_arguments)]
        unsafe fn visit(
            view: *mut AnyObject,
            content: *mut AnyObject,
            content_height: f64,
            flipped: bool,
            area: &str,
            scope: &Option<String>,
            out: &mut Vec<ChromeElement>,
        ) {
            unsafe {
                let hidden: bool = msg_send![view, isHiddenOrHasHiddenAncestor];
                let name = class_name(view);
                if hidden || name.contains("WKWebView") || name.ends_with("GpuPageView") {
                    return;
                }
                let here = if name.contains("Sidebar") {
                    "sidebar"
                } else if name.contains("Toolbar")
                    || name.contains("NavigationBar")
                    || name.contains("TitleBar")
                {
                    "navigationBar"
                } else {
                    area
                };
                let control: bool = msg_send![view, isKindOfClass: objc2::class!(NSControl)];
                let row = name.ends_with("ItemView");
                if (control || row) && scope.as_deref().is_none_or(|s| s == here) {
                    let ax: *const AnyObject = msg_send![view, accessibilityLabel];
                    let mut label = string(ax);
                    if label.trim().is_empty() {
                        let button: bool = msg_send![view, isKindOfClass: objc2::class!(NSButton)];
                        if button {
                            let title: *const AnyObject = msg_send![view, title];
                            label = string(title);
                        }
                    }
                    if label.trim().is_empty() {
                        label = texts(view);
                    }
                    if !label.trim().is_empty() && name != "NSTextField" {
                        let bounds: NSRect = msg_send![view, bounds];
                        let rect: NSRect = msg_send![view, convertRect: bounds, toView: content];
                        let top = if flipped {
                            rect.origin.y
                        } else {
                            content_height - (rect.origin.y + rect.size.height)
                        };
                        let selected: bool = msg_send![view, isAccessibilitySelected];
                        out.push(ChromeElement {
                            label: label.trim().to_owned(),
                            role: if row { "tab".into() } else { "button".into() },
                            scope: here.into(),
                            selected,
                            bounds: [rect.origin.x, top, rect.size.width, rect.size.height],
                        });
                        return;
                    }
                }
                let subviews: *mut AnyObject = msg_send![view, subviews];
                let count: usize = msg_send![subviews, count];
                for i in 0..count {
                    let child: *mut AnyObject = msg_send![subviews, objectAtIndex: i];
                    visit(child, content, content_height, flipped, here, scope, out);
                }
            }
        }

        /// A collapsed sidebar rail shows its tabs as a pointer at the edge would.
        unsafe fn reveal_tabs(view: *mut AnyObject) -> bool {
            unsafe {
                let reveal = objc2::sel!(lxRevealTabsForAutomation);
                let responds: bool = msg_send![view, respondsToSelector: reveal];
                if responds {
                    let revealed: bool = msg_send![view, lxRevealTabsForAutomation];
                    return revealed;
                }
                let subviews: *mut AnyObject = msg_send![view, subviews];
                let count: usize = msg_send![subviews, count];
                (0..count).any(|i| reveal_tabs(msg_send![subviews, objectAtIndex: i]))
            }
        }

        if scope
            .as_deref()
            .is_none_or(|s| s == "sidebar" || s == "tabBar")
        {
            let revealed = on_main(|| unsafe {
                let app: *mut AnyObject =
                    msg_send![objc2::class!(NSApplication), sharedApplication];
                let window = resolve_window(app, None).ok_or("no NSWindow available")?;
                let content: *mut AnyObject = msg_send![window, contentView];
                Ok::<_, String>(!content.is_null() && reveal_tabs(content))
            })
            .await?;
            if revealed {
                // The rail slides open before its rows can be pressed.
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
        }

        on_main(move || unsafe {
            let app: *mut AnyObject = msg_send![objc2::class!(NSApplication), sharedApplication];
            let window = resolve_window(app, None).ok_or("no NSWindow available")?;
            let content: *mut AnyObject = msg_send![window, contentView];
            if content.is_null() {
                return Err("NSWindow has no contentView".to_string());
            }
            let bounds: NSRect = msg_send![content, bounds];
            let flipped: bool = msg_send![content, isFlipped];
            let mut out = Vec::new();
            visit(
                content,
                content,
                bounds.size.height,
                flipped,
                "window",
                &scope,
                &mut out,
            );
            Ok(out)
        })
        .await
    }

    pub(super) async fn pull_refresh_indicator(
        app_id: &str,
        webtag: &str,
    ) -> Result<Option<PullRefreshIndicator>, PlatformError> {
        use std::ffi::{CString, c_char, c_void};
        type StateFn = unsafe extern "C" fn(
            *const c_char,
            *const c_char,
            *mut c_char,
            isize,
            *mut f64,
        ) -> bool;
        unsafe extern "C" {
            fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
        }
        const RTLD_DEFAULT: *mut c_void = -2isize as *mut c_void;
        let app =
            CString::new(app_id).map_err(|e| PlatformError::InvalidParameter(e.to_string()))?;
        let tag =
            CString::new(webtag).map_err(|e| PlatformError::InvalidParameter(e.to_string()))?;
        on_main(move || {
            let symbol = unsafe { dlsym(RTLD_DEFAULT, c"lx_mac_pull_refresh_state".as_ptr()) };
            if symbol.is_null() {
                return Err("this host has no macOS pull-to-refresh".to_string());
            }
            let read: StateFn = unsafe { std::mem::transmute(symbol) };
            let mut phase = [0 as c_char; 16];
            let mut offset = 0.0;
            let found = unsafe {
                read(
                    app.as_ptr(),
                    tag.as_ptr(),
                    phase.as_mut_ptr(),
                    phase.len() as isize,
                    &mut offset,
                )
            };
            if !found {
                return Ok(None);
            }
            let phase = unsafe { std::ffi::CStr::from_ptr(phase.as_ptr()) }
                .to_string_lossy()
                .into_owned();
            Ok(Some(PullRefreshIndicator { phase, offset }))
        })
        .await
    }
}
