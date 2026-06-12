//! Window placement persistence and live move/size layout pumping.

use super::*;

#[derive(Debug, Clone, Copy)]
pub(crate) struct WindowPlacement {
    pub(super) left: i32,
    pub(super) top: i32,
    pub(super) width: i32,
    pub(super) height: i32,
}

pub(crate) static WINDOW_GROUP_PLACEMENTS: OnceLock<Mutex<HashMap<String, WindowPlacement>>> =
    OnceLock::new();

/// Per-UI-thread handle to the window's WebView2 controller so the window
/// procedure can drive layout itself. Interactive move/size runs inside
/// `DefWindowProcW`'s modal loop, which starves both the command channel
/// and posted thread messages, so anything that must track a drag live has
/// to run on the window-message path, never the command queue.
struct LiveLayoutContext {
    hwnd: HWND,
    webtag_key: String,
    controller: ICoreWebView2Controller,
}

thread_local! {
    static LIVE_LAYOUT_CONTEXT: RefCell<Option<LiveLayoutContext>> = const { RefCell::new(None) };
}

pub(crate) fn register_live_layout_context(state: &UiState) {
    LIVE_LAYOUT_CONTEXT.with(|slot| {
        *slot.borrow_mut() = Some(LiveLayoutContext {
            hwnd: state.hwnd,
            webtag_key: state.webtag_key.clone(),
            controller: state.controller.clone(),
        });
    });
}

pub(crate) fn clear_live_layout_context() {
    LIVE_LAYOUT_CONTEXT.with(|slot| {
        slot.borrow_mut().take();
    });
}

/// Timer pumping layout during an interactive move/size: inside
/// `DefWindowProcW`'s modal loop `WM_WINDOWPOSCHANGED` can be coalesced
/// (live drags then visibly outrun the webview area), but `WM_TIMER` keeps
/// firing in that loop, so the drag is tracked at the timer cadence
/// regardless. Armed on `WM_ENTERSIZEMOVE`, killed on `WM_EXITSIZEMOVE`.
pub(crate) const SIZEMOVE_TIMER_ID: usize = 0x4C58_4D56; // "LXMV"

pub(crate) const SIZEMOVE_TIMER_INTERVAL_MS: u32 = 16;

/// Last seen window rect per window, so the move/size timer can skip ticks
/// where the window did not actually move or resize (the timer fires every
/// ~16ms for the whole drag, including while the cursor rests).
static LAST_WINDOW_RECTS: OnceLock<Mutex<HashMap<isize, (i32, i32, i32, i32)>>> = OnceLock::new();

fn current_window_rect(hwnd: HWND) -> Option<(i32, i32, i32, i32)> {
    let mut rect = RECT::default();
    unsafe {
        WindowsAndMessaging::GetWindowRect(hwnd, &mut rect).ok()?;
    }
    Some((rect.left, rect.top, rect.right, rect.bottom))
}

/// `WM_TIMER` tick of the move/size timer: runs the full geometry sync only
/// when the window rect changed since the last pass, then repaints the
/// chrome so newly exposed strips don't linger as stale content.
pub(crate) fn handle_live_sizemove_tick(hwnd: HWND) {
    let rect = current_window_rect(hwnd);
    let unchanged = LAST_WINDOW_RECTS
        .get()
        .and_then(|rects| rects.lock().ok())
        .is_some_and(|rects| rect.is_some() && rects.get(&hwnd_handle(hwnd)).copied() == rect);
    if unchanged {
        return;
    }
    handle_window_geometry_change(hwnd);
    if windows_chrome_renderer().is_some() {
        unsafe {
            let _ = InvalidateRect(Some(hwnd), None, false);
        }
    }
}

/// Lays out the window owned by this UI thread directly from the window
/// procedure: syncs the WebView2 controller bounds, notifies the controller
/// of the new window position, re-arranges attached group windows, and
/// stores the placement. Cheap and idempotent: `sync_controller_bounds_for`
/// skips the controller `SetBounds` when the target bounds are unchanged,
/// so it can run on every `WM_WINDOWPOSCHANGED` step and every ~16ms
/// move/size timer tick of an interactive drag.
pub(crate) fn handle_window_geometry_change(hwnd: HWND) {
    let context = LIVE_LAYOUT_CONTEXT.with(|slot| {
        slot.borrow()
            .as_ref()
            .filter(|context| context.hwnd == hwnd)
            .map(|context| (context.webtag_key.clone(), context.controller.clone()))
    });
    let Some((webtag_key, controller)) = context else {
        return;
    };
    if let Some(rect) = current_window_rect(hwnd)
        && let Ok(mut rects) = LAST_WINDOW_RECTS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
    {
        rects.insert(hwnd_handle(hwnd), rect);
    }
    let _ = sync_controller_bounds_for(hwnd, &webtag_key, &controller);
    unsafe {
        let _ = controller.NotifyParentWindowPositionChanged();
    }
    // A presented MainChild (e.g. a browser tab) occupies the main card rect
    // of this host, and the host's own WebView2 child-window chain would
    // z-fight with it — hide the host controller while another webview is
    // the group's active main, restore it once the presentation ends.
    if matches!(
        window_attachment(&webtag_key).map(|attachment| attachment.kind),
        Some(WindowAttachmentKind::MainHost)
    ) {
        let group_key = layout_group_key_for_webtag(&webtag_key);
        let covered = group_active_main(&group_key).is_some_and(|active| active != webtag_key);
        unsafe {
            let _ = controller.SetIsVisible(!covered);
        }
        if !covered {
            // The visibility flip can churn WebView2's child z-order after
            // the bounds sync already placed the caps; re-assert them.
            raise_corner_caps(hwnd);
        }
    }
    layout_group_for_main_host(&webtag_key);
    store_window_placement(hwnd, &webtag_key);
    sync_device_frame_for_content(hwnd);
}

pub(crate) fn store_current_window_placement(state: &UiState) {
    store_window_placement(state.hwnd, &state.webtag_key);
}

pub(crate) fn store_window_placement(hwnd: HWND, webtag_key: &str) {
    if matches!(
        window_attachment(webtag_key).map(|attachment| attachment.kind),
        Some(WindowAttachmentKind::MainChild | WindowAttachmentKind::Panel { .. })
    ) {
        return;
    }
    if unsafe { WindowsAndMessaging::IsZoomed(hwnd).as_bool() } {
        return;
    }
    let mut rect = RECT::default();
    if !unsafe { WindowsAndMessaging::IsWindowVisible(hwnd).as_bool() }
        || unsafe { WindowsAndMessaging::GetWindowRect(hwnd, &mut rect) }.is_err()
    {
        return;
    }
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    if width <= 0 || height <= 0 {
        return;
    }

    let placements = WINDOW_GROUP_PLACEMENTS.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut placements) = placements.lock() {
        placements.insert(
            webtag_group_key(webtag_key),
            WindowPlacement {
                left: rect.left,
                top: rect.top,
                width,
                height,
            },
        );
    }
}

pub(crate) fn current_group_window_placement_for_group(group_key: &str) -> Option<WindowPlacement> {
    WINDOW_GROUP_PLACEMENTS
        .get()
        .and_then(|placements| placements.lock().ok())
        .and_then(|placements| placements.get(group_key).copied())
}

pub(super) fn forget_live_layout_rect(hwnd: HWND) {
    let key = hwnd_handle(hwnd);
    if let Some(rects) = LAST_WINDOW_RECTS.get()
        && let Ok(mut rects) = rects.lock()
    {
        rects.remove(&key);
    }
}
