//! Native window management: creation, window proc, hit testing,
//! show/hide flows, bounds syncing, and placement persistence.

use super::*;

pub(crate) struct WindowUserData {
    webtag_key: String,
    /// Frame button currently under the cursor (client or non-client
    /// space). Only touched on the window's UI thread, hence `Cell`.
    hovered_frame_button: Cell<Option<WindowsFrameButton>>,
    /// Frame button with an in-progress left click.
    pressed_frame_button: Cell<Option<WindowsFrameButton>>,
    /// Whether `TrackMouseEvent(TME_LEAVE)` is armed for the client area.
    tracking_client_mouse: Cell<bool>,
    /// Whether `TrackMouseEvent(TME_LEAVE | TME_NONCLIENT)` is armed.
    tracking_nc_mouse: Cell<bool>,
}

impl WindowUserData {
    fn new(webtag_key: String) -> Self {
        Self {
            webtag_key,
            hovered_frame_button: Cell::new(None),
            pressed_frame_button: Cell::new(None),
            tracking_client_mouse: Cell::new(false),
            tracking_nc_mouse: Cell::new(false),
        }
    }
}

fn with_window_user_data<R>(hwnd: HWND, f: impl FnOnce(&WindowUserData) -> R) -> Option<R> {
    let raw =
        unsafe { WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA) }
            as *mut WindowUserData;
    if raw.is_null() {
        None
    } else {
        Some(f(unsafe { &*raw }))
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct WindowPlacement {
    left: i32,
    top: i32,
    width: i32,
    height: i32,
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
        let covered =
            group_active_main(&group_key).is_some_and(|active| active != webtag_key);
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
}

pub(crate) fn hwnd_handle(hwnd: HWND) -> isize {
    hwnd.0 as isize
}

pub(crate) fn hwnd_from_handle(handle: isize) -> HWND {
    HWND(handle as *mut c_void)
}

pub(crate) fn is_window_handle_valid(handle: isize) -> bool {
    unsafe { WindowsAndMessaging::IsWindow(Some(hwnd_from_handle(handle))).as_bool() }
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

pub(crate) fn attach_child_window_to_host(child: HWND, host: HWND) {
    unsafe {
        let _ = WindowsAndMessaging::SetParent(child, Some(host));
        let style =
            WindowsAndMessaging::GetWindowLongPtrW(child, WindowsAndMessaging::GWL_STYLE) as u32;
        let child_style = (style & !WS_OVERLAPPEDWINDOW.0 & !WindowsAndMessaging::WS_POPUP.0)
            | WindowsAndMessaging::WS_CHILD.0
            | WindowsAndMessaging::WS_CLIPCHILDREN.0
            | WindowsAndMessaging::WS_CLIPSIBLINGS.0;
        let _ = WindowsAndMessaging::SetWindowLongPtrW(
            child,
            WindowsAndMessaging::GWL_STYLE,
            child_style as isize,
        );
        let _ = WindowsAndMessaging::SetWindowPos(
            child,
            Some(WindowsAndMessaging::HWND_TOP),
            0,
            0,
            0,
            0,
            WindowsAndMessaging::SWP_NOMOVE
                | WindowsAndMessaging::SWP_NOSIZE
                | WindowsAndMessaging::SWP_NOACTIVATE
                | WindowsAndMessaging::SWP_NOOWNERZORDER
                | WindowsAndMessaging::SWP_FRAMECHANGED,
        );
    }
}

pub(crate) fn show_shell_host(group_key: &str, host: HWND, title: &str, activate: bool) {
    let host_visible = unsafe { WindowsAndMessaging::IsWindowVisible(host).as_bool() };
    let host_zoomed = unsafe { WindowsAndMessaging::IsZoomed(host).as_bool() };
    if !host_visible
        && !host_zoomed
        && let Some(placement) = current_group_window_placement_for_group(group_key)
    {
        unsafe {
            let _ = WindowsAndMessaging::SetWindowPos(
                host,
                None,
                placement.left,
                placement.top,
                placement.width,
                placement.height,
                WindowsAndMessaging::SWP_NOZORDER | WindowsAndMessaging::SWP_NOACTIVATE,
            );
        }
    }

    fit_window_to_work_area(host);

    // With custom chrome the renderer paints the title area itself; plain
    // OS-frame windows keep the real title and title-bar icon.
    let custom_chrome = windows_chrome_renderer().is_some();
    let title = to_wide(if custom_chrome { "" } else { title });
    unsafe {
        let _ = WindowsAndMessaging::SetWindowTextW(host, PCWSTR(title.as_ptr()));
        let mut flags = WindowsAndMessaging::SWP_NOMOVE | WindowsAndMessaging::SWP_NOSIZE;
        if !activate {
            flags |= WindowsAndMessaging::SWP_NOACTIVATE;
        }
        let _ = WindowsAndMessaging::SetWindowPos(
            host,
            None,
            0,
            0,
            0,
            0,
            flags | WindowsAndMessaging::SWP_SHOWWINDOW,
        );
        if activate {
            let _ = WindowsAndMessaging::BringWindowToTop(host);
            let _ = WindowsAndMessaging::SetForegroundWindow(host);
        }
    }
}

pub(crate) fn monitor_info_for_window(hwnd: HWND) -> Option<MONITORINFO> {
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        rcMonitor: RECT::default(),
        rcWork: RECT::default(),
        dwFlags: 0,
    };
    unsafe {
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        if GetMonitorInfoW(monitor, &mut info).as_bool() {
            Some(info)
        } else {
            None
        }
    }
}

pub(crate) fn apply_window_maximized_bounds(hwnd: HWND, lparam: LPARAM) {
    if lparam.0 == 0 {
        return;
    }
    let Some(info) = monitor_info_for_window(hwnd) else {
        return;
    };
    let work = info.rcWork;
    let monitor = info.rcMonitor;
    unsafe {
        let minmax = &mut *(lparam.0 as *mut MINMAXINFO);
        minmax.ptMaxPosition.x = work.left - monitor.left;
        minmax.ptMaxPosition.y = work.top - monitor.top;
        minmax.ptMaxSize.x = rect_width(&work);
        minmax.ptMaxSize.y = rect_height(&work);
    }
}

pub(crate) fn fit_window_to_work_area(hwnd: HWND) {
    unsafe {
        if WindowsAndMessaging::IsZoomed(hwnd).as_bool() {
            return;
        }
    }
    let Some(info) = monitor_info_for_window(hwnd) else {
        return;
    };
    let mut rect = RECT::default();
    unsafe {
        if WindowsAndMessaging::GetWindowRect(hwnd, &mut rect).is_err() {
            return;
        }
    }

    let work = info.rcWork;
    let work_width = rect_width(&work);
    let work_height = rect_height(&work);
    if work_width <= 0 || work_height <= 0 {
        return;
    }

    let min_width = 320.min(work_width);
    let min_height = 240.min(work_height);
    let width = rect_width(&rect).clamp(min_width, work_width);
    let height = rect_height(&rect).clamp(min_height, work_height);
    let max_left = work.right - width;
    let max_top = work.bottom - height;
    let left = rect.left.clamp(work.left, max_left.max(work.left));
    let top = rect.top.clamp(work.top, max_top.max(work.top));

    if left == rect.left
        && top == rect.top
        && width == rect_width(&rect)
        && height == rect_height(&rect)
    {
        return;
    }

    unsafe {
        let _ = WindowsAndMessaging::SetWindowPos(
            hwnd,
            None,
            left,
            top,
            width,
            height,
            WindowsAndMessaging::SWP_NOZORDER | WindowsAndMessaging::SWP_NOACTIVATE,
        );
    }
}

pub(crate) fn current_group_window_placement_for_group(group_key: &str) -> Option<WindowPlacement> {
    WINDOW_GROUP_PLACEMENTS
        .get()
        .and_then(|placements| placements.lock().ok())
        .and_then(|placements| placements.get(group_key).copied())
}

pub(crate) fn set_attached_window_rect(hwnd: HWND, rect: RECT, visible: bool) {
    let width = rect_width(&rect);
    let height = rect_height(&rect);
    if width == 0 || height == 0 || !visible {
        hide_attached_window(hwnd);
        return;
    }
    unsafe {
        // SWP_NOCOPYBITS: during live resizes the old surface contents must
        // not be blitted into the new position (stale-content ghosting);
        // the webview repaints the full card anyway.
        let _ = WindowsAndMessaging::SetWindowPos(
            hwnd,
            Some(WindowsAndMessaging::HWND_TOP),
            rect.left,
            rect.top,
            width,
            height,
            WindowsAndMessaging::SWP_NOACTIVATE
                | WindowsAndMessaging::SWP_NOOWNERZORDER
                | WindowsAndMessaging::SWP_NOCOPYBITS
                | WindowsAndMessaging::SWP_SHOWWINDOW,
        );
    }
    update_corner_caps(
        hwnd,
        RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        },
    );
}

