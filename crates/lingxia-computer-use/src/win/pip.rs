//! A passive activity viewer on a dedicated Win32 UI thread.
//!
//! Frames are scaled straight from the visible desktop into the small viewer
//! with GDI. This deliberately does not reuse screenshot/WGC: those paths
//! allocate a capture stack and PNG-encode every call, while the viewer needs a
//! cheap live view. The tradeoff is honest and useful here: an occluded target
//! is shown occluded, exactly as it appears to the person at the machine.

use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, mpsc};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{
    COLORREF, ERROR_CLASS_ALREADY_EXISTS, GetLastError, HWND, LPARAM, LRESULT, RECT, WPARAM,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreatePen, DeleteObject, Ellipse, EndPaint, GetDC, GetStockObject, HALFTONE,
    HGDIOBJ, InvalidateRect, NULL_BRUSH, PAINTSTRUCT, PS_SOLID, ReleaseDC, SRCCOPY, SelectObject,
    SetStretchBltMode, StretchBlt,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

use crate::model::{Acted, Rect};
use crate::pip_state::{ActivityState, ActivityTarget, Transition};

const FPS: u32 = 8;
const IDLE_REST: Duration = Duration::from_secs(12);
const MARKER_LINGER: Duration = Duration::from_millis(1200);
const TIMER_ID: usize = 1;
const WM_PIP_UPDATE: u32 = WM_APP + 1;
const WM_PIP_HIDE: u32 = WM_APP + 2;
pub(crate) const VIEWER_CLASS: &str = "LingXiaActivityViewer";

pub(crate) fn is_viewer_class(class: &str) -> bool {
    class == VIEWER_CLASS
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

struct State {
    activity: ActivityState,
    corner: Corner,
    panel: Option<Rect>,
}

static STATE: Mutex<State> = Mutex::new(State {
    activity: ActivityState::new(),
    corner: Corner::BottomRight,
    panel: None,
});

#[derive(Clone, Copy)]
struct UiThread {
    id: u32,
}

#[derive(Clone, Copy)]
enum UiStart {
    Ready(UiThread),
    Pending,
    Failed,
}

#[derive(Clone, Copy)]
enum UiSlot {
    Starting(u64),
    Ready(UiThread),
}

static UI: Mutex<Option<UiSlot>> = Mutex::new(None);
static UI_TOKEN: AtomicU64 = AtomicU64::new(1);

struct Viewer {
    hwnd: HWND,
    source: Rect,
    generation: u64,
    epoch: u64,
    dark_pen: windows::Win32::Graphics::Gdi::HPEN,
    bright_pen: windows::Win32::Graphics::Gdi::HPEN,
}

thread_local! {
    static VIEWER: RefCell<Option<Viewer>> = const { RefCell::new(None) };
}

fn state() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|error| error.into_inner())
}

pub fn note_activity(acted: Acted) {
    let (point, target) = match acted {
        Acted::At { x, y } => {
            let display = display_holding(x, y);
            (Some((x, y)), Some(ActivityTarget::Display(display)))
        }
        Acted::Window(id) => (None, Some(ActivityTarget::Window(id))),
        Acted::Somewhere => (None, None),
    };
    let (transition, current_generation, current_epoch) = {
        let mut state = state();
        let transition = state
            .activity
            .note(target, point, Instant::now(), IDLE_REST);
        (transition, state.activity.generation, state.activity.epoch)
    };
    let update = !matches!(transition, Transition::Nothing);
    let (generation, epoch) = match transition {
        Transition::Ignored => return,
        Transition::Nothing => (current_generation, current_epoch),
        Transition::Open { generation, epoch } => (generation, epoch),
        Transition::Repoint { epoch } => (current_generation, epoch),
    };

    let ui = match ui() {
        UiStart::Ready(ui) => ui,
        // The UI thread reads the current reducer state once its window and
        // timer exist; a slow initialization needs no queued wake-up.
        UiStart::Pending => return,
        UiStart::Failed => {
            let hide = {
                let mut state = state();
                state.activity.rest(generation, epoch)
            };
            if hide.is_some() {
                log::debug!("picture-in-picture UI thread could not start");
            }
            return;
        }
    };
    if !update {
        return;
    }
    unsafe {
        if let Err(error) =
            PostThreadMessageW(ui.id, WM_PIP_UPDATE, WPARAM(epoch as usize), LPARAM(0))
        {
            log::debug!("picture-in-picture update wake failed: {error}");
            clear_ui(ui.id);
            let _ = state().activity.rest(generation, epoch);
        }
    }
}

