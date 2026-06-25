//! Anti-aliased screen-corner caps for the simulated device screen.

use super::*;

struct ScreenCornerCapSet {
    caps: [isize; 4],
    side: i32,
    color: u32,
}

static SCREEN_CORNER_CAPS: OnceLock<Mutex<HashMap<isize, ScreenCornerCapSet>>> = OnceLock::new();

unsafe extern "system" fn screen_corner_cap_proc(
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

fn screen_corner_cap_class() -> PCWSTR {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let module = unsafe { LibraryLoader::GetModuleHandleW(None) }
            .map(|module| HINSTANCE(module.0))
            .unwrap_or_default();
        let class = WNDCLASSW {
            lpfnWndProc: Some(screen_corner_cap_proc),
            hInstance: module,
            lpszClassName: w!("LingXiaDeviceScreenCornerCap"),
            ..Default::default()
        };
        if unsafe { WindowsAndMessaging::RegisterClassW(&class) } == 0 {
            log::error!(
                "device screen corner cap class registration failed: {}",
                windows::core::Error::from_thread()
            );
        }
    });
    w!("LingXiaDeviceScreenCornerCap")
}

pub(super) fn update_screen_corner_caps(
    parent: HWND,
    screen_rect: RECT,
    side: i32,
    color: COLORREF,
) {
    if side <= 0 || rect_width(&screen_rect) < side * 2 || rect_height(&screen_rect) < side * 2 {
        destroy_screen_corner_caps(parent);
        return;
    }

    let sets = SCREEN_CORNER_CAPS.get_or_init(|| Mutex::new(HashMap::new()));
    let existing = sets
        .lock()
        .ok()
        .and_then(|sets| {
            sets.get(&hwnd_handle(parent))
                .map(|set| (set.caps, set.side, set.color))
        })
        .filter(|(caps, cap_side, cap_color)| {
            *cap_side == side
                && *cap_color == color.0
                && caps.iter().all(|cap| is_window_handle_valid(*cap))
        });
    let caps = match existing {
        Some((caps, _, _)) => caps,
        None => {
            destroy_screen_corner_caps(parent);
            let Some(caps) = create_screen_corner_caps(parent, side, color) else {
                return;
            };
            if let Ok(mut sets) = sets.lock() {
                sets.insert(
                    hwnd_handle(parent),
                    ScreenCornerCapSet {
                        caps,
                        side,
                        color: color.0,
                    },
                );
            }
            caps
        }
    };

    let positions = [
        (screen_rect.left, screen_rect.top),
        (screen_rect.right - side, screen_rect.top),
        (screen_rect.left, screen_rect.bottom - side),
        (screen_rect.right - side, screen_rect.bottom - side),
    ];
    for (cap, (x, y)) in caps.iter().zip(positions) {
        unsafe {
            let _ = WindowsAndMessaging::SetWindowPos(
                hwnd_from_handle(*cap),
                Some(WindowsAndMessaging::HWND_TOP),
                x,
                y,
                side,
                side,
                WindowsAndMessaging::SWP_NOACTIVATE
                    | WindowsAndMessaging::SWP_NOOWNERZORDER
                    | WindowsAndMessaging::SWP_NOCOPYBITS
                    | WindowsAndMessaging::SWP_SHOWWINDOW,
            );
        }
    }
}

pub(super) fn raise_screen_corner_caps(parent: HWND) {
    let caps = SCREEN_CORNER_CAPS
        .get()
        .and_then(|sets| sets.lock().ok())
        .and_then(|sets| sets.get(&hwnd_handle(parent)).map(|set| set.caps));
    let Some(caps) = caps else {
        return;
    };
    for cap in caps {
        unsafe {
            let _ = WindowsAndMessaging::SetWindowPos(
                hwnd_from_handle(cap),
                Some(WindowsAndMessaging::HWND_TOP),
                0,
                0,
                0,
                0,
                WindowsAndMessaging::SWP_NOMOVE
                    | WindowsAndMessaging::SWP_NOSIZE
                    | WindowsAndMessaging::SWP_NOACTIVATE
                    | WindowsAndMessaging::SWP_NOOWNERZORDER,
            );
        }
    }
}

