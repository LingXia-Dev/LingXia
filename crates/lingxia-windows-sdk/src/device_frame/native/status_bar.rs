//! Simulated iOS status bar overlay: time (leading) + signal/battery (trailing).
//!
//! Like the [`cutout`](super::cutout), this is a topmost owned-popup per-pixel
//! alpha layered window, so it composites above the windowed WebView2 surface
//! (which can't be clipped). The hosted app's top safe-area sits beneath it.
//!
//! A normal page paints the strip as an *opaque* fill (the page navigation-bar
//! color, set by the shell, or the chrome color for a plain page) so the bar
//! color extends up over the status bar like the macOS runner. The time +
//! signal/battery are drawn in the foreground color directly over that opaque
//! fill — GDI text anti-aliases cleanly against it, so the alpha is simply
//! restored to full afterwards (no transparent-text fringing to work around).
//!
//! An immersive (custom navigation-style) page instead bleeds its WebView
//! content up under the bar, so the strip is painted *transparent*: only the
//! clock + indicators show, floating over the page. GDI can't write alpha, so
//! the clock is drawn in white with grayscale anti-aliasing and then recolored
//! to the foreground with per-pixel coverage as the alpha (premultiplied), and
//! the indicators are filled as already-premultiplied opaque foreground.

use super::*;