pub fn dismiss() {
    let epoch = state().activity.dismiss();
    let ui = UI
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .and_then(|slot| match slot {
            UiSlot::Ready(ui) => Some(ui),
            UiSlot::Starting(_) => None,
        });
    if let Some(ui) = ui {
        unsafe {
            if let Err(error) =
                PostThreadMessageW(ui.id, WM_PIP_HIDE, WPARAM(epoch as usize), LPARAM(0))
            {
                log::debug!("picture-in-picture hide wake failed: {error}");
            }
        }
    }
}

fn ui() -> UiStart {
    let token = {
        let mut slot = UI.lock().unwrap_or_else(|error| error.into_inner());
        match *slot {
            Some(UiSlot::Ready(ui)) => return UiStart::Ready(ui),
            Some(UiSlot::Starting(_)) => return UiStart::Pending,
            None => {
                let token = UI_TOKEN.fetch_add(1, Ordering::Relaxed);
                *slot = Some(UiSlot::Starting(token));
                token
            }
        }
    };

    let outcome = start_ui(token);
    let mut slot = UI.lock().unwrap_or_else(|error| error.into_inner());
    match (*slot, outcome) {
        (Some(UiSlot::Ready(ui)), _) => UiStart::Ready(ui),
        (Some(UiSlot::Starting(current)), UiStart::Ready(ui)) if current == token => {
            *slot = Some(UiSlot::Ready(ui));
            UiStart::Ready(ui)
        }
        (Some(UiSlot::Starting(current)), UiStart::Pending) if current == token => UiStart::Pending,
        (Some(UiSlot::Starting(current)), UiStart::Failed) if current == token => {
            *slot = None;
            UiStart::Failed
        }
        _ => UiStart::Pending,
    }
}

fn register_ui(token: u64, ui: UiThread) -> bool {
    let mut slot = UI.lock().unwrap_or_else(|error| error.into_inner());
    if matches!(*slot, Some(UiSlot::Starting(current)) if current == token) {
        *slot = Some(UiSlot::Ready(ui));
        true
    } else {
        false
    }
}

fn clear_ui(id: u32) {
    let mut slot = UI.lock().unwrap_or_else(|error| error.into_inner());
    if matches!(*slot, Some(UiSlot::Ready(ui)) if ui.id == id) {
        *slot = None;
    }
}

fn fail_ui_start(token: u64, ready: &mpsc::SyncSender<Option<u32>>) {
    let _ = ready.send(None);
    let mut slot = UI.lock().unwrap_or_else(|error| error.into_inner());
    if matches!(*slot, Some(UiSlot::Starting(current)) if current == token) {
        *slot = None;
    }
}

fn start_ui(token: u64) -> UiStart {
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    if std::thread::Builder::new()
        .name("lingxia-pip-ui".into())
        .spawn(move || ui_main(token, ready_tx))
        .is_err()
    {
        return UiStart::Failed;
    }
    match ready_rx.recv_timeout(Duration::from_millis(100)) {
        Ok(Some(id)) => UiStart::Ready(UiThread { id }),
        Ok(None) | Err(mpsc::RecvTimeoutError::Disconnected) => UiStart::Failed,
        Err(mpsc::RecvTimeoutError::Timeout) => UiStart::Pending,
    }
}

