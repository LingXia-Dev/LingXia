//! Gestures on GPU pages: touch goes to the page's window through the message its touch
//! path reads, mouse through posted button messages. A WebView page takes no touch here.
use super::Platform;
use super::mouse::deepest_child_at;
use super::screenshot::resolve_screenshot_window;
use crate::error::PlatformError;
use crate::traits::gesture::{
    AppGesture, AppGestureRequest, AppGestureResult, GestureDelivery, GestureEnd, GestureInput,
    GesturePoint, GestureTarget,
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{ClientToScreen, ScreenToClient};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, GetClassNameW, GetClientRect, GetWindowRect, IsWindowVisible, PostMessageW,
    RegisterWindowMessageW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER, SetWindowPos, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MOUSEMOVE,
};
use windows::core::BOOL;
use windows::core::w;

const MK_LBUTTON: usize = 0x0001;

#[derive(Clone, Copy)]
struct Held {
    input: GestureInput,
    /// The window the gesture went down in, and how its client maps from target points.
    window: isize,
    origin: (f64, f64),
    scale: f64,
    touch: bool,
    last: (f64, f64),
}

static NEXT: AtomicU64 = AtomicU64::new(1);

fn held() -> std::sync::MutexGuard<'static, HashMap<u64, Held>> {
    static HELD: OnceLock<Mutex<HashMap<u64, Held>>> = OnceLock::new();
    HELD.get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|p| p.into_inner())
}

/// The message the GPU page window's touch path reads (registered there under this name).
fn touch_message() -> u32 {
    // SAFETY: a constant name.
    unsafe { RegisterWindowMessageW(w!("LingXiaViewTouch")) }
}

#[async_trait]
impl AppGesture for Platform {
    fn gesture_inputs(&self) -> Vec<GestureInput> {
        vec![GestureInput::Touch, GestureInput::Mouse]
    }

    fn webview_page_gestures(&self) -> bool {
        false
    }

    async fn perform_app_gesture(
        &self,
        request: AppGestureRequest,
    ) -> Result<AppGestureResult, PlatformError> {
        perform(request).await
    }

    async fn orient_window(&self, landscape: bool) -> Result<[f64; 2], PlatformError> {
        let window = resolve_screenshot_window(None)?.0 as isize;
        let (width, height) = client_size(window);
        // The page decides the orientation, not the window: the sidebar and title bar make a
        // squarish window's page portrait.
        let (pw, ph) = largest_page(window).unwrap_or((width, height));
        if (pw > ph) != landscape {
            let mut frame = RECT::default();
            // SAFETY: reads and resizes a live top-level window; the resize is sent to its thread.
            unsafe {
                GetWindowRect(HWND(window as *mut _), &mut frame)
                    .map_err(|e| PlatformError::Platform(format!("window rect: {e}")))?;
                let outer = (
                    f64::from(frame.right - frame.left),
                    f64::from(frame.bottom - frame.top),
                );
                let size = (outer.0 - pw + ph, outer.1 - ph + pw);
                SetWindowPos(
                    HWND(window as *mut _),
                    None,
                    0,
                    0,
                    size.0.round() as i32,
                    size.1.round() as i32,
                    SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                )
                .map_err(|e| PlatformError::Platform(format!("resize window: {e}")))?;
            }
        }
        let (width, height) = client_size(window);
        // SAFETY: reads a window's DPI.
        let dpi = unsafe { GetDpiForWindow(HWND(window as *mut _)) };
        let scale = if dpi == 0 { 1.0 } else { f64::from(dpi) / 96.0 };
        Ok([width / scale, height / scale])
    }

    async fn gesture_viewport(&self, target: GestureTarget) -> Result<[f64; 4], PlatformError> {
        let (window, scale) = resolve(&target)?;
        let (width, height) = client_size(window);
        Ok([0.0, 0.0, width / scale, height / scale])
    }
}

