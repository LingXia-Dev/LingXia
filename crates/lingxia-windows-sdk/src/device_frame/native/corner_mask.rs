//! Single anti-aliased overlay that rounds the simulated device screen.
//!
//! The host window region clips the windowed WebView2 to the screen silhouette,
//! but that region has an aliased edge. A layered window composited above the
//! WebView2 supplies the smooth screen arc and matching bezel pixels. The
//! [`cutout`](super::cutout) overlay uses the same owned-popup approach.
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

use windows::Win32::Graphics::Gdi::{CombineRgn, CreateRectRgn, RGN_OR, SetWindowRgn};

/// Anti-aliasing margin (px) past the device's outer silhouette so the mask's
/// outer edge blends instead of clipping.
const MASK_AA_MARGIN: i32 = 1;

/// How far the overlay extends past the screen on every side: over the whole
/// bezel ring plus the anti-aliasing margin.
fn mask_margin(spec: &WindowsDeviceFrame) -> i32 {
    spec.bezel_width.max(0) + MASK_AA_MARGIN
}

/// Opacity of the cut-away corner wedges. The WebView2 surface is windowed, so
/// a translucent wedge lets the square WebView corner leak through. Keep the
/// exterior opaque and use anti-aliasing only on the rounded edge.
const MASK_OPACITY: f32 = 1.0;

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

