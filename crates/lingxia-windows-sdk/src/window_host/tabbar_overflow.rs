//! Device-frame TabBar overflow sheet.

use super::*;
use std::time::{Duration, Instant};
use windows::Win32::System::Threading::GetCurrentThreadId;

const TIMER_ID: usize = 0x5A1A;
const TIMER_MS: u32 = 16;
const ENTER_DURATION: Duration = Duration::from_millis(160);

#[derive(Clone)]
struct TabbarOverflowOverlay {
    window: isize,
    owner: isize,
    layout: crate::shell::TabbarOverflowLayout,
    screen_corner_radius: i32,
    opened_at: Instant,
}

static OVERLAYS: OnceLock<Mutex<HashMap<isize, TabbarOverflowOverlay>>> = OnceLock::new();

pub(crate) fn toggle_tabbar_overflow(owner: isize, tabbar: crate::shell::WindowsShellTabBarLayout) {
    if !post_to_window_thread(
        owner,
        Box::new(move || toggle_tabbar_overflow_on_thread(owner, tabbar)),
    ) {
        log::warn!("failed to post TabBar overflow sheet to owner={owner}");
    }
}

fn toggle_tabbar_overflow_on_thread(owner: isize, tabbar: crate::shell::WindowsShellTabBarLayout) {
    let owner_hwnd = hwnd_from_handle(owner);
    // The overlay is keyed only by the owner HWND — one device-frame window
    // hosts every lxapp. A leftover sheet from app A must not toggle-close
    // and skip app B's sheet; replace it in the same click.
    if let Some(existing) = overlay_for_owner(owner_hwnd) {
        let same_app = existing.layout.tabbar.group_id == tabbar.group_id;
        destroy_tabbar_overflow(owner_hwnd);
        if same_app {
            return;
        }
    }

    let mut client = RECT::default();
    unsafe {
        if WindowsAndMessaging::GetClientRect(owner_hwnd, &mut client).is_err() {
            return;
        }
    }
    let width = client.right - client.left;
    let height = client.bottom - client.top;
    let strip_top = active_webtag_key_for_window(owner_hwnd)
        .and_then(|key| {
            crate::shell::bottom_tabbar_rect(client, &current_window_layout(&key))
                .map(|rect| rect.top)
        })
        .unwrap_or_else(|| height.saturating_sub(tabbar.dimension));
    let Some(layout) = crate::shell::tabbar_overflow_layout(width, height, strip_top, tabbar)
    else {
        return;
    };

    #[cfg(feature = "device-frame")]
    let screen_corner_radius = crate::device_frame::device_frame_screen_clip_style(owner)
        .map(|(radius, _)| radius)
        .unwrap_or(0);
    #[cfg(not(feature = "device-frame"))]
    let screen_corner_radius = 0;

    let Ok(window) = (unsafe {
        WindowsAndMessaging::CreateWindowExW(
            WindowsAndMessaging::WS_EX_LAYERED
                | WindowsAndMessaging::WS_EX_TOOLWINDOW
                | WindowsAndMessaging::WS_EX_NOACTIVATE,
            tabbar_overflow_class(),
            PCWSTR::null(),
            WS_POPUP,
            0,
            0,
            width,
            height,
            Some(owner_hwnd),
            None,
            LibraryLoader::GetModuleHandleW(None)
                .ok()
                .map(|module| HINSTANCE(module.0)),
            None,
        )
    }) else {
        log::warn!(
            "TabBar overflow sheet creation failed: {}",
            windows::core::Error::from_thread()
        );
        return;
    };

    let overlay = TabbarOverflowOverlay {
        window: hwnd_handle(window),
        owner,
        layout,
        screen_corner_radius,
        opened_at: Instant::now(),
    };
    if OVERLAYS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map(|mut overlays| overlays.insert(owner, overlay.clone()))
        .is_err()
    {
        unsafe {
            let _ = WindowsAndMessaging::DestroyWindow(window);
        }
        return;
    }

    upload_tabbar_overflow(window, &overlay, 0.0);
    pin_tabbar_overflow(window, owner_hwnd, Some((width, height)));
    unsafe {
        let _ = WindowsAndMessaging::SetTimer(Some(window), TIMER_ID, TIMER_MS, None);
    }
}

fn overlay_for_owner(owner: HWND) -> Option<TabbarOverflowOverlay> {
    OVERLAYS
        .get()
        .and_then(|overlays| overlays.lock().ok())
        .and_then(|overlays| overlays.get(&hwnd_handle(owner)).cloned())
}

fn overlay_for_window(window: HWND) -> Option<TabbarOverflowOverlay> {
    OVERLAYS
        .get()
        .and_then(|overlays| overlays.lock().ok())
        .and_then(|overlays| {
            overlays
                .values()
                .find(|overlay| overlay.window == hwnd_handle(window))
                .cloned()
        })
}