pub(crate) fn hide_attached_window(hwnd: HWND) {
    destroy_corner_caps(hwnd);
    unsafe {
        let _ = WindowsAndMessaging::SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            WindowsAndMessaging::SWP_NOMOVE
                | WindowsAndMessaging::SWP_NOSIZE
                | WindowsAndMessaging::SWP_NOZORDER
                | WindowsAndMessaging::SWP_NOACTIVATE
                | WindowsAndMessaging::SWP_HIDEWINDOW,
        );
    }
}

/// Corner-cap overlays: attached cards/panels are `WS_CHILD` windows, so
/// the DWM corner rounding used for top-level windows cannot apply, and a
/// GDI window region (`SetWindowRgn`) clips to an aliased staircase edge.
/// Instead, four tiny per-pixel-alpha "cap" child windows are layered over
/// each card's corners, above the card's WebView2 child: each cap paints
/// the renderer's [`card_corner_color`](WindowsChromeRenderer::card_corner_color)
/// outside the rounded-corner arc, anti-aliased coverage along the arc, and
/// full transparency inside, visually rounding the card without clipping
/// it. Caps are input-transparent, created lazily per card window by the
/// attached layout paths, repositioned on every layout, and destroyed when
/// their card hides or goes away.
struct CornerCapSet {
    /// Cap handles ordered top-left, top-right, bottom-left, bottom-right.
    caps: [isize; 4],
    /// Cap side length (the corner radius) the bitmaps were rendered at.
    side: i32,
    /// `COLORREF` value the bitmaps were rendered with.
    color: u32,
}

/// Live corner-cap sets, keyed by the window the caps are children of (an
/// attached card window, or a group host for its own main card).
static CORNER_CAPS: OnceLock<Mutex<HashMap<isize, CornerCapSet>>> = OnceLock::new();

/// Cap windows take no input: `WS_EX_TRANSPARENT` already excludes the
/// layered caps from hit testing, and `HTTRANSPARENT` covers any hit test
/// that still reaches the window.
unsafe extern "system" fn corner_cap_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WindowsAndMessaging::WM_NCHITTEST {
        return LRESULT(WindowsAndMessaging::HTTRANSPARENT as isize);
    }
    unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn corner_cap_class() -> PCWSTR {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        // Register with the same module handle that cap creation passes to
        // `CreateWindowExW`: window classes are keyed by (name, module), so
        // a mismatched module would make every cap creation fail.
        let module = unsafe { LibraryLoader::GetModuleHandleW(None) }
            .map(|module| HINSTANCE(module.0))
            .unwrap_or_default();
        let class = WNDCLASSW {
            lpfnWndProc: Some(corner_cap_proc),
            hInstance: module,
            lpszClassName: w!("LingXiaCardCornerCap"),
            ..Default::default()
        };
        if unsafe { WindowsAndMessaging::RegisterClassW(&class) } == 0 {
            // A failed registration leaves every later cap creation failing;
            // surface it instead of silently losing the rounded corners.
            log::error!(
                "corner cap class registration failed: {}",
                windows::core::Error::from_thread()
            );
        }
    });
    w!("LingXiaCardCornerCap")
}

/// Creates (lazily) and lays out the four corner caps of one card surface.
/// `card_rect` is in `parent`'s client coordinates: the full client rect
/// for attached card windows, the controller bounds for a group host's own
/// main card. Skipped when the renderer reports no corner color (plain
/// OS-frame fallback) or no corner radius.
pub(crate) fn update_corner_caps(parent: HWND, card_rect: RECT) {
    // Cap windows are children of `parent` and must be owned by the thread
    // that owns `parent`: group layout also runs on short-lived helper
    // threads (chrome-event dispatch, async tasks), and Windows destroys a
    // thread's windows when the thread exits — caps created there silently
    // vanish moments later. Marshal the update onto the parent's UI thread.
    let owner_thread =
        unsafe { WindowsAndMessaging::GetWindowThreadProcessId(parent, None) };
    if owner_thread != 0 && owner_thread != unsafe { Threading::GetCurrentThreadId() } {
        let parent_handle = hwnd_handle(parent);
        post_to_window_thread(
            parent_handle,
            Box::new(move || update_corner_caps(hwnd_from_handle(parent_handle), card_rect)),
        );
        return;
    }
    let Some(color) = renderer_card_corner_color() else {
        return;
    };
    let side = renderer_panel_radius();
    if side <= 0 {
        return;
    }
    if rect_width(&card_rect) < side * 2 || rect_height(&card_rect) < side * 2 {
        destroy_corner_caps(parent);
        return;
    }

    let sets = CORNER_CAPS.get_or_init(|| Mutex::new(HashMap::new()));
    let existing = sets
        .lock()
        .ok()
        .and_then(|sets| {
            sets.get(&hwnd_handle(parent))
                .map(|set| (set.caps, set.side, set.color))
        })
        .filter(|(caps, cap_side, cap_color)| {
            *cap_side == side
                && *cap_color == color.0
                && caps.iter().all(|cap| is_window_handle_valid(*cap))
        });
    let caps = match existing {
        Some((caps, _, _)) => caps,
        None => {
            destroy_corner_caps(parent);
            let Some(caps) = create_corner_caps(parent, side, color) else {
                return;
            };
            if let Ok(mut sets) = sets.lock() {
                sets.insert(
                    hwnd_handle(parent),
                    CornerCapSet {
                        caps,
                        side,
                        color: color.0,
                    },
                );
            }
            log::debug!(
                "created corner caps for {:?} (side {side}, color #{:06x})",
                parent,
                color.0
            );
            caps
        }
    };

    // A main-card surface flush above a docked bottom panel keeps square
    // bottom corners: its bottom caps would notch the shared dock edge.
    let square_bottom = window_webtag_key(parent)
        .is_some_and(|webtag_key| main_surface_has_docked_bottom_panel(&webtag_key));

    let positions = [
        (card_rect.left, card_rect.top),
        (card_rect.right - side, card_rect.top),
        (card_rect.left, card_rect.bottom - side),
        (card_rect.right - side, card_rect.bottom - side),
    ];
    for (index, (cap, (x, y))) in caps.iter().zip(positions).enumerate() {
        let hide = square_bottom && index >= 2;
        unsafe {
            let _ = WindowsAndMessaging::SetWindowPos(
                hwnd_from_handle(*cap),
                Some(WindowsAndMessaging::HWND_TOP),
                x,
                y,
                side,
                side,
                WindowsAndMessaging::SWP_NOACTIVATE
                    | WindowsAndMessaging::SWP_NOOWNERZORDER
                    | WindowsAndMessaging::SWP_NOCOPYBITS
                    | if hide {
                        WindowsAndMessaging::SWP_HIDEWINDOW
                    } else {
                        WindowsAndMessaging::SWP_SHOWWINDOW
                    },
            );
        }
    }
}

/// Re-asserts the caps of `parent` at the top of its child z-order without
/// moving or resizing them. WebView2 reorders its own `Chrome_WidgetWin`
/// child chain on visibility/focus changes, which can bury the caps under
/// the webview surface; every layout pass re-asserts `HWND_TOP` through
/// [`update_corner_caps`], and the controller visibility flips call this
/// directly. Hidden caps (square-bottom dock seam) stay hidden.
pub(crate) fn raise_corner_caps(parent: HWND) {
    let caps = CORNER_CAPS
        .get()
        .and_then(|sets| sets.lock().ok())
        .and_then(|sets| sets.get(&hwnd_handle(parent)).map(|set| set.caps));
    let Some(caps) = caps else {
        return;
    };
    for cap in caps {
        unsafe {
            let _ = WindowsAndMessaging::SetWindowPos(
                hwnd_from_handle(cap),
                Some(WindowsAndMessaging::HWND_TOP),
                0,
                0,
                0,
                0,
                WindowsAndMessaging::SWP_NOMOVE
                    | WindowsAndMessaging::SWP_NOSIZE
                    | WindowsAndMessaging::SWP_NOACTIVATE
                    | WindowsAndMessaging::SWP_NOOWNERZORDER,
            );
        }
    }
}

