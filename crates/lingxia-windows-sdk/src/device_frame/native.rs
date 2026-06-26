//! Native device-frame presentation mechanic for Windows host windows.
//!
//! Presents a host window as a fixed-size framed content surface:
//! the host window is restyled borderless at the content size, its screen
//! corners are rounded by anti-aliased companion overlays (see
//! `corner_caps.rs`) painted over the WebView2 surface, and a per-pixel-alpha
//! layered companion window is kept glued behind it, painting the optional
//! toolbar, the bezel with anti-aliased outer corners, and a soft drop
//! shadow.
//!
//! Device sizes, radii, colors, toolbar labels, and toolbar command ids are
//! supplied by the host layer through [`WindowsDeviceFrame`]. Dragging the
//! toolbar or the bezel moves the assembly.

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::{Arc, Mutex, OnceLock};

#[cfg(feature = "shell-chrome")]
use crate::shell::WindowsShellTabBarPosition;
use crate::window_host::{
    find_host_window_for_webview, post_to_window_thread,
    request_host_window_layout_forced as request_sdk_host_window_layout,
};
use lingxia_webview::WebTag;
use lingxia_windows_host::{WindowsHostWindow, add_host_window_created_handler};
use windows::Win32::Foundation::{
    COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM,
};
use windows::Win32::Graphics::Dwm::{
    DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND, DWMWCP_ROUND, DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, ClientToScreen,
    CreateCompatibleDC, CreateDIBSection, CreateRoundRectRgn, DIB_RGB_COLORS, DeleteDC,
    DeleteObject, GetDC, HDC, HGDIOBJ, ReleaseDC, SelectObject, SetWindowRgn,
};
use windows::Win32::System::LibraryLoader;
use windows::Win32::UI::WindowsAndMessaging::{self, WNDCLASSW, WNDPROC};
use windows::core::{PCWSTR, w};

use super::{WindowsDeviceFrame, WindowsDeviceFrameStatusBar, WindowsDeviceFrameToolbar};

mod capsule;
mod corner_mask;
mod cutout;
mod frame_window;
mod info_sheet;
mod paint;
mod status_bar;

use capsule::{create_capsule_window, destroy_capsule, hide_capsule, reposition_capsule};
use corner_mask::{
    create_corner_mask, destroy_corner_mask, hide_corner_mask, reposition_corner_mask,
};
use cutout::{create_cutout_window, destroy_cutout, hide_cutout, reposition_cutout};
use frame_window::create_frame_window;
pub(super) use info_sheet::{DeviceFrameInfoSheet, InfoSheetBadge, SheetAction};
use status_bar::{
    create_status_bar, destroy_status_bar, hide_status_bar, repaint_status_bar,
    reposition_status_bar,
};

pub(super) type WindowsDeviceFrameCommandHandler = Arc<dyn Fn(u32) + Send + Sync>;

static DEVICE_FRAME_COMMAND_HANDLER: OnceLock<Mutex<Option<WindowsDeviceFrameCommandHandler>>> =
    OnceLock::new();
static INITIAL_DEVICE_FRAME: OnceLock<Mutex<Option<WindowsDeviceFrame>>> = OnceLock::new();
static INITIAL_DEVICE_FRAME_HOOK: OnceLock<()> = OnceLock::new();

pub(super) fn set_device_frame_command_handler(handler: WindowsDeviceFrameCommandHandler) {
    let slot = DEVICE_FRAME_COMMAND_HANDLER.get_or_init(|| Mutex::new(None));
    if let Ok(mut slot) = slot.lock() {
        *slot = Some(handler);
    }
}

pub(super) fn set_initial_device_frame(frame: WindowsDeviceFrame) {
    let slot = INITIAL_DEVICE_FRAME.get_or_init(|| Mutex::new(None));
    if let Ok(mut slot) = slot.lock() {
        *slot = Some(frame);
    }
    INITIAL_DEVICE_FRAME_HOOK.get_or_init(|| {
        // Consume the armed frame on the next host window only — framing every
        // window the lxapp creates would leak a bezel + capsule per window. A
        // runner re-arms this slot before a restart so the recreated window
        // comes back framed.
        add_host_window_created_handler(Arc::new(|window| {
            let frame = INITIAL_DEVICE_FRAME
                .get()
                .and_then(|slot| slot.lock().ok())
                .and_then(|mut slot| slot.take());
            if let Some(frame) = frame {
                apply_device_frame(hwnd_from_handle(window), frame);
            }
        }));
    });
}

