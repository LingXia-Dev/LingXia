//! Top-centered phone screen cutout / Dynamic Island overlay.

use super::*;

const CUTOUT_TOP_INSET: i32 = 8;

fn cutout_class() -> PCWSTR {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let module = unsafe { LibraryLoader::GetModuleHandleW(None) }
            .map(|module| HINSTANCE(module.0))
            .unwrap_or_default();
        let class = WNDCLASSW {
            lpfnWndProc: Some(cutout_proc),
            hInstance: module,
            lpszClassName: w!("LingXiaDeviceCutout"),
            ..Default::default()
        };
        if unsafe { WindowsAndMessaging::RegisterClassW(&class) } == 0 {
            log::error!(
                "device cutout class registration failed: {}",
                windows::core::Error::from_thread()
            );
        }
    });
    w!("LingXiaDeviceCutout")
}

pub(super) fn create_cutout_window(content: HWND, spec: &WindowsDeviceFrame) -> Option<isize> {
    let cutout = spec
        .cutout
        .as_ref()
        .filter(|cutout| cutout.width > 0 && cutout.height > 0)?;
    let cutout = unsafe {
        WindowsAndMessaging::CreateWindowExW(
            WindowsAndMessaging::WS_EX_LAYERED
                | WindowsAndMessaging::WS_EX_TOOLWINDOW
                | WindowsAndMessaging::WS_EX_NOACTIVATE
                | WindowsAndMessaging::WS_EX_TOPMOST,
            cutout_class(),
            PCWSTR::null(),
            WindowsAndMessaging::WS_POPUP,
            0,
            0,
            cutout.width,
            cutout.height,
            Some(content),
            None,
            LibraryLoader::GetModuleHandleW(None)
                .ok()
                .map(|module| HINSTANCE(module.0)),
            None,
        )
    };
    let cutout = match cutout {
        Ok(cutout) => cutout,
        Err(err) => {
            log::warn!("device cutout window creation failed: {err}");
            return None;
        }
    };
    paint_cutout(cutout, spec);
    Some(hwnd_handle(cutout))
}

fn paint_cutout(window: HWND, spec: &WindowsDeviceFrame) {
    let Some(cutout) = spec.cutout.as_ref() else {
        return;
    };
    let width = cutout.width.max(1);
    let height = cutout.height.max(1);
    let pixels = cutout_pixels(width, height, cutout.corner_radius.max(1) as f32);
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

fn cutout_pixels(width: i32, height: i32, radius: f32) -> Vec<u32> {
    let cx = width as f32 / 2.0;
    let cy = height as f32 / 2.0;
    let half_x = width as f32 / 2.0 - radius;
    let half_y = height as f32 / 2.0 - radius;
    let mut pixels = Vec::with_capacity((width * height) as usize);
    for y in 0..height {
        for x in 0..width {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let qx = (px - cx).abs() - half_x;
            let qy = (py - cy).abs() - half_y;
            let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
            let distance = outside + qx.max(qy).min(0.0) - radius;
            let alpha = (0.5 - distance).clamp(0.0, 1.0);
            pixels.push(((alpha * 255.0).round() as u32) << 24);
        }
    }
    pixels
}

pub(super) fn reposition_cutout(content: HWND) {
    let Some((cutout, spec)) = frame_state(hwnd_handle(content), |state| {
        (state.cutout, state.spec.clone())
    })
    .filter(|(cutout, _)| *cutout != 0) else {
        return;
    };
    let Some(cutout_spec) = spec.cutout else {
        return;
    };
    if !is_window_handle_valid(cutout) {
        return;
    }
    let mut rect = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetWindowRect(content, &mut rect);
    }
    let x = rect.left + (spec.screen_width - cutout_spec.width) / 2;
    let y = rect.top + CUTOUT_TOP_INSET;
    unsafe {
        let _ = WindowsAndMessaging::SetWindowPos(
            hwnd_from_handle(cutout),
            Some(WindowsAndMessaging::HWND_TOPMOST),
            x,
            y,
            cutout_spec.width,
            cutout_spec.height,
            WindowsAndMessaging::SWP_NOACTIVATE
                | WindowsAndMessaging::SWP_NOOWNERZORDER
                | WindowsAndMessaging::SWP_SHOWWINDOW,
        );
    }
}

pub(super) fn hide_cutout(content: HWND) {
    if let Some(cutout) =
        frame_state(hwnd_handle(content), |state| state.cutout).filter(|cutout| *cutout != 0)
    {
        unsafe {
            let _ = WindowsAndMessaging::ShowWindow(
                hwnd_from_handle(cutout),
                WindowsAndMessaging::SW_HIDE,
            );
        }
    }
}

pub(super) fn destroy_cutout(cutout: isize) {
    if cutout != 0 && is_window_handle_valid(cutout) {
        unsafe {
            let _ = WindowsAndMessaging::DestroyWindow(hwnd_from_handle(cutout));
        }
    }
}

unsafe extern "system" fn cutout_proc(
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