/// Creates the four layered cap windows of one card and renders their
/// per-pixel-alpha bitmaps. Returns `None` (destroying any partial set)
/// when a window fails to create.
fn create_corner_caps(parent: HWND, side: i32, color: COLORREF) -> Option<[isize; 4]> {
    let class = corner_cap_class();
    let mut caps = [0isize; 4];
    for corner in 0..4 {
        let result = unsafe {
            WindowsAndMessaging::CreateWindowExW(
                WindowsAndMessaging::WS_EX_LAYERED
                    | WindowsAndMessaging::WS_EX_TRANSPARENT
                    | WindowsAndMessaging::WS_EX_NOACTIVATE,
                class,
                PCWSTR::null(),
                WindowsAndMessaging::WS_CHILD,
                0,
                0,
                side,
                side,
                Some(parent),
                None,
                LibraryLoader::GetModuleHandleW(None)
                    .ok()
                    .map(|module| HINSTANCE(module.0)),
                None,
            )
        };
        let cap = match result {
            Ok(cap) => cap,
            Err(err) => {
                log::warn!("corner cap creation failed for {parent:?}: {err}");
                for created in &caps[..corner] {
                    unsafe {
                        let _ = WindowsAndMessaging::DestroyWindow(hwnd_from_handle(*created));
                    }
                }
                return None;
            }
        };
        paint_corner_cap(cap, corner, side, color);
        caps[corner] = hwnd_handle(cap);
    }
    Some(caps)
}

/// Uploads one cap's premultiplied 32-bit ARGB bitmap via
/// `UpdateLayeredWindow` (`ULW_ALPHA`): opaque `color` outside the
/// quarter-circle arc, anti-aliased coverage along it, transparent inside.
fn paint_corner_cap(cap: HWND, corner: usize, side: i32, color: COLORREF) {
    let pixels = corner_cap_pixels(corner, side, color);
    unsafe {
        let screen_dc = GetDC(None);
        if screen_dc.is_invalid() {
            return;
        }
        let memory_dc = CreateCompatibleDC(Some(screen_dc));
        if !memory_dc.is_invalid() {
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: side,
                    // Negative height: top-down rows, matching `pixels`.
                    biHeight: -side,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits: *mut c_void = std::ptr::null_mut();
            if let Ok(bitmap) =
                CreateDIBSection(Some(screen_dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)
                && !bits.is_null()
            {
                std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits.cast::<u32>(), pixels.len());
                let old_bitmap = SelectObject(memory_dc, HGDIOBJ(bitmap.0));
                let size = SIZE { cx: side, cy: side };
                let origin = POINT { x: 0, y: 0 };
                let blend = BLENDFUNCTION {
                    BlendOp: AC_SRC_OVER as u8,
                    BlendFlags: 0,
                    SourceConstantAlpha: 255,
                    AlphaFormat: AC_SRC_ALPHA as u8,
                };
                let _ = WindowsAndMessaging::UpdateLayeredWindow(
                    cap,
                    None,
                    None,
                    Some(&size),
                    Some(memory_dc),
                    Some(&origin),
                    COLORREF(0),
                    Some(&blend),
                    WindowsAndMessaging::ULW_ALPHA,
                );
                if !old_bitmap.is_invalid() {
                    let _ = SelectObject(memory_dc, old_bitmap);
                }
                let _ = DeleteObject(HGDIOBJ(bitmap.0));
            }
            let _ = DeleteDC(memory_dc);
        }
        let _ = ReleaseDC(None, screen_dc);
    }
}

/// Premultiplied ARGB pixels of one corner cap, top-down row order.
/// `corner`: 0 top-left, 1 top-right, 2 bottom-left, 3 bottom-right. Alpha
/// is the 4x4-supersampled coverage of "outside the rounded corner", so
/// the arc edge blends smoothly between the cap color and the webview
/// pixels underneath.
fn corner_cap_pixels(corner: usize, side: i32, color: COLORREF) -> Vec<u32> {
    let radius = side as f32;
    // Arc center in cap-local coordinates: the cap corner that points into
    // the card interior.
    let (center_x, center_y) = match corner {
        0 => (radius, radius),
        1 => (0.0, radius),
        2 => (radius, 0.0),
        _ => (0.0, 0.0),
    };
    let red = color.0 & 0xff;
    let green = (color.0 >> 8) & 0xff;
    let blue = (color.0 >> 16) & 0xff;
    let mut pixels = Vec::with_capacity((side * side) as usize);
    for y in 0..side {
        for x in 0..side {
            let mut outside = 0u32;
            for sub_y in 0..4 {
                for sub_x in 0..4 {
                    let sample_x = x as f32 + (sub_x as f32 + 0.5) / 4.0;
                    let sample_y = y as f32 + (sub_y as f32 + 0.5) / 4.0;
                    let dx = sample_x - center_x;
                    let dy = sample_y - center_y;
                    if dx * dx + dy * dy > radius * radius {
                        outside += 1;
                    }
                }
            }
            let alpha = outside * 255 / 16;
            let premultiply = |channel: u32| channel * alpha / 255;
            pixels.push(
                (alpha << 24)
                    | (premultiply(red) << 16)
                    | (premultiply(green) << 8)
                    | premultiply(blue),
            );
        }
    }
    pixels
}

/// Destroys the caps of one card and forgets its registry entry. Group
/// layout runs on whichever UI thread triggered it, so a cap may belong to
/// a different thread than the caller; `DestroyWindow` fails cross-thread,
/// and those caps are instead closed via `WM_CLOSE` on their owning thread.
pub(crate) fn destroy_corner_caps(parent: HWND) {
    let removed = CORNER_CAPS
        .get()
        .and_then(|sets| sets.lock().ok())
        .and_then(|mut sets| sets.remove(&hwnd_handle(parent)));
    let Some(set) = removed else {
        return;
    };
    log::debug!("destroying corner caps for {parent:?}");
    for cap in set.caps {
        let cap = hwnd_from_handle(cap);
        unsafe {
            if WindowsAndMessaging::DestroyWindow(cap).is_err() {
                let _ = WindowsAndMessaging::PostMessageW(
                    Some(cap),
                    WindowsAndMessaging::WM_CLOSE,
                    WPARAM::default(),
                    LPARAM::default(),
                );
            }
        }
    }
}

/// Drops the per-window layout caches and corner caps of a window that is
/// going away.
pub(crate) fn forget_window_layout_state(hwnd: HWND) {
    destroy_corner_caps(hwnd);
    let key = hwnd_handle(hwnd);
    if let Some(rects) = LAST_WINDOW_RECTS.get()
        && let Ok(mut rects) = rects.lock()
    {
        rects.remove(&key);
    }
    if let Some(bounds) = LAST_CONTROLLER_BOUNDS.get()
        && let Ok(mut bounds) = bounds.lock()
    {
        bounds.remove(&key);
    }
}