pub(super) fn show_info_sheet(window: isize, info: DeviceFrameInfoSheet) {
    info_sheet::show_info_sheet(hwnd_from_handle(window), info);
}

/// Timer used while the content or shell window is in an interactive
/// move/size loop. It mirrors the host window's live-layout cadence but is
/// owned here because the device-frame shell is Windows-host chrome.
const SIZEMOVE_TIMER_ID: usize = 0x4C58_4456; // "LXDV"

const SIZEMOVE_TIMER_INTERVAL_MS: u32 = 16;

/// Timer that ticks the simulated status-bar clock.
const CLOCK_TIMER_ID: usize = 0x4C58_434B; // "LXCK"
const CLOCK_TIMER_INTERVAL_MS: u32 = 20_000;
const CONTENT_REGION_MAX_RADIUS: i32 = 36;

fn dispatch_device_frame_command(id: u32) -> bool {
    let handler = DEVICE_FRAME_COMMAND_HANDLER
        .get()
        .and_then(|slot| slot.lock().ok())
        .and_then(|slot| slot.clone());
    let Some(handler) = handler else {
        return false;
    };
    let _ = std::thread::Builder::new()
        .name(format!("lingxia-windows-device-frame-{id}"))
        .spawn(move || handler(id));
    true
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
    rotate_rect: RECT,
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
        content_offset: (margin + spec.bezel_width, bezel_top + spec.bezel_width),
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
    /// The floating capsule overlay pinned over the content (0 when absent).
    capsule: isize,
    /// The top-centered phone screen cutout overlay (0 when absent).
    cutout: isize,
    /// The overlay that rounds the screen corners over the WebView2
    /// surface (0 when the device keeps square corners).
    corner_mask: isize,
    /// The top status-bar overlay (time + signal/battery), 0 when absent.
    status_bar: isize,
    spec: WindowsDeviceFrame,
    layout: FrameLayout,
    /// `GWL_STYLE` of the content window before the borderless restyle.
    saved_style: isize,
}

/// Active device-frame presentations, keyed by content window handle.
static DEVICE_FRAMES: OnceLock<Mutex<HashMap<isize, DeviceFrameState>>> = OnceLock::new();

#[derive(Debug, Clone, Copy)]
struct DeviceFrameWindowState {
    original_proc: isize,
}

static WINDOW_STATES: OnceLock<Mutex<HashMap<isize, DeviceFrameWindowState>>> = OnceLock::new();

/// Last content-window screen rect a sync repositioned the frame + overlays to,
/// keyed by content window. A page switch (tab tap, device selector) fires
/// geometry/z-order changes that don't move the window; re-running the shell +
/// overlay `SetWindowPos` for an unchanged rect makes the bezel and corner
/// overlays visibly jitter. Skipping the no-op reposition removes that.
static LAST_SYNC_RECT: OnceLock<Mutex<HashMap<isize, RECT>>> = OnceLock::new();

fn rects_equal(a: &RECT, b: &RECT) -> bool {
    a.left == b.left && a.top == b.top && a.right == b.right && a.bottom == b.bottom
}

fn last_sync_rect(content: isize) -> Option<RECT> {
    LAST_SYNC_RECT
        .get()
        .and_then(|map| map.lock().ok())
        .and_then(|map| map.get(&content).copied())
}

fn set_last_sync_rect(content: isize, rect: RECT) {
    let map = LAST_SYNC_RECT.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut map) = map.lock() {
        map.insert(content, rect);
    }
}

fn clear_last_sync_rect(content: isize) {
    if let Some(map) = LAST_SYNC_RECT.get()
        && let Ok(mut map) = map.lock()
    {
        map.remove(&content);
    }
}

fn frame_state<T>(content: isize, read: impl FnOnce(&DeviceFrameState) -> T) -> Option<T> {
    DEVICE_FRAMES
        .get()
        .and_then(|frames| frames.lock().ok())
        .and_then(|frames| frames.get(&content).map(read))
}

