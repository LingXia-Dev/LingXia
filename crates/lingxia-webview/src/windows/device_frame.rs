//! Generic device-frame presentation mechanic for top-level host windows.
//!
//! Presents a webview host window as the fixed-size "screen" of a simulated
//! device (the Windows counterpart of the macOS Runner's simulator shell —
//! `SimulatorToolbar` + `DeviceFrame`): the host window is restyled
//! borderless at exactly the screen size, its corners are rounded with a
//! window region plus the anti-aliased corner-cap overlays (see
//! `window.rs`), and a per-pixel-alpha layered companion window is kept
//! glued behind it, painting what a GDI window region alone cannot: the
//! floating toolbar (close/minimize dots, device selector, action glyph),
//! the device bezel with anti-aliased outer corners, and a soft drop
//! shadow.
//!
//! Like the menu-bar mechanic, this module owns no policy: device sizes,
//! radii, colors, the toolbar labels, and the command ids behind the
//! selector and the action glyph are all supplied by the product layer
//! through [`WindowsDeviceFrame`]. Toolbar selections are dispatched
//! through the registered app-menu command handler (see `menu.rs`).
//! Dragging the toolbar or the bezel moves the assembly; a right-click
//! offers the installed app-menu model as a context menu.

use super::*;

use windows::Win32::Graphics::Dwm::DWMWCP_DONOTROUND;
use windows::Win32::Graphics::Gdi::{
    CreateFontW, CreateRoundRectRgn, FW_SEMIBOLD, GetTextExtentPoint32W, SetBkMode, SetTextColor,
    SetWindowRgn, TRANSPARENT, TextOutW,
};

/// Toolbar model floating above the device: a centered drop-down selector
/// and an optional trailing action glyph. Selecting an item (or clicking
/// the glyph) dispatches the item's command id through the registered
/// app-menu command handler, exactly like a menu-bar selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsDeviceFrameToolbar {
    /// Label shown on the selector (e.g. the current device name).
    pub selector_label: String,
    /// Drop-down items offered by the selector.
    pub selector_items: Vec<WindowsAppMenuItem>,
    /// Command id dispatched by the trailing gear glyph, when present.
    pub action_command: Option<u32>,
}

/// Visual description of one simulated device, in physical pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsDeviceFrame {
    /// Screen (content) width — the host window client size.
    pub screen_width: i32,
    /// Screen (content) height.
    pub screen_height: i32,
    /// Bezel ring width around the screen.
    pub bezel_width: i32,
    /// Corner radius of the bezel's outer silhouette.
    pub outer_corner_radius: i32,
    /// Corner radius of the screen. `0` keeps square screen corners.
    pub screen_corner_radius: i32,
    /// Bezel fill color as `0xRRGGBB`.
    pub bezel_color: u32,
    /// Simulator toolbar floating above the device, when present.
    pub toolbar: Option<WindowsDeviceFrameToolbar>,
}

/// Soft drop-shadow ring rendered around the bezel in the layered bitmap.
const FRAME_SHADOW_MARGIN: i32 = 24;

/// Vertical shadow offset (shadow biased downwards, like a light from
/// above).
const FRAME_SHADOW_OFFSET_Y: f32 = 5.0;

/// Peak shadow alpha at the bezel edge.
const FRAME_SHADOW_ALPHA: f32 = 0.38;

// Toolbar metrics and colors, mirroring the macOS `SimulatorToolbar`.
const TOOLBAR_HEIGHT: i32 = 32;
const TOOLBAR_GAP: i32 = 12;
const TOOLBAR_RADIUS: i32 = 8;
const TOOLBAR_COLOR: u32 = 0x2E2E2E;
const TOOLBAR_ALPHA: u32 = 250;
const TOOLBAR_DOT_RADIUS: f32 = 6.0;
const TOOLBAR_SIDE_MARGIN: i32 = 10;
const TOOLBAR_DOT_SPACING: i32 = 8;
const CLOSE_DOT_COLOR: u32 = 0xFF6159;
const MINIMIZE_DOT_COLOR: u32 = 0xFFBD38;

/// Pixel geometry of one presented frame, all in shell-window coordinates.
#[derive(Debug, Clone, Copy, Default)]
struct FrameLayout {
    /// Shell (layered) window size.
    width: i32,
    height: i32,
    /// Toolbar capsule rect; empty (all zero) without a toolbar.
    toolbar: RECT,
    /// Bezel rect (device silhouette).
    bezel: RECT,
    /// Content window offset from the shell window origin.
    content_offset: (i32, i32),
    /// Interactive toolbar rects (empty without a toolbar).
    close_rect: RECT,
    minimize_rect: RECT,
    selector_rect: RECT,
    action_rect: RECT,
}

