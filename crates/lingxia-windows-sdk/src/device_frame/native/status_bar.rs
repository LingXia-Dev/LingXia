//! Simulated iOS status bar overlay: time (leading) + signal/battery (trailing).
//!
//! Like the [`cutout`](super::cutout), this is a topmost owned-popup per-pixel
//! alpha layered window, so it composites above the windowed WebView2 surface
//! (which can't be clipped). The hosted app's top safe-area sits beneath it.
//!
//! The strip is painted as an *opaque* fill (the page navigation-bar color, set
//! by the shell, or the chrome color for a plain page) so the bar color extends
//! up over the status bar like the macOS runner. The time + signal/battery are
//! drawn in the foreground color directly over that opaque fill — GDI text
//! anti-aliases cleanly against it, so the alpha is simply restored to full
//! afterwards (no transparent-text fringing to work around).

use super::*;

use windows::Win32::Graphics::Gdi::{
    CreateFontW, FW_SEMIBOLD, GetTextExtentPoint32W, SetBkMode, SetTextColor, TRANSPARENT, TextOutW,
};
use windows::Win32::System::SystemInformation::GetLocalTime;

/// Current local time formatted like the iOS status bar (12-hour, no AM/PM).
fn current_time_string() -> String {
    let now = unsafe { GetLocalTime() };
    let hour12 = match now.wHour % 12 {
        0 => 12,
        other => other,
    };
    format!("{hour12}:{:02}", now.wMinute)
}

/// Inset of the time / battery from the screen's side edges.
const SIDE_MARGIN: i32 = 16;

fn status_bar_class() -> PCWSTR {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let module = unsafe { LibraryLoader::GetModuleHandleW(None) }
            .map(|module| HINSTANCE(module.0))
            .unwrap_or_default();
        let class = WNDCLASSW {
            lpfnWndProc: Some(status_bar_proc),
            hInstance: module,
            lpszClassName: w!("LingXiaDeviceStatusBar"),
            ..Default::default()
        };
        if unsafe { WindowsAndMessaging::RegisterClassW(&class) } == 0 {
            log::error!(
                "device status bar class registration failed: {}",
                windows::core::Error::from_thread()
            );
        }
    });
    w!("LingXiaDeviceStatusBar")
}

unsafe extern "system" fn status_bar_proc(
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

/// Creates the status bar overlay (topmost layered popup owned by `content`).
/// Returns `0` when the device has no status bar. Created hidden;
/// [`reposition_status_bar`] places and shows it.
pub(super) fn create_status_bar(content: HWND, spec: &WindowsDeviceFrame) -> isize {
    let Some(bar) = spec
        .status_bar
        .as_ref()
        .filter(|bar| bar.height > 0 && spec.screen_width > 0)
        .cloned()
    else {
        return 0;
    };
    let window = unsafe {
        WindowsAndMessaging::CreateWindowExW(
            WindowsAndMessaging::WS_EX_LAYERED
                | WindowsAndMessaging::WS_EX_TOOLWINDOW
                | WindowsAndMessaging::WS_EX_TRANSPARENT
                | WindowsAndMessaging::WS_EX_NOACTIVATE
                | WindowsAndMessaging::WS_EX_TOPMOST,
            status_bar_class(),
            PCWSTR::null(),
            WindowsAndMessaging::WS_POPUP,
            0,
            0,
            spec.screen_width,
            bar.height,
            Some(content),
            None,
            LibraryLoader::GetModuleHandleW(None)
                .ok()
                .map(|module| HINSTANCE(module.0)),
            None,
        )
    };
    match window {
        Ok(window) => {
            paint_status_bar(window, spec.screen_width, &bar);
            hwnd_handle(window)
        }
        Err(err) => {
            log::warn!("device status bar creation failed for {content:?}: {err}");
            0
        }
    }
}

/// Repaints the status bar from the (possibly updated) spec — used when the
/// shell changes the bar's colors for a new page.
pub(super) fn repaint_status_bar(content: HWND) {
    let Some((handle, bar, width)) = frame_state(hwnd_handle(content), |state| {
        (
            state.status_bar,
            state.spec.status_bar.clone(),
            state.spec.screen_width,
        )
    }) else {
        return;
    };
    let Some(bar) = bar.filter(|bar| bar.height > 0) else {
        return;
    };
    if handle == 0 || !is_window_handle_valid(handle) {
        return;
    }
    paint_status_bar(hwnd_from_handle(handle), width, &bar);
}

/// Re-pins the status bar to the top edge of the screen and keeps it topmost.
pub(super) fn reposition_status_bar(content: HWND) {
    let Some((status_bar, height)) = frame_state(hwnd_handle(content), |state| {
        (
            state.status_bar,
            state
                .spec
                .status_bar
                .as_ref()
                .map(|bar| bar.height)
                .unwrap_or(0),
        )
    })
    .filter(|(status_bar, height)| *status_bar != 0 && *height > 0) else {
        return;
    };
    if !is_window_handle_valid(status_bar) {
        return;
    }
    let mut rect = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetWindowRect(content, &mut rect);
    }
    unsafe {
        let _ = WindowsAndMessaging::SetWindowPos(
            hwnd_from_handle(status_bar),
            Some(WindowsAndMessaging::HWND_TOPMOST),
            rect.left,
            rect.top,
            rect.right - rect.left,
            height,
            WindowsAndMessaging::SWP_NOACTIVATE
                | WindowsAndMessaging::SWP_NOOWNERZORDER
                | WindowsAndMessaging::SWP_SHOWWINDOW,
        );
    }
}