pub(crate) fn create_hidden_window(webtag: &WebTag) -> StdResult<HWND> {
    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if msg == WM_NCCREATE {
            let create = lparam.0 as *const CREATESTRUCTW;
            if !create.is_null() {
                let user_data = unsafe { (*create).lpCreateParams } as *mut WindowUserData;
                unsafe {
                    let _ = WindowsAndMessaging::SetWindowLongPtrW(
                        hwnd,
                        WindowsAndMessaging::GWLP_USERDATA,
                        user_data as isize,
                    );
                }
            }
        } else if msg == WindowsAndMessaging::WM_GETMINMAXINFO {
            // Custom-chrome (borderless) windows compute maximized bounds
            // themselves; plain OS-frame windows use default handling.
            if windows_chrome_renderer().is_some() {
                apply_window_maximized_bounds(hwnd, lparam);
                return LRESULT(0);
            }
        } else if msg == WindowsAndMessaging::WM_NCCALCSIZE {
            if windows_chrome_renderer().is_some() {
                return LRESULT(0);
            }
        } else if msg == WindowsAndMessaging::WM_NCHITTEST {
            if windows_chrome_renderer().is_some() {
                let raw = unsafe {
                    WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA)
                } as *mut WindowUserData;
                if !raw.is_null() {
                    let result =
                        handle_window_frame_hit_test(hwnd, unsafe { &(*raw).webtag_key }, lparam);
                    return LRESULT(result as isize);
                }
            }
        } else if msg == WindowsAndMessaging::WM_ERASEBKGND {
            if windows_chrome_renderer().is_some() {
                return LRESULT(1);
            }
        } else if msg == WindowsAndMessaging::WM_WINDOWPOSCHANGED {
            // Interactive move/size runs inside DefWindowProc's modal loop
            // where the command queue and posted thread messages are starved,
            // so layout must track the drag live from this message path.
            let pos = lparam.0 as *const WindowsAndMessaging::WINDOWPOS;
            if !pos.is_null() {
                let flags = unsafe { (*pos).flags };
                let sized = !flags.contains(WindowsAndMessaging::SWP_NOSIZE)
                    || flags.contains(WindowsAndMessaging::SWP_FRAMECHANGED);
                let moved = !flags.contains(WindowsAndMessaging::SWP_NOMOVE);
                if sized || moved {
                    handle_window_geometry_change(hwnd);
                }
                if sized && windows_chrome_renderer().is_some() {
                    // Chrome elements are anchored to the client edges, so a
                    // size change must repaint the whole window, not just the
                    // newly exposed strip.
                    unsafe {
                        let _ = InvalidateRect(Some(hwnd), None, false);
                    }
                }
            }
            // Fall through so DefWindowProc still produces WM_SIZE/WM_MOVE.
        } else if msg == WindowsAndMessaging::WM_ENTERSIZEMOVE {
            // WM_WINDOWPOSCHANGED is coalesced inside DefWindowProc's modal
            // move/size loop, so a real mouse drag can outrun the layout;
            // timers still fire inside that loop and keep layout pumping.
            unsafe {
                let _ = WindowsAndMessaging::SetTimer(
                    Some(hwnd),
                    SIZEMOVE_TIMER_ID,
                    SIZEMOVE_TIMER_INTERVAL_MS,
                    None,
                );
            }
        } else if msg == WindowsAndMessaging::WM_EXITSIZEMOVE {
            unsafe {
                let _ = WindowsAndMessaging::KillTimer(Some(hwnd), SIZEMOVE_TIMER_ID);
            }
            handle_window_geometry_change(hwnd);
            unsafe {
                let _ = InvalidateRect(Some(hwnd), None, false);
            }
        } else if msg == WindowsAndMessaging::WM_TIMER {
            if wparam.0 == SIZEMOVE_TIMER_ID {
                handle_live_sizemove_tick(hwnd);
                return LRESULT(0);
            }
        } else if msg == WM_LINGXIA_LAYOUT {
            handle_window_geometry_change(hwnd);
            return LRESULT(0);
        } else if msg == WM_LINGXIA_RUN_CALLBACK {
            // Closure marshalled from another thread via
            // `post_to_window_thread` (e.g. a product layer creating child
            // controls that must live on this UI thread).
            run_posted_window_callback(wparam);
            return LRESULT(0);
        } else if msg == WindowsAndMessaging::WM_PAINT {
            if windows_chrome_renderer().is_some() {
                paint_window_chrome(hwnd);
                return LRESULT(0);
            }
        } else if msg == WindowsAndMessaging::WM_CHAR {
            if handle_native_panel_char(wparam) {
                return LRESULT(0);
            }
        } else if msg == WindowsAndMessaging::WM_KEYDOWN {
            if handle_native_panel_keydown(wparam) {
                return LRESULT(0);
            }
        } else if msg == WindowsAndMessaging::WM_LBUTTONDOWN
            || msg == WindowsAndMessaging::WM_LBUTTONDBLCLK
        {
            // CS_DBLCLKS turns the second press of a double-click into
            // WM_LBUTTONDBLCLK; a native-panel tab maps it to a rename
            // request, everything else keeps plain button-down handling so
            // fast double clicks on dividers/buttons behave like clicks.
            let raw = unsafe {
                WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA)
            } as *mut WindowUserData;
            if !raw.is_null() {
                let webtag_key = unsafe { &(*raw).webtag_key };
                let point = lparam_to_point(lparam);
                if msg == WindowsAndMessaging::WM_LBUTTONDBLCLK
                    && handle_window_chrome_double_click(hwnd, webtag_key, point)
                {
                    return LRESULT(0);
                }
                if handle_window_chrome_mouse_down(hwnd, webtag_key, point)
                    || handle_frame_button_mouse_down(hwnd, webtag_key, point)
                {
                    return LRESULT(0);
                }
            }
        } else if msg == WindowsAndMessaging::WM_RBUTTONDOWN {
            let raw = unsafe {
                WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA)
            } as *mut WindowUserData;
            if !raw.is_null() {
                let webtag_key = unsafe { &(*raw).webtag_key };
                if handle_window_chrome_right_click(hwnd, webtag_key, lparam_to_point(lparam)) {
                    return LRESULT(0);
                }
            }
        } else if msg == WindowsAndMessaging::WM_MOUSEMOVE {
            handle_frame_button_client_mouse_move(hwnd, lparam_to_point(lparam));
            if handle_window_chrome_mouse_move(hwnd, lparam_to_point(lparam)) {
                return LRESULT(0);
            }
        } else if msg == WM_MOUSELEAVE {
            handle_frame_button_client_mouse_leave(hwnd);
        } else if msg == WindowsAndMessaging::WM_NCMOUSEMOVE {
            handle_frame_button_nc_mouse_move(hwnd, wparam.0 as u32);
        } else if msg == WindowsAndMessaging::WM_NCMOUSELEAVE {
            handle_frame_button_nc_mouse_leave(hwnd);
        } else if msg == WindowsAndMessaging::WM_NCLBUTTONDOWN
            || msg == WindowsAndMessaging::WM_NCLBUTTONDBLCLK
            || msg == WindowsAndMessaging::WM_NCLBUTTONUP
        {
            if handle_frame_button_nc_button(hwnd, msg, wparam.0 as u32) {
                return LRESULT(0);
            }
        } else if msg == WindowsAndMessaging::WM_LBUTTONUP {
            if handle_window_chrome_mouse_up(hwnd) {
                return LRESULT(0);
            }
            let raw = unsafe {
                WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA)
            } as *mut WindowUserData;
            if !raw.is_null() {
                let webtag_key = unsafe { &(*raw).webtag_key };
                let point = lparam_to_point(lparam);
                if handle_frame_button_mouse_up(hwnd, webtag_key, point)
                    || handle_window_chrome_click(hwnd, webtag_key, point)
                {
                    return LRESULT(0);
                }
            }
        } else if msg == WindowsAndMessaging::WM_CLOSE {
            let raw = unsafe {
                WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA)
            } as *mut WindowUserData;
            if !raw.is_null() && invoke_close_handler(unsafe { &(*raw).webtag_key }) {
                return LRESULT(0);
            }
            unsafe {
                let _ = WindowsAndMessaging::DestroyWindow(hwnd);
            }
            return LRESULT(0);
        } else if msg == WindowsAndMessaging::WM_DESTROY {
            unsafe {
                WindowsAndMessaging::PostQuitMessage(0);
            }
            return LRESULT(0);
        } else if msg == WindowsAndMessaging::WM_NCDESTROY {
            let raw = unsafe {
                WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA)
            } as *mut WindowUserData;
            if !raw.is_null() {
                unsafe {
                    let _ = Box::from_raw(raw);
                    let _ = WindowsAndMessaging::SetWindowLongPtrW(
                        hwnd,
                        WindowsAndMessaging::GWLP_USERDATA,
                        0,
                    );
                }
            }
        }
        unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
    }

    let app_icons = current_app_icon_handles();
    let class = WNDCLASSW {
        // CS_HREDRAW | CS_VREDRAW: a resize invalidates the whole window,
        // not just the exposed strip — the chrome is anchored to all client
        // edges, and stale strips would otherwise linger during live drags.
        // CS_DBLCLKS: native-panel tab titles are renamed via double-click.
        style: WindowsAndMessaging::CS_HREDRAW
            | WindowsAndMessaging::CS_VREDRAW
            | WindowsAndMessaging::CS_DBLCLKS,
        lpfnWndProc: Some(window_proc),
        hIcon: app_icons
            .map(|icons| hicon(icons.large))
            .unwrap_or_default(),
        lpszClassName: w!("LingXiaHiddenWebViewHost"),
        ..Default::default()
    };

    unsafe {
        WindowsAndMessaging::RegisterClassW(&class);
        let user_data = Box::new(WindowUserData::new(webtag.key().to_string()));
        let user_data_ptr = Box::into_raw(user_data);

        // Both modes keep the WS_OVERLAPPEDWINDOW styles. With a registered
        // chrome renderer the renderer paints the whole frame: the standard
        // styles (WS_THICKFRAME | WS_CAPTION) stay so DWM keeps drawing the
        // drop shadow and Win11 snap keeps working, while the visible frame
        // is removed in WM_NCCALCSIZE (client covers the window) and DWM is
        // extended 1px into the client area after creation. Without a
        // renderer the standard OS frame is left untouched.
        let window_style = WS_OVERLAPPEDWINDOW;
        let result = WindowsAndMessaging::CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("LingXiaHiddenWebViewHost"),
            w!("LingXiaHiddenWebViewHost"),
            window_style,
            WindowsAndMessaging::CW_USEDEFAULT,
            WindowsAndMessaging::CW_USEDEFAULT,
            1024,
            768,
            None,
            None,
            LibraryLoader::GetModuleHandleW(None)
                .ok()
                .map(|module| HINSTANCE(module.0)),
            Some(user_data_ptr.cast()),
        );

        match result {
            Ok(hwnd) => {
                if let Some(icons) = app_icons {
                    apply_window_icons(hwnd, icons);
                }
                if windows_chrome_renderer().is_some() {
                    extend_frame_into_client_area(hwnd);
                    apply_round_corner_preference(hwnd);
                }
                Ok(hwnd)
            }
            Err(err) => {
                let _ = Box::from_raw(user_data_ptr);
                Err(WebViewError::WebView(format!(
                    "CreateWindowExW failed: {err}"
                )))
            }
        }
    }
}