/// True while `content` (a top-level host window handle) is wrapped in a
/// simulator device frame. The shell drops its own window caption on a framed
/// screen, since the frame's toolbar owns those controls.
#[cfg_attr(not(feature = "browser-runtime"), allow(dead_code))]
pub(super) fn window_has_frame(content: isize) -> bool {
    frame_state(content, |_| ()).is_some()
}

/// Updates the simulated status bar's foreground + background for `content` and
/// repaints it (on the window thread). The shell calls this per page so the bar
/// color extends the page's navigation-bar color and the text stays legible.
#[cfg_attr(not(feature = "shell-chrome"), allow(dead_code))]
pub(super) fn set_status_bar_style(
    content: isize,
    foreground: u32,
    background: u32,
    transparent: bool,
) {
    let changed = DEVICE_FRAMES
        .get()
        .and_then(|frames| frames.lock().ok())
        .map(|mut frames| {
            let Some(state) = frames.get_mut(&content) else {
                return false;
            };
            let Some(bar) = state.spec.status_bar.as_mut() else {
                return false;
            };
            if bar.foreground == foreground
                && bar.background == background
                && bar.transparent == transparent
            {
                return false;
            }
            bar.foreground = foreground;
            bar.background = background;
            bar.transparent = transparent;
            true
        })
        .unwrap_or(false);
    if changed && is_window_handle_valid(content) {
        repaint_status_bar(hwnd_from_handle(content));
    }
}

/// Height of the simulated status bar for the framed window `content`, or 0 when
/// it is not framed / has no status bar. The shell reserves this strip so its
/// nav bar + content sit below the status bar overlay.
#[cfg_attr(not(feature = "shell-chrome"), allow(dead_code))]
pub(super) fn status_bar_height(content: isize) -> i32 {
    frame_state(content, |state| {
        state
            .spec
            .status_bar
            .as_ref()
            .map(|bar| bar.height)
            .unwrap_or(0)
    })
    .unwrap_or(0)
}

pub(super) fn set_frame_overlays_visible(content: isize, visible: bool) {
    if !window_has_frame(content) || !is_window_handle_valid(content) {
        return;
    }
    let hwnd = hwnd_from_handle(content);
    // The overlays must never float over another app: a caller asking to show
    // them while the screen is actually minimized or hidden (a layout pass can
    // race a minimize) is treated as "hide". This mirrors the gate in
    // `sync_device_frame_for_content`.
    let truly_visible = visible
        && unsafe {
            WindowsAndMessaging::IsWindowVisible(hwnd).as_bool()
                && !WindowsAndMessaging::IsIconic(hwnd).as_bool()
        };
    if truly_visible {
        reposition_status_bar(hwnd);
        reposition_cutout(hwnd);
        reposition_corner_mask(hwnd);
        reposition_capsule(hwnd);
    } else {
        info_sheet::dismiss_info_sheet_for_content(content);
        hide_capsule(hwnd);
        hide_cutout(hwnd);
        hide_corner_mask(hwnd);
        hide_status_bar(hwnd);
    }
}

fn hide_device_frame_chrome(content: HWND) {
    let handle = hwnd_handle(content);
    info_sheet::dismiss_info_sheet_for_content(handle);
    if let Some(frame) = frame_state(handle, |state| state.frame).filter(|frame| *frame != 0)
        && is_window_handle_valid(frame)
    {
        unsafe {
            let _ = WindowsAndMessaging::ShowWindow(
                hwnd_from_handle(frame),
                WindowsAndMessaging::SW_HIDE,
            );
        }
    }
    hide_capsule(content);
    hide_cutout(content);
    hide_corner_mask(content);
    hide_status_bar(content);
    // Force the next active sync to re-pin everything after a hide.
    clear_last_sync_rect(handle);
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
pub(super) fn set_webview_device_frame(
    webtag: &WebTag,
    frame: Option<WindowsDeviceFrame>,
) -> Result<(), String> {
    let host_window = find_host_window_for_webview(webtag).map_err(|err| err.to_string())?;
    let handle = host_window.window;
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
        Err("device frame target window is not accepting messages".to_string())
    }
}