/// The window a target's points are in, and its device pixels per point.
fn resolve(target: &GestureTarget) -> Result<(isize, f64), PlatformError> {
    match target {
        GestureTarget::Window { window_id } => {
            let window = resolve_screenshot_window(window_id.as_deref())?;
            Ok((window.0 as isize, 1.0))
        }
        GestureTarget::Page { webtag, .. } => {
            let tag = lingxia_webview::WebTag::from(webtag.as_str());
            let handler = lingxia_webview::platform::windows::find_webview_handler(&tag)
                .ok_or_else(|| PlatformError::Platform(format!("no page window for {webtag}")))?;
            // A GPU page's window moves between host parents; its points are its own.
            let window = handler
                .content_window()
                .unwrap_or_else(|| handler.native_view().window);
            // SAFETY: reads a window's DPI.
            let dpi = unsafe { GetDpiForWindow(HWND(window as *mut _)) };
            let scale = if dpi == 0 { 1.0 } else { f64::from(dpi) / 96.0 };
            Ok((window, scale))
        }
    }
}

fn client_size(window: isize) -> (f64, f64) {
    let mut rect = RECT::default();
    // SAFETY: an out-parameter on the stack.
    let _ = unsafe { GetClientRect(HWND(window as *mut _), &mut rect) };
    (
        f64::from(rect.right - rect.left),
        f64::from(rect.bottom - rect.top),
    )
}

/// The largest visible page window (GPU page or WebView) under `window`, client pixels.
fn largest_page(window: isize) -> Option<(f64, f64)> {
    unsafe extern "system" fn visit(child: HWND, found: LPARAM) -> BOOL {
        // SAFETY: `found` is the `Option` below, alive for the enumeration.
        let found = unsafe { &mut *(found.0 as *mut Option<(f64, f64)>) };
        let mut name = [0u16; 64];
        // SAFETY: a buffer of its stated length; reads of a live child.
        let (len, visible) = unsafe {
            (
                GetClassNameW(child, &mut name),
                IsWindowVisible(child).as_bool(),
            )
        };
        let class = String::from_utf16_lossy(&name[..len.max(0) as usize]);
        if visible && (class == "LingXiaGpuPage" || class.starts_with("Chrome_WidgetWin")) {
            let (w, h) = client_size(child.0 as isize);
            if found.is_none_or(|(fw, fh)| w * h > fw * fh) {
                *found = Some((w, h));
            }
        }
        true.into()
    }
    let mut found: Option<(f64, f64)> = None;
    // SAFETY: enumerates a live window's children with a callback that only reads them.
    unsafe {
        let _ = EnumChildWindows(
            Some(HWND(window as *mut _)),
            Some(visit),
            LPARAM(&mut found as *mut _ as isize),
        );
    }
    found
}

fn is_gpu_page(window: HWND) -> bool {
    let mut name = [0u16; 32];
    // SAFETY: a buffer of its stated length.
    let len = unsafe { GetClassNameW(window, &mut name) };
    String::from_utf16_lossy(&name[..len.max(0) as usize]) == "LingXiaGpuPage"
}

fn lparam(x: f64, y: f64) -> LPARAM {
    let (x, y) = (x.round() as i32, y.round() as i32);
    LPARAM((((y as u16) as isize) << 16) | ((x as u16) as isize))
}

fn post(window: isize, message: u32, wparam: usize, at: (f64, f64)) -> Result<(), PlatformError> {
    // SAFETY: posts to a window handle; a stale one fails.
    unsafe {
        PostMessageW(
            Some(HWND(window as *mut _)),
            message,
            WPARAM(wparam),
            lparam(at.0, at.1),
        )
    }
    .map_err(|err| PlatformError::Platform(format!("PostMessageW failed: {err}")))
}

/// Point `at` (target points) in `held`'s window client pixels.
fn client_point(held: &Held, at: (f64, f64)) -> (f64, f64) {
    (
        held.origin.0 + at.0 * held.scale,
        held.origin.1 + at.1 * held.scale,
    )
}

