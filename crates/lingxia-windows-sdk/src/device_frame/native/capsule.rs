//! Floating iOS-style lxapp capsule: a topmost, layered pill pinned over the
//! content's top-right corner with a "…" menu button (lxapp info) and a
//! close button. Like the corner overlays, it must be topmost to sit above the
//! focused WebView2 surface, and is hidden while the screen is in the
//! background so it never floats over another app.

use super::*;

use crate::WindowsDesignIcon;
use crate::design_icons::design_icon_argb_premultiplied;

const CAPSULE_ICON: i32 = 18;
const CAPSULE_PAD: i32 = 11;
const CAPSULE_GAP: i32 = 12;
const CAPSULE_H: i32 = 30;
const CAPSULE_SHADOW: i32 = 7;
/// Inset from the content's top-right corner.
const CAPSULE_INSET: i32 = 12;
/// Nav-bar row height the shell reserves below the status bar (matches the
/// shell `SHELL_TOP_BAR_HEIGHT`); the capsule centers in it.
const NAV_BAR_ROW_HEIGHT: i32 = 32;
/// Lift the capsule a few px above the nav-bar center so it sits a touch higher
/// (toward the status bar) without reaching the status bar's signal/battery.
const CAPSULE_NAV_LIFT: i32 = 6;
const CAPSULE_RADIUS: f32 = 15.0;
/// Pill fill: white at ~95% (straight alpha, premultiplied below).
const CAPSULE_FILL_ALPHA: f32 = 0.95;
/// Icon + divider tint (iOS secondary-label gray).
const CAPSULE_ICON_TINT: u32 = 0x3C3C43;

/// Pixel geometry of the capsule window (origin at the window's top-left,
/// which sits `CAPSULE_SHADOW` outside the pill on every side).
struct CapsuleGeometry {
    width: i32,
    height: i32,
    pill: RECT,
    divider_x: i32,
    left_icon: RECT,
    right_icon: RECT,
}

fn capsule_geometry() -> CapsuleGeometry {
    let pill_w = CAPSULE_PAD * 2 + CAPSULE_ICON * 2 + CAPSULE_GAP * 2 + 1;
    let width = pill_w + CAPSULE_SHADOW * 2;
    let height = CAPSULE_H + CAPSULE_SHADOW * 2;
    let pill = RECT {
        left: CAPSULE_SHADOW,
        top: CAPSULE_SHADOW,
        right: CAPSULE_SHADOW + pill_w,
        bottom: CAPSULE_SHADOW + CAPSULE_H,
    };
    let icon_top = pill.top + (CAPSULE_H - CAPSULE_ICON) / 2;
    let left_icon_left = pill.left + CAPSULE_PAD;
    let divider_x = left_icon_left + CAPSULE_ICON + CAPSULE_GAP;
    let right_icon_left = divider_x + 1 + CAPSULE_GAP;
    CapsuleGeometry {
        width,
        height,
        pill,
        divider_x,
        left_icon: RECT {
            left: left_icon_left,
            top: icon_top,
            right: left_icon_left + CAPSULE_ICON,
            bottom: icon_top + CAPSULE_ICON,
        },
        right_icon: RECT {
            left: right_icon_left,
            top: icon_top,
            right: right_icon_left + CAPSULE_ICON,
            bottom: icon_top + CAPSULE_ICON,
        },
    }
}

fn capsule_class() -> PCWSTR {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let module = unsafe { LibraryLoader::GetModuleHandleW(None) }
            .map(|module| HINSTANCE(module.0))
            .unwrap_or_default();
        // Without an explicit class cursor the pill inherits whatever cursor
        // was last shown (often the window-edge resize double-arrow), which is
        // confusing over a small overlay. Pin a normal arrow.
        let cursor =
            unsafe { WindowsAndMessaging::LoadCursorW(None, WindowsAndMessaging::IDC_ARROW) }
                .unwrap_or_default();
        let class = WNDCLASSW {
            lpfnWndProc: Some(capsule_proc),
            hInstance: module,
            hCursor: cursor,
            lpszClassName: w!("LingXiaDeviceCapsule"),
            ..Default::default()
        };
        if unsafe { WindowsAndMessaging::RegisterClassW(&class) } == 0 {
            log::error!(
                "device capsule class registration failed: {}",
                windows::core::Error::from_thread()
            );
        }
    });
    w!("LingXiaDeviceCapsule")
}