fn ui_main(token: u64, ready: mpsc::SyncSender<Option<u32>>) {
    super::ensure_dpi_aware();
    unsafe {
        let hinstance = match GetModuleHandleW(None) {
            Ok(hinstance) => hinstance,
            Err(error) => {
                log::debug!("picture-in-picture module lookup failed: {error}");
                fail_ui_start(token, &ready);
                return;
            }
        };
        let class_name = w!("LingXiaActivityViewer");
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: hinstance.into(),
            lpszClassName: class_name,
            ..Default::default()
        };
        if RegisterClassW(&class) == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS {
            log::debug!("picture-in-picture class registration failed");
            fail_ui_start(token, &ready);
            return;
        }
        let hwnd = match CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_TRANSPARENT,
            class_name,
            w!("LingXia activity viewer"),
            WS_POPUP,
            0,
            0,
            1,
            1,
            None,
            None,
            Some(hinstance.into()),
            None,
        ) {
            Ok(hwnd) => hwnd,
            Err(error) => {
                log::debug!("picture-in-picture window creation failed: {error}");
                fail_ui_start(token, &ready);
                return;
            }
        };
        if let Err(error) = SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA) {
            log::debug!("picture-in-picture layering failed: {error}");
            let _ = DestroyWindow(hwnd);
            fail_ui_start(token, &ready);
            return;
        }
        if SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE).is_err()
            && let Err(error) = SetWindowDisplayAffinity(hwnd, WDA_MONITOR)
        {
            log::debug!("picture-in-picture capture exclusion unavailable: {error}");
        }

        VIEWER.with_borrow_mut(|slot| {
            *slot = Some(Viewer {
                hwnd,
                source: Rect {
                    x: 0,
                    y: 0,
                    w: 1,
                    h: 1,
                },
                generation: 0,
                epoch: 0,
                dark_pen: CreatePen(PS_SOLID, 7, COLORREF(0x001e28)),
                bright_pen: CreatePen(PS_SOLID, 3, COLORREF(0x000ad6ff)),
            });
        });
        if SetTimer(Some(hwnd), TIMER_ID, 1000 / FPS, None) == 0 {
            log::debug!("picture-in-picture timer creation failed");
            let _ = DestroyWindow(hwnd);
            fail_ui_start(token, &ready);
            return;
        }
        let thread = UiThread {
            id: GetCurrentThreadId(),
        };
        if !register_ui(token, thread) {
            let _ = DestroyWindow(hwnd);
            return;
        }
        let _ = ready.send(Some(thread.id));

        // The caller waits only briefly. If initialization outlives that bound,
        // the UI thread still converges from the shared state on its own.
        apply_update(state().activity.epoch);

        let mut message = MSG::default();
        loop {
            let result = GetMessageW(&mut message, None, 0, 0).0;
            if result == 0 {
                break;
            }
            if result < 0 {
                log::debug!("picture-in-picture message loop failed");
                break;
            }
            match message.message {
                WM_PIP_UPDATE => apply_update(message.wParam.0 as u64),
                WM_PIP_HIDE => apply_hide(message.wParam.0 as u64),
                _ => {
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
        }
        clear_ui(thread.id);
    }
}

fn apply_update(epoch: u64) {
    let (generation, target, corner) = {
        let state = state();
        if state.activity.dismissed
            || state.activity.epoch != epoch
            || state.activity.target.is_none()
        {
            return;
        }
        (
            state.activity.generation,
            state.activity.target.clone().expect("checked target"),
            state.corner,
        )
    };
    let Ok(source) = source_rect(&target) else {
        stop_if_current(generation, epoch);
        return;
    };
    let Some((x, y, width, height)) = placement(source, corner) else {
        stop_if_current(generation, epoch);
        return;
    };

    let desired = Rect {
        x,
        y,
        w: width,
        h: height,
    };
    let changed = {
        let state = state();
        if !state.activity.current(generation, epoch) {
            return;
        }
        !state.panel.is_some_and(|panel| same_rect(panel, desired))
    };
    let hwnd = VIEWER.with_borrow_mut(|slot| {
        if let Some(viewer) = slot.as_mut() {
            viewer.source = source;
            viewer.generation = generation;
            viewer.epoch = epoch;
            Some(viewer.hwnd)
        } else {
            None
        }
    });
    let Some(hwnd) = hwnd else { return };
    unsafe {
        if changed {
            if let Err(error) = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                x,
                y,
                width,
                height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            ) {
                log::debug!("picture-in-picture placement failed: {error}");
            } else {
                let mut state = state();
                if state.activity.current(generation, epoch) {
                    state.panel = Some(desired);
                }
            }
        }
        let _ = InvalidateRect(Some(hwnd), None, false);
    }
    if !state().activity.current(generation, epoch) {
        apply_hide(state().activity.epoch);
    }
}