fn compute_layout(spec: &WindowsDeviceFrame) -> FrameLayout {
    let margin = FRAME_SHADOW_MARGIN;
    let bezel_width = spec.screen_width + 2 * spec.bezel_width;
    let bezel_height = spec.screen_height + 2 * spec.bezel_width;
    let has_toolbar = spec.toolbar.is_some();
    let toolbar_block = if has_toolbar {
        TOOLBAR_HEIGHT + TOOLBAR_GAP
    } else {
        0
    };
    let bezel_top = margin + toolbar_block;
    let mut layout = FrameLayout {
        width: bezel_width + 2 * margin,
        height: bezel_height + toolbar_block + 2 * margin,
        bezel: RECT {
            left: margin,
            top: bezel_top,
            right: margin + bezel_width,
            bottom: bezel_top + bezel_height,
        },
        content_offset: (
            margin + spec.bezel_width,
            bezel_top + spec.bezel_width,
        ),
        ..Default::default()
    };
    if has_toolbar {
        layout.toolbar = RECT {
            left: margin,
            top: margin,
            right: margin + bezel_width,
            bottom: margin + TOOLBAR_HEIGHT,
        };
        let dot = (TOOLBAR_DOT_RADIUS * 2.0) as i32;
        let dot_top = layout.toolbar.top + (TOOLBAR_HEIGHT - dot) / 2;
        layout.close_rect = RECT {
            left: layout.toolbar.left + TOOLBAR_SIDE_MARGIN,
            top: dot_top,
            right: layout.toolbar.left + TOOLBAR_SIDE_MARGIN + dot,
            bottom: dot_top + dot,
        };
        layout.minimize_rect = RECT {
            left: layout.close_rect.right + TOOLBAR_DOT_SPACING,
            top: dot_top,
            right: layout.close_rect.right + TOOLBAR_DOT_SPACING + dot,
            bottom: dot_top + dot,
        };
        // selector_rect and action_rect depend on text metrics; they are
        // filled in during painting (see paint_frame_window).
    }
    layout
}

struct DeviceFrameState {
    /// The layered shell window glued behind the content window.
    frame: isize,
    spec: WindowsDeviceFrame,
    layout: FrameLayout,
    /// `GWL_STYLE` of the content window before the borderless restyle.
    saved_style: isize,
}

/// Active device-frame presentations, keyed by content window handle.
static DEVICE_FRAMES: OnceLock<Mutex<HashMap<isize, DeviceFrameState>>> = OnceLock::new();

/// Whether `window` is currently presented inside a device frame. The menu
/// mechanic consults this: a framed screen window carries no menu bar (the
/// model is offered from the toolbar and the bezel context menu instead).
pub(crate) fn has_device_frame(window: HWND) -> bool {
    frame_state(hwnd_handle(window), |_| ()).is_some()
}

fn frame_state<T>(content: isize, read: impl FnOnce(&DeviceFrameState) -> T) -> Option<T> {
    DEVICE_FRAMES
        .get()
        .and_then(|frames| frames.lock().ok())
        .and_then(|frames| frames.get(&content).map(read))
}

fn frame_state_by_frame<T>(
    frame: HWND,
    read: impl FnOnce(isize, &DeviceFrameState) -> T,
) -> Option<T> {
    let frame = hwnd_handle(frame);
    DEVICE_FRAMES
        .get()
        .and_then(|frames| frames.lock().ok())
        .and_then(|frames| {
            frames
                .iter()
                .find(|(_, state)| state.frame == frame)
                .map(|(content, state)| read(*content, state))
        })
}

/// Presents (or clears) a device frame around the top-level window showing
/// `webtag`. Attached surfaces resolve to their group host window. The
/// restyle runs on the window's UI thread; this call only fails when no
/// window exists for `webtag`. Safe to call from any thread.
pub fn set_webview_device_frame(
    webtag: &WebTag,
    frame: Option<WindowsDeviceFrame>,
) -> StdResult<()> {
    let hwnd = window_handle_for_key(webtag.key()).ok_or_else(|| {
        WebViewError::WebView(format!("no window registered for {}", webtag.key()))
    })?;
    let hwnd = match window_attachment(webtag.key()) {
        Some(WindowAttachment {
            group_key,
            kind: WindowAttachmentKind::MainChild | WindowAttachmentKind::Panel { .. },
        }) => host_handle_for_group(&group_key).ok_or_else(|| {
            WebViewError::WebView(format!("no host window for Windows shell group {group_key}"))
        })?,
        _ => hwnd,
    };
    let handle = hwnd_handle(hwnd);
    let posted = post_to_window_thread(
        handle,
        Box::new(move || match frame {
            Some(spec) => apply_device_frame(hwnd_from_handle(handle), spec),
            None => clear_device_frame(hwnd_from_handle(handle)),
        }),
    );
    if posted {
        Ok(())
    } else {
        Err(WebViewError::WebView(
            "device frame target window is not accepting messages".to_string(),
        ))
    }
}

