//! A passive activity viewer on a dedicated Win32 UI thread.
//!
//! Window targets use a live DWM thumbnail, which keeps showing the real
//! composited window while it is covered and avoids building a capture stack
//! for every frame. Untargeted work still mirrors the visible display through
//! GDI because there is no single window for DWM to follow.

use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, mpsc};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{
    COLORREF, ERROR_CLASS_ALREADY_EXISTS, GetLastError, HWND, LPARAM, LRESULT, RECT, WPARAM,
};
use windows::Win32::Graphics::Dwm::{
    DWM_THUMBNAIL_PROPERTIES, DWM_TNP_OPACITY, DWM_TNP_RECTDESTINATION,
    DWM_TNP_SOURCECLIENTAREAONLY, DWM_TNP_VISIBLE, DwmRegisterThumbnail, DwmUnregisterThumbnail,
    DwmUpdateThumbnailProperties,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreatePen, CreateSolidBrush, DEFAULT_GUI_FONT, DeleteObject, Ellipse, EndPaint,
    FillRect, GetDC, GetStockObject, HALFTONE, HGDIOBJ, InvalidateRect, NULL_BRUSH, PAINTSTRUCT,
    PS_SOLID, ReleaseDC, SRCCOPY, SelectObject, SetBkMode, SetStretchBltMode, SetTextColor,
    StretchBlt, TRANSPARENT, TextOutW,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

use crate::model::{Acted, Rect, WindowTarget};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewerMode {
    Compact,
    Full,
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
    thumbnail: Option<Thumbnail>,
    mode: ViewerMode,
    label: String,
    generation: u64,
    epoch: u64,
    dark_pen: windows::Win32::Graphics::Gdi::HPEN,
    bright_pen: windows::Win32::Graphics::Gdi::HPEN,
}

struct Thumbnail {
    handle: isize,
    source: HWND,
}

thread_local! {
    static VIEWER: RefCell<Option<Viewer>> = const { RefCell::new(None) };
}

fn state() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|error| error.into_inner())
}

pub fn note_activity(acted: Acted) {
    // Target discovery below can block and complete out of order across
    // callers. Timestamp entry so the reducer can reject the older action.
    let observed_at = Instant::now();
    let (point, target) = match acted {
        Acted::At { x, y } => {
            let display = display_holding(x, y);
            let current = state().activity.target.clone();
            let bounds = current.as_ref().and_then(|target| source_rect(target).ok());
            let target = if preserve_window_target(current.as_ref(), bounds, (x, y)) {
                None
            } else {
                Some(ActivityTarget::Display(display))
            };
            (Some((x, y)), target)
        }
        Acted::Display { x, y } => (None, Some(ActivityTarget::Display(display_holding(x, y)))),
        Acted::AtWindow { x, y, id } => (
            Some((x, y)),
            Some(ActivityTarget::Window {
                id,
                fallback_display: display_holding(x, y),
            }),
        ),
        Acted::WindowWithFallback { id, x, y } => (
            None,
            Some(ActivityTarget::Window {
                id,
                fallback_display: display_holding(x, y),
            }),
        ),
        Acted::Somewhere => (None, None),
    };
    let (transition, current_generation, current_epoch) = {
        let mut state = state();
        let transition = state.activity.note(target, point, observed_at, IDLE_REST);
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
    let mut shared = state();
    reap_ui(&mut slot, &mut shared.panel, id);
}

/// Retire only the UI thread that still owns the slot, and invalidate its
/// placement together with it. A replacement HWND starts at 1x1 and hidden;
/// retaining the old rect would make an identical desired placement look
/// unchanged and skip the SWP_SHOWWINDOW that reveals the replacement.
fn reap_ui(slot: &mut Option<UiSlot>, panel: &mut Option<Rect>, id: u32) -> bool {
    if !matches!(*slot, Some(UiSlot::Ready(ui)) if ui.id == id) {
        return false;
    }
    *slot = None;
    *panel = None;
    true
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
                thumbnail: None,
                mode: ViewerMode::Full,
                label: String::new(),
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
        // Drop the reducer guard before `apply_update` locks it again.
        let epoch = state().activity.epoch;
        apply_update(epoch);

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
    let source = match source_rect(&target) {
        Ok(source) => source,
        Err(error) => {
            let fallback_epoch = state().activity.fallback_to_display(generation, epoch);
            if let Some(fallback_epoch) = fallback_epoch {
                log::debug!("picture-in-picture window vanished, showing its display: {error}");
                apply_update(fallback_epoch);
            } else {
                stop_if_current(generation, epoch);
            }
            return;
        }
    };
    let product = product_anchor();
    let anchor = product.unwrap_or(source);
    let mode = viewer_mode(&target, source, product);
    let Some((x, y, width, height)) = placement(source, anchor, corner, mode) else {
        stop_if_current(generation, epoch);
        return;
    };
    let label = compact_label(&target);

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
            viewer.mode = mode;
            viewer.label.clone_from(&label);
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
        VIEWER.with_borrow_mut(|slot| {
            if let Some(viewer) = slot.as_mut() {
                if mode == ViewerMode::Full {
                    let _ = sync_thumbnail(viewer, &target, width, height);
                } else {
                    clear_thumbnail(viewer);
                }
            }
        });
        let _ = InvalidateRect(Some(hwnd), None, false);
    }
    let current = state().activity.current(generation, epoch);
    if !current {
        let epoch = state().activity.epoch;
        apply_hide(epoch);
    }
}

