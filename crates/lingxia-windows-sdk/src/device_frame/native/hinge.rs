//! Click-through optical hinge/crease overlay for simulated foldable screens.

use super::*;

fn hinge_class() -> PCWSTR {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let module = unsafe { LibraryLoader::GetModuleHandleW(None) }
            .map(|module| HINSTANCE(module.0))
            .unwrap_or_default();
        let class = WNDCLASSW {
            lpfnWndProc: Some(hinge_proc),
            hInstance: module,
            lpszClassName: w!("LingXiaDeviceHinge"),
            ..Default::default()
        };
        if unsafe { WindowsAndMessaging::RegisterClassW(&class) } == 0 {
            log::error!(
                "device hinge class registration failed: {}",
                windows::core::Error::from_thread()
            );
        }
    });
    w!("LingXiaDeviceHinge")
}

/// Overlay rect relative to the content window's top-left corner, in physical pixels.
fn hinge_band_rect(
    spec: &WindowsDeviceFrame,
    hinge: WindowsDeviceFrameHinge,
) -> (i32, i32, i32, i32) {
    let min_band = if hinge.half_opened {
        if hinge.vertical { 14 } else { 16 }
    } else {
        10
    };
    if hinge.vertical {
        let band_w = hinge.width.max(min_band).max(1);
        let center_x = hinge.x + hinge.width / 2;
        let x = (center_x - band_w / 2).clamp(0, (spec.screen_width - band_w).max(0));
        let y = hinge.y.clamp(0, spec.screen_height.max(0));
        let h = if hinge.height > 0 {
            hinge.height.min(spec.screen_height - y)
        } else {
            spec.screen_height - y
        }
        .max(1);
        (x, y, band_w, h)
    } else {
        let band_h = hinge.height.max(min_band).max(1);
        let center_y = hinge.y + hinge.height / 2;
        let y = (center_y - band_h / 2).clamp(0, (spec.screen_height - band_h).max(0));
        let x = hinge.x.clamp(0, spec.screen_width.max(0));
        let w = if hinge.width > 0 {
            hinge.width.min(spec.screen_width - x)
        } else {
            spec.screen_width - x
        }
        .max(1);
        (x, y, w, band_h)
    }
}

pub(super) fn create_hinge_window(content: HWND, spec: &WindowsDeviceFrame) -> Option<isize> {
    let hinge = spec.hinge?;
    let (_, _, width, height) = hinge_band_rect(spec, hinge);
    let window = unsafe {
        WindowsAndMessaging::CreateWindowExW(
            WindowsAndMessaging::WS_EX_LAYERED
                | WindowsAndMessaging::WS_EX_TRANSPARENT
                | WindowsAndMessaging::WS_EX_TOOLWINDOW
                | WindowsAndMessaging::WS_EX_NOACTIVATE,
            hinge_class(),
            PCWSTR::null(),
            WindowsAndMessaging::WS_POPUP,
            0,
            0,
            width,
            height,
            Some(content),
            None,
            LibraryLoader::GetModuleHandleW(None)
                .ok()
                .map(|module| HINSTANCE(module.0)),
            None,
        )
    };
    let window = match window {
        Ok(window) => window,
        Err(err) => {
            log::warn!("device hinge window creation failed: {err}");
            return None;
        }
    };
    paint_hinge(window, width, height, hinge, overlay_is_dark());
    Some(hwnd_handle(window))
}

