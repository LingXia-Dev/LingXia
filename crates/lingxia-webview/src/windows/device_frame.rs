//! Generic device-frame presentation mechanic for top-level host windows.
//!
//! Presents a webview host window as a fixed-size framed content surface:
//! the host window is restyled borderless at the content size, its corners
//! are rounded with a window region plus the anti-aliased corner-cap
//! overlays (see `window.rs`), and a per-pixel-alpha layered companion
//! window is kept glued behind it, painting what a GDI window region alone
//! cannot: the optional toolbar, the bezel with anti-aliased outer corners,
//! and a soft drop shadow.
//!
//! Like the menu-bar mechanic, this module owns no policy: device sizes,
//! radii, colors, the toolbar labels, and the command ids behind the
//! selector and the action glyph are all supplied by the product layer
//! through [`WindowsDeviceFrame`]. Toolbar selections are dispatched
//! through the registered app-menu command handler (see `menu.rs`).
//! Dragging the toolbar or the bezel moves the assembly; a right-click
//! offers the installed app-menu model as a context menu.

use super::*;

mod frame_window;
mod paint;

use frame_window::create_frame_window;

use windows::Win32::Graphics::Dwm::DWMWCP_DONOTROUND;
use windows::Win32::Graphics::Gdi::{CreateRoundRectRgn, SetWindowRgn};

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

/// Visual description of one framed content surface, in physical pixels.
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
    /// Toolbar floating above the frame, when present.
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
    let hwnd = webview_host_hwnd(webtag)?;
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