/// Standard custom-frame setup: WM_NCCALCSIZE already makes the client area
/// cover the whole window, so extend the DWM frame 1px into the top of the
/// client area to keep the DWM drop shadow (and Win11 rounded corners) on a
/// window without a visible non-client frame, then force WM_NCCALCSIZE so
/// the borderless client area applies immediately.
pub(crate) fn extend_frame_into_client_area(hwnd: HWND) {
    let margins = MARGINS {
        cxLeftWidth: 0,
        cxRightWidth: 0,
        cyTopHeight: 1,
        cyBottomHeight: 0,
    };
    unsafe {
        let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);
        let _ = WindowsAndMessaging::SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            WindowsAndMessaging::SWP_NOMOVE
                | WindowsAndMessaging::SWP_NOSIZE
                | WindowsAndMessaging::SWP_NOZORDER
                | WindowsAndMessaging::SWP_NOACTIVATE
                | WindowsAndMessaging::SWP_FRAMECHANGED,
        );
    }
}

/// Opts a top-level window into DWM-rounded corners (Win11): unlike a GDI
/// window region, DWM rounding is anti-aliased and keeps the drop shadow.
/// Top-level windows must therefore never get `SetWindowRgn` (a region
/// disables DWM corner rounding); attached child surfaces — where DWM
/// rounding cannot apply — are rounded visually by the corner-cap overlays
/// instead (see [`update_corner_caps`]).
pub(crate) fn apply_round_corner_preference(hwnd: HWND) {
    let preference = DWMWCP_ROUND;
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&preference as *const _) as *const c_void,
            std::mem::size_of_val(&preference) as u32,
        );
    }
}

pub(crate) fn normalize_rect(mut rect: RECT) -> RECT {
    if rect.right < rect.left {
        rect.right = rect.left;
    }
    if rect.bottom < rect.top {
        rect.bottom = rect.top;
    }
    rect
}

pub(crate) fn rect_width(rect: &RECT) -> i32 {
    (rect.right - rect.left).max(0)
}

pub(crate) fn rect_height(rect: &RECT) -> i32 {
    (rect.bottom - rect.top).max(0)
}

pub(crate) fn rect_contains(rect: &RECT, point: (i32, i32)) -> bool {
    point.0 >= rect.left && point.0 < rect.right && point.1 >= rect.top && point.1 < rect.bottom
}

pub(crate) fn lparam_to_point(lparam: LPARAM) -> (i32, i32) {
    let value = lparam.0 as u32;
    let x = (value & 0xffff) as i16 as i32;
    let y = ((value >> 16) & 0xffff) as i16 as i32;
    (x, y)
}

pub(crate) fn lparam_screen_to_client(hwnd: HWND, lparam: LPARAM) -> (i32, i32) {
    let (x, y) = lparam_to_point(lparam);
    let mut point = POINT { x, y };
    unsafe {
        let _ = ScreenToClient(hwnd, &mut point);
    }
    (point.x, point.y)
}

pub(crate) fn handle_window_frame_hit_test(hwnd: HWND, webtag_key: &str, lparam: LPARAM) -> u32 {
    if !window_draws_shell_chrome(webtag_key) {
        return WindowsAndMessaging::HTCLIENT;
    }

    let mut client = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetClientRect(hwnd, &mut client);
    }
    let point = lparam_screen_to_client(hwnd, lparam);
    if let Some(hit) = window_resize_hit_test(hwnd, client, point) {
        return hit;
    }

    let Some(renderer) = windows_chrome_renderer() else {
        return WindowsAndMessaging::HTCLIENT;
    };
    let state = chrome_state_for_window(hwnd, webtag_key);
    match renderer.hit_test(&state, point) {
        Some(WindowsChromeHit::Caption) => WindowsAndMessaging::HTCAPTION,
        // Win11 Snap Layouts: the flyout only appears when WM_NCHITTEST
        // reports HTMAXBUTTON over the maximize button. DefWindowProc does
        // not click client-drawn snap buttons, so the click itself is
        // performed in WM_NCLBUTTONDOWN/WM_NCLBUTTONUP. Minimize and close
        // stay HTCLIENT and keep their client-message click handling.
        Some(WindowsChromeHit::FrameButton(WindowsFrameButton::Maximize)) => {
            WindowsAndMessaging::HTMAXBUTTON
        }
        _ => WindowsAndMessaging::HTCLIENT,
    }
}

/// Hover/pressed frame-button state of a window, surfaced to the chrome
/// renderer through [`WindowsChromeState`].
pub(crate) fn frame_button_visual_state(
    hwnd: HWND,
) -> (Option<WindowsFrameButton>, Option<WindowsFrameButton>) {
    with_window_user_data(hwnd, |data| {
        (
            data.hovered_frame_button.get(),
            data.pressed_frame_button.get(),
        )
    })
    .unwrap_or((None, None))
}

/// Invalidates just the rect of one frame button (no full-window flicker on
/// hover changes). Falls back to a full invalidation when the renderer does
/// not expose button rects.
fn invalidate_frame_button(hwnd: HWND, button: WindowsFrameButton) {
    let Some(renderer) = windows_chrome_renderer() else {
        return;
    };
    let Some(webtag_key) = window_webtag_key(hwnd) else {
        return;
    };
    let state = chrome_state_for_window(hwnd, &webtag_key);
    match renderer.frame_button_rect(&state, button) {
        Some(rect) => unsafe {
            let _ = InvalidateRect(Some(hwnd), Some(&rect), false);
        },
        None => unsafe {
            let _ = InvalidateRect(Some(hwnd), None, false);
        },
    }
}

fn set_frame_button_hover(hwnd: HWND, hovered: Option<WindowsFrameButton>) {
    let previous = with_window_user_data(hwnd, |data| data.hovered_frame_button.replace(hovered));
    let Some(previous) = previous else {
        return;
    };
    if previous == hovered {
        return;
    }
    if let Some(button) = previous {
        invalidate_frame_button(hwnd, button);
    }
    if let Some(button) = hovered {
        invalidate_frame_button(hwnd, button);
    }
}

fn set_frame_button_pressed(hwnd: HWND, pressed: Option<WindowsFrameButton>) {
    let previous = with_window_user_data(hwnd, |data| data.pressed_frame_button.replace(pressed));
    let Some(previous) = previous else {
        return;
    };
    if previous == pressed {
        return;
    }
    if let Some(button) = previous {
        invalidate_frame_button(hwnd, button);
    }
    if let Some(button) = pressed {
        invalidate_frame_button(hwnd, button);
    }
}

/// Arms `TrackMouseEvent` so the window receives WM_MOUSELEAVE (client) or
/// WM_NCMOUSELEAVE (non-client) once, deduplicated per window via the
/// tracking flags in [`WindowUserData`].
fn begin_mouse_tracking(hwnd: HWND, nonclient: bool) {
    let already = with_window_user_data(hwnd, |data| {
        if nonclient {
            data.tracking_nc_mouse.replace(true)
        } else {
            data.tracking_client_mouse.replace(true)
        }
    })
    .unwrap_or(true);
    if already {
        return;
    }
    let mut track = TRACKMOUSEEVENT {
        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
        dwFlags: if nonclient {
            TME_LEAVE | TME_NONCLIENT
        } else {
            TME_LEAVE
        },
        hwndTrack: hwnd,
        dwHoverTime: 0,
    };
    unsafe {
        let _ = TrackMouseEvent(&mut track);
    }
}

/// Frame-button element under a client-space point, or `None`.
fn frame_button_at_point(
    hwnd: HWND,
    webtag_key: &str,
    point: (i32, i32),
) -> Option<WindowsFrameButton> {
    if !window_draws_shell_chrome(webtag_key) {
        return None;
    }
    let renderer = windows_chrome_renderer()?;
    let state = chrome_state_for_window(hwnd, webtag_key);
    match renderer.hit_test(&state, point) {
        Some(WindowsChromeHit::FrameButton(button)) => Some(button),
        _ => None,
    }
}

/// WM_MOUSEMOVE path: tracks hover for the client-handled frame buttons
/// (minimize/close; the maximize button lives in non-client space).
pub(crate) fn handle_frame_button_client_mouse_move(hwnd: HWND, point: (i32, i32)) {
    if windows_chrome_renderer().is_none() {
        return;
    }
    let Some(webtag_key) = window_webtag_key(hwnd) else {
        return;
    };
    let hovered = frame_button_at_point(hwnd, &webtag_key, point);
    set_frame_button_hover(hwnd, hovered);
    if hovered.is_some() {
        begin_mouse_tracking(hwnd, false);
    }
}