/// Creates the capsule overlay owned by `content` and uploads its bitmap.
/// Returns the window handle, or `None` on failure / when the toolbar has no
/// capsule actions.
pub(super) fn create_capsule_window(content: HWND, spec: &WindowsDeviceFrame) -> Option<isize> {
    let has_menu = spec
        .toolbar
        .as_ref()
        .is_some_and(|toolbar| !toolbar.capsule_items.is_empty());
    if !has_menu {
        return None;
    }
    let geometry = capsule_geometry();
    let capsule = unsafe {
        WindowsAndMessaging::CreateWindowExW(
            WindowsAndMessaging::WS_EX_LAYERED
                | WindowsAndMessaging::WS_EX_TOOLWINDOW
                | WindowsAndMessaging::WS_EX_NOACTIVATE
                | WindowsAndMessaging::WS_EX_TOPMOST,
            capsule_class(),
            PCWSTR::null(),
            WindowsAndMessaging::WS_POPUP,
            0,
            0,
            geometry.width,
            geometry.height,
            Some(content),
            None,
            LibraryLoader::GetModuleHandleW(None)
                .ok()
                .map(|module| HINSTANCE(module.0)),
            None,
        )
    };
    let capsule = match capsule {
        Ok(capsule) => capsule,
        Err(err) => {
            log::warn!("device capsule window creation failed: {err}");
            return None;
        }
    };
    paint_capsule(capsule, &geometry);
    Some(hwnd_handle(capsule))
}