#[cfg(feature = "shell-chrome")]
pub(super) fn set_webview_device_frame_and_tabbar_position(
    webtag: &WebTag,
    appid: String,
    frame: WindowsDeviceFrame,
    tabbar_position: WindowsShellTabBarPosition,
) -> Result<(), String> {
    let host_window = find_host_window_for_webview(webtag).map_err(|err| err.to_string())?;
    let handle = host_window.window;
    let posted = post_to_window_thread(
        handle,
        Box::new(move || {
            let content = hwnd_from_handle(handle);
            apply_device_frame_deferred_layout(content, frame);
            crate::shell::set_windows_shell_tabbar_position_on_window_thread(
                &appid,
                tabbar_position,
            );
            request_host_window_layout(content);
        }),
    );
    if posted {
        Ok(())
    } else {
        Err("device frame target window is not accepting messages".to_string())
    }
}

/// Applies `spec` to `content` (on its UI thread): restyles the window
/// borderless at screen size, rounds its corners, and creates/updates the
/// layered shell window behind it.
fn apply_device_frame(content: HWND, spec: WindowsDeviceFrame) {
    apply_device_frame_inner(content, spec, true);
}

#[cfg(feature = "shell-chrome")]
fn apply_device_frame_deferred_layout(content: HWND, spec: WindowsDeviceFrame) {
    apply_device_frame_inner(content, spec, false);
}