pub(super) fn hide_status_bar(content: HWND) {
    if let Some(status_bar) =
        frame_state(hwnd_handle(content), |state| state.status_bar).filter(|bar| *bar != 0)
    {
        unsafe {
            let _ = WindowsAndMessaging::ShowWindow(
                hwnd_from_handle(status_bar),
                WindowsAndMessaging::SW_HIDE,
            );
        }
    }
}

pub(super) fn destroy_status_bar(status_bar: isize) {
    if status_bar != 0 && is_window_handle_valid(status_bar) {
        unsafe {
            let _ = WindowsAndMessaging::DestroyWindow(hwnd_from_handle(status_bar));
        }
    }
}

fn paint_status_bar(window: HWND, width: i32, bar: &WindowsDeviceFrameStatusBar) {
    let width = width.max(1);
    let height = bar.height.max(1);
    // Opaque strip fill + analytic signal/battery glyphs on the trailing edge.
    let pixels = status_bar_pixels(width, height, bar.foreground, bar.background);
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
            if let Ok(bitmap) =
                CreateDIBSection(Some(screen_dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)
                && !bits.is_null()
            {
                std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits.cast::<u32>(), pixels.len());
                let old_bitmap = SelectObject(memory_dc, HGDIOBJ(bitmap.0));
                draw_time(memory_dc, height, bar.foreground);
                // GDI text zeroes the alpha byte of every pixel it touches; the
                // strip is opaque, so restore full alpha across it (the RGB it
                // wrote already blends the foreground over the opaque fill).
                let dib = std::slice::from_raw_parts_mut(bits.cast::<u32>(), pixels.len());
                force_opaque(dib);
                let size = SIZE {
                    cx: width,
                    cy: height,
                };
                let origin = POINT { x: 0, y: 0 };
                let blend = BLENDFUNCTION {
                    BlendOp: AC_SRC_OVER as u8,
                    BlendFlags: 0,
                    SourceConstantAlpha: 255,
                    AlphaFormat: AC_SRC_ALPHA as u8,
                };
                let _ = WindowsAndMessaging::UpdateLayeredWindow(
                    window,
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

fn draw_time(dc: HDC, height: i32, foreground: u32) {
    let time = current_time_string();
    let font_height = -(height * 5 / 16).clamp(13, 22);
    let font = unsafe {
        CreateFontW(
            font_height,
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
    let wide = to_wide(&time);
    let chars = &wide[..wide.len().saturating_sub(1)];
    unsafe {
        SetBkMode(dc, TRANSPARENT);
        let old_font = SelectObject(dc, HGDIOBJ(font.0));
        // GDI COLORREF is 0x00BBGGRR; convert from the 0xRRGGBB foreground.
        let bgr = ((foreground & 0xff) << 16) | (foreground & 0xff00) | ((foreground >> 16) & 0xff);
        SetTextColor(dc, COLORREF(bgr));
        let mut extent = SIZE::default();
        let _ = GetTextExtentPoint32W(dc, chars, &mut extent);
        let y = (height - extent.cy) / 2;
        let _ = TextOutW(dc, SIDE_MARGIN, y, chars);
        if !old_font.is_invalid() {
            let _ = SelectObject(dc, old_font);
        }
        let _ = DeleteObject(HGDIOBJ(font.0));
    }
}

/// Restores full alpha across the opaque strip after GDI text zeroed the alpha
/// byte of every pixel it touched. The RGB GDI wrote already blends the
/// foreground over the opaque fill, so only the alpha needs fixing.
fn force_opaque(dib: &mut [u32]) {
    for pixel in dib.iter_mut() {
        *pixel = 0xff00_0000 | (*pixel & 0x00ff_ffff);
    }
}

/// Opaque ARGB strip: `bg` fill plus the trailing signal bars + battery (in
/// `fg`), vertically centered and inset by `SIDE_MARGIN` from the right edge.
fn status_bar_pixels(width: i32, height: i32, fg: u32, bg: u32) -> Vec<u32> {
    let bg_opaque = 0xff00_0000 | (bg & 0x00ff_ffff);
    let mut pixels = vec![bg_opaque; (width * height).max(0) as usize];
    let fg_opaque = 0xff00_0000 | (fg & 0x00ff_ffff);
    let center_y = height / 2;

    let fill = |pixels: &mut [u32], x0: i32, y0: i32, x1: i32, y1: i32| {
        for y in y0.max(0)..y1.min(height) {
            for x in x0.max(0)..x1.min(width) {
                pixels[(y * width + x) as usize] = fg_opaque;
            }
        }
    };

    // Battery: rounded-ish body outline + fill + tip at the far right.
    let body_w = 24;
    let body_h = 12;
    let tip_w = 2;
    let tip_h = 5;
    let body_right = width - SIDE_MARGIN - tip_w - 1;
    let body_left = body_right - body_w;
    let body_top = center_y - body_h / 2;
    let body_bottom = body_top + body_h;
    // Outline (1px border).
    fill(&mut pixels, body_left, body_top, body_right, body_top + 1);
    fill(&mut pixels, body_left, body_bottom - 1, body_right, body_bottom);
    fill(&mut pixels, body_left, body_top, body_left + 1, body_bottom);
    fill(&mut pixels, body_right - 1, body_top, body_right, body_bottom);
    // Inner fill (full battery), inset 2px.
    fill(
        &mut pixels,
        body_left + 2,
        body_top + 2,
        body_right - 2,
        body_bottom - 2,
    );
    // Tip.
    fill(
        &mut pixels,
        body_right,
        center_y - tip_h / 2,
        body_right + tip_w,
        center_y - tip_h / 2 + tip_h,
    );

    // Signal: four bars of increasing height, bottom-aligned, left of battery.
    let bars = 4;
    let bar_w = 3;
    let bar_gap = 2;
    let bar_max_h = 10;
    let signal_right = body_left - 8;
    let signal_left = signal_right - (bars * bar_w + (bars - 1) * bar_gap);
    let baseline = center_y + bar_max_h / 2;
    for index in 0..bars {
        let bar_h = 3 + index * (bar_max_h - 3) / (bars - 1);
        let x0 = signal_left + index * (bar_w + bar_gap);
        fill(&mut pixels, x0, baseline - bar_h, x0 + bar_w, baseline);
    }

    pixels
}