fn apply_hide(epoch: u64) {
    let guard = state();
    if guard.activity.epoch != epoch || guard.activity.target.is_some() {
        return;
    }
    drop(guard);
    VIEWER.with_borrow(|slot| {
        if let Some(viewer) = slot.as_ref() {
            unsafe {
                let _ = ShowWindow(viewer.hwnd, SW_HIDE);
            }
        }
    });
    state().panel = None;
}

fn tick() {
    let (generation, epoch, target, marker, idle, panel) = {
        let state = state();
        let marker = state
            .activity
            .marker
            .filter(|(_, _, at)| at.elapsed() < MARKER_LINGER)
            .map(|(x, y, _)| (x, y));
        (
            state.activity.generation,
            state.activity.epoch,
            state.activity.target.clone(),
            marker,
            state
                .activity
                .last_activity
                .is_some_and(|at| at.elapsed() > IDLE_REST),
            state.panel,
        )
    };
    let Some(target) = target else {
        VIEWER.with_borrow(|slot| {
            if let Some(viewer) = slot.as_ref() {
                unsafe {
                    let _ = ShowWindow(viewer.hwnd, SW_HIDE);
                }
            }
        });
        state().panel = None;
        return;
    };
    if idle {
        stop_if_current(generation, epoch);
        return;
    }

    if marker.is_some_and(|point| panel.is_some_and(|panel| contains(panel, point))) {
        let source = source_rect(&target).ok();
        if let Some(source) = source {
            let mut state = state();
            if state.activity.current(generation, epoch)
                && let Some(marker) = marker
            {
                state.corner = corner_away_from(marker, source);
            }
        }
        apply_update(epoch);
        return;
    }

    apply_update(epoch);
}