fn apply_device_frame_inner(content: HWND, mut spec: WindowsDeviceFrame, sync_host_layout: bool) {
    if spec.screen_width <= 0 || spec.screen_height <= 0 {
        log::warn!("ignoring device frame with empty screen: {spec:?}");
        return;
    }
    install_device_frame_subclass(content);
    let handle = hwnd_handle(content);
    // The status bar's transparent/foreground/background are page-driven — the
    // shell sets them per active page (e.g. an immersive `custom` page floats a
    // transparent strip). The device frame only owns the bar's geometry, so
    // carry the live style across a re-apply. Otherwise re-framing (a device
    // switch, or the window-created hook re-firing) rebuilds with the opaque
    // `frame_spec` default and clobbers the immersive page's transparent strip.
    // Merging before the equality check also makes a same-device re-apply
    // compare equal, downgrading it to a reposition-only sync (no rebuild).
    if let (Some(old_sb), Some(new_sb)) = (
        frame_state(handle, |state| state.spec.status_bar.clone()).flatten(),
        spec.status_bar.as_mut(),
    ) {
        new_sb.transparent = old_sb.transparent;
        new_sb.foreground = old_sb.foreground;
        new_sb.background = old_sb.background;
    }
    if frame_state(handle, |state| state.spec.clone()) == Some(spec.clone()) {
        if sync_host_layout {
            sync_device_frame_for_content(content);
        }
        return;
    }
    // A different device: rebuild the shell window, but keep the originally
    // saved style so repeated device switches don't save the borderless
    // style as the restore target.
    let saved_style = match frame_state(handle, |state| {
        (
            state.saved_style,
            state.frame,
            state.capsule,
            state.cutout,
            state.corner_mask,
            state.status_bar,
        )
    }) {
        Some((saved, old_frame, old_capsule, old_cutout, old_corner_mask, old_status_bar)) => {
            // Destroy only the old bezel window; keep the registry entry so the
            // window stays "framed" across the rebuild and the shell never
            // briefly un-suppresses its caption (the entry is overwritten with
            // the new frame below). A sync racing the stale frame handle is
            // guarded by `is_window_handle_valid`.
            info_sheet::dismiss_info_sheet_for_content(handle);
            unsafe {
                let _ = WindowsAndMessaging::DestroyWindow(hwnd_from_handle(old_frame));
            }
            destroy_capsule(old_capsule);
            destroy_cutout(old_cutout);
            destroy_corner_mask(old_corner_mask);
            destroy_status_bar(old_status_bar);
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
        // The menu bar cannot live on the device screen. Higher host layers
        // own any alternate menu presentation for simulator chrome.
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
    apply_content_screen_region(content, &spec);

    let Some((frame, layout)) = create_frame_window(content, &spec, layout) else {
        return;
    };
    let capsule = create_capsule_window(content, &spec).unwrap_or(0);
    let cutout = create_cutout_window(content, &spec).unwrap_or(0);
    let corner_mask = create_corner_mask(content, &spec);
    let status_bar = create_status_bar(content, &spec);
    let frames = DEVICE_FRAMES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut frames) = frames.lock() {
        frames.insert(
            handle,
            DeviceFrameState {
                frame: hwnd_handle(frame),
                capsule,
                cutout,
                corner_mask,
                status_bar,
                spec,
                layout,
                saved_style,
            },
        );
    }
    // Tick the status-bar clock while the frame is up.
    if status_bar != 0 {
        unsafe {
            let _ = WindowsAndMessaging::SetTimer(
                Some(content),
                CLOCK_TIMER_ID,
                CLOCK_TIMER_INTERVAL_MS,
                None,
            );
        }
    }
    // A device switch or rotation changed the screen size; keep the content
    // window visible/active before revealing the companion frame. Otherwise the
    // layered frame can appear first, leaving a transparent phone shell until
    // the user clicks the taskbar icon and Windows activates the content window.
    foreground_content_window(content);
    if sync_host_layout {
        request_host_window_layout(content);
    }
    sync_device_frame_for_content(content);
    // The host layout pass resizes the WebView2 surface asynchronously, and a
    // freshly-activated screen can re-composite it above the capsule; re-pin it
    // once that work lands (same-thread FIFO).
    post_to_window_thread(
        handle,
        Box::new(move || reposition_status_bar(hwnd_from_handle(handle))),
    );
    post_to_window_thread(
        handle,
        Box::new(move || reposition_cutout(hwnd_from_handle(handle))),
    );
    post_to_window_thread(
        handle,
        Box::new(move || reposition_corner_mask(hwnd_from_handle(handle))),
    );
    post_to_window_thread(
        handle,
        Box::new(move || reposition_capsule(hwnd_from_handle(handle))),
    );
    schedule_startup_frame_resync(handle);
}

/// Removes the device frame from `content` (on its UI thread) and restores
/// the standard window: original styles and DWM rounded corners.
fn clear_device_frame(content: HWND) {
    let handle = hwnd_handle(content);
    let removed = DEVICE_FRAMES
        .get()
        .and_then(|frames| frames.lock().ok())
        .and_then(|mut frames| frames.remove(&handle));
    let Some(state) = removed else {
        return;
    };
    info_sheet::dismiss_info_sheet_for_content(handle);
    clear_last_sync_rect(handle);
    unsafe {
        let _ = WindowsAndMessaging::KillTimer(Some(content), CLOCK_TIMER_ID);
        let _ = WindowsAndMessaging::DestroyWindow(hwnd_from_handle(state.frame));
    }
    destroy_capsule(state.capsule);
    destroy_cutout(state.cutout);
    destroy_corner_mask(state.corner_mask);
    destroy_status_bar(state.status_bar);
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
    request_host_window_layout(content);
}

/// Drops the registry entry of a content window that is going away and
/// destroys its shell window.
fn forget_device_frame(content: HWND) {
    remove_frame_window(hwnd_handle(content));
}

fn remove_frame_window(content: isize) {
    let removed = DEVICE_FRAMES
        .get()
        .and_then(|frames| frames.lock().ok())
        .and_then(|mut frames| frames.remove(&content));
    if let Some(state) = removed {
        info_sheet::dismiss_info_sheet_for_content(content);
        clear_last_sync_rect(content);
        unsafe {
            let _ = WindowsAndMessaging::DestroyWindow(hwnd_from_handle(state.frame));
        }
        destroy_capsule(state.capsule);
        destroy_cutout(state.cutout);
        destroy_corner_mask(state.corner_mask);
        destroy_status_bar(state.status_bar);
    }
}

/// Glues the shell window to the content window: hidden while the content
/// is minimized or hidden, otherwise offset around it and kept directly
/// below it in z-order. Runs on every content geometry change (see
/// `handle_window_geometry_change`) and after shell-initiated moves.
fn sync_device_frame_for_content(content: HWND) {
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
        hide_device_frame_chrome(content);
        return;
    }
    let mut content_rect = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetWindowRect(content, &mut content_rect);
    }
    // A page switch / z-order change fires this with the window in the same
    // place. The frame + overlays are already pinned there, so re-running
    // their SetWindowPos only makes them flicker — skip the no-op.
    if last_sync_rect(handle).is_some_and(|last| rects_equal(&last, &content_rect)) {
        return;
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
    // Overlay z-order (back to front): the opaque status bar sits behind, then
    // the cutout + corner mask, then the capsule on top (it's interactive and
    // may sit slightly over the status bar's lower edge). Re-pinning in this
    // order keeps the capsule from being clipped by the status bar.
    reposition_status_bar(content);
    reposition_cutout(content);
    reposition_corner_mask(content);
    reposition_capsule(content);
    info_sheet::reposition_info_sheet(content);
    set_last_sync_rect(handle, content_rect);
}

