//! Native device-frame presentation mechanic for Windows host windows.
//!
//! Presents a host window as a fixed-size framed content surface:
//! the host window is restyled borderless at the content size, its corners
//! are rounded with a window region plus the anti-aliased corner-cap
//! overlays (see `window.rs`), and a per-pixel-alpha layered companion
//! window is kept glued behind it, painting what a GDI window region alone
//! cannot: the optional toolbar, the bezel with anti-aliased outer corners,
//! and a soft drop shadow.
//!
//! Device sizes, radii, colors, toolbar labels, and toolbar command ids are
//! supplied by the host layer through [`WindowsDeviceFrame`]. Dragging the
//! toolbar or the bezel moves the assembly.

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::{Arc, Mutex, OnceLock};

use crate::window_host::{
    find_host_window_for_webview, post_to_window_thread,
    request_host_window_layout as request_sdk_host_window_layout,
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
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, HDC,
    HGDIOBJ, ReleaseDC, SelectObject, SetWindowRgn,
};
use windows::Win32::System::LibraryLoader;
use windows::Win32::UI::WindowsAndMessaging::{self, WNDCLASSW, WNDPROC};
use windows::core::{PCWSTR, w};

use super::{WindowsDeviceFrame, WindowsDeviceFrameToolbar};

mod about_sheet;
mod capsule;
mod cutout;
mod frame_window;
mod paint;

pub(super) use about_sheet::{DeviceFrameInfoSheet, InfoSheetBadge, SheetAction};
use capsule::{create_capsule_window, destroy_capsule, hide_capsule, reposition_capsule};
use cutout::{create_cutout_window, destroy_cutout, hide_cutout, reposition_cutout};
use frame_window::create_frame_window;

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
    about_sheet::show_info_sheet(hwnd_from_handle(window), info);
}

/// Timer used while the content or shell window is in an interactive
/// move/size loop. It mirrors the host window's live-layout cadence but is
/// owned here because the device-frame shell is Windows-host chrome.
const SIZEMOVE_TIMER_ID: usize = 0x4C58_4456; // "LXDV"

const SIZEMOVE_TIMER_INTERVAL_MS: u32 = 16;

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

pub(super) fn set_frame_overlays_visible(content: isize, visible: bool) {
    if !window_has_frame(content) || !is_window_handle_valid(content) {
        return;
    }
    let hwnd = hwnd_from_handle(content);
    if visible {
        reposition_capsule(hwnd);
        reposition_cutout(hwnd);
    } else {
        about_sheet::dismiss_about_sheet_for_content(content);
        hide_capsule(hwnd);
        hide_cutout(hwnd);
    }
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

/// Applies `spec` to `content` (on its UI thread): restyles the window
/// borderless at screen size, rounds its corners, and creates/updates the
/// layered shell window behind it.
fn apply_device_frame(content: HWND, spec: WindowsDeviceFrame) {
    if spec.screen_width <= 0 || spec.screen_height <= 0 {
        log::warn!("ignoring device frame with empty screen: {spec:?}");
        return;
    }
    install_device_frame_subclass(content);
    let handle = hwnd_handle(content);
    if frame_state(handle, |state| state.spec.clone()) == Some(spec.clone()) {
        sync_device_frame_for_content(content);
        return;
    }
    // A different device: rebuild the shell window, but keep the originally
    // saved style so repeated device switches don't save the borderless
    // style as the restore target.
    let saved_style = match frame_state(handle, |state| {
        (state.saved_style, state.frame, state.capsule, state.cutout)
    }) {
        Some((saved, old_frame, old_capsule, old_cutout)) => {
            // Destroy only the old bezel window; keep the registry entry so the
            // window stays "framed" across the rebuild and the shell never
            // briefly un-suppresses its caption (the entry is overwritten with
            // the new frame below). A sync racing the stale frame handle is
            // guarded by `is_window_handle_valid`.
            about_sheet::dismiss_about_sheet_for_content(handle);
            unsafe {
                let _ = WindowsAndMessaging::DestroyWindow(hwnd_from_handle(old_frame));
            }
            destroy_capsule(old_capsule);
            destroy_cutout(old_cutout);
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

    let Some((frame, layout)) = create_frame_window(content, &spec, layout) else {
        return;
    };
    let capsule = create_capsule_window(content, &spec).unwrap_or(0);
    let cutout = create_cutout_window(content, &spec).unwrap_or(0);
    let frames = DEVICE_FRAMES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut frames) = frames.lock() {
        frames.insert(
            handle,
            DeviceFrameState {
                frame: hwnd_handle(frame),
                capsule,
                cutout,
                spec,
                layout,
                saved_style,
            },
        );
    }
    // A device switch or rotation changed the screen size; keep the window on
    // screen and re-layout the shell chrome so the webview fills the new
    // dimensions (the SWP resize alone leaves the old content width).
    unsafe {
        let _ = WindowsAndMessaging::ShowWindow(content, WindowsAndMessaging::SW_SHOWNA);
    }
    request_host_window_layout(content);
    sync_device_frame_for_content(content);
    // The host layout pass resizes the WebView2 surface asynchronously, and a
    // freshly-activated screen can re-composite it above the capsule; re-pin it
    // once that work lands (same-thread FIFO).
    post_to_window_thread(
        handle,
        Box::new(move || reposition_capsule(hwnd_from_handle(handle))),
    );
    post_to_window_thread(
        handle,
        Box::new(move || reposition_cutout(hwnd_from_handle(handle))),
    );
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
    about_sheet::dismiss_about_sheet_for_content(handle);
    unsafe {
        let _ = WindowsAndMessaging::DestroyWindow(hwnd_from_handle(state.frame));
    }
    destroy_capsule(state.capsule);
    destroy_cutout(state.cutout);
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
        about_sheet::dismiss_about_sheet_for_content(content);
        unsafe {
            let _ = WindowsAndMessaging::DestroyWindow(hwnd_from_handle(state.frame));
        }
        destroy_capsule(state.capsule);
        destroy_cutout(state.cutout);
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
        about_sheet::dismiss_about_sheet_for_content(handle);
        unsafe {
            let _ = WindowsAndMessaging::ShowWindow(frame, WindowsAndMessaging::SW_HIDE);
        }
        hide_capsule(content);
        hide_cutout(content);
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
    // The capsule is a top-level overlay owned by the screen window; re-pin it
    // to the new top-right corner and keep it above the WebView2 surface.
    reposition_capsule(content);
    reposition_cutout(content);
    about_sheet::reposition_about_sheet(content);
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
        about_sheet::dismiss_about_sheet_for_content(hwnd_handle(hwnd));
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
        // The capsule is topmost; hide it while the screen is in the background
        // (WA_INACTIVE == 0) so it never floats over another app, and re-pin it
        // on return.
        if (wparam.0 & 0xffff) == 0 {
            hide_capsule(hwnd);
            hide_cutout(hwnd);
        } else {
            reposition_capsule(hwnd);
            reposition_cutout(hwnd);
            about_sheet::reposition_about_sheet(hwnd);
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
        }
    } else if msg == WindowsAndMessaging::WM_DESTROY {
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