/// WM_NCMOUSEMOVE path: the maximize button reports HTMAXBUTTON from
/// WM_NCHITTEST, so its hover updates arrive as non-client mouse moves.
pub(crate) fn handle_frame_button_nc_mouse_move(hwnd: HWND, hit_code: u32) {
    if windows_chrome_renderer().is_none() {
        return;
    }
    if hit_code == WindowsAndMessaging::HTMAXBUTTON {
        set_frame_button_hover(hwnd, Some(WindowsFrameButton::Maximize));
        begin_mouse_tracking(hwnd, true);
    } else if frame_button_visual_state(hwnd).0 == Some(WindowsFrameButton::Maximize) {
        set_frame_button_hover(hwnd, None);
    }
}

/// WM_MOUSELEAVE: clears hover for client-tracked buttons only; the maximize
/// button is cleared by WM_NCMOUSELEAVE (the cursor moving from a client
/// button onto the maximize button produces WM_MOUSELEAVE after the
/// non-client move already set the new hover).
pub(crate) fn handle_frame_button_client_mouse_leave(hwnd: HWND) {
    with_window_user_data(hwnd, |data| data.tracking_client_mouse.set(false));
    if frame_button_visual_state(hwnd).0 != Some(WindowsFrameButton::Maximize) {
        set_frame_button_hover(hwnd, None);
    }
}

/// WM_NCMOUSELEAVE: clears maximize-button hover/pressed state.
pub(crate) fn handle_frame_button_nc_mouse_leave(hwnd: HWND) {
    with_window_user_data(hwnd, |data| data.tracking_nc_mouse.set(false));
    let (hovered, pressed) = frame_button_visual_state(hwnd);
    if hovered == Some(WindowsFrameButton::Maximize) {
        set_frame_button_hover(hwnd, None);
    }
    if pressed == Some(WindowsFrameButton::Maximize) {
        set_frame_button_pressed(hwnd, None);
    }
}

/// WM_LBUTTONDOWN on a client-handled frame button: records the pressed
/// state for painting and captures the mouse so the release is seen even
/// when it happens outside the button.
pub(crate) fn handle_frame_button_mouse_down(
    hwnd: HWND,
    webtag_key: &str,
    point: (i32, i32),
) -> bool {
    let Some(button) = frame_button_at_point(hwnd, webtag_key, point) else {
        return false;
    };
    set_frame_button_pressed(hwnd, Some(button));
    unsafe {
        let _ = SetCapture(hwnd);
    }
    true
}

/// WM_LBUTTONUP with a pressed frame button: executes the button only when
/// the release still lands on it (standard button-cancel semantics).
pub(crate) fn handle_frame_button_mouse_up(
    hwnd: HWND,
    webtag_key: &str,
    point: (i32, i32),
) -> bool {
    let (_, pressed) = frame_button_visual_state(hwnd);
    let Some(button) = pressed else {
        return false;
    };
    set_frame_button_pressed(hwnd, None);
    unsafe {
        let _ = ReleaseCapture();
    }
    if frame_button_at_point(hwnd, webtag_key, point) == Some(button) {
        handle_window_frame_button(hwnd, button);
    }
    true
}

/// WM_NCLBUTTONDOWN/WM_NCLBUTTONUP for HTMAXBUTTON: DefWindowProc does not
/// click client-drawn snap buttons, so the maximize/restore click is
/// performed here. Returns `true` when the message was consumed.
pub(crate) fn handle_frame_button_nc_button(hwnd: HWND, msg: u32, hit_code: u32) -> bool {
    if windows_chrome_renderer().is_none() {
        return false;
    }
    match msg {
        WindowsAndMessaging::WM_NCLBUTTONDOWN | WindowsAndMessaging::WM_NCLBUTTONDBLCLK => {
            if hit_code != WindowsAndMessaging::HTMAXBUTTON {
                return false;
            }
            set_frame_button_pressed(hwnd, Some(WindowsFrameButton::Maximize));
            true
        }
        WindowsAndMessaging::WM_NCLBUTTONUP => {
            if frame_button_visual_state(hwnd).1 != Some(WindowsFrameButton::Maximize) {
                return false;
            }
            set_frame_button_pressed(hwnd, None);
            if hit_code == WindowsAndMessaging::HTMAXBUTTON {
                handle_window_frame_button(hwnd, WindowsFrameButton::Maximize);
            }
            true
        }
        _ => false,
    }
}

pub(crate) fn window_resize_hit_test(hwnd: HWND, client: RECT, point: (i32, i32)) -> Option<u32> {
    if unsafe { WindowsAndMessaging::IsZoomed(hwnd).as_bool() } {
        return None;
    }
    let border = resize_border_thickness();
    let left = point.0 >= client.left && point.0 < client.left + border;
    let right = point.0 < client.right && point.0 >= client.right - border;
    let top = point.1 >= client.top && point.1 < client.top + border;
    let bottom = point.1 < client.bottom && point.1 >= client.bottom - border;

    match (left, right, top, bottom) {
        (true, _, true, _) => Some(WindowsAndMessaging::HTTOPLEFT),
        (_, true, true, _) => Some(WindowsAndMessaging::HTTOPRIGHT),
        (true, _, _, true) => Some(WindowsAndMessaging::HTBOTTOMLEFT),
        (_, true, _, true) => Some(WindowsAndMessaging::HTBOTTOMRIGHT),
        (_, _, true, _) => Some(WindowsAndMessaging::HTTOP),
        (_, _, _, true) => Some(WindowsAndMessaging::HTBOTTOM),
        (true, _, _, _) => Some(WindowsAndMessaging::HTLEFT),
        (_, true, _, _) => Some(WindowsAndMessaging::HTRIGHT),
        _ => None,
    }
}

pub(crate) fn resize_border_thickness() -> i32 {
    unsafe {
        let frame = WindowsAndMessaging::GetSystemMetrics(WindowsAndMessaging::SM_CXFRAME);
        let padded = WindowsAndMessaging::GetSystemMetrics(WindowsAndMessaging::SM_CXPADDEDBORDER);
        (frame + padded).max(6)
    }
}

pub(crate) fn window_draws_shell_chrome(webtag_key: &str) -> bool {
    !matches!(
        window_attachment(webtag_key).map(|attachment| attachment.kind),
        Some(WindowAttachmentKind::MainChild | WindowAttachmentKind::Panel { .. })
    )
}

pub(crate) fn window_webtag_key(hwnd: HWND) -> Option<String> {
    let raw =
        unsafe { WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWLP_USERDATA) }
            as *mut WindowUserData;
    if raw.is_null() {
        None
    } else {
        Some(unsafe { (*raw).webtag_key.clone() })
    }
}

pub(crate) fn handle_window_frame_button(hwnd: HWND, button: WindowsFrameButton) {
    unsafe {
        match button {
            WindowsFrameButton::Minimize => {
                let _ = WindowsAndMessaging::ShowWindow(hwnd, WindowsAndMessaging::SW_MINIMIZE);
            }
            WindowsFrameButton::Maximize => {
                let cmd = if WindowsAndMessaging::IsZoomed(hwnd).as_bool() {
                    WindowsAndMessaging::SW_RESTORE
                } else {
                    WindowsAndMessaging::SW_MAXIMIZE
                };
                let _ = WindowsAndMessaging::ShowWindow(hwnd, cmd);
            }
            WindowsFrameButton::Close => {
                let _ = WindowsAndMessaging::SendMessageW(
                    hwnd,
                    WindowsAndMessaging::WM_CLOSE,
                    None,
                    None,
                );
            }
        }
    }
}