fn install_device_frame_subclass(hwnd: HWND) {
    let states = WINDOW_STATES.get_or_init(|| Mutex::new(HashMap::new()));
    if states
        .lock()
        .map(|states| states.contains_key(&hwnd_handle(hwnd)))
        .unwrap_or(true)
    {
        return;
    }
    let original = unsafe {
        WindowsAndMessaging::SetWindowLongPtrW(
            hwnd,
            WindowsAndMessaging::GWLP_WNDPROC,
            device_frame_host_proc as *const () as usize as isize,
        )
    };
    if original == 0 {
        log::warn!("failed to subclass device-frame host window {hwnd:?}");
        return;
    }
    if let Ok(mut states) = states.lock() {
        states.insert(
            hwnd_handle(hwnd),
            DeviceFrameWindowState {
                original_proc: original,
            },
        );
    }
}

unsafe extern "system" fn device_frame_host_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let original = device_frame_window_state(hwnd).map(|state| state.original_proc);
    if msg == WindowsAndMessaging::WM_WINDOWPOSCHANGED {
        let pos = lparam.0 as *const WindowsAndMessaging::WINDOWPOS;
        if !pos.is_null() {
            let flags = unsafe { (*pos).flags };
            let sized = !flags.contains(WindowsAndMessaging::SWP_NOSIZE)
                || flags.contains(WindowsAndMessaging::SWP_FRAMECHANGED);
            let moved = !flags.contains(WindowsAndMessaging::SWP_NOMOVE);
            let z_order_only =
                !sized && !moved && !flags.contains(WindowsAndMessaging::SWP_NOZORDER);
            // A page switch can hide then re-show the content window (e.g. when
            // a tab brings up a page hosting native component overlays). The
            // glued bezel must track that visibility, or it stays hidden after
            // the screen comes back.
            let shown_or_hidden = flags.contains(WindowsAndMessaging::SWP_SHOWWINDOW)
                || flags.contains(WindowsAndMessaging::SWP_HIDEWINDOW);
            if sized || moved || z_order_only || shown_or_hidden {
                sync_device_frame_for_content(hwnd);
            }
        }
    } else if msg == WindowsAndMessaging::WM_SIZE
        && wparam.0 == WindowsAndMessaging::SIZE_MINIMIZED as usize
    {
        hide_device_frame_chrome(hwnd);
    } else if msg == WindowsAndMessaging::WM_SHOWWINDOW {
        // `ShowWindow` reports visibility changes through `WM_SHOWWINDOW`, not
        // always `WM_WINDOWPOSCHANGED`; re-glue (or hide) the bezel to match.
        let result = original
            .map(|original| unsafe { call_original(original, hwnd, msg, wparam, lparam) })
            .unwrap_or_else(|| unsafe {
                WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam)
            });
        sync_device_frame_for_content(hwnd);
        return result;
    } else if msg == WindowsAndMessaging::WM_ACTIVATE {
        // Re-pin the companion windows when the content returns to the
        // foreground; deactivation alone should not hide them because Windows
        // may briefly report inactive during startup or no-activate sheet
        // interactions.
        if (wparam.0 & 0xffff) != 0 {
            // On return, re-pin everything — *including* the bezel frame, which
            // is z-glued directly below the content window (not topmost) and so
            // gets stranded behind whatever was in front while we were in the
            // background. Reactivation doesn't move the window, so the rect cache
            // would short-circuit the sync; clear it to force a full re-pin
            // (frame SetWindowPos + every overlay), otherwise the device-frame
            // toolbar stays hidden until the next move/resize.
            clear_last_sync_rect(hwnd_handle(hwnd));
            sync_device_frame_for_content(hwnd);
        }
    } else if msg == WindowsAndMessaging::WM_ENTERSIZEMOVE {
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
        sync_device_frame_for_content(hwnd);
    } else if msg == WindowsAndMessaging::WM_TIMER {
        if wparam.0 == SIZEMOVE_TIMER_ID {
            sync_device_frame_for_content(hwnd);
            return LRESULT(0);
        } else if wparam.0 == CLOCK_TIMER_ID {
            repaint_status_bar(hwnd);
            return LRESULT(0);
        }
    } else if msg == WindowsAndMessaging::WM_DESTROY {
        unsafe {
            let _ = WindowsAndMessaging::KillTimer(Some(hwnd), CLOCK_TIMER_ID);
        }
        forget_device_frame(hwnd);
    } else if msg == WindowsAndMessaging::WM_NCDESTROY {
        forget_device_frame(hwnd);
        let result = original
            .map(|original| unsafe { call_original(original, hwnd, msg, wparam, lparam) })
            .unwrap_or_else(|| unsafe {
                WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam)
            });
        remove_device_frame_window_state(hwnd);
        return result;
    }
    original
        .map(|original| unsafe { call_original(original, hwnd, msg, wparam, lparam) })
        .unwrap_or_else(|| unsafe {
            WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam)
        })
}