/// Applies `spec` to `content` (on its UI thread): restyles the window
/// borderless at screen size, rounds its corners, and creates/updates the
/// layered shell window behind it.
fn apply_device_frame(content: HWND, spec: WindowsDeviceFrame) {
    if spec.screen_width <= 0 || spec.screen_height <= 0 {
        log::warn!("ignoring device frame with empty screen: {spec:?}");
        return;
    }
    let handle = hwnd_handle(content);
    if frame_state(handle, |state| state.spec.clone()) == Some(spec.clone()) {
        sync_device_frame_for_content(content);
        return;
    }
    // A different device: rebuild the shell window, but keep the originally
    // saved style so repeated device switches don't save the borderless
    // style as the restore target.
    let saved_style = match frame_state(handle, |state| state.saved_style) {
        Some(saved) => {
            remove_frame_window(handle);
            saved
        }
        None => unsafe {
            WindowsAndMessaging::GetWindowLongPtrW(content, WindowsAndMessaging::GWL_STYLE)
        },
    };

    // Borderless screen window. WS_SYSMENU and WS_MINIMIZEBOX stay so
    // Alt+F4 and the taskbar minimize/close actions keep working without a
    // visible caption.
    let kept = saved_style as u32
        & (WindowsAndMessaging::WS_VISIBLE.0
            | WindowsAndMessaging::WS_CLIPCHILDREN.0
            | WindowsAndMessaging::WS_CLIPSIBLINGS.0);
    let borderless = kept
        | WindowsAndMessaging::WS_POPUP.0
        | WindowsAndMessaging::WS_SYSMENU.0
        | WindowsAndMessaging::WS_MINIMIZEBOX.0;
    unsafe {
        WindowsAndMessaging::SetWindowLongPtrW(
            content,
            WindowsAndMessaging::GWL_STYLE,
            borderless as isize,
        );
        // The menu bar cannot live on the device screen; the model stays
        // installed and is offered from the toolbar selector and the bezel
        // context menu instead.
        let menu = WindowsAndMessaging::GetMenu(content);
        if !menu.is_invalid() {
            let _ = WindowsAndMessaging::SetMenu(content, None);
            let _ = WindowsAndMessaging::DestroyMenu(menu);
        }
        // The window region below replaces DWM rounding (a region disables
        // it anyway); state it explicitly.
        let preference = DWMWCP_DONOTROUND;
        let _ = DwmSetWindowAttribute(
            content,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&preference as *const _) as *const c_void,
            std::mem::size_of_val(&preference) as u32,
        );
    }

    // Keep the assembly on screen: the shell extends past the content by
    // the content offset (shadow + toolbar + bezel).
    let layout = compute_layout(&spec);
    let mut rect = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetWindowRect(content, &mut rect);
    }
    let x = rect.left.max(layout.content_offset.0);
    let y = rect.top.max(layout.content_offset.1);
    unsafe {
        let _ = WindowsAndMessaging::SetWindowPos(
            content,
            None,
            x,
            y,
            spec.screen_width,
            spec.screen_height,
            WindowsAndMessaging::SWP_NOZORDER
                | WindowsAndMessaging::SWP_NOACTIVATE
                | WindowsAndMessaging::SWP_NOCOPYBITS
                | WindowsAndMessaging::SWP_FRAMECHANGED,
        );
    }

    apply_screen_corners(content, &spec);

    let Some((frame, layout)) = create_frame_window(content, &spec, layout) else {
        return;
    };
    let frames = DEVICE_FRAMES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut frames) = frames.lock() {
        frames.insert(
            handle,
            DeviceFrameState {
                frame: hwnd_handle(frame),
                spec,
                layout,
                saved_style,
            },
        );
    }
    sync_device_frame_for_content(content);
}

/// Rounds the screen window's corners: a `SetWindowRgn` rounded rect keeps
/// the rectangular window inside the bezel silhouette, and the corner-cap
/// overlays paint anti-aliased bezel-colored arcs over the staircase the
/// region clips (the region edge itself sits bezel-on-bezel, so its
/// aliasing is invisible).
fn apply_screen_corners(content: HWND, spec: &WindowsDeviceFrame) {
    let radius = spec.screen_corner_radius.max(0);
    // COLORREF is 0x00BBGGRR; the spec color is 0xRRGGBB.
    let bgr = ((spec.bezel_color & 0xff) << 16)
        | (spec.bezel_color & 0xff00)
        | ((spec.bezel_color >> 16) & 0xff);
    destroy_corner_caps(content);
    if radius > 0 {
        set_corner_cap_style_override(content, Some((radius, bgr)));
        unsafe {
            let region = CreateRoundRectRgn(
                0,
                0,
                spec.screen_width + 1,
                spec.screen_height + 1,
                radius * 2,
                radius * 2,
            );
            let _ = SetWindowRgn(content, Some(region), true);
        }
        update_corner_caps(
            content,
            RECT {
                left: 0,
                top: 0,
                right: spec.screen_width,
                bottom: spec.screen_height,
            },
        );
    } else {
        set_corner_cap_style_override(content, None);
        unsafe {
            let _ = SetWindowRgn(content, None, true);
        }
    }
}