pub(crate) fn handle_window_chrome_click(hwnd: HWND, webtag_key: &str, point: (i32, i32)) -> bool {
    if !window_draws_shell_chrome(webtag_key) {
        return false;
    }
    let Some(renderer) = windows_chrome_renderer() else {
        return false;
    };
    let state = chrome_state_for_window(hwnd, webtag_key);

    match renderer.hit_test(&state, point) {
        Some(WindowsChromeHit::FrameButton(button)) => {
            handle_window_frame_button(hwnd, button);
            true
        }
        Some(WindowsChromeHit::NativePanel { panel_id }) => {
            set_active_native_panel(Some(panel_id));
            unsafe {
                let _ = SetFocus(Some(hwnd));
            }
            true
        }
        Some(WindowsChromeHit::NavigationBack) => {
            invoke_chrome_event_handler(webtag_key, WindowsChromeEvent::NavigationBack)
        }
        Some(WindowsChromeHit::NavigationHome) => {
            invoke_chrome_event_handler(webtag_key, WindowsChromeEvent::NavigationHome)
        }
        Some(WindowsChromeHit::PanelActivator { panel_id }) => invoke_chrome_event_handler(
            webtag_key,
            WindowsChromeEvent::PanelActivatorClick { panel_id },
        ),
        Some(WindowsChromeHit::TabBarItem { index }) => {
            invoke_chrome_event_handler(webtag_key, WindowsChromeEvent::TabBarClick { index })
        }
        Some(WindowsChromeHit::BrowserNewTab) => {
            invoke_chrome_event_handler(webtag_key, WindowsChromeEvent::BrowserNewTabClick)
        }
        Some(WindowsChromeHit::BrowserTab { tab_id }) => {
            invoke_chrome_event_handler(webtag_key, WindowsChromeEvent::BrowserTabClick { tab_id })
        }
        Some(WindowsChromeHit::BrowserTabClose { tab_id }) => invoke_chrome_event_handler(
            webtag_key,
            WindowsChromeEvent::BrowserTabCloseClick { tab_id },
        ),
        Some(WindowsChromeHit::NativePanelTab { panel_id, tab_id }) => {
            // Switching tabs keeps keyboard input flowing into the panel.
            set_active_native_panel(Some(panel_id.clone()));
            unsafe {
                let _ = SetFocus(Some(hwnd));
            }
            invoke_chrome_event_handler(
                webtag_key,
                WindowsChromeEvent::NativePanelTabClick { panel_id, tab_id },
            )
        }
        Some(WindowsChromeHit::NativePanelTabClose { panel_id, tab_id }) => {
            invoke_chrome_event_handler(
                webtag_key,
                WindowsChromeEvent::NativePanelTabCloseClick { panel_id, tab_id },
            )
        }
        Some(WindowsChromeHit::NativePanelNewTab { panel_id }) => invoke_chrome_event_handler(
            webtag_key,
            WindowsChromeEvent::NativePanelNewTabClick { panel_id },
        ),
        Some(WindowsChromeHit::NativePanelMaximize { panel_id }) => invoke_chrome_event_handler(
            webtag_key,
            WindowsChromeEvent::NativePanelMaximizeClick { panel_id },
        ),
        Some(WindowsChromeHit::Chrome) => true,
        // Caption points never arrive as client clicks (WM_NCHITTEST maps
        // them to HTCAPTION first); treat defensively as unhandled.
        Some(WindowsChromeHit::Caption) | None => false,
    }
}

/// WM_LBUTTONDBLCLK on chrome: double-clicking the ACTIVE tab of a native
/// panel requests an inline rename; an inactive tab is treated as a plain
/// tab click. Returns `false` for all other chrome (the caller then runs
/// the regular button-down path).
pub(crate) fn handle_window_chrome_double_click(
    hwnd: HWND,
    webtag_key: &str,
    point: (i32, i32),
) -> bool {
    if !window_draws_shell_chrome(webtag_key) {
        return false;
    }
    let Some(renderer) = windows_chrome_renderer() else {
        return false;
    };
    let state = chrome_state_for_window(hwnd, webtag_key);
    let Some(WindowsChromeHit::NativePanelTab { panel_id, tab_id }) =
        renderer.hit_test(&state, point)
    else {
        return false;
    };

    let is_active_tab = state
        .attached
        .as_ref()
        .and_then(|attached| {
            attached
                .panels
                .iter()
                .find(|panel| panel.panel_id == panel_id)
        })
        .and_then(|panel| panel.native.as_ref())
        .is_some_and(|native| {
            native
                .tabs
                .iter()
                .any(|tab| tab.id == tab_id && tab.active)
        });
    let event = if is_active_tab {
        WindowsChromeEvent::NativePanelTabRenameRequest { panel_id, tab_id }
    } else {
        WindowsChromeEvent::NativePanelTabClick { panel_id, tab_id }
    };
    invoke_chrome_event_handler(webtag_key, event)
}

/// WM_RBUTTONDOWN on chrome: a right-click on a native panel's content area
/// is dispatched to the product layer (terminals treat it as paste). Returns
/// `false` for all other chrome so the message falls through.
pub(crate) fn handle_window_chrome_right_click(
    hwnd: HWND,
    webtag_key: &str,
    point: (i32, i32),
) -> bool {
    if !window_draws_shell_chrome(webtag_key) {
        return false;
    }
    let Some(renderer) = windows_chrome_renderer() else {
        return false;
    };
    let state = chrome_state_for_window(hwnd, webtag_key);
    let Some(WindowsChromeHit::NativePanel { panel_id }) = renderer.hit_test(&state, point) else {
        return false;
    };

    // Keep keyboard input flowing into the panel the user right-clicked.
    set_active_native_panel(Some(panel_id.clone()));
    unsafe {
        let _ = SetFocus(Some(hwnd));
    }
    invoke_chrome_event_handler(webtag_key, WindowsChromeEvent::NativePanelRightClick { panel_id })
}

pub(crate) fn show_native_window(
    state: &mut UiState,
    title: &str,
    activate: bool,
    role: WindowsWindowRole,
) -> StdResult<()> {
    match role {
        WindowsWindowRole::Main => show_native_main_window(state, title, activate),
        WindowsWindowRole::Panel { panel_id } => show_native_panel_window(state, &panel_id),
    }
}

pub(crate) fn show_native_main_window(
    state: &mut UiState,
    title: &str,
    activate: bool,
) -> StdResult<()> {
    let (group_key, host, is_host) = ensure_main_attachment(state);
    set_active_group(&group_key);
    set_group_active_main(&group_key, &state.webtag_key);
    // A regular main webview taking over the main surface ends any
    // in-flight presentation (there is nothing left to restore).
    clear_presented_main_for_new_main(&group_key, &state.webtag_key);

    if is_host {
        show_shell_host(&group_key, host, title, activate);
        sync_controller_bounds(state)?;
        layout_group_windows(&group_key);
        set_controller_visible(state, true)?;
    } else {
        attach_child_window_to_host(state.hwnd, host);
        show_shell_host(&group_key, host, title, activate);
        layout_group_windows(&group_key);
        sync_controller_bounds(state)?;
        set_controller_visible(state, true)?;
    }

    request_group_shell_refresh(&group_key);
    state.window_visible = true;
    store_current_window_placement(state);
    Ok(())
}

/// Presents this window as the main-content child of `group_key`'s host:
/// reparents it into the host (same SetParent/child-style machinery as
/// attached main children), makes it the group's active main surface over
/// the main card rect, and remembers the displaced main webview for
/// `restore_presented_group_main`.
pub(crate) fn present_native_window_as_group_main(
    state: &mut UiState,
    group_key: &str,
) -> StdResult<()> {
    let Some(host) = host_handle_for_group(group_key) else {
        return Err(WebViewError::WebView(format!(
            "no host window for Windows shell group {group_key}"
        )));
    };
    if hwnd_handle(host) == hwnd_handle(state.hwnd) {
        return Err(WebViewError::WebView(
            "cannot present a group host window as its own main child".to_string(),
        ));
    }

    register_window_handle(&state.webtag_key, state.hwnd);
    let previous_main = group_active_main(group_key)
        .filter(|previous| previous.as_str() != state.webtag_key.as_str());
    if previous_main.is_some() || group_active_main(group_key).is_none() {
        remember_presented_main(group_key, &state.webtag_key, previous_main);
    }

    attach_child_window_to_host(state.hwnd, host);
    set_window_attachment(
        &state.webtag_key,
        WindowAttachment {
            group_key: group_key.to_string(),
            kind: WindowAttachmentKind::MainChild,
        },
    );
    set_group_active_main(group_key, &state.webtag_key);
    layout_group_windows(group_key);
    sync_controller_bounds(state)?;
    set_controller_visible(state, true)?;
    request_group_shell_refresh(group_key);
    state.window_visible = true;
    Ok(())
}

pub(crate) fn show_native_panel_window(state: &mut UiState, panel_id: &str) -> StdResult<()> {
    register_window_handle(&state.webtag_key, state.hwnd);
    let group_key = active_group_key().unwrap_or_else(|| webtag_group_key(&state.webtag_key));
    let Some(host) = host_handle_for_group(&group_key) else {
        return show_native_main_window(state, "", true);
    };
    let position = panel_position_for_group(&group_key, panel_id);
    attach_child_window_to_host(state.hwnd, host);
    set_window_attachment(
        &state.webtag_key,
        WindowAttachment {
            group_key: group_key.clone(),
            kind: WindowAttachmentKind::Panel {
                panel_id: panel_id.to_string(),
                position,
            },
        },
    );
    register_group_panel(
        &group_key,
        GroupPanel {
            webtag_key: state.webtag_key.clone(),
            panel_id: panel_id.to_string(),
            position,
            native_kind: NativePanelKind::Text,
            native_title: None,
            native_body: None,
            native_tabs: Vec::new(),
            maximized: false,
        },
    );
    set_controller_visible(state, true)?;
    layout_group_windows(&group_key);
    request_group_shell_refresh(&group_key);
    state.window_visible = true;
    Ok(())
}

