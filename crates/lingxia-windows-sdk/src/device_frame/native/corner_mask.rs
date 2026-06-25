//! Single anti-aliased overlay that rounds the simulated device screen.
//!
//! Windowed WebView2 presents through DirectComposition and ignores both a
//! `SetWindowRgn` on its host window and the z-order of sibling child windows,
//! so the screen corners can be rounded only by a layered window that
//! composites *above* the WebView2 surface. The [`cutout`](super::cutout)
//! overlay proves a topmost owned-popup layered window does exactly that.
//!
//! Rather than four separate corner windows (which update independently and can
//! show a half-rounded "ghost" corner mid-drag, and risk a one-pixel seam at
//! each corner), this is a *single* per-pixel-alpha popup covering the whole
//! screen: transparent over the screen interior (the WebView2 surface shows
//! through), opaque bezel color outside the screen's rounded-rect silhouette —
//! i.e. exactly the four corner wedges the rounded corners cut away, painted in
//! one atomic `UpdateLayeredWindow` and moved in one `SetWindowPos`. A small
//! outward bleed keeps the wedges flush with the bezel with no sliver.

use super::*;

/// Outward bleed (px) so the painted wedges meet the bezel with no seam. The
/// overlay extends this far past the screen on every side; the ring it adds
/// sits over the real bezel (same color), so it is invisible.
const MASK_BLEED: i32 = 1;

/// Opacity of the corner wedges (0..1). Below 1 gives a soft, frosted-glass
/// rounding that tints what's behind rather than a solid block — the corner is
/// present but easy to overlook, which is what the device wants.
const GLASS_OPACITY: f32 = 0.5;

fn corner_mask_class() -> PCWSTR {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let module = unsafe { LibraryLoader::GetModuleHandleW(None) }
            .map(|module| HINSTANCE(module.0))
            .unwrap_or_default();
        let class = WNDCLASSW {
            lpfnWndProc: Some(corner_mask_proc),
            hInstance: module,
            lpszClassName: w!("LingXiaDeviceCornerMask"),
            ..Default::default()
        };
        if unsafe { WindowsAndMessaging::RegisterClassW(&class) } == 0 {
            log::error!(
                "device corner mask class registration failed: {}",
                windows::core::Error::from_thread()
            );
        }
    });
    w!("LingXiaDeviceCornerMask")
}