fn device_frame_window_state(hwnd: HWND) -> Option<DeviceFrameWindowState> {
    WINDOW_STATES
        .get()
        .and_then(|states| states.lock().ok())
        .and_then(|states| states.get(&hwnd_handle(hwnd)).copied())
}

fn remove_device_frame_window_state(hwnd: HWND) -> Option<DeviceFrameWindowState> {
    WINDOW_STATES
        .get()
        .and_then(|states| states.lock().ok())
        .and_then(|mut states| states.remove(&hwnd_handle(hwnd)))
}

unsafe fn call_original(
    original: isize,
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let proc: WNDPROC = unsafe { std::mem::transmute(original) };
    unsafe { WindowsAndMessaging::CallWindowProcW(proc, hwnd, msg, wparam, lparam) }
}

fn request_host_window_layout(hwnd: HWND) {
    let _ = request_sdk_host_window_layout(WindowsHostWindow {
        window: hwnd_handle(hwnd),
    });
}

fn foreground_content_window(hwnd: HWND) {
    clear_last_sync_rect(hwnd_handle(hwnd));
    unsafe {
        let _ = WindowsAndMessaging::ShowWindow(hwnd, WindowsAndMessaging::SW_SHOWNORMAL);
        let _ = WindowsAndMessaging::BringWindowToTop(hwnd);
        let _ = WindowsAndMessaging::SetForegroundWindow(hwnd);
        let _ = windows::Win32::UI::Input::KeyboardAndMouse::SetFocus(Some(hwnd));
    }
}

fn schedule_startup_frame_resync(handle: isize) {
    let _ = std::thread::Builder::new()
        .name("lingxia-device-frame-startup-sync".to_string())
        .spawn(move || {
            for delay_ms in [80_u64, 220] {
                std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                let _ = post_to_window_thread(
                    handle,
                    Box::new(move || {
                        let hwnd = hwnd_from_handle(handle);
                        if !is_window_handle_valid(handle) {
                            return;
                        }
                        foreground_content_window(hwnd);
                        request_host_window_layout(hwnd);
                        sync_device_frame_for_content(hwnd);
                    }),
                );
            }
        });
}

fn apply_round_corner_preference(hwnd: HWND) {
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

fn apply_content_screen_region(content: HWND, spec: &WindowsDeviceFrame) {
    let radius = spec
        .screen_corner_radius
        .clamp(0, CONTENT_REGION_MAX_RADIUS);
    if radius <= 0 {
        unsafe {
            let _ = SetWindowRgn(content, None, true);
        }
        return;
    }
    unsafe {
        let region = CreateRoundRectRgn(
            0,
            0,
            spec.screen_width + 1,
            spec.screen_height + 1,
            radius * 2,
            radius * 2,
        );
        let applied = SetWindowRgn(content, Some(region), true);
        if applied == 0 {
            let _ = DeleteObject(HGDIOBJ(region.0));
        }
    }
}

fn hwnd_handle(hwnd: HWND) -> isize {
    hwnd.0 as isize
}

fn hwnd_from_handle(handle: isize) -> HWND {
    HWND(handle as *mut c_void)
}

fn is_window_handle_valid(handle: isize) -> bool {
    unsafe { WindowsAndMessaging::IsWindow(Some(hwnd_from_handle(handle))).as_bool() }
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