/// Removes the device frame from `content` (on its UI thread) and restores
/// the standard window: original styles, DWM rounded corners, and the
/// app-menu bar when a model is installed.
fn clear_device_frame(content: HWND) {
    let handle = hwnd_handle(content);
    let removed = DEVICE_FRAMES
        .get()
        .and_then(|frames| frames.lock().ok())
        .and_then(|mut frames| frames.remove(&handle));
    let Some(state) = removed else {
        return;
    };
    unsafe {
        let _ = WindowsAndMessaging::DestroyWindow(hwnd_from_handle(state.frame));
    }
    set_corner_cap_style_override(content, None);
    destroy_corner_caps(content);
    unsafe {
        let _ = SetWindowRgn(content, None, true);
        WindowsAndMessaging::SetWindowLongPtrW(
            content,
            WindowsAndMessaging::GWL_STYLE,
            state.saved_style,
        );
        let _ = WindowsAndMessaging::SetWindowPos(
            content,
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
    apply_round_corner_preference(content);
    // Reattach the menu bar (rebuilt from the installed model) and re-sync
    // the controller bounds for the restored frame.
    apply_app_menu_to_window(content);
}

/// Drops the registry entry of a content window that is going away and
/// destroys its shell window.
pub(crate) fn forget_device_frame(content: HWND) {
    remove_frame_window(hwnd_handle(content));
}

fn remove_frame_window(content: isize) {
    let removed = DEVICE_FRAMES
        .get()
        .and_then(|frames| frames.lock().ok())
        .and_then(|mut frames| frames.remove(&content));
    if let Some(state) = removed {
        unsafe {
            let _ = WindowsAndMessaging::DestroyWindow(hwnd_from_handle(state.frame));
        }
    }
}

/// Glues the shell window to the content window: hidden while the content
/// is minimized or hidden, otherwise offset around it and kept directly
/// below it in z-order. Runs on every content geometry change (see
/// `handle_window_geometry_change`) and after shell-initiated moves.
pub(crate) fn sync_device_frame_for_content(content: HWND) {
    let handle = hwnd_handle(content);
    let Some((frame, offset)) =
        frame_state(handle, |state| (state.frame, state.layout.content_offset))
    else {
        return;
    };
    if !is_window_handle_valid(frame) {
        return;
    }
    let frame = hwnd_from_handle(frame);
    let visible = unsafe {
        WindowsAndMessaging::IsWindowVisible(content).as_bool()
            && !WindowsAndMessaging::IsIconic(content).as_bool()
    };
    if !visible {
        unsafe {
            let _ = WindowsAndMessaging::ShowWindow(frame, WindowsAndMessaging::SW_HIDE);
        }
        return;
    }
    let mut content_rect = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetWindowRect(content, &mut content_rect);
    }
    unsafe {
        // hWndInsertAfter = content: the shell sits directly below the
        // screen window. The layered bitmap never resizes, so only move.
        let _ = WindowsAndMessaging::SetWindowPos(
            frame,
            Some(content),
            content_rect.left - offset.0,
            content_rect.top - offset.1,
            0,
            0,
            WindowsAndMessaging::SWP_NOSIZE
                | WindowsAndMessaging::SWP_NOACTIVATE
                | WindowsAndMessaging::SWP_SHOWWINDOW,
        );
    }
    // WebView2 child churn can bury the screen-corner caps; geometry passes
    // come through here, so re-assert them alongside the shell.
    raise_corner_caps(content);
}

/// Moves the content window to track a shell-initiated drag.
fn sync_content_to_frame(frame: HWND) {
    let Some((content, offset)) =
        frame_state_by_frame(frame, |content, state| (content, state.layout.content_offset))
    else {
        return;
    };
    if !is_window_handle_valid(content) {
        return;
    }
    let mut frame_rect = RECT::default();
    let mut content_rect = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetWindowRect(frame, &mut frame_rect);
        let _ = WindowsAndMessaging::GetWindowRect(hwnd_from_handle(content), &mut content_rect);
    }
    let x = frame_rect.left + offset.0;
    let y = frame_rect.top + offset.1;
    if content_rect.left == x && content_rect.top == y {
        return;
    }
    unsafe {
        let _ = WindowsAndMessaging::SetWindowPos(
            hwnd_from_handle(content),
            None,
            x,
            y,
            0,
            0,
            WindowsAndMessaging::SWP_NOSIZE
                | WindowsAndMessaging::SWP_NOZORDER
                | WindowsAndMessaging::SWP_NOACTIVATE,
        );
    }
}

fn point_in_rect(rect: &RECT, x: i32, y: i32) -> bool {
    x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom
}

/// Handles a click on one of the toolbar's interactive rects (shell-local
/// `x`/`y`). Close and minimize act on the content window directly; the
/// selector and the action glyph dispatch product command ids through the
/// registered app-menu command handler.
fn handle_toolbar_click(frame: HWND, x: i32, y: i32) {
    let Some((content, spec, layout)) =
        frame_state_by_frame(frame, |content, state| {
            (content, state.spec.clone(), state.layout)
        })
    else {
        return;
    };
    let content = hwnd_from_handle(content);
    if point_in_rect(&layout.close_rect, x, y) {
        unsafe {
            let _ = WindowsAndMessaging::PostMessageW(
                Some(content),
                WindowsAndMessaging::WM_CLOSE,
                WPARAM::default(),
                LPARAM::default(),
            );
        }
    } else if point_in_rect(&layout.minimize_rect, x, y) {
        unsafe {
            let _ = WindowsAndMessaging::ShowWindow(content, WindowsAndMessaging::SW_MINIMIZE);
        }
    } else if point_in_rect(&layout.selector_rect, x, y) {
        let Some(toolbar) = spec.toolbar else {
            return;
        };
        show_selector_menu(frame, content, &layout, &toolbar);
    } else if point_in_rect(&layout.action_rect, x, y) {
        if let Some(command) = spec.toolbar.and_then(|toolbar| toolbar.action_command) {
            dispatch_app_menu_command(command);
        }
    }
}