fn stop_if_current(generation: u64, epoch: u64) {
    let epoch = state().activity.rest(generation, epoch);
    if let Some(epoch) = epoch {
        apply_hide(epoch);
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match message {
            WM_PAINT => {
                paint(hwnd);
                LRESULT(0)
            }
            WM_TIMER if wparam.0 == TIMER_ID => {
                tick();
                LRESULT(0)
            }
            WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
            WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
            WM_CLOSE => {
                let epoch = state().activity.dismiss();
                apply_hide(epoch);
                LRESULT(0)
            }
            WM_DISPLAYCHANGE | WM_DPICHANGED => {
                let epoch = state().activity.epoch;
                let _ = PostMessageW(Some(hwnd), WM_PIP_UPDATE, WPARAM(epoch as usize), LPARAM(0));
                LRESULT(0)
            }
            WM_ERASEBKGND => LRESULT(1),
            WM_DESTROY => {
                VIEWER.with_borrow_mut(|slot| {
                    if let Some(viewer) = slot.take() {
                        let _ = DeleteObject(viewer.dark_pen.into());
                        let _ = DeleteObject(viewer.bright_pen.into());
                    }
                });
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}

unsafe fn paint(hwnd: HWND) {
    unsafe {
        let mut paint = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut paint);
        let mut client = RECT::default();
        if GetClientRect(hwnd, &mut client).is_err() {
            let _ = EndPaint(hwnd, &paint);
            return;
        }
        VIEWER.with_borrow(|slot| {
            let Some(viewer) = slot.as_ref() else { return };
            if !state().activity.current(viewer.generation, viewer.epoch) {
                return;
            }
            let screen = GetDC(None);
            if !screen.is_invalid() {
                let _ = SetStretchBltMode(hdc, HALFTONE);
                let _ = StretchBlt(
                    hdc,
                    0,
                    0,
                    client.right,
                    client.bottom,
                    Some(screen),
                    viewer.source.x,
                    viewer.source.y,
                    viewer.source.w,
                    viewer.source.h,
                    SRCCOPY,
                );
                ReleaseDC(None, screen);
            }

            let marker = {
                let state = state();
                if state.activity.generation != viewer.generation {
                    None
                } else {
                    state
                        .activity
                        .marker
                        .filter(|(_, _, at)| at.elapsed() < MARKER_LINGER)
                        .map(|(x, y, _)| (x, y))
                }
            };
            if let Some((x, y)) = marker {
                draw_marker(hdc, client, viewer.source, x, y, viewer);
            }
        });
        let _ = EndPaint(hwnd, &paint);
    }
}

unsafe fn draw_marker(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    client: RECT,
    source: Rect,
    x: i32,
    y: i32,
    viewer: &Viewer,
) {
    if !contains(source, (x, y)) {
        return;
    }
    unsafe {
        let px =
            (((x as i64 - source.x as i64) * client.right as i64) / source.w.max(1) as i64) as i32;
        let py =
            (((y as i64 - source.y as i64) * client.bottom as i64) / source.h.max(1) as i64) as i32;
        let radius = (client.right.min(client.bottom) * 35 / 1000).clamp(10, 24);
        let old_brush = SelectObject(hdc, HGDIOBJ(GetStockObject(NULL_BRUSH).0));
        let old_pen = SelectObject(hdc, viewer.dark_pen.into());
        let _ = Ellipse(hdc, px - radius, py - radius, px + radius, py + radius);
        let _ = SelectObject(hdc, viewer.bright_pen.into());
        let _ = Ellipse(hdc, px - radius, py - radius, px + radius, py + radius);
        let _ = SelectObject(hdc, old_pen);
        let _ = SelectObject(hdc, old_brush);
    }
}

fn source_rect(target: &ActivityTarget) -> crate::Result<Rect> {
    match target {
        ActivityTarget::Display(index) => super::displays()?
            .get(index.wrapping_sub(1))
            .map(|display| display.bounds)
            .ok_or_else(|| crate::Error::NotFound(format!("no display {index}"))),
        ActivityTarget::Window(id) => {
            let hwnd = super::parse_hwnd(id)?;
            let mut rect = RECT::default();
            unsafe {
                GetWindowRect(hwnd, &mut rect)
                    .map_err(|_| crate::Error::Stale(format!("window {id} is not available")))?;
            }
            let rect = super::rect_to(rect);
            if rect.w <= 0 || rect.h <= 0 {
                return Err(crate::Error::Stale(format!("window {id} has zero size")));
            }
            Ok(rect)
        }
    }
}

fn display_holding(x: i32, y: i32) -> usize {
    super::displays()
        .ok()
        .and_then(|displays| {
            displays
                .iter()
                .position(|display| contains(display.bounds, (x, y)))
        })
        .map_or(1, |index| index + 1)
}

fn contains(rect: Rect, (x, y): (i32, i32)) -> bool {
    let (x, y) = (x as i64, y as i64);
    let (left, top) = (rect.x as i64, rect.y as i64);
    x >= left && x < left + rect.w as i64 && y >= top && y < top + rect.h as i64
}

fn same_rect(a: Rect, b: Rect) -> bool {
    a.x == b.x && a.y == b.y && a.w == b.w && a.h == b.h
}

fn placement(source: Rect, corner: Corner) -> Option<(i32, i32, i32, i32)> {
    let monitor_rect = RECT {
        left: source.x,
        top: source.y,
        right: source.x.saturating_add(source.w),
        bottom: source.y.saturating_add(source.h),
    };
    let monitor = unsafe {
        windows::Win32::Graphics::Gdi::MonitorFromRect(
            &monitor_rect,
            windows::Win32::Graphics::Gdi::MONITOR_DEFAULTTONEAREST,
        )
    };
    let mut info = windows::Win32::Graphics::Gdi::MONITORINFO {
        cbSize: std::mem::size_of::<windows::Win32::Graphics::Gdi::MONITORINFO>() as u32,
        ..Default::default()
    };
    unsafe {
        if !windows::Win32::Graphics::Gdi::GetMonitorInfoW(monitor, &mut info).as_bool() {
            return None;
        }
    }
    let dpi = super::monitor_dpi(monitor) as i64;
    let inset = 16 * dpi / 96;
    let available_width = (info.rcWork.right as i64 - info.rcWork.left as i64 - inset * 2).max(1);
    let available_height = (info.rcWork.bottom as i64 - info.rcWork.top as i64 - inset * 2).max(1);
    let available_width = available_width.min(i32::MAX as i64) as i32;
    let available_height = available_height.min(i32::MAX as i64) as i32;
    let (width, height) = fit_size(
        source.w,
        source.h,
        available_width,
        available_height,
        dpi as u32,
    );
    let inset = inset as i32;
    let (x, y) = match corner {
        Corner::TopLeft => (info.rcWork.left + inset, info.rcWork.top + inset),
        Corner::TopRight => (info.rcWork.right - width - inset, info.rcWork.top + inset),
        Corner::BottomLeft => (
            info.rcWork.left + inset,
            info.rcWork.bottom - height - inset,
        ),
        Corner::BottomRight => (
            info.rcWork.right - width - inset,
            info.rcWork.bottom - height - inset,
        ),
    };
    Some((x, y, width, height))
}

fn fit_size(
    source_width: i32,
    source_height: i32,
    available_width: i32,
    available_height: i32,
    dpi: u32,
) -> (i32, i32) {
    let mut width = (360_i64 * dpi as i64 / 96).min(available_width.max(1) as i64);
    let mut height = width * source_height.max(1) as i64 / source_width.max(1) as i64;
    if height > available_height.max(1) as i64 {
        height = available_height.max(1) as i64;
        width = height * source_width.max(1) as i64 / source_height.max(1) as i64;
    }
    (width.max(1) as i32, height.max(1) as i32)
}

fn corner_away_from((x, y): (i32, i32), source: Rect) -> Corner {
    match (
        (x as i64) < source.x as i64 + source.w as i64 / 2,
        (y as i64) < source.y as i64 + source.h as i64 / 2,
    ) {
        (true, true) => Corner::BottomRight,
        (false, true) => Corner::BottomLeft,
        (true, false) => Corner::TopRight,
        (false, false) => Corner::TopLeft,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewer_class_is_excluded_exactly() {
        assert!(is_viewer_class(VIEWER_CLASS));
        assert!(!is_viewer_class("LingXiaActivityViewerTarget"));
    }

    #[test]
    fn fit_preserves_aspect_and_stays_inside_work_area() {
        assert_eq!(fit_size(1920, 1080, 800, 600, 96), (360, 202));
        assert_eq!(fit_size(1080, 1920, 800, 400, 96), (225, 400));
        let (width, height) = fit_size(i32::MAX, i32::MAX, 320, 180, 192);
        assert!(width <= 320 && height <= 180);
    }

    #[test]
    fn negative_desktop_coordinates_keep_half_open_bounds() {
        let rect = Rect {
            x: -1920,
            y: -400,
            w: 1920,
            h: 1080,
        };
        assert!(contains(rect, (-1920, -400)));
        assert!(contains(rect, (-1, 679)));
        assert!(!contains(rect, (0, 0)));
        assert!(!contains(rect, (-1921, -400)));
    }

    #[test]
    fn move_away_chooses_the_diagonal_corner() {
        let source = Rect {
            x: -1000,
            y: -500,
            w: 1000,
            h: 800,
        };
        assert!(matches!(
            corner_away_from((-900, -400), source),
            Corner::BottomRight
        ));
        assert!(matches!(
            corner_away_from((-100, 200), source),
            Corner::TopLeft
        ));
    }
}