/// Renders the pill (white rounded rect + soft shadow + divider) and composites
/// the two design icons, then uploads via `UpdateLayeredWindow`.
fn paint_capsule(capsule: HWND, geometry: &CapsuleGeometry) {
    let mut pixels = capsule_pixels(geometry);
    if let Some(menu) = design_icon_argb_premultiplied(
        WindowsDesignIcon::CapsuleMenu,
        CAPSULE_ICON as u32,
        Some(CAPSULE_ICON_TINT),
    ) {
        blit_premultiplied(
            &mut pixels,
            geometry.width,
            geometry.left_icon,
            &menu,
            CAPSULE_ICON,
        );
    }
    if let Some(close) = design_icon_argb_premultiplied(
        WindowsDesignIcon::CapsuleClose,
        CAPSULE_ICON as u32,
        Some(CAPSULE_ICON_TINT),
    ) {
        blit_premultiplied(
            &mut pixels,
            geometry.width,
            geometry.right_icon,
            &close,
            CAPSULE_ICON,
        );
    }
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
                    biWidth: geometry.width,
                    biHeight: -geometry.height,
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
                let size = SIZE {
                    cx: geometry.width,
                    cy: geometry.height,
                };
                let origin = POINT { x: 0, y: 0 };
                let blend = BLENDFUNCTION {
                    BlendOp: AC_SRC_OVER as u8,
                    BlendFlags: 0,
                    SourceConstantAlpha: 255,
                    AlphaFormat: AC_SRC_ALPHA as u8,
                };
                let _ = WindowsAndMessaging::UpdateLayeredWindow(
                    capsule,
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

/// Premultiplied ARGB for the pill: white rounded rect, soft shadow, and a
/// 1px divider between the two halves.
fn capsule_pixels(geometry: &CapsuleGeometry) -> Vec<u32> {
    let pill = geometry.pill;
    let cx = (pill.left + pill.right) as f32 / 2.0;
    let cy = (pill.top + pill.bottom) as f32 / 2.0;
    let half_x = (pill.right - pill.left) as f32 / 2.0 - CAPSULE_RADIUS;
    let half_y = (pill.bottom - pill.top) as f32 / 2.0 - CAPSULE_RADIUS;
    let rounded_distance = |x: f32, y: f32| -> f32 {
        let qx = (x - cx).abs() - half_x;
        let qy = (y - cy).abs() - half_y;
        let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
        outside + qx.max(qy).min(0.0) - CAPSULE_RADIUS
    };
    let mut pixels = Vec::with_capacity((geometry.width * geometry.height) as usize);
    for y in 0..geometry.height {
        for x in 0..geometry.width {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let dist = rounded_distance(px, py);
            let fill = (0.5 - dist).clamp(0.0, 1.0) * CAPSULE_FILL_ALPHA;
            // Soft drop shadow, biased slightly downward.
            let shadow_d = rounded_distance(px, py - 1.0).max(0.0);
            let shadow = (1.0 - shadow_d / CAPSULE_SHADOW as f32).clamp(0.0, 1.0);
            let shadow_a = 0.18 * shadow * shadow;
            let mut alpha = fill + (1.0 - fill) * shadow_a;
            let mut r = 255.0 * fill;
            let mut g = 255.0 * fill;
            let mut b = 255.0 * fill;
            // Divider line inside the pill.
            if x == geometry.divider_x
                && py > pill.top as f32 + 7.0
                && py < pill.bottom as f32 - 7.0
            {
                let dr = ((CAPSULE_ICON_TINT >> 16) & 0xff) as f32;
                let dg = ((CAPSULE_ICON_TINT >> 8) & 0xff) as f32;
                let db = (CAPSULE_ICON_TINT & 0xff) as f32;
                let da = 0.25;
                r = r * (1.0 - da) + dr * da;
                g = g * (1.0 - da) + dg * da;
                b = b * (1.0 - da) + db * da;
                alpha = alpha.max(da);
            }
            let a = (alpha.clamp(0.0, 1.0) * 255.0).round() as u32;
            pixels.push(
                (a << 24)
                    | ((r.round() as u32).min(255) << 16)
                    | ((g.round() as u32).min(255) << 8)
                    | (b.round() as u32).min(255),
            );
        }
    }
    pixels
}

/// Source-over composite of a premultiplied-ARGB icon into the pill buffer.
fn blit_premultiplied(pixels: &mut [u32], width: i32, dst: RECT, icon: &[u32], icon_size: i32) {
    for iy in 0..icon_size {
        for ix in 0..icon_size {
            let src = icon[(iy * icon_size + ix) as usize];
            let sa = src >> 24;
            if sa == 0 {
                continue;
            }
            let (px, py) = (dst.left + ix, dst.top + iy);
            if px < 0 || py < 0 || px >= width {
                continue;
            }
            let di = (py * width + px) as usize;
            if di >= pixels.len() {
                continue;
            }
            let bg = pixels[di];
            let inv = 255 - sa;
            let ch = |shift: u32| {
                let s = (src >> shift) & 0xff;
                let d = (bg >> shift) & 0xff;
                (s + d * inv / 255).min(255)
            };
            let a = (sa + ((bg >> 24) & 0xff) * inv / 255).min(255);
            pixels[di] = (a << 24) | (ch(16) << 16) | (ch(8) << 8) | ch(0);
        }
    }
}

/// Moves the capsule to the content's top-right corner and pins it topmost
/// above the (focused) WebView2 surface.
pub(super) fn reposition_capsule(content: HWND) {
    let handle = hwnd_handle(content);
    let Some(capsule) = frame_state(handle, |state| state.capsule).filter(|c| *c != 0) else {
        return;
    };
    let capsule = hwnd_from_handle(capsule);
    if !is_window_handle_valid(hwnd_handle(capsule)) {
        return;
    }
    let mut rect = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetWindowRect(content, &mut rect);
    }
    // Sit in the nav-bar row, just below the simulated status bar, vertically
    // centered so the capsule aligns with the nav bar's title/buttons (mirrors
    // the macOS runner, which floats the capsule in the nav-bar region under the
    // status bar). The nav-bar row matches the shell's top bar height.
    let status_bar_height = frame_state(handle, |state| {
        state
            .spec
            .status_bar
            .as_ref()
            .map(|bar| bar.height)
            .unwrap_or(0)
    })
    .unwrap_or(0);
    let geometry = capsule_geometry();
    let pill_w = geometry.pill.right - geometry.pill.left;
    let x = rect.right - CAPSULE_INSET - pill_w - CAPSULE_SHADOW;
    let y = if status_bar_height > 0 {
        let nav_center = ((NAV_BAR_ROW_HEIGHT - CAPSULE_H).max(0)) / 2;
        rect.top + status_bar_height + nav_center - CAPSULE_NAV_LIFT - CAPSULE_SHADOW
    } else {
        rect.top + CAPSULE_INSET - CAPSULE_SHADOW
    };
    unsafe {
        let _ = WindowsAndMessaging::SetWindowPos(
            capsule,
            Some(WindowsAndMessaging::HWND_TOPMOST),
            x,
            y,
            0,
            0,
            WindowsAndMessaging::SWP_NOSIZE
                | WindowsAndMessaging::SWP_NOACTIVATE
                | WindowsAndMessaging::SWP_NOOWNERZORDER
                | WindowsAndMessaging::SWP_SHOWWINDOW,
        );
    }
}

pub(super) fn hide_capsule(content: HWND) {
    if let Some(capsule) =
        frame_state(hwnd_handle(content), |state| state.capsule).filter(|c| *c != 0)
    {
        unsafe {
            let _ = WindowsAndMessaging::ShowWindow(
                hwnd_from_handle(capsule),
                WindowsAndMessaging::SW_HIDE,
            );
        }
    }
}

pub(super) fn destroy_capsule(capsule: isize) {
    if capsule != 0 && is_window_handle_valid(capsule) {
        unsafe {
            let _ = WindowsAndMessaging::DestroyWindow(hwnd_from_handle(capsule));
        }
    }
}

unsafe extern "system" fn capsule_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WindowsAndMessaging::WM_NCHITTEST {
        let geometry = capsule_geometry();
        let screen_x = (lparam.0 & 0xffff) as i16 as i32;
        let screen_y = ((lparam.0 >> 16) & 0xffff) as i16 as i32;
        let mut rect = RECT::default();
        unsafe {
            let _ = WindowsAndMessaging::GetWindowRect(hwnd, &mut rect);
        }
        let x = screen_x - rect.left;
        let y = screen_y - rect.top;
        let inside = x >= geometry.pill.left
            && x < geometry.pill.right
            && y >= geometry.pill.top
            && y < geometry.pill.bottom;
        let hit = if inside {
            WindowsAndMessaging::HTCLIENT
        } else {
            WindowsAndMessaging::HTTRANSPARENT as u32
        };
        return LRESULT(hit as isize);
    } else if msg == WindowsAndMessaging::WM_MOUSEACTIVATE {
        return LRESULT(WindowsAndMessaging::MA_NOACTIVATE as isize);
    } else if msg == WindowsAndMessaging::WM_LBUTTONDOWN {
        let x = (lparam.0 & 0xffff) as i16 as i32;
        let geometry = capsule_geometry();
        let owner = unsafe { WindowsAndMessaging::GetWindow(hwnd, WindowsAndMessaging::GW_OWNER) }
            .unwrap_or_default();
        // The left "…" half opens the lxapp info sheet; the right circle is the
        // close button, which the caller (dev runner) maps to "quit emulator".
        if x < geometry.divider_x {
            dispatch_capsule_primary_command(owner);
        } else {
            dispatch_capsule_close_command(owner);
        }
        return LRESULT(0);
    }
    unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// Dispatches the capsule menu button's primary command. The runner maps this
/// to the app-info bottom sheet; lifecycle actions live in the device selector.
fn dispatch_capsule_primary_command(owner: HWND) {
    if owner.is_invalid() {
        return;
    }
    let Some(toolbar) =
        frame_state(hwnd_handle(owner), |state| state.spec.toolbar.clone()).flatten()
    else {
        return;
    };
    if let Some(id) = toolbar
        .capsule_items
        .iter()
        .find(|item| item.id != 0)
        .map(|item| item.id)
    {
        dispatch_device_frame_command(id);
    }
}

/// Dispatches the capsule close (right) circle's command. The dev runner maps
/// it to quitting the emulator; an unset command leaves the circle inert.
fn dispatch_capsule_close_command(owner: HWND) {
    if owner.is_invalid() {
        return;
    }
    let Some(command) = frame_state(hwnd_handle(owner), |state| {
        state
            .spec
            .toolbar
            .as_ref()
            .and_then(|toolbar| toolbar.capsule_close_command)
    })
    .flatten() else {
        return;
    };
    dispatch_device_frame_command(command);
}