fn create_screen_corner_caps(parent: HWND, side: i32, color: COLORREF) -> Option<[isize; 4]> {
    let class = screen_corner_cap_class();
    let mut caps = [0isize; 4];
    for corner in 0..4 {
        let result = unsafe {
            WindowsAndMessaging::CreateWindowExW(
                WindowsAndMessaging::WS_EX_LAYERED
                    | WindowsAndMessaging::WS_EX_TRANSPARENT
                    | WindowsAndMessaging::WS_EX_NOACTIVATE,
                class,
                PCWSTR::null(),
                WindowsAndMessaging::WS_CHILD,
                0,
                0,
                side,
                side,
                Some(parent),
                None,
                LibraryLoader::GetModuleHandleW(None)
                    .ok()
                    .map(|module| HINSTANCE(module.0)),
                None,
            )
        };
        let cap = match result {
            Ok(cap) => cap,
            Err(err) => {
                log::warn!("device screen corner cap creation failed for {parent:?}: {err}");
                for created in &caps[..corner] {
                    unsafe {
                        let _ = WindowsAndMessaging::DestroyWindow(hwnd_from_handle(*created));
                    }
                }
                return None;
            }
        };
        upload_layered_window_pixels(
            cap,
            side,
            side,
            &screen_corner_cap_pixels(corner, side, color),
        );
        caps[corner] = hwnd_handle(cap);
    }
    Some(caps)
}

fn upload_layered_window_pixels(window: HWND, width: i32, height: i32, pixels: &[u32]) {
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

fn screen_corner_cap_pixels(corner: usize, side: i32, color: COLORREF) -> Vec<u32> {
    let radius = side as f32;
    let (center_x, center_y) = match corner {
        0 => (radius, radius),
        1 => (0.0, radius),
        2 => (radius, 0.0),
        _ => (0.0, 0.0),
    };
    let red = color.0 & 0xff;
    let green = (color.0 >> 8) & 0xff;
    let blue = (color.0 >> 16) & 0xff;
    let mut pixels = Vec::with_capacity((side * side) as usize);
    for y in 0..side {
        for x in 0..side {
            let mut outside = 0u32;
            for sub_y in 0..4 {
                for sub_x in 0..4 {
                    let sample_x = x as f32 + (sub_x as f32 + 0.5) / 4.0;
                    let sample_y = y as f32 + (sub_y as f32 + 0.5) / 4.0;
                    let dx = sample_x - center_x;
                    let dy = sample_y - center_y;
                    if dx * dx + dy * dy > radius * radius {
                        outside += 1;
                    }
                }
            }
            let alpha = outside * 255 / 16;
            let premultiply = |channel: u32| channel * alpha / 255;
            pixels.push(
                (alpha << 24)
                    | (premultiply(red) << 16)
                    | (premultiply(green) << 8)
                    | premultiply(blue),
            );
        }
    }
    pixels
}

pub(super) fn destroy_screen_corner_caps(parent: HWND) {
    let removed = SCREEN_CORNER_CAPS
        .get()
        .and_then(|sets| sets.lock().ok())
        .and_then(|mut sets| sets.remove(&hwnd_handle(parent)));
    let Some(set) = removed else {
        return;
    };
    for cap in set.caps {
        let cap = hwnd_from_handle(cap);
        unsafe {
            if WindowsAndMessaging::DestroyWindow(cap).is_err() {
                let _ = WindowsAndMessaging::PostMessageW(
                    Some(cap),
                    WindowsAndMessaging::WM_CLOSE,
                    WPARAM::default(),
                    LPARAM::default(),
                );
            }
        }
    }
}

fn rect_width(rect: &RECT) -> i32 {
    (rect.right - rect.left).max(0)
}

fn rect_height(rect: &RECT) -> i32 {
    (rect.bottom - rect.top).max(0)
}