/// Creates the corner-mask overlay as a layered popup. Returns `0` when the
/// device keeps square screen corners (no
/// radius) or the screen is too small to round. Created hidden;
/// [`reposition_corner_mask`] places and shows it.
pub(super) fn create_corner_mask(content: HWND, spec: &WindowsDeviceFrame) -> isize {
    // The full physical screen radius: the corner ring is painted in the
    // bezel color between the screen silhouette and the device's outer
    // silhouette, so the corners read as the uniform hardware bezel —
    // concentric arcs, transparent outside (the frame's shadow shows there).
    let radius = screen_corner_radius(spec);
    if radius <= 0 {
        return 0;
    }
    let margin = mask_margin(spec);
    let width = spec.screen_width + 2 * margin;
    let height = spec.screen_height + 2 * margin;
    let mask = unsafe {
        WindowsAndMessaging::CreateWindowExW(
            WindowsAndMessaging::WS_EX_LAYERED
                | WindowsAndMessaging::WS_EX_TOOLWINDOW
                | WindowsAndMessaging::WS_EX_TRANSPARENT
                | WindowsAndMessaging::WS_EX_NOACTIVATE,
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
    let outer_radius = outer_corner_radius(spec) as f32;
    apply_corner_mask_region(mask, width, height, outer_radius as i32 + margin);
    if !upload_corner_mask_pixels(
        mask,
        width,
        height,
        &corner_mask_pixels(
            spec.screen_width,
            spec.screen_height,
            radius as f32,
            spec.bezel_width.max(0) as f32,
            outer_radius,
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

fn apply_corner_mask_region(mask: HWND, width: i32, height: i32, corner_extent: i32) {
    let extent = (corner_extent + 2).min(width / 2).min(height / 2).max(1);
    unsafe {
        let region = CreateRectRgn(0, 0, extent, extent);
        let top_right = CreateRectRgn(width - extent, 0, width, extent);
        let bottom_left = CreateRectRgn(0, height - extent, extent, height);
        let bottom_right = CreateRectRgn(width - extent, height - extent, width, height);

        let _ = CombineRgn(Some(region), Some(region), Some(top_right), RGN_OR);
        let _ = CombineRgn(Some(region), Some(region), Some(bottom_left), RGN_OR);
        let _ = CombineRgn(Some(region), Some(region), Some(bottom_right), RGN_OR);

        let applied = SetWindowRgn(mask, Some(region), true);
        let _ = DeleteObject(HGDIOBJ(top_right.0));
        let _ = DeleteObject(HGDIOBJ(bottom_left.0));
        let _ = DeleteObject(HGDIOBJ(bottom_right.0));
        if applied == 0 {
            let _ = DeleteObject(HGDIOBJ(region.0));
        }
    }
}

/// Re-pins the overlay over the screen (in screen coordinates, offset outward
/// by the bleed) above the content. Runs on every content geometry change,
/// so a moving or re-activated screen keeps rounded corners.
pub(super) fn reposition_corner_mask(content: HWND) {
    let Some((mask, margin)) = frame_state(hwnd_handle(content), |state| {
        (state.corner_mask, mask_margin(&state.spec))
    })
    .filter(|(mask, _)| *mask != 0) else {
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
            Some(WindowsAndMessaging::HWND_TOP),
            rect.left - margin,
            rect.top - margin,
            0,
            0,
            WindowsAndMessaging::SWP_NOSIZE
                | WindowsAndMessaging::SWP_NOACTIVATE
                | WindowsAndMessaging::SWP_SHOWWINDOW,
        );
    }
}

/// Hides the overlay while the screen is minimized, hidden, or in the
/// background.
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

/// Premultiplied ARGB pixels for the overlay: `fill_color` between the screen's
/// rounded-rect silhouette and the device's outer silhouette (a uniform bezel
/// ring with concentric corner arcs), transparent inside so the WebView2
/// surface shows through. Without the outer clip the wedges would fill out to
/// the overlay's square corners, reading as heavy black blocks far thicker
/// than the bezel edges.
///
/// The content window itself is clipped to the full screen radius. Pixels past
/// the outer silhouette therefore stay transparent here, exposing the frame
/// window's single continuous shadow instead of painting a second corner veil.
/// `fill_color` is `0xRRGGBB`; output is premultiplied.
fn corner_mask_pixels(
    screen_width: i32,
    screen_height: i32,
    radius: f32,
    bezel_width: f32,
    outer_radius: f32,
    fill_color: u32,
) -> Vec<u32> {
    let margin = bezel_width as i32 + MASK_AA_MARGIN;
    let width = screen_width + 2 * margin;
    let height = screen_height + 2 * margin;
    let center_x = width as f32 / 2.0;
    let center_y = height as f32 / 2.0;
    // Signed distance to a centered rounded rect: negative inside.
    let rounded_distance = |x: f32, y: f32, half_x: f32, half_y: f32, r: f32| -> f32 {
        let qx = (x - center_x).abs() - half_x;
        let qy = (y - center_y).abs() - half_y;
        let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
        outside + qx.max(qy).min(0.0) - r
    };
    let screen_half_x = screen_width as f32 / 2.0 - radius;
    let screen_half_y = screen_height as f32 / 2.0 - radius;
    let outer_half_x = screen_width as f32 / 2.0 + bezel_width - outer_radius;
    let outer_half_y = screen_height as f32 / 2.0 + bezel_width - outer_radius;
    let red = (fill_color >> 16) & 0xff;
    let green = (fill_color >> 8) & 0xff;
    let blue = fill_color & 0xff;
    let mut pixels = Vec::with_capacity((width * height) as usize);
    for y in 0..height {
        for x in 0..width {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let screen_d = rounded_distance(px, py, screen_half_x, screen_half_y, radius);
            let outer_d = rounded_distance(px, py, outer_half_x, outer_half_y, outer_radius);
            // Anti-aliased coverage of the bezel ring: outside the screen
            // silhouette AND inside the outer silhouette.
            let ring = (screen_d + 0.5).clamp(0.0, 1.0) * (0.5 - outer_d).clamp(0.0, 1.0);
            let alpha = (ring * 255.0 * MASK_OPACITY).round() as u32;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixels_past_outer_silhouette_are_transparent() {
        let screen_width = 393;
        let bezel_width = 10;
        let pixels =
            corner_mask_pixels(screen_width, 852, 54.0, bezel_width as f32, 64.0, 0x141414);
        let margin = bezel_width + MASK_AA_MARGIN;
        let width = screen_width + 2 * margin;
        let former_triangle_pixel = ((margin + 1) * width + margin + 1) as usize;

        assert_eq!(pixels[former_triangle_pixel] >> 24, 0);
    }
}