/// The window under the gesture's first point, which takes the whole gesture.
fn begin(
    target: &GestureTarget,
    input: GestureInput,
    at: (f64, f64),
) -> Result<Held, PlatformError> {
    let (window, scale) = resolve(target)?;
    let (child, _) = deepest_child_at(window, (at.0 * scale, at.1 * scale));
    // The target's origin in the child's client space.
    let mut origin = POINT { x: 0, y: 0 };
    // SAFETY: maps between two live windows.
    unsafe {
        let _ = ClientToScreen(HWND(window as *mut _), &mut origin);
        let _ = ScreenToClient(child, &mut origin);
    }
    Ok(Held {
        input,
        window: child.0 as isize,
        origin: (f64::from(origin.x), f64::from(origin.y)),
        scale,
        // Touch reaches a non-GPU window as the mouse messages Windows promotes it to.
        touch: input == GestureInput::Touch && is_gpu_page(child),
        last: at,
    })
}

fn send(held: &Held, phase: usize, at: (f64, f64)) -> Result<(), PlatformError> {
    let client = client_point(held, at);
    if held.touch {
        return post(held.window, touch_message(), phase, client);
    }
    let marker = lingxia_webview::platform::windows::SYNTHETIC_MOUSE_WPARAM_MARKER;
    match phase {
        0 => post(held.window, WM_LBUTTONDOWN, MK_LBUTTON | marker, client),
        1 => post(held.window, WM_MOUSEMOVE, MK_LBUTTON | marker, client),
        _ => post(held.window, WM_LBUTTONUP, marker, client),
    }
}

async fn perform(request: AppGestureRequest) -> Result<AppGestureResult, PlatformError> {
    let points = request
        .shape
        .points()
        .map_err(PlatformError::InvalidParameter)?;
    let continued =
        match request.finger {
            Some(id) => Some(held().remove(&id).ok_or_else(|| {
                PlatformError::InvalidParameter(format!("finger {id} is not held"))
            })?),
            None => None,
        };
    let input = continued
        .map(|held| held.input)
        .or(request.input)
        .unwrap_or(GestureInput::Touch);
    if input == GestureInput::Trackpad {
        return Err(PlatformError::NotSupported(
            "Windows gestures imitate touch or a mouse".into(),
        ));
    }
    if input == GestureInput::Mouse && request.end == GestureEnd::Cancel {
        return Err(PlatformError::InvalidParameter(
            "a mouse gesture cannot be cancelled; end it with up".into(),
        ));
    }
    let mut state = match continued {
        Some(held) => held,
        None => {
            let first = points
                .first()
                .map(|p| (p.x, p.y))
                .ok_or_else(|| PlatformError::InvalidParameter("empty gesture".into()))?;
            let state = begin(&request.target, input, first)?;
            send(&state, 0, first)?;
            state
        }
    };
    let started = Instant::now();
    let mut events = u32::from(continued.is_none());
    for GesturePoint { x, y, at_ms } in points.into_iter().skip(usize::from(continued.is_none())) {
        let due = started + Duration::from_secs_f64(at_ms / 1000.0);
        tokio::time::sleep_until(due.into()).await;
        send(&state, 1, (x, y))?;
        state.last = (x, y);
        events += 1;
    }
    let finger = match request.end {
        GestureEnd::Hold => {
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            held().insert(id, state);
            Some(id)
        }
        end => {
            send(
                &state,
                if end == GestureEnd::Cancel { 3 } else { 2 },
                state.last,
            )?;
            events += 1;
            None
        }
    };
    Ok(AppGestureResult {
        input,
        delivered: if state.touch {
            GestureDelivery::View
        } else {
            GestureDelivery::Window
        },
        events,
        duration_ms: started.elapsed().as_secs_f64() * 1000.0,
        finger,
    })
}