pub(super) fn destroy_tabbar_overflow(owner: HWND) {
    let overlay = OVERLAYS
        .get()
        .and_then(|overlays| overlays.lock().ok())
        .and_then(|mut overlays| overlays.remove(&hwnd_handle(owner)));
    if let Some(overlay) = overlay
        && is_window_handle_valid(overlay.window)
    {
        unsafe {
            let _ =
                WindowsAndMessaging::KillTimer(Some(hwnd_from_handle(overlay.window)), TIMER_ID);
            let _ = WindowsAndMessaging::DestroyWindow(hwnd_from_handle(overlay.window));
        }
    }
}

pub(super) fn dismiss_tabbar_overflow(owner: HWND) -> bool {
    if overlay_for_owner(owner).is_none() {
        return false;
    }
    destroy_tabbar_overflow(owner);
    true
}

/// Owned `WS_POPUP` sheets do not move with their owner, so the More overlay
/// has to be re-pinned whenever the device window moves or restacks.
pub(super) fn reposition_tabbar_overflow(owner: HWND) {
    let Some(overlay) = overlay_for_owner(owner) else {
        return;
    };
    if !is_window_handle_valid(overlay.window) {
        return;
    }
    pin_tabbar_overflow(hwnd_from_handle(overlay.window), owner, None);
}

fn pin_tabbar_overflow(window: HWND, owner: HWND, size: Option<(i32, i32)>) {
    if !is_window_visible(owner) || is_minimized(owner) {
        unsafe {
            let _ = WindowsAndMessaging::ShowWindow(window, WindowsAndMessaging::SW_HIDE);
        }
        return;
    }
    let mut origin = POINT::default();
    let mut flags = WindowsAndMessaging::SWP_NOACTIVATE | WindowsAndMessaging::SWP_SHOWWINDOW;
    let (width, height) = match size {
        Some(size) => size,
        None => {
            flags |= WindowsAndMessaging::SWP_NOSIZE;
            (0, 0)
        }
    };
    unsafe {
        let _ = windows::Win32::Graphics::Gdi::ClientToScreen(owner, &mut origin);
        let _ = WindowsAndMessaging::SetWindowPos(
            window,
            Some(WindowsAndMessaging::HWND_TOP),
            origin.x,
            origin.y,
            width,
            height,
            flags,
        );
    }
}

/// Drop a leftover overflow sheet when the owner window presents another lxapp.
pub(crate) fn dismiss_tabbar_overflow_on_owner(owner: isize) {
    if owner == 0 {
        return;
    }
    let hwnd = hwnd_from_handle(owner);
    let owner_thread = unsafe { WindowsAndMessaging::GetWindowThreadProcessId(hwnd, None) };
    if owner_thread != 0 && owner_thread == unsafe { GetCurrentThreadId() } {
        dismiss_tabbar_overflow(hwnd);
        return;
    }
    if !post_to_window_thread(
        owner,
        Box::new(move || {
            dismiss_tabbar_overflow(hwnd_from_handle(owner));
        }),
    ) {
        log::warn!("failed to post TabBar overflow dismiss to owner={owner}");
    }
}

fn remove_destroyed_overlay(window: HWND) {
    if let Some(overlays) = OVERLAYS.get()
        && let Ok(mut overlays) = overlays.lock()
    {
        overlays.retain(|_, overlay| overlay.window != hwnd_handle(window));
    }
}

fn tabbar_overflow_class() -> PCWSTR {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let module = unsafe { LibraryLoader::GetModuleHandleW(None) }
            .map(|module| HINSTANCE(module.0))
            .unwrap_or_default();
        let cursor =
            unsafe { WindowsAndMessaging::LoadCursorW(None, WindowsAndMessaging::IDC_ARROW) }
                .unwrap_or_default();
        let class = WNDCLASSW {
            lpfnWndProc: Some(tabbar_overflow_proc),
            hInstance: module,
            hCursor: cursor,
            lpszClassName: w!("LingXiaTabbarOverflow"),
            ..Default::default()
        };
        if unsafe { WindowsAndMessaging::RegisterClassW(&class) } == 0 {
            log::error!(
                "TabBar overflow class registration failed: {}",
                windows::core::Error::from_thread()
            );
        }
    });
    w!("LingXiaTabbarOverflow")
}