fn preserve_window_target(
    current: Option<&ActivityTarget>,
    bounds: Option<Rect>,
    point: (i32, i32),
) -> bool {
    matches!(current, Some(ActivityTarget::Window { .. }))
        && bounds.is_some_and(|bounds| contains(bounds, point))
}

fn apply_hide(epoch: u64) {
    let guard = state();
    if guard.activity.epoch != epoch || guard.activity.target.is_some() {
        return;
    }
    drop(guard);
    VIEWER.with_borrow_mut(|slot| {
        if let Some(viewer) = slot.as_mut() {
            unsafe {
                let _ = ShowWindow(viewer.hwnd, SW_HIDE);
            }
            clear_thumbnail(viewer);
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
        VIEWER.with_borrow_mut(|slot| {
            if let Some(viewer) = slot.as_mut() {
                unsafe {
                    let _ = ShowWindow(viewer.hwnd, SW_HIDE);
                }
                clear_thumbnail(viewer);
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
            let anchor = product_anchor().unwrap_or(source);
            let mut state = state();
            if state.activity.current(generation, epoch)
                && let Some(marker) = marker
            {
                state.corner = corner_away_from(marker, anchor);
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
                    if let Some(mut viewer) = slot.take() {
                        clear_thumbnail(&mut viewer);
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

fn clear_thumbnail(viewer: &mut Viewer) {
    if let Some(thumbnail) = viewer.thumbnail.take() {
        unsafe {
            if let Err(error) = DwmUnregisterThumbnail(thumbnail.handle) {
                log::debug!("picture-in-picture thumbnail cleanup failed: {error}");
            }
        }
    }
}

fn sync_thumbnail(viewer: &mut Viewer, target: &ActivityTarget, width: i32, height: i32) -> bool {
    let ActivityTarget::Window { id, .. } = target else {
        clear_thumbnail(viewer);
        return false;
    };
    let Ok(source) = super::parse_hwnd(id) else {
        clear_thumbnail(viewer);
        return false;
    };
    if viewer
        .thumbnail
        .as_ref()
        .is_none_or(|thumbnail| thumbnail.source != source)
    {
        clear_thumbnail(viewer);
        let handle = unsafe { DwmRegisterThumbnail(viewer.hwnd, source) };
        match handle {
            Ok(handle) => viewer.thumbnail = Some(Thumbnail { handle, source }),
            Err(error) => {
                log::debug!("picture-in-picture thumbnail registration failed: {error}");
                return false;
            }
        }
    }
    let Some(handle) = viewer.thumbnail.as_ref().map(|thumbnail| thumbnail.handle) else {
        return false;
    };
    let properties = DWM_THUMBNAIL_PROPERTIES {
        dwFlags: DWM_TNP_RECTDESTINATION
            | DWM_TNP_OPACITY
            | DWM_TNP_VISIBLE
            | DWM_TNP_SOURCECLIENTAREAONLY,
        rcDestination: RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        },
        opacity: 255,
        fVisible: true.into(),
        fSourceClientAreaOnly: false.into(),
        ..Default::default()
    };
    if let Err(error) = unsafe { DwmUpdateThumbnailProperties(handle, &properties) } {
        log::debug!("picture-in-picture thumbnail update failed: {error}");
        clear_thumbnail(viewer);
        return false;
    }
    true
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
            if viewer.mode == ViewerMode::Compact {
                draw_compact(hdc, client, &viewer.label, viewer);
            } else {
                if viewer.thumbnail.is_none() {
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
            }
        });
        let _ = EndPaint(hwnd, &paint);
    }
}

unsafe fn draw_compact(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    client: RECT,
    label: &str,
    viewer: &Viewer,
) {
    unsafe {
        let background = CreateSolidBrush(COLORREF(0x00241f1c));
        let _ = FillRect(hdc, &client, background);
        let accent = CreateSolidBrush(COLORREF(0x000ad6ff));
        let old_brush = SelectObject(hdc, accent.into());
        let old_pen = SelectObject(hdc, viewer.bright_pen.into());
        let center_y = client.bottom / 2;
        let _ = Ellipse(hdc, 16, center_y - 6, 28, center_y + 6);
        let _ = SelectObject(hdc, old_pen);
        let _ = SelectObject(hdc, old_brush);
        let _ = DeleteObject(accent.into());

        let old_font = SelectObject(hdc, GetStockObject(DEFAULT_GUI_FONT));
        let _ = SetBkMode(hdc, TRANSPARENT);
        let _ = SetTextColor(hdc, COLORREF(0x00f4f4f4));
        let text: Vec<u16> = label.encode_utf16().take(42).collect();
        let _ = TextOutW(hdc, 42, (center_y - 8).max(0), &text);
        let _ = SelectObject(hdc, old_font);
        let _ = DeleteObject(background.into());
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
        ActivityTarget::Window { id, .. } => {
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

/// Keep the product's viewer beside the product when it has a visible window.
/// The mirrored source may live on another monitor; DWM does not require the
/// source and destination windows to share one.
fn product_anchor() -> Option<Rect> {
    let query = crate::model::WindowQuery::parse(&format!("pid:{}", std::process::id()));
    let windows = super::windows(&query).ok()?;
    windows
        .iter()
        .find(|window| window.focused && !window.minimized)
        .or_else(|| windows.iter().find(|window| !window.minimized))
        .map(|window| window.bounds)
}

fn viewer_mode(target: &ActivityTarget, source: Rect, product: Option<Rect>) -> ViewerMode {
    let Some(product) = product else {
        return ViewerMode::Full;
    };
    let ActivityTarget::Window { id, .. } = target else {
        return ViewerMode::Full;
    };
    let Ok(hwnd) = super::parse_hwnd(id) else {
        return ViewerMode::Full;
    };
    mode_for(
        true,
        unsafe { GetForegroundWindow() == hwnd },
        same_monitor(source, product),
    )
}

fn mode_for(product_visible: bool, target_foreground: bool, same_monitor: bool) -> ViewerMode {
    if product_visible && target_foreground && same_monitor {
        ViewerMode::Compact
    } else {
        ViewerMode::Full
    }
}

fn same_monitor(a: Rect, b: Rect) -> bool {
    monitor_for(a) == monitor_for(b)
}

fn monitor_for(rect: Rect) -> windows::Win32::Graphics::Gdi::HMONITOR {
    let rect = RECT {
        left: rect.x,
        top: rect.y,
        right: rect.x.saturating_add(rect.w),
        bottom: rect.y.saturating_add(rect.h),
    };
    unsafe {
        windows::Win32::Graphics::Gdi::MonitorFromRect(
            &rect,
            windows::Win32::Graphics::Gdi::MONITOR_DEFAULTTONEAREST,
        )
    }
}

fn compact_label(target: &ActivityTarget) -> String {
    let ActivityTarget::Window { id, .. } = target else {
        return "Foreground control".into();
    };
    let identity = super::window_status(&WindowTarget::Id(id.clone()))
        .ok()
        .map(|window| {
            if window.process.is_empty() {
                window.title
            } else {
                window.process
            }
        })
        .filter(|identity| !identity.is_empty());
    identity.map_or_else(
        || "Foreground control".into(),
        |identity| format!("Foreground control - {identity}"),
    )
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

fn placement(
    source: Rect,
    anchor: Rect,
    corner: Corner,
    mode: ViewerMode,
) -> Option<(i32, i32, i32, i32)> {
    let monitor = monitor_for(anchor);
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
    let (width, height) = match mode {
        ViewerMode::Compact => compact_size(available_width, available_height, dpi as u32),
        ViewerMode::Full => fit_size(
            source.w,
            source.h,
            available_width,
            available_height,
            dpi as u32,
        ),
    };
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

fn compact_size(available_width: i32, available_height: i32, dpi: u32) -> (i32, i32) {
    let width = (300_i64 * dpi as i64 / 96).min(available_width.max(1) as i64);
    let height = (48_i64 * dpi as i64 / 96).min(available_height.max(1) as i64);
    (width.max(1) as i32, height.max(1) as i32)
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
    fn reaping_ui_invalidates_same_geometry_for_the_replacement_window() {
        let desired = Rect {
            x: 100,
            y: 200,
            w: 360,
            h: 225,
        };
        let mut slot = Some(UiSlot::Ready(UiThread { id: 17 }));
        let mut panel = Some(desired);
        assert!(!reap_ui(&mut slot, &mut panel, 16));
        assert!(panel.is_some_and(|old| same_rect(old, desired)));

        assert!(reap_ui(&mut slot, &mut panel, 17));
        assert!(slot.is_none());
        assert!(
            !panel.is_some_and(|old| same_rect(old, desired)),
            "the replacement 1x1 HWND must be positioned and shown even when geometry is unchanged"
        );
    }

    #[test]
    fn fit_preserves_aspect_and_stays_inside_work_area() {
        assert_eq!(fit_size(1920, 1080, 800, 600, 96), (360, 202));
        assert_eq!(fit_size(1080, 1920, 800, 400, 96), (225, 400));
        let (width, height) = fit_size(i32::MAX, i32::MAX, 320, 180, 192);
        assert!(width <= 320 && height <= 180);
    }

    #[test]
    fn compact_mode_is_only_for_foreground_work_on_the_product_monitor() {
        assert_eq!(mode_for(true, true, true), ViewerMode::Compact);
        assert_eq!(mode_for(false, true, true), ViewerMode::Full);
        assert_eq!(mode_for(true, false, true), ViewerMode::Full);
        assert_eq!(mode_for(true, true, false), ViewerMode::Full);
    }

    #[test]
    fn compact_size_scales_with_dpi_and_stays_inside_work_area() {
        assert_eq!(compact_size(800, 600, 96), (300, 48));
        assert_eq!(compact_size(800, 600, 144), (450, 72));
        assert_eq!(compact_size(200, 30, 192), (200, 30));
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
    fn targeted_pointer_keeps_the_window_source_inside_its_bounds() {
        let target = ActivityTarget::Window {
            id: "0x42".into(),
            fallback_display: 1,
        };
        let bounds = Rect {
            x: 100,
            y: 200,
            w: 800,
            h: 600,
        };
        assert!(preserve_window_target(
            Some(&target),
            Some(bounds),
            (500, 400)
        ));
        assert!(!preserve_window_target(
            Some(&target),
            Some(bounds),
            (50, 400)
        ));
        assert!(!preserve_window_target(
            Some(&ActivityTarget::Display(1)),
            Some(bounds),
            (500, 400)
        ));
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