fn paint_hinge(
    window: HWND,
    width: i32,
    height: i32,
    hinge: WindowsDeviceFrameHinge,
    is_dark: bool,
) {
    let width = width.max(1);
    let height = height.max(1);
    let pixels = hinge_pixels(width, height, hinge, is_dark);
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

/// Premultiplied BGRA pixels for a subtle shadow-highlight-shadow crease gradient
/// with a center hairline (dashed when half-opened).
fn hinge_pixels(
    width: i32,
    height: i32,
    hinge: WindowsDeviceFrameHinge,
    is_dark: bool,
) -> Vec<u32> {
    let shadow_alpha = if hinge.half_opened {
        if is_dark { 0.32f32 } else { 0.20f32 }
    } else if is_dark {
        0.20f32
    } else {
        0.11f32
    };
    let highlight_alpha = if hinge.half_opened {
        if is_dark { 0.16f32 } else { 0.24f32 }
    } else if is_dark {
        0.10f32
    } else {
        0.16f32
    };
    let line_alpha = if hinge.half_opened {
        if is_dark { 0.28f32 } else { 0.20f32 }
    } else if is_dark {
        0.14f32
    } else {
        0.10f32
    };
    let line_rgb: f32 = if is_dark { 255.0 } else { 0.0 };

    let mut pixels = Vec::with_capacity((width * height) as usize);
    let span = if hinge.vertical { width } else { height }.max(1) as f32;
    let mid = (if hinge.vertical { width } else { height }) / 2;

    for y in 0..height {
        for x in 0..width {
            let across = if hinge.vertical { x } else { y };
            let along = if hinge.vertical { y } else { x };
            let t = (across as f32 + 0.5) / span;
            // Piecewise gradient: 0 -> 0.35 shadow -> 0.5 highlight -> 0.65 shadow -> 1.0 clear.
            let (mut rgb, mut a) = if t < 0.35 {
                let k = t / 0.35;
                (0.0f32, shadow_alpha * k)
            } else if t < 0.5 {
                let k = (t - 0.35) / 0.15;
                (255.0 * k, shadow_alpha * (1.0 - k) + highlight_alpha * k)
            } else if t < 0.65 {
                let k = (t - 0.5) / 0.15;
                (
                    255.0 * (1.0 - k),
                    highlight_alpha * (1.0 - k) + shadow_alpha * k,
                )
            } else {
                let k = (1.0 - t) / 0.35;
                (0.0f32, shadow_alpha * k.max(0.0))
            };

            let on_center = across == mid;
            let dash_on = !hinge.half_opened || (along % 7) < 4;
            if on_center && dash_on {
                // Blend hairline over gradient.
                let out_a = line_alpha + a * (1.0 - line_alpha);
                if out_a > 0.0 {
                    rgb = (line_rgb * line_alpha + rgb * a * (1.0 - line_alpha)) / out_a;
                    a = out_a;
                }
            }

            let a_u8 = (a.clamp(0.0, 1.0) * 255.0).round() as u32;
            let c_u8 = ((rgb * a.clamp(0.0, 1.0)).clamp(0.0, 255.0)).round() as u32;
            pixels.push((a_u8 << 24) | (c_u8 << 16) | (c_u8 << 8) | c_u8);
        }
    }
    pixels
}

pub(super) fn reposition_hinge(content: HWND) {
    let Some((hinge_hwnd, spec)) = frame_state(hwnd_handle(content), |state| {
        (state.hinge, state.spec.clone())
    })
    .filter(|(hinge_hwnd, _)| *hinge_hwnd != 0) else {
        return;
    };
    let Some(hinge_spec) = spec.hinge else {
        return;
    };
    if !is_window_handle_valid(hinge_hwnd) {
        return;
    }
    let mut rect = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetWindowRect(content, &mut rect);
    }
    let (rx, ry, width, height) = hinge_band_rect(&spec, hinge_spec);
    unsafe {
        let _ = WindowsAndMessaging::SetWindowPos(
            hwnd_from_handle(hinge_hwnd),
            Some(WindowsAndMessaging::HWND_TOP),
            rect.left + rx,
            rect.top + ry,
            width,
            height,
            WindowsAndMessaging::SWP_NOACTIVATE | WindowsAndMessaging::SWP_SHOWWINDOW,
        );
    }
}

pub(super) fn hide_hinge(content: HWND) {
    if let Some(hinge_hwnd) =
        frame_state(hwnd_handle(content), |state| state.hinge).filter(|h| *h != 0)
    {
        unsafe {
            let _ = WindowsAndMessaging::ShowWindow(
                hwnd_from_handle(hinge_hwnd),
                WindowsAndMessaging::SW_HIDE,
            );
        }
    }
}

pub(super) fn destroy_hinge(hinge_hwnd: isize) {
    if hinge_hwnd != 0 && is_window_handle_valid(hinge_hwnd) {
        unsafe {
            let _ = WindowsAndMessaging::DestroyWindow(hwnd_from_handle(hinge_hwnd));
        }
    }
}

unsafe extern "system" fn hinge_proc(
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