/// Drops the selector's item list below the selector rect and dispatches
/// the chosen command id.
fn show_selector_menu(
    frame: HWND,
    content: HWND,
    layout: &FrameLayout,
    toolbar: &WindowsDeviceFrameToolbar,
) {
    let Ok(popup) = (unsafe { WindowsAndMessaging::CreatePopupMenu() }) else {
        return;
    };
    for item in &toolbar.selector_items {
        let mut flags = WindowsAndMessaging::MF_STRING;
        if item.checked {
            flags |= WindowsAndMessaging::MF_CHECKED;
        }
        let label = to_wide(&item.label);
        unsafe {
            let _ = WindowsAndMessaging::AppendMenuW(
                popup,
                flags,
                item.id as usize,
                PCWSTR(label.as_ptr()),
            );
        }
    }
    let mut anchor = POINT {
        x: layout.selector_rect.left,
        y: layout.toolbar.bottom + 2,
    };
    unsafe {
        let _ = ClientToScreen(frame, &mut anchor);
        let _ = WindowsAndMessaging::SetForegroundWindow(content);
    }
    let selected = unsafe {
        WindowsAndMessaging::TrackPopupMenu(
            popup,
            WindowsAndMessaging::TPM_LEFTALIGN
                | WindowsAndMessaging::TPM_TOPALIGN
                | WindowsAndMessaging::TPM_RETURNCMD
                | WindowsAndMessaging::TPM_NONOTIFY,
            anchor.x,
            anchor.y,
            None,
            content,
            None,
        )
    };
    unsafe {
        let _ = WindowsAndMessaging::DestroyMenu(popup);
    }
    let id = selected.0 as u32;
    if id != 0 {
        dispatch_app_menu_command(id);
    }
}

/// Shell window procedure: a layered window supporting dragging the
/// assembly (toolbar background and bezel act as a caption), the toolbar
/// buttons, and the app-menu context on right-click. It never takes
/// activation — focus stays on the screen window.
unsafe extern "system" fn frame_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WindowsAndMessaging::WM_NCHITTEST {
        let screen_x = (lparam.0 & 0xffff) as i16 as i32;
        let screen_y = ((lparam.0 >> 16) & 0xffff) as i16 as i32;
        let mut rect = RECT::default();
        unsafe {
            let _ = WindowsAndMessaging::GetWindowRect(hwnd, &mut rect);
        }
        let x = screen_x - rect.left;
        let y = screen_y - rect.top;
        let hit = frame_state_by_frame(hwnd, |_, state| {
            let layout = &state.layout;
            if point_in_rect(&layout.close_rect, x, y)
                || point_in_rect(&layout.minimize_rect, x, y)
                || point_in_rect(&layout.selector_rect, x, y)
                || point_in_rect(&layout.action_rect, x, y)
            {
                WindowsAndMessaging::HTCLIENT
            } else if point_in_rect(&layout.toolbar, x, y) || point_in_rect(&layout.bezel, x, y) {
                WindowsAndMessaging::HTCAPTION
            } else {
                WindowsAndMessaging::HTTRANSPARENT as u32
            }
        })
        .unwrap_or(WindowsAndMessaging::HTTRANSPARENT as u32);
        return LRESULT(hit as isize);
    } else if msg == WindowsAndMessaging::WM_MOUSEACTIVATE {
        return LRESULT(WindowsAndMessaging::MA_NOACTIVATE as isize);
    } else if msg == WindowsAndMessaging::WM_LBUTTONDOWN {
        let x = (lparam.0 & 0xffff) as i16 as i32;
        let y = ((lparam.0 >> 16) & 0xffff) as i16 as i32;
        handle_toolbar_click(hwnd, x, y);
        return LRESULT(0);
    } else if msg == WindowsAndMessaging::WM_NCRBUTTONUP {
        let content = frame_state_by_frame(hwnd, |content, _| content);
        if let Some(content) = content.filter(|content| is_window_handle_valid(*content)) {
            let x = (lparam.0 & 0xffff) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xffff) as i16 as i32;
            if show_app_menu_context(hwnd_from_handle(content), x, y) {
                return LRESULT(0);
            }
        }
    } else if msg == WindowsAndMessaging::WM_NCLBUTTONDBLCLK {
        // No maximize semantics on a fixed-size device.
        return LRESULT(0);
    } else if msg == WindowsAndMessaging::WM_WINDOWPOSCHANGING {
        // The shell's device face is opaque, so it must never stack above
        // the screen window — but DefWindowProc raises a window dragged by
        // HTCAPTION. Rewrite every pending placement to sit directly below
        // the content window instead.
        let content = frame_state_by_frame(hwnd, |content, _| content);
        if let Some(content) = content.filter(|content| is_window_handle_valid(*content)) {
            let pos = lparam.0 as *mut WindowsAndMessaging::WINDOWPOS;
            if !pos.is_null() {
                unsafe {
                    (*pos).hwndInsertAfter = hwnd_from_handle(content);
                    (*pos).flags &= !WindowsAndMessaging::SWP_NOZORDER;
                }
            }
        }
        // Fall through so DefWindowProc still applies the placement.
    } else if msg == WindowsAndMessaging::WM_WINDOWPOSCHANGED {
        let pos = lparam.0 as *const WindowsAndMessaging::WINDOWPOS;
        if !pos.is_null() && !unsafe { (*pos).flags }.contains(WindowsAndMessaging::SWP_NOMOVE) {
            sync_content_to_frame(hwnd);
        }
        // Fall through for default WM_MOVE generation.
    } else if msg == WindowsAndMessaging::WM_ENTERSIZEMOVE {
        // Dragging grabs the shell, but the assembly should rise as one;
        // raising the content also restacks the shell directly below it
        // (see the z-order sync in the content's WM_WINDOWPOSCHANGED).
        let content = frame_state_by_frame(hwnd, |content, _| content);
        if let Some(content) = content.filter(|content| is_window_handle_valid(*content)) {
            unsafe {
                let _ = WindowsAndMessaging::SetWindowPos(
                    hwnd_from_handle(content),
                    Some(WindowsAndMessaging::HWND_TOP),
                    0,
                    0,
                    0,
                    0,
                    WindowsAndMessaging::SWP_NOMOVE
                        | WindowsAndMessaging::SWP_NOSIZE
                        | WindowsAndMessaging::SWP_NOACTIVATE,
                );
            }
        }
        // Track the modal drag loop at timer cadence — WM_WINDOWPOSCHANGED
        // is coalesced inside it (same pattern as the host windows).
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
        sync_content_to_frame(hwnd);
    } else if msg == WindowsAndMessaging::WM_TIMER {
        if wparam.0 == SIZEMOVE_TIMER_ID {
            sync_content_to_frame(hwnd);
            return LRESULT(0);
        }
    }
    unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn frame_class() -> PCWSTR {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let module = unsafe { LibraryLoader::GetModuleHandleW(None) }
            .map(|module| HINSTANCE(module.0))
            .unwrap_or_default();
        let class = WNDCLASSW {
            lpfnWndProc: Some(frame_proc),
            hInstance: module,
            lpszClassName: w!("LingXiaDeviceFrame"),
            ..Default::default()
        };
        if unsafe { WindowsAndMessaging::RegisterClassW(&class) } == 0 {
            log::error!(
                "device frame class registration failed: {}",
                windows::core::Error::from_thread()
            );
        }
    });
    w!("LingXiaDeviceFrame")
}