unsafe extern "system" fn tabbar_overflow_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WindowsAndMessaging::WM_MOUSEACTIVATE => {
            LRESULT(WindowsAndMessaging::MA_NOACTIVATE as isize)
        }
        WindowsAndMessaging::WM_NCHITTEST => {
            if let Some(overlay) = overlay_for_window(hwnd) {
                let mut point = lparam_screen_point(lparam);
                unsafe {
                    let _ = windows::Win32::Graphics::Gdi::ScreenToClient(hwnd, &mut point);
                }
                if point.y >= overlay.layout.strip_top {
                    return LRESULT(WindowsAndMessaging::HTTRANSPARENT as isize);
                }
            }
            LRESULT(WindowsAndMessaging::HTCLIENT as isize)
        }
        WindowsAndMessaging::WM_ERASEBKGND => LRESULT(1),
        WindowsAndMessaging::WM_TIMER if wparam.0 == TIMER_ID => {
            if let Some(overlay) = overlay_for_window(hwnd) {
                let progress = enter_progress(&overlay);
                upload_tabbar_overflow(hwnd, &overlay, progress);
                if progress >= 1.0 {
                    let _ = unsafe { WindowsAndMessaging::KillTimer(Some(hwnd), TIMER_ID) };
                }
            }
            LRESULT(0)
        }
        WindowsAndMessaging::WM_LBUTTONUP => {
            if let Some(overlay) = overlay_for_window(hwnd) {
                if enter_progress(&overlay) < 1.0 {
                    return LRESULT(0);
                }
                let owner = hwnd_from_handle(overlay.owner);
                match crate::shell::tabbar_overflow_hit(
                    &overlay.layout,
                    lparam_client_point(lparam),
                ) {
                    crate::shell::TabbarOverflowHit::Item(slot) => {
                        // The hit is a position in the shipped list; the click
                        // has to name the item's declared index.
                        let Some(item) = overlay.layout.tabbar.items.get(slot) else {
                            return LRESULT(0);
                        };
                        let command = crate::shell::collapsed_sidebar_tabbar_click_command(
                            &overlay.layout.tabbar.group_id,
                            item.index,
                        );
                        destroy_tabbar_overflow(owner);
                        dispatch_phone_switcher_command(owner, command);
                    }
                    crate::shell::TabbarOverflowHit::Sheet => {}
                    crate::shell::TabbarOverflowHit::Dismiss => {
                        destroy_tabbar_overflow(owner);
                    }
                }
            }
            LRESULT(0)
        }
        WindowsAndMessaging::WM_DESTROY => {
            let _ = unsafe { WindowsAndMessaging::KillTimer(Some(hwnd), TIMER_ID) };
            remove_destroyed_overlay(hwnd);
            LRESULT(0)
        }
        _ => unsafe { WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn enter_progress(overlay: &TabbarOverflowOverlay) -> f32 {
    (overlay.opened_at.elapsed().as_secs_f32() / ENTER_DURATION.as_secs_f32()).clamp(0.0, 1.0)
}

fn upload_tabbar_overflow(hwnd: HWND, overlay: &TabbarOverflowOverlay, progress: f32) {
    let width = overlay.layout.width;
    let height = overlay.layout.height;
    if width <= 0 || height <= 0 {
        return;
    }
    unsafe {
        let screen = GetDC(None);
        if screen.is_invalid() {
            return;
        }
        let dc = CreateCompatibleDC(Some(screen));
        if dc.is_invalid() {
            let _ = ReleaseDC(None, screen);
            return;
        }
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut c_void = std::ptr::null_mut();
        let Ok(bitmap) = CreateDIBSection(Some(screen), &info, DIB_RGB_COLORS, &mut bits, None, 0)
        else {
            let _ = DeleteDC(dc);
            let _ = ReleaseDC(None, screen);
            return;
        };
        if bits.is_null() {
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = DeleteDC(dc);
            let _ = ReleaseDC(None, screen);
            return;
        }
        let old_bitmap = SelectObject(dc, HGDIOBJ(bitmap.0));
        let panel_height = overlay.layout.sheet.bottom - overlay.layout.sheet.top;
        let eased = 1.0 - (1.0 - progress) * (1.0 - progress);
        let panel_offset = ((1.0 - eased) * panel_height as f32).round() as i32;
        let text_runs = crate::shell::paint_tabbar_overflow(dc, &overlay.layout, panel_offset);
        let pixels = std::slice::from_raw_parts_mut(bits.cast::<u32>(), (width * height) as usize);
        apply_tabbar_overflow_alpha(
            pixels,
            width,
            height,
            overlay.layout.sheet,
            overlay.layout.strip_top,
            panel_offset,
            progress,
            overlay.screen_corner_radius,
        );
        // Geometric alpha already filled every pixel, so the transparent-only
        // mask compositor would skip the labels. Blend them over the panel.
        for text_run in text_runs {
            crate::layered_text::blend_supersampled_text_mask(
                dc,
                pixels,
                width,
                height,
                &text_run.text,
                text_run.rect,
                text_run.color,
                text_run.font_height,
                text_run.font_weight,
                true,
            );
        }
        let size = SIZE {
            cx: width,
            cy: height,
        };
        let origin = POINT::default();
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        let _ = WindowsAndMessaging::UpdateLayeredWindow(
            hwnd,
            Some(screen),
            None,
            Some(&size),
            Some(dc),
            Some(&origin),
            COLORREF(0),
            Some(&blend),
            WindowsAndMessaging::ULW_ALPHA,
        );
        if !old_bitmap.is_invalid() {
            let _ = SelectObject(dc, old_bitmap);
        }
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(dc);
        let _ = ReleaseDC(None, screen);
    }
}

fn apply_tabbar_overflow_alpha(
    pixels: &mut [u32],
    width: i32,
    height: i32,
    sheet: RECT,
    strip_top: i32,
    panel_offset: i32,
    progress: f32,
    screen_corner_radius: i32,
) {
    const DIM: f32 = 0x59 as f32;
    let panel = RECT {
        left: sheet.left,
        top: sheet.top + panel_offset,
        right: sheet.right,
        bottom: sheet.bottom,
    };
    let panel_radius = crate::shell::TABBAR_OVERFLOW_PANEL_RADIUS;
    let screen_radius = screen_corner_radius.clamp(0, width.min(height) / 2);
    let rounded_rect_coverage = |x: i32, y: i32| -> f32 {
        if x < panel.left || x >= panel.right || y < panel.top || y >= panel.bottom {
            return 0.0;
        }
        let radius = panel_radius
            .min((panel.right - panel.left) / 2)
            .min((panel.bottom - panel.top) / 2)
            .max(0);
        if radius == 0 {
            return 1.0;
        }
        let in_x_mid = x >= panel.left + radius && x < panel.right - radius;
        let in_y_mid = y >= panel.top + radius && y < panel.bottom - radius;
        if in_x_mid || in_y_mid {
            return 1.0;
        }
        let center_x = if x < panel.left + radius {
            panel.left + radius
        } else {
            panel.right - radius
        };
        let center_y = if y < panel.top + radius {
            panel.top + radius
        } else {
            panel.bottom - radius
        };
        let dx = x as f32 + 0.5 - center_x as f32;
        let dy = y as f32 + 0.5 - center_y as f32;
        (radius as f32 - (dx * dx + dy * dy).sqrt() + 0.5).clamp(0.0, 1.0)
    };
    let screen_coverage = |x: i32, y: i32| -> f32 {
        if screen_radius == 0 {
            return 1.0;
        }
        let center_x = if x < screen_radius {
            screen_radius
        } else if x >= width - screen_radius {
            width - screen_radius
        } else {
            return 1.0;
        };
        let center_y = if y < screen_radius {
            screen_radius
        } else if y >= height - screen_radius {
            height - screen_radius
        } else {
            return 1.0;
        };
        let dx = x as f32 + 0.5 - center_x as f32;
        let dy = y as f32 + 0.5 - center_y as f32;
        (screen_radius as f32 - (dx * dx + dy * dy).sqrt() + 0.5).clamp(0.0, 1.0)
    };

    for y in 0..height {
        for x in 0..width {
            let index = (y * width + x) as usize;
            let pixel = pixels[index];
            let alpha = if y >= strip_top {
                0.0
            } else {
                let coverage = rounded_rect_coverage(x, y);
                DIM * progress + (255.0 - DIM * progress) * coverage
            };
            let alpha = (alpha * screen_coverage(x, y)).round() as u32;
            let premultiply = |channel: u32| (channel * alpha + 127) / 255;
            pixels[index] = (alpha << 24)
                | (premultiply((pixel >> 16) & 0xff) << 16)
                | (premultiply((pixel >> 8) & 0xff) << 8)
                | premultiply(pixel & 0xff);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::apply_tabbar_overflow_alpha;
    use windows::Win32::Foundation::RECT;

    #[test]
    fn alpha_keeps_the_sliding_panel_clipped_above_the_tabbar() {
        let mut pixels = vec![0x00ff_ffff; 100 * 100];
        apply_tabbar_overflow_alpha(
            &mut pixels,
            100,
            100,
            RECT {
                left: 12,
                top: 40,
                right: 88,
                bottom: 80,
            },
            88,
            20,
            1.0,
            0,
        );
        assert_eq!(pixels[70 * 100 + 50] >> 24, 255);
        assert_eq!(pixels[70 * 100 + 4] >> 24, 0x59);
        assert_eq!(pixels[90 * 100 + 50] >> 24, 0);
    }
}