use windows::Win32::Graphics::Gdi::{
    ANTIALIASED_QUALITY, CLEARTYPE_QUALITY, CreateFontW, FONT_QUALITY, FW_BOLD,
    GetTextExtentPoint32W, SetBkMode, SetTextColor, TRANSPARENT, TextOutW,
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
            paint_status_bar(window, spec.screen_width, spec_cutout_width(spec), &bar);
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
    let Some((handle, bar, width, cutout_width)) = frame_state(hwnd_handle(content), |state| {
        (
            state.status_bar,
            state.spec.status_bar.clone(),
            state.spec.screen_width,
            spec_cutout_width(&state.spec),
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
    paint_status_bar(hwnd_from_handle(handle), width, cutout_width, &bar);
}

/// Width of the screen cutout (Dynamic Island / notch), or 0 when the device has
/// none — used to size the clock's leading ear.
fn spec_cutout_width(spec: &WindowsDeviceFrame) -> i32 {
    spec.cutout
        .as_ref()
        .filter(|cutout| cutout.width > 0 && cutout.height > 0)
        .map(|cutout| cutout.width)
        .unwrap_or(0)
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

fn paint_status_bar(window: HWND, width: i32, cutout_width: i32, bar: &WindowsDeviceFrameStatusBar) {
    let width = width.max(1);
    let height = bar.height.max(1);
    // The clock is centered in the leading "ear" — the space between the screen
    // edge and the (top-centered) cutout, matching the device's real status bar.
    // With no cutout the ear is the leading half of the bar.
    let clock_slot_right = (width - cutout_width.max(0)) / 2;
    // Opaque strip fill (or fully transparent for an immersive page) + analytic
    // signal/battery glyphs on the trailing edge.
    let pixels = if bar.transparent {
        status_bar_pixels_transparent(width, height, bar.foreground)
    } else {
        status_bar_pixels(width, height, bar.foreground, bar.background)
    };
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
                let dib = std::slice::from_raw_parts_mut(bits.cast::<u32>(), pixels.len());
                if bar.transparent {
                    // Draw the clock in white with grayscale AA so each pixel's
                    // luminance is its coverage, then recolor to the foreground
                    // with that coverage as the (premultiplied) alpha. The
                    // already-opaque indicators keep their alpha and are skipped.
                    draw_time(memory_dc, height, 0xff_ffff, true, clock_slot_right, cutout_width > 0);
                    premultiply_glyph_pixels(dib, bar.foreground);
                } else {
                    draw_time(memory_dc, height, bar.foreground, false, clock_slot_right, cutout_width > 0);
                    // GDI text zeroes the alpha byte of every pixel it touches;
                    // the strip is opaque, so restore full alpha across it (the
                    // RGB it wrote already blends the foreground over the fill).
                    force_opaque(dib);
                }
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

fn draw_time(dc: HDC, height: i32, color: u32, antialiased: bool, slot_right: i32, has_cutout: bool) {
    let time = current_time_string();
    let font_height = -(height * 5 / 16).clamp(13, 22);
    // A transparent strip derives per-pixel alpha from text coverage, so it must
    // use grayscale AA (ClearType's colored sub-pixels would fringe once
    // recolored). An opaque strip can use ClearType for the crispest text.
    let quality: FONT_QUALITY = if antialiased {
        ANTIALIASED_QUALITY
    } else {
        CLEARTYPE_QUALITY
    };
    let font = unsafe {
        CreateFontW(
            font_height,
            0,
            0,
            0,
            FW_BOLD.0 as i32,
            0,
            0,
            0,
            Default::default(),
            Default::default(),
            Default::default(),
            quality,
            Default::default(),
            w!("Segoe UI"),
        )
    };
    let wide = to_wide(&time);
    let chars = &wide[..wide.len().saturating_sub(1)];
    unsafe {
        SetBkMode(dc, TRANSPARENT);
        let old_font = SelectObject(dc, HGDIOBJ(font.0));
        // GDI COLORREF is 0x00BBGGRR; convert from the 0xRRGGBB color.
        let bgr = ((color & 0xff) << 16) | (color & 0xff00) | ((color >> 16) & 0xff);
        SetTextColor(dc, COLORREF(bgr));
        let mut extent = SIZE::default();
        let _ = GetTextExtentPoint32W(dc, chars, &mut extent);
        let y = (height - extent.cy) / 2;
        // With a cutout (notch / Dynamic Island), center the clock within the
        // leading "ear" [0, slot_right] left of it. With no cutout the device has
        // a flat pre-notch status bar, so left-align the clock at the side margin
        // (matching iOS on an iPhone SE) instead of floating it mid-left.
        let x = if has_cutout {
            ((slot_right - extent.cx) / 2).max(SIDE_MARGIN)
        } else {
            SIDE_MARGIN
        };
        let _ = TextOutW(dc, x, y, chars);
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

/// Recolors the white grayscale-AA clock GDI drew over a transparent strip into
/// premultiplied `fg`, using each touched pixel's coverage as its alpha. Pixels
/// that already carry alpha (the indicators) and untouched transparent pixels
/// are left as-is.
fn premultiply_glyph_pixels(dib: &mut [u32], fg: u32) {
    let fr = (fg >> 16) & 0xff;
    let fg_g = (fg >> 8) & 0xff;
    let fb = fg & 0xff;
    for pixel in dib.iter_mut() {
        if (*pixel >> 24) != 0 {
            continue; // already-opaque indicator pixel
        }
        // White-on-(transparent-)black text: max channel is the coverage.
        let r = (*pixel >> 16) & 0xff;
        let g = (*pixel >> 8) & 0xff;
        let b = *pixel & 0xff;
        let coverage = r.max(g).max(b);
        if coverage == 0 {
            continue; // untouched transparent pixel
        }
        let pr = fr * coverage / 255;
        let pg = fg_g * coverage / 255;
        let pb = fb * coverage / 255;
        *pixel = (coverage << 24) | (pr << 16) | (pg << 8) | pb;
    }
}

/// Opaque ARGB strip: `bg` fill plus the trailing signal bars + battery (in
/// `fg`), vertically centered and inset by `SIDE_MARGIN` from the right edge.
fn status_bar_pixels(width: i32, height: i32, fg: u32, bg: u32) -> Vec<u32> {
    let bg_opaque = 0xff00_0000 | (bg & 0x00ff_ffff);
    let mut pixels = vec![bg_opaque; (width * height).max(0) as usize];
    draw_indicators(&mut pixels, width, height, fg);
    pixels
}

/// Transparent ARGB strip: no fill, just the trailing signal bars + battery in
/// (premultiplied-opaque) `fg`. The clock is added later by [`draw_time`] +
/// [`premultiply_glyph_pixels`]. Used for immersive pages so WebView content
/// shows through.
fn status_bar_pixels_transparent(width: i32, height: i32, fg: u32) -> Vec<u32> {
    let mut pixels = vec![0u32; (width * height).max(0) as usize];
    draw_indicators(&mut pixels, width, height, fg);
    pixels
}

/// Fills the trailing signal bars + battery into `pixels` as premultiplied
/// opaque `fg`, vertically centered and inset by `SIDE_MARGIN` from the right
/// edge. Shared by the opaque and transparent strip builders.
fn draw_indicators(pixels: &mut [u32], width: i32, height: i32, fg: u32) {
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
    fill(pixels, body_left, body_top, body_right, body_top + 1);
    fill(pixels, body_left, body_bottom - 1, body_right, body_bottom);
    fill(pixels, body_left, body_top, body_left + 1, body_bottom);
    fill(pixels, body_right - 1, body_top, body_right, body_bottom);
    // Inner fill (full battery), inset 2px.
    fill(
        pixels,
        body_left + 2,
        body_top + 2,
        body_right - 2,
        body_bottom - 2,
    );
    // Tip.
    fill(
        pixels,
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
        fill(pixels, x0, baseline - bar_h, x0 + bar_w, baseline);
    }
}