/// Creates the layered shell window behind `content` and uploads its
/// per-pixel-alpha bitmap (toolbar + bezel + shadow). Returns the window
/// and the layout completed with the text-dependent toolbar rects, or
/// `None` when creation fails (requires the Win8+ `supportedOS` manifest,
/// like the corner caps).
fn create_frame_window(
    content: HWND,
    spec: &WindowsDeviceFrame,
    mut layout: FrameLayout,
) -> Option<(HWND, FrameLayout)> {
    let result = unsafe {
        WindowsAndMessaging::CreateWindowExW(
            WindowsAndMessaging::WS_EX_LAYERED
                | WindowsAndMessaging::WS_EX_TOOLWINDOW
                | WindowsAndMessaging::WS_EX_NOACTIVATE,
            frame_class(),
            PCWSTR::null(),
            WindowsAndMessaging::WS_POPUP,
            0,
            0,
            layout.width,
            layout.height,
            None,
            None,
            LibraryLoader::GetModuleHandleW(None)
                .ok()
                .map(|module| HINSTANCE(module.0)),
            Some(hwnd_handle(content) as *const c_void),
        )
    };
    let frame = match result {
        Ok(frame) => frame,
        Err(err) => {
            log::warn!("device frame window creation failed: {err}");
            return None;
        }
    };
    paint_frame_window(frame, spec, &mut layout);
    Some((frame, layout))
}