unsafe extern "system" fn corner_mask_proc(
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

/// Creates the corner-mask overlay as a topmost layered popup owned by
/// `content`. Returns `0` when the device keeps square screen corners (no
/// radius) or the screen is too small to round. Created hidden;
/// [`reposition_corner_mask`] places and shows it.
pub(super) fn create_corner_mask(content: HWND, spec: &WindowsDeviceFrame) -> isize {
    let radius = spec.screen_corner_radius.max(0);
    if radius <= 0 || spec.screen_width < radius * 2 || spec.screen_height < radius * 2 {
        return 0;
    }
    let width = spec.screen_width + 2 * MASK_BLEED;
    let height = spec.screen_height + 2 * MASK_BLEED;
    let mask = unsafe {
        WindowsAndMessaging::CreateWindowExW(
            WindowsAndMessaging::WS_EX_LAYERED
                | WindowsAndMessaging::WS_EX_TOOLWINDOW
                | WindowsAndMessaging::WS_EX_TRANSPARENT
                | WindowsAndMessaging::WS_EX_NOACTIVATE
                | WindowsAndMessaging::WS_EX_TOPMOST,
            corner_mask_class(),
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
    let mask = match mask {
        Ok(mask) => mask,
        Err(err) => {
            log::warn!("device corner mask creation failed for {content:?}: {err}");
            return 0;
        }
    };
    if !upload_corner_mask_pixels(
        mask,
        width,
        height,
        &corner_mask_pixels(
            spec.screen_width,
            spec.screen_height,
            radius as f32,
            spec.screen_corner_color,
        ),
    ) {
        unsafe {
            let _ = WindowsAndMessaging::DestroyWindow(mask);
        }
        return 0;
    }
    hwnd_handle(mask)
}

/// Re-pins the overlay over the screen (in screen coordinates, offset outward
/// by the bleed) and keeps it topmost. Runs on every content geometry change,
/// so a moving or re-activated screen keeps rounded corners.
pub(super) fn reposition_corner_mask(content: HWND) {
    let Some(mask) =
        frame_state(hwnd_handle(content), |state| state.corner_mask).filter(|mask| *mask != 0)
    else {
        return;
    };
    if !is_window_handle_valid(mask) {
        return;
    }
    let mut rect = RECT::default();
    unsafe {
        let _ = WindowsAndMessaging::GetWindowRect(content, &mut rect);
    }
    unsafe {
        let _ = WindowsAndMessaging::SetWindowPos(
            hwnd_from_handle(mask),
            Some(WindowsAndMessaging::HWND_TOPMOST),
            rect.left - MASK_BLEED,
            rect.top - MASK_BLEED,
            0,
            0,
            WindowsAndMessaging::SWP_NOSIZE
                | WindowsAndMessaging::SWP_NOACTIVATE
                | WindowsAndMessaging::SWP_SHOWWINDOW,
        );
    }
}

/// Hides the overlay while the screen is minimized, hidden, or in the
/// background (it is topmost and would otherwise float over another app).
pub(super) fn hide_corner_mask(content: HWND) {
    if let Some(mask) =
        frame_state(hwnd_handle(content), |state| state.corner_mask).filter(|mask| *mask != 0)
    {
        unsafe {
            let _ = WindowsAndMessaging::ShowWindow(
                hwnd_from_handle(mask),
                WindowsAndMessaging::SW_HIDE,
            );
        }
    }
}

/// Destroys the overlay window. Safe to call with `0`.
pub(super) fn destroy_corner_mask(mask: isize) {
    if mask != 0 && is_window_handle_valid(mask) {
        unsafe {
            let _ = WindowsAndMessaging::DestroyWindow(hwnd_from_handle(mask));
        }
    }
}

/// Uploads the premultiplied ARGB mask to the layered window. Returns `false`
/// when any GDI step fails (the caller drops the window so a blank mask is
/// never left shown).
fn upload_corner_mask_pixels(window: HWND, width: i32, height: i32, pixels: &[u32]) -> bool {
    let mut ok = false;
    unsafe {
        let screen_dc = GetDC(None);
        if screen_dc.is_invalid() {
            return false;
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
                ok = WindowsAndMessaging::UpdateLayeredWindow(
                    window,
                    None,
                    None,
                    Some(&size),
                    Some(memory_dc),
                    Some(&origin),
                    COLORREF(0),
                    Some(&blend),
                    WindowsAndMessaging::ULW_ALPHA,
                )
                .is_ok();
                if !ok {
                    log::warn!(
                        "device corner mask UpdateLayeredWindow failed: {}",
                        windows::core::Error::from_thread()
                    );
                }
                if !old_bitmap.is_invalid() {
                    let _ = SelectObject(memory_dc, old_bitmap);
                }
                let _ = DeleteObject(HGDIOBJ(bitmap.0));
            }
            let _ = DeleteDC(memory_dc);
        }
        let _ = ReleaseDC(None, screen_dc);
    }
    ok
}

/// Premultiplied ARGB pixels for the overlay: `fill_color` outside the screen's
/// rounded-rect silhouette (the four corner wedges plus a one-pixel bleed ring),
/// transparent inside (the WebView2 surface shows through). The silhouette is
/// the `screen_width`x`screen_height` rounded rect inset by `MASK_BLEED`.
/// `fill_color` is `0xRRGGBB`; output is premultiplied.
fn corner_mask_pixels(
    screen_width: i32,
    screen_height: i32,
    radius: f32,
    fill_color: u32,
) -> Vec<u32> {
    let width = screen_width + 2 * MASK_BLEED;
    let height = screen_height + 2 * MASK_BLEED;
    // Screen silhouette centered in the overlay (inset by the bleed).
    let center_x = width as f32 / 2.0;
    let center_y = height as f32 / 2.0;
    let half_x = screen_width as f32 / 2.0 - radius;
    let half_y = screen_height as f32 / 2.0 - radius;
    // Signed distance to the rounded rect: negative inside, positive outside.
    let rounded_distance = |x: f32, y: f32| -> f32 {
        let qx = (x - center_x).abs() - half_x;
        let qy = (y - center_y).abs() - half_y;
        let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
        outside + qx.max(qy).min(0.0) - radius
    };
    let red = (fill_color >> 16) & 0xff;
    let green = (fill_color >> 8) & 0xff;
    let blue = fill_color & 0xff;
    let mut pixels = Vec::with_capacity((width * height) as usize);
    for y in 0..height {
        for x in 0..width {
            let distance = rounded_distance(x as f32 + 0.5, y as f32 + 0.5);
            // Coverage of the area *outside* the screen silhouette, anti-aliased
            // across the one-pixel boundary band, scaled to a translucent
            // frosted-glass opacity.
            let coverage = (distance + 0.5).clamp(0.0, 1.0);
            let alpha = (coverage * 255.0 * GLASS_OPACITY).round() as u32;
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