pub(crate) fn hide_native_window(state: &mut UiState) -> StdResult<()> {
    store_current_window_placement(state);
    match window_attachment(&state.webtag_key).map(|attachment| attachment.kind) {
        Some(WindowAttachmentKind::MainHost) => hide_native_main_host_window(state),
        Some(WindowAttachmentKind::MainChild) => {
            set_controller_visible(state, false)?;
            hide_attached_window(state.hwnd);
            state.window_visible = false;
            Ok(())
        }
        Some(WindowAttachmentKind::Panel { .. }) => {
            let group_key = layout_group_key_for_webtag(&state.webtag_key);
            set_controller_visible(state, false)?;
            hide_attached_window(state.hwnd);
            remove_group_panel(&group_key, &state.webtag_key);
            layout_group_windows(&group_key);
            request_group_shell_refresh(&group_key);
            state.window_visible = false;
            Ok(())
        }
        None => hide_detached_window(state),
    }
}

pub(crate) fn hide_native_main_host_window(state: &mut UiState) -> StdResult<()> {
    let group_key = layout_group_key_for_webtag(&state.webtag_key);
    if group_active_main(&group_key).as_deref() != Some(state.webtag_key.as_str()) {
        set_controller_visible(state, false)?;
        state.window_visible = false;
        return Ok(());
    }
    hide_detached_window(state)
}

pub(crate) fn hide_detached_window(state: &mut UiState) -> StdResult<()> {
    set_controller_visible(state, false)?;
    // A hidden group host drops its main-card corner caps; they are
    // recreated by the next bounds sync when the window shows again.
    destroy_corner_caps(state.hwnd);
    unsafe {
        let _ = WindowsAndMessaging::SetWindowPos(
            state.hwnd,
            None,
            0,
            0,
            0,
            0,
            WindowsAndMessaging::SWP_NOMOVE
                | WindowsAndMessaging::SWP_NOSIZE
                | WindowsAndMessaging::SWP_NOZORDER
                | WindowsAndMessaging::SWP_NOACTIVATE
                | WindowsAndMessaging::SWP_HIDEWINDOW,
        );
    }
    state.window_visible = false;
    Ok(())
}

pub(crate) fn set_controller_visible(state: &UiState, visible: bool) -> StdResult<()> {
    unsafe {
        state
            .controller
            .SetIsVisible(visible)
            .map_err(|err| WebViewError::WebView(format!("SetIsVisible failed: {err}")))?;
    }
    if visible {
        // WebView2 may reorder its child-window chain while it becomes
        // visible; keep the corner caps above the webview surface.
        raise_corner_caps(state.hwnd);
    }
    Ok(())
}

pub(crate) fn set_native_window_layout(
    state: &UiState,
    layout: WindowsWindowLayout,
) -> StdResult<()> {
    set_window_layout_for_key(&state.webtag_key, layout);
    sync_controller_bounds(state)?;
    if let Some(attachment) = window_attachment(&state.webtag_key)
        && !matches!(attachment.kind, WindowAttachmentKind::Panel { .. })
    {
        layout_group_windows(&attachment.group_key);
        request_group_shell_refresh(&attachment.group_key);
    }
    unsafe {
        let _ = InvalidateRect(Some(state.hwnd), None, false);
    }
    Ok(())
}

pub(crate) fn sync_controller_bounds(state: &UiState) -> StdResult<()> {
    sync_controller_bounds_for(state.hwnd, &state.webtag_key, &state.controller)
}

/// Last bounds applied to each window's WebView2 controller. The controller
/// resize is the expensive part of a layout pass, and the interactive
/// move/size paths re-enter the layout far more often than the bounds
/// actually change, so unchanged `SetBounds` calls are skipped.
static LAST_CONTROLLER_BOUNDS: OnceLock<Mutex<HashMap<isize, (i32, i32, i32, i32)>>> =
    OnceLock::new();

pub(crate) fn sync_controller_bounds_for(
    hwnd: HWND,
    webtag_key: &str,
    controller: &ICoreWebView2Controller,
) -> StdResult<()> {
    let mut rect = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetClientRect(hwnd, &mut rect);
    }
    if rect.right <= rect.left || rect.bottom <= rect.top {
        rect = RECT {
            left: 0,
            top: 0,
            right: 1024,
            bottom: 768,
        };
    }
    let rect = controller_bounds_for_window(hwnd, webtag_key, rect);

    let bounds = (rect.left, rect.top, rect.right, rect.bottom);
    let cache = LAST_CONTROLLER_BOUNDS.get_or_init(|| Mutex::new(HashMap::new()));
    let unchanged = cache
        .lock()
        .map(|cache| cache.get(&hwnd_handle(hwnd)) == Some(&bounds))
        .unwrap_or(false);
    if !unchanged {
        unsafe {
            controller
                .SetBounds(rect)
                .map_err(|err| WebViewError::WebView(format!("SetBounds failed: {err}")))?;
        }
        if let Ok(mut cache) = cache.lock() {
            cache.insert(hwnd_handle(hwnd), bounds);
        }
    }
    // A group host's own main card is not an attached child window; its
    // corner caps are managed here, where its card rect is known. Attached
    // cards get theirs from `set_attached_window_rect`.
    if matches!(
        window_attachment(webtag_key).map(|attachment| attachment.kind),
        Some(WindowAttachmentKind::MainHost)
    ) {
        update_corner_caps(hwnd, rect);
    }
    Ok(())
}

pub(crate) fn controller_bounds_for_window(hwnd: HWND, webtag_key: &str, client: RECT) -> RECT {
    match window_attachment(webtag_key) {
        Some(WindowAttachment {
            kind: WindowAttachmentKind::MainChild | WindowAttachmentKind::Panel { .. },
            ..
        }) => normalize_rect(client),
        Some(WindowAttachment {
            group_key,
            kind: WindowAttachmentKind::MainHost,
        }) => {
            let content = renderer_content_rect(client, &current_window_layout(webtag_key));
            attached_group_rects(&group_key, hwnd)
                .map(|rects| rects.main)
                .unwrap_or(content)
        }
        None => renderer_content_rect(client, &current_window_layout(webtag_key)),
    }
}

pub(crate) fn window_snapshot(state: &UiState) -> StdResult<WindowsWebViewWindowSnapshot> {
    let mut window_rect = RECT::default();
    let mut client_rect = RECT::default();
    let mut client_origin = POINT { x: 0, y: 0 };

    let window_id = if let Some(attachment) = window_attachment(&state.webtag_key) {
        if matches!(
            attachment.kind,
            WindowAttachmentKind::MainChild | WindowAttachmentKind::Panel { .. }
        ) {
            let host = host_handle_for_group(&attachment.group_key).unwrap_or(state.hwnd);
            unsafe {
                WindowsAndMessaging::GetWindowRect(host, &mut window_rect)
                    .map_err(|err| WebViewError::WebView(format!("GetWindowRect failed: {err}")))?;
            }
            hwnd_handle(host) as usize
        } else {
            unsafe {
                WindowsAndMessaging::GetWindowRect(state.hwnd, &mut window_rect)
                    .map_err(|err| WebViewError::WebView(format!("GetWindowRect failed: {err}")))?;
            }
            hwnd_handle(state.hwnd) as usize
        }
    } else {
        unsafe {
            WindowsAndMessaging::GetWindowRect(state.hwnd, &mut window_rect)
                .map_err(|err| WebViewError::WebView(format!("GetWindowRect failed: {err}")))?;
        }
        hwnd_handle(state.hwnd) as usize
    };

    unsafe {
        WindowsAndMessaging::GetClientRect(state.hwnd, &mut client_rect)
            .map_err(|err| WebViewError::WebView(format!("GetClientRect failed: {err}")))?;
        if !ClientToScreen(state.hwnd, &mut client_origin).as_bool() {
            return Err(WebViewError::WebView("ClientToScreen failed".to_string()));
        }
    }

    let content = controller_bounds_for_window(state.hwnd, &state.webtag_key, client_rect);
    let content_left = client_origin.x - window_rect.left + content.left;
    let content_top = client_origin.y - window_rect.top + content.top;
    let content_width = rect_width(&content) as u32;
    let content_height = rect_height(&content) as u32;

    Ok(WindowsWebViewWindowSnapshot {
        window_id,
        webtag_key: state.webtag_key.clone(),
        visible: state.window_visible
            && unsafe { WindowsAndMessaging::IsWindowVisible(state.hwnd).as_bool() },
        content_left,
        content_top,
        content_width,
        content_height,
    })
}

pub(crate) fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