/// Renders the shell bitmap — analytic bezel/shadow/toolbar pixels, then
/// GDI text for the selector label and the action glyph — and uploads it
/// via `UpdateLayeredWindow`. Fills the text-dependent rects of `layout`.
fn paint_frame_window(frame: HWND, spec: &WindowsDeviceFrame, layout: &mut FrameLayout) {
    let mut pixels = frame_pixels(spec, layout);
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
                    biWidth: layout.width,
                    biHeight: -layout.height,
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
                if let Some(toolbar) = &spec.toolbar {
                    draw_toolbar_text(memory_dc, toolbar, layout);
                    // GDI writes zero alpha bytes; restore the toolbar
                    // alpha over the text it touched.
                    let dib =
                        std::slice::from_raw_parts_mut(bits.cast::<u32>(), pixels.len());
                    fix_toolbar_alpha(dib, layout);
                    pixels.copy_from_slice(dib);
                }
                let size = SIZE {
                    cx: layout.width,
                    cy: layout.height,
                };
                let origin = POINT { x: 0, y: 0 };
                let blend = BLENDFUNCTION {
                    BlendOp: AC_SRC_OVER as u8,
                    BlendFlags: 0,
                    SourceConstantAlpha: 255,
                    AlphaFormat: AC_SRC_ALPHA as u8,
                };
                let _ = WindowsAndMessaging::UpdateLayeredWindow(
                    frame,
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

/// Draws the selector label (centered, with a drop-down arrow) and the
/// trailing gear glyph into the toolbar with GDI, and fills
/// `layout.selector_rect` / `layout.action_rect` from the text metrics.
fn draw_toolbar_text(dc: HDC, toolbar: &WindowsDeviceFrameToolbar, layout: &mut FrameLayout) {
    unsafe {
        SetBkMode(dc, TRANSPARENT);
    }
    let toolbar_rect = layout.toolbar;
    let center_y = (toolbar_rect.top + toolbar_rect.bottom) / 2;

    // Selector: "<label>  ⌄" centered in the toolbar.
    let label = format!("{}  \u{2304}", toolbar.selector_label);
    let label_wide = to_wide(&label);
    let label_chars = &label_wide[..label_wide.len().saturating_sub(1)];
    let font = unsafe {
        CreateFontW(
            -15,
            0,
            0,
            0,
            FW_SEMIBOLD.0 as i32,
            0,
            0,
            0,
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
            w!("Segoe UI"),
        )
    };
    unsafe {
        let old_font = SelectObject(dc, HGDIOBJ(font.0));
        SetTextColor(dc, COLORREF(0x00E8E8E8));
        let mut extent = SIZE::default();
        let _ = GetTextExtentPoint32W(dc, label_chars, &mut extent);
        let x = (toolbar_rect.left + toolbar_rect.right - extent.cx) / 2;
        let y = center_y - extent.cy / 2;
        let _ = TextOutW(dc, x, y, label_chars);
        layout.selector_rect = RECT {
            left: x - 8,
            top: toolbar_rect.top,
            right: x + extent.cx + 8,
            bottom: toolbar_rect.bottom,
        };
        if !old_font.is_invalid() {
            let _ = SelectObject(dc, old_font);
        }
        let _ = DeleteObject(HGDIOBJ(font.0));
    }

    // Trailing gear glyph (Segoe MDL2 Assets "Settings", U+E713).
    if toolbar.action_command.is_some() {
        let glyph = to_wide("\u{E713}");
        let glyph_chars = &glyph[..glyph.len().saturating_sub(1)];
        let icon_font = unsafe {
            CreateFontW(
                -16,
                0,
                0,
                0,
                FW_SEMIBOLD.0 as i32,
                0,
                0,
                0,
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                w!("Segoe MDL2 Assets"),
            )
        };
        unsafe {
            let old_font = SelectObject(dc, HGDIOBJ(icon_font.0));
            SetTextColor(dc, COLORREF(0x00B4B4B4));
            let mut extent = SIZE::default();
            let _ = GetTextExtentPoint32W(dc, glyph_chars, &mut extent);
            let x = toolbar_rect.right - TOOLBAR_SIDE_MARGIN - extent.cx;
            let y = center_y - extent.cy / 2;
            let _ = TextOutW(dc, x, y, glyph_chars);
            layout.action_rect = RECT {
                left: x - 6,
                top: toolbar_rect.top,
                right: toolbar_rect.right,
                bottom: toolbar_rect.bottom,
            };
            if !old_font.is_invalid() {
                let _ = SelectObject(dc, old_font);
            }
            let _ = DeleteObject(HGDIOBJ(icon_font.0));
        }
    }
}

/// GDI text output zeroes the alpha byte of every pixel it touches; inside
/// the toolbar capsule the alpha is a known constant, so restore it (and
/// re-premultiply the straight GDI colors) wherever it was knocked out.
fn fix_toolbar_alpha(pixels: &mut [u32], layout: &FrameLayout) {
    let toolbar = layout.toolbar;
    for y in toolbar.top..toolbar.bottom {
        for x in toolbar.left..toolbar.right {
            let index = (y * layout.width + x) as usize;
            let pixel = pixels[index];
            if pixel >> 24 == 0 && pixel != 0 {
                let premultiply = |channel: u32| channel * TOOLBAR_ALPHA / 255;
                let red = (pixel >> 16) & 0xff;
                let green = (pixel >> 8) & 0xff;
                let blue = pixel & 0xff;
                pixels[index] = (TOOLBAR_ALPHA << 24)
                    | (premultiply(red) << 16)
                    | (premultiply(green) << 8)
                    | premultiply(blue);
            }
        }
    }
}

/// Premultiplied ARGB pixels of the shell bitmap: the toolbar capsule with
/// its close/minimize dots, the device bezel as an anti-aliased rounded
/// rect, and a soft downward-biased shadow ringing both. The screen area
/// stays opaque bezel color — the screen window covers it, and any
/// sub-pixel gap along its clipped edge then reads as bezel instead of
/// flashing the desktop.
fn frame_pixels(spec: &WindowsDeviceFrame, layout: &FrameLayout) -> Vec<u32> {
    let bezel_red = (spec.bezel_color >> 16) & 0xff;
    let bezel_green = (spec.bezel_color >> 8) & 0xff;
    let bezel_blue = spec.bezel_color & 0xff;

    let bezel = layout.bezel;
    let bezel_center_x = (bezel.left + bezel.right) as f32 / 2.0;
    let bezel_center_y = (bezel.top + bezel.bottom) as f32 / 2.0;
    let radius = spec
        .outer_corner_radius
        .max(1)
        .min(spec.screen_width / 2 + spec.bezel_width) as f32;
    let half_x = (bezel.right - bezel.left) as f32 / 2.0 - radius;
    let half_y = (bezel.bottom - bezel.top) as f32 / 2.0 - radius;
    // Rounded-rect signed distance: negative inside the silhouette.
    let rounded_distance = |x: f32, y: f32, hx: f32, hy: f32, r: f32| -> f32 {
        let qx = x.abs() - hx;
        let qy = y.abs() - hy;
        let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
        outside + qx.max(qy).min(0.0) - r
    };
    let bezel_distance =
        move |x: f32, y: f32| rounded_distance(x - bezel_center_x, y - bezel_center_y, half_x, half_y, radius);

    let has_toolbar = spec.toolbar.is_some();
    let toolbar = layout.toolbar;
    let toolbar_center_x = (toolbar.left + toolbar.right) as f32 / 2.0;
    let toolbar_center_y = (toolbar.top + toolbar.bottom) as f32 / 2.0;
    let toolbar_half_x = (toolbar.right - toolbar.left) as f32 / 2.0 - TOOLBAR_RADIUS as f32;
    let toolbar_half_y = (toolbar.bottom - toolbar.top) as f32 / 2.0 - TOOLBAR_RADIUS as f32;
    let toolbar_distance = move |x: f32, y: f32| {
        rounded_distance(
            x - toolbar_center_x,
            y - toolbar_center_y,
            toolbar_half_x,
            toolbar_half_y,
            TOOLBAR_RADIUS as f32,
        )
    };

    // Toolbar dots (close red, minimize yellow), anti-aliased circles.
    let dots = [
        (&layout.close_rect, CLOSE_DOT_COLOR),
        (&layout.minimize_rect, MINIMIZE_DOT_COLOR),
    ];

    let shadow_reach = FRAME_SHADOW_MARGIN as f32;
    let mut pixels = Vec::with_capacity((layout.width * layout.height) as usize);
    for y in 0..layout.height {
        for x in 0..layout.width {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;

            let bezel_d = bezel_distance(px, py);
            let bezel_coverage = (0.5 - bezel_d).clamp(0.0, 1.0);
            let toolbar_coverage = if has_toolbar {
                (0.5 - toolbar_distance(px, py)).clamp(0.0, 1.0)
            } else {
                0.0
            };

            // Quadratic shadow falloff around the bezel, sampled against a
            // downward-shifted silhouette; the toolbar gets a tighter ring.
            let shadow_d = bezel_distance(px, py - FRAME_SHADOW_OFFSET_Y).max(0.0);
            let falloff = (1.0 - shadow_d / shadow_reach).clamp(0.0, 1.0);
            let mut shadow = FRAME_SHADOW_ALPHA * falloff * falloff;
            if has_toolbar {
                let toolbar_shadow_d = toolbar_distance(px, py - 2.0).max(0.0);
                let toolbar_falloff = (1.0 - toolbar_shadow_d / 8.0).clamp(0.0, 1.0);
                shadow = shadow.max(0.3 * toolbar_falloff * toolbar_falloff);
            }

            // Composite: toolbar capsule over bezel over shadow.
            let toolbar_alpha = toolbar_coverage * (TOOLBAR_ALPHA as f32 / 255.0);
            let mut alpha = toolbar_alpha
                + (1.0 - toolbar_alpha)
                    * (bezel_coverage + (1.0 - bezel_coverage) * shadow);
            let toolbar_red = (TOOLBAR_COLOR >> 16) & 0xff;
            let toolbar_green = (TOOLBAR_COLOR >> 8) & 0xff;
            let toolbar_blue = TOOLBAR_COLOR & 0xff;
            let mut red = toolbar_red as f32 * toolbar_alpha
                + bezel_red as f32 * bezel_coverage * (1.0 - toolbar_alpha);
            let mut green = toolbar_green as f32 * toolbar_alpha
                + bezel_green as f32 * bezel_coverage * (1.0 - toolbar_alpha);
            let mut blue = toolbar_blue as f32 * toolbar_alpha
                + bezel_blue as f32 * bezel_coverage * (1.0 - toolbar_alpha);

            // Dots paint opaquely over the toolbar.
            for (rect, color) in dots {
                let cx = (rect.left + rect.right) as f32 / 2.0;
                let cy = (rect.top + rect.bottom) as f32 / 2.0;
                let distance = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
                let coverage = (TOOLBAR_DOT_RADIUS + 0.5 - distance).clamp(0.0, 1.0);
                if coverage > 0.0 {
                    let dot_red = ((color >> 16) & 0xff) as f32;
                    let dot_green = ((color >> 8) & 0xff) as f32;
                    let dot_blue = (color & 0xff) as f32;
                    red = red * (1.0 - coverage) + dot_red * coverage;
                    green = green * (1.0 - coverage) + dot_green * coverage;
                    blue = blue * (1.0 - coverage) + dot_blue * coverage;
                    alpha = alpha.max(coverage);
                }
            }

            let a = (alpha.clamp(0.0, 1.0) * 255.0).round() as u32;
            pixels.push(
                (a << 24)
                    | ((red.round() as u32).min(255) << 16)
                    | ((green.round() as u32).min(255) << 8)
                    | (blue.round() as u32).min(255),
            );
        }
    }
    pixels
}
