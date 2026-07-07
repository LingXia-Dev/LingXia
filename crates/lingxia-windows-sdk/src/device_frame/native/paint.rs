//! Layered bitmap painting for the Windows device-frame shell.

use super::*;

use windows::Win32::Graphics::Gdi::{
    CreateFontW, FW_SEMIBOLD, GetTextExtentPoint32W, SetBkMode, SetTextColor, TRANSPARENT, TextOutW,
};

/// Renders the shell bitmap — analytic bezel/shadow/toolbar pixels, then
/// GDI text for the selector label and the action glyph — and uploads it
/// via `UpdateLayeredWindow`. Fills the text-dependent rects of `layout`.
pub(super) fn paint_frame_window(frame: HWND, spec: &WindowsDeviceFrame, layout: &mut FrameLayout) {
    let mut pixels = frame_pixels(spec, layout);
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
                    biWidth: layout.width,
                    biHeight: -layout.height,
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
                if let Some(toolbar) = &spec.toolbar {
                    draw_toolbar_text(memory_dc, toolbar, layout);
                    // GDI writes zero alpha bytes; restore the toolbar
                    // alpha over the text it touched.
                    let dib = std::slice::from_raw_parts_mut(bits.cast::<u32>(), pixels.len());
                    fix_toolbar_alpha(dib, layout);
                    if toolbar.rotate_command.is_some() {
                        composite_rotate_icon(dib, layout);
                    }
                    pixels.copy_from_slice(dib);
                }
                let size = SIZE {
                    cx: layout.width,
                    cy: layout.height,
                };
                let origin = POINT { x: 0, y: 0 };
                let blend = BLENDFUNCTION {
                    BlendOp: AC_SRC_OVER as u8,
                    BlendFlags: 0,
                    SourceConstantAlpha: 255,
                    AlphaFormat: AC_SRC_ALPHA as u8,
                };
                let _ = WindowsAndMessaging::UpdateLayeredWindow(
                    frame,
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

/// Draws the selector label (centered, with a drop-down arrow) and the
/// trailing gear glyph into the toolbar with GDI, and fills
/// `layout.selector_rect` / `layout.action_rect` from the text metrics.
fn draw_toolbar_text(dc: HDC, toolbar: &WindowsDeviceFrameToolbar, layout: &mut FrameLayout) {
    unsafe {
        SetBkMode(dc, TRANSPARENT);
    }
    let toolbar_rect = layout.toolbar;
    let center_y = (toolbar_rect.top + toolbar_rect.bottom) / 2;

    // Selector: "<label>  ⌄" centered in the toolbar.
    let label = format!("{}  \u{2304}", toolbar.selector_label);
    let label_wide = to_wide(&label);
    let label_chars = &label_wide[..label_wide.len().saturating_sub(1)];
    let font = unsafe {
        CreateFontW(
            -15,
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
    unsafe {
        let old_font = SelectObject(dc, HGDIOBJ(font.0));
        SetTextColor(dc, COLORREF(0x00E8E8E8));
        let mut extent = SIZE::default();
        let _ = GetTextExtentPoint32W(dc, label_chars, &mut extent);
        let x = (toolbar_rect.left + toolbar_rect.right - extent.cx) / 2;
        let y = center_y - extent.cy / 2;
        let _ = TextOutW(dc, x, y, label_chars);
        layout.selector_rect = RECT {
            left: x - 8,
            top: toolbar_rect.top,
            right: x + extent.cx + 8,
            bottom: toolbar_rect.bottom,
        };
        if !old_font.is_invalid() {
            let _ = SelectObject(dc, old_font);
        }
        let _ = DeleteObject(HGDIOBJ(font.0));
    }

    // Trailing gear glyph (Segoe MDL2 Assets "Settings", U+E713).
    if toolbar.action_command.is_some() {
        let glyph = to_wide("\u{E713}");
        let glyph_chars = &glyph[..glyph.len().saturating_sub(1)];
        let icon_font = unsafe {
            CreateFontW(
                -16,
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
                w!("Segoe MDL2 Assets"),
            )
        };
        unsafe {
            let old_font = SelectObject(dc, HGDIOBJ(icon_font.0));
            SetTextColor(dc, COLORREF(0x00B4B4B4));
            let mut extent = SIZE::default();
            let _ = GetTextExtentPoint32W(dc, glyph_chars, &mut extent);
            let x = toolbar_rect.right - TOOLBAR_SIDE_MARGIN - extent.cx;
            let y = center_y - extent.cy / 2;
            let _ = TextOutW(dc, x, y, glyph_chars);
            layout.action_rect = RECT {
                left: x - 6,
                top: toolbar_rect.top,
                right: toolbar_rect.right,
                bottom: toolbar_rect.bottom,
            };
            if !old_font.is_invalid() {
                let _ = SelectObject(dc, old_font);
            }
            let _ = DeleteObject(HGDIOBJ(icon_font.0));
        }
    }

    // The rotate control is a shared design icon (design/icons/svg/icon_rotate),
    // composited after the GDI text pass in `composite_rotate_icon`.
}

/// Width/height of the rotate design icon in the toolbar.
const ROTATE_ICON_SIZE: i32 = 16;

/// Composites the rotate design icon just left of the gear (or the trailing
/// edge when there is no gear) and records `rotate_rect` for hit-testing.
/// Runs after `fix_toolbar_alpha`, writing premultiplied ARGB straight into
/// the layered DIB — `DrawIconEx` does not reliably set alpha on a per-pixel
/// alpha surface.
fn composite_rotate_icon(dib: &mut [u32], layout: &mut FrameLayout) {
    let toolbar = layout.toolbar;
    let center_y = (toolbar.top + toolbar.bottom) / 2;
    let right_anchor = if layout.action_rect.right > layout.action_rect.left {
        layout.action_rect.left - 4
    } else {
        toolbar.right - TOOLBAR_SIDE_MARGIN
    };
    let x = right_anchor - ROTATE_ICON_SIZE;
    let y = center_y - ROTATE_ICON_SIZE / 2;
    layout.rotate_rect = RECT {
        left: x - 6,
        top: toolbar.top,
        right: right_anchor,
        bottom: toolbar.bottom,
    };
    let Some(icon) = crate::design_icons::design_icon_argb_premultiplied(
        crate::WindowsDesignIcon::Rotate,
        ROTATE_ICON_SIZE as u32,
        Some(0x00B4B4B4),
    ) else {
        return;
    };
    for iy in 0..ROTATE_ICON_SIZE {
        for ix in 0..ROTATE_ICON_SIZE {
            let src = icon[(iy * ROTATE_ICON_SIZE + ix) as usize];
            let sa = src >> 24;
            if sa == 0 {
                continue;
            }
            let (px, py) = (x + ix, y + iy);
            if px < 0 || py < 0 || px >= layout.width || py >= layout.height {
                continue;
            }
            let di = (py * layout.width + px) as usize;
            let dst = dib[di];
            let inv = 255 - sa;
            let ch = |shift: u32| {
                let s = (src >> shift) & 0xff;
                let d = (dst >> shift) & 0xff;
                (s + d * inv / 255).min(255)
            };
            let a = (sa + ((dst >> 24) & 0xff) * inv / 255).min(255);
            dib[di] = (a << 24) | (ch(16) << 16) | (ch(8) << 8) | ch(0);
        }
    }
}

/// GDI text output zeroes the alpha byte of every pixel it touches; inside
/// the toolbar capsule the alpha is a known constant, so restore it (and
/// re-premultiply the straight GDI colors) wherever it was knocked out.
fn fix_toolbar_alpha(pixels: &mut [u32], layout: &FrameLayout) {
    let toolbar = layout.toolbar;
    for y in toolbar.top..toolbar.bottom {
        for x in toolbar.left..toolbar.right {
            let index = (y * layout.width + x) as usize;
            let pixel = pixels[index];
            if pixel >> 24 == 0 && pixel != 0 {
                let premultiply = |channel: u32| channel * TOOLBAR_ALPHA / 255;
                let red = (pixel >> 16) & 0xff;
                let green = (pixel >> 8) & 0xff;
                let blue = pixel & 0xff;
                pixels[index] = (TOOLBAR_ALPHA << 24)
                    | (premultiply(red) << 16)
                    | (premultiply(green) << 8)
                    | premultiply(blue);
            }
        }
    }
}

/// Premultiplied ARGB pixels of the shell bitmap: the toolbar capsule with
/// its close/minimize dots, the device bezel as an anti-aliased rounded
/// rect, and a soft downward-biased shadow ringing both. The screen area
/// stays opaque bezel color — the screen window covers it, and any
/// sub-pixel gap along its clipped edge then reads as bezel instead of
/// flashing the desktop.
fn frame_pixels(spec: &WindowsDeviceFrame, layout: &FrameLayout) -> Vec<u32> {
    let bezel_red = (spec.bezel_color >> 16) & 0xff;
    let bezel_green = (spec.bezel_color >> 8) & 0xff;
    let bezel_blue = spec.bezel_color & 0xff;

    let bezel = layout.bezel;
    let bezel_center_x = (bezel.left + bezel.right) as f32 / 2.0;
    let bezel_center_y = (bezel.top + bezel.bottom) as f32 / 2.0;
    let radius = spec
        .outer_corner_radius
        .max(1)
        .min(spec.screen_width / 2 + spec.bezel_width) as f32;
    let half_x = (bezel.right - bezel.left) as f32 / 2.0 - radius;
    let half_y = (bezel.bottom - bezel.top) as f32 / 2.0 - radius;
    // Rounded-rect signed distance: negative inside the silhouette.
    let rounded_distance = |x: f32, y: f32, hx: f32, hy: f32, r: f32| -> f32 {
        let qx = x.abs() - hx;
        let qy = y.abs() - hy;
        let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
        outside + qx.max(qy).min(0.0) - r
    };
    let bezel_distance = move |x: f32, y: f32| {
        rounded_distance(
            x - bezel_center_x,
            y - bezel_center_y,
            half_x,
            half_y,
            radius,
        )
    };

    let has_toolbar = spec.toolbar.is_some();
    let toolbar = layout.toolbar;
    let toolbar_center_x = (toolbar.left + toolbar.right) as f32 / 2.0;
    let toolbar_center_y = (toolbar.top + toolbar.bottom) as f32 / 2.0;
    let toolbar_half_x = (toolbar.right - toolbar.left) as f32 / 2.0 - TOOLBAR_RADIUS as f32;
    let toolbar_half_y = (toolbar.bottom - toolbar.top) as f32 / 2.0 - TOOLBAR_RADIUS as f32;
    let toolbar_distance = move |x: f32, y: f32| {
        rounded_distance(
            x - toolbar_center_x,
            y - toolbar_center_y,
            toolbar_half_x,
            toolbar_half_y,
            TOOLBAR_RADIUS as f32,
        )
    };

    // Toolbar dots (close red, minimize yellow), anti-aliased circles.
    let dots = [
        (&layout.close_rect, CLOSE_DOT_COLOR),
        (&layout.minimize_rect, MINIMIZE_DOT_COLOR),
    ];
    // The screen cutout / Dynamic Island is rendered by a floating `cutout`
    // overlay window (see `cutout.rs`), which is the only surface that can
    // paint above the opaque WebView2 content. The layered frame bitmap sits
    // *behind* the content, so carving the notch here would never be
    // visible — keep it out of the frame paint.

    let shadow_reach = FRAME_SHADOW_MARGIN as f32;
    let mut pixels = Vec::with_capacity((layout.width * layout.height) as usize);
    for y in 0..layout.height {
        for x in 0..layout.width {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;

            let bezel_d = bezel_distance(px, py);
            let bezel_coverage = (0.5 - bezel_d).clamp(0.0, 1.0);
            let toolbar_coverage = if has_toolbar {
                (0.5 - toolbar_distance(px, py)).clamp(0.0, 1.0)
            } else {
                0.0
            };

            // Quadratic shadow falloff around the bezel, sampled against a
            // downward-shifted silhouette; the toolbar gets a tighter ring.
            let shadow_d = bezel_distance(px, py - FRAME_SHADOW_OFFSET_Y).max(0.0);
            let falloff = (1.0 - shadow_d / shadow_reach).clamp(0.0, 1.0);
            let mut shadow = FRAME_SHADOW_ALPHA * falloff * falloff;
            if has_toolbar {
                let toolbar_shadow_d = toolbar_distance(px, py - 2.0).max(0.0);
                let toolbar_falloff = (1.0 - toolbar_shadow_d / 8.0).clamp(0.0, 1.0);
                shadow = shadow.max(0.3 * toolbar_falloff * toolbar_falloff);
            }

            // Composite: toolbar capsule over bezel over shadow.
            let toolbar_alpha = toolbar_coverage * (TOOLBAR_ALPHA as f32 / 255.0);
            let mut alpha = toolbar_alpha
                + (1.0 - toolbar_alpha) * (bezel_coverage + (1.0 - bezel_coverage) * shadow);
            let toolbar_red = (TOOLBAR_COLOR >> 16) & 0xff;
            let toolbar_green = (TOOLBAR_COLOR >> 8) & 0xff;
            let toolbar_blue = TOOLBAR_COLOR & 0xff;
            let mut red = toolbar_red as f32 * toolbar_alpha
                + bezel_red as f32 * bezel_coverage * (1.0 - toolbar_alpha);
            let mut green = toolbar_green as f32 * toolbar_alpha
                + bezel_green as f32 * bezel_coverage * (1.0 - toolbar_alpha);
            let mut blue = toolbar_blue as f32 * toolbar_alpha
                + bezel_blue as f32 * bezel_coverage * (1.0 - toolbar_alpha);

            // Dots paint opaquely over the toolbar. A device that keeps the
            // shell's standard caption buttons (e.g. a simulated desktop) leaves
            // these rects empty — skip them so no stray dot lands at the origin.
            for (rect, color) in dots {
                if rect.right <= rect.left || rect.bottom <= rect.top {
                    continue;
                }
                let cx = (rect.left + rect.right) as f32 / 2.0;
                let cy = (rect.top + rect.bottom) as f32 / 2.0;
                let distance = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
                let coverage = (TOOLBAR_DOT_RADIUS + 0.5 - distance).clamp(0.0, 1.0);
                if coverage > 0.0 {
                    let dot_red = ((color >> 16) & 0xff) as f32;
                    let dot_green = ((color >> 8) & 0xff) as f32;
                    let dot_blue = (color & 0xff) as f32;
                    red = red * (1.0 - coverage) + dot_red * coverage;
                    green = green * (1.0 - coverage) + dot_green * coverage;
                    blue = blue * (1.0 - coverage) + dot_blue * coverage;
                    alpha = alpha.max(coverage);
                }
            }

            let a = (alpha.clamp(0.0, 1.0) * 255.0).round() as u32;
            pixels.push(
                (a << 24)
                    | ((red.round() as u32).min(255) << 16)
                    | ((green.round() as u32).min(255) << 8)
                    | (blue.round() as u32).min(255),
            );
        }
    }
    pixels
}
