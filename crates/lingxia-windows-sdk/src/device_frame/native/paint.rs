//! Layered bitmap painting for the Windows device-frame shell.

use super::*;

use windows::Win32::Graphics::Gdi::{
    CreateFontW, DRAW_TEXT_FORMAT, DT_CALCRECT, DT_END_ELLIPSIS, DT_LEFT, DT_NOPREFIX, DT_RIGHT,
    DT_SINGLELINE, DT_VCENTER, DrawTextW, FW_SEMIBOLD, GetTextExtentPoint32W, SetBkMode,
    SetTextColor, TRANSPARENT, TextOutW,
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
                    // A toolbar-only repaint reuses the previous layout; drop
                    // the optional rects so a control the new model omits
                    // cannot keep a stale hit region.
                    layout.action_rect = RECT::default();
                    layout.rotate_rect = RECT::default();
                    layout.appearance_rect = RECT::default();
                    draw_toolbar_text(memory_dc, toolbar, layout);
                    // GDI writes zero alpha bytes; restore the toolbar
                    // alpha over the text it touched.
                    let dib = std::slice::from_raw_parts_mut(bits.cast::<u32>(), pixels.len());
                    fix_toolbar_alpha(dib, layout);
                    if toolbar.rotate_command.is_some() {
                        composite_toolbar_icon(
                            dib,
                            layout,
                            crate::WindowsDesignIcon::Rotate,
                            layout.rotate_rect,
                        );
                    }
                    if toolbar.appearance_command.is_some() {
                        let icon = if toolbar.appearance_dark {
                            crate::WindowsDesignIcon::AppearanceDark
                        } else {
                            crate::WindowsDesignIcon::AppearanceLight
                        };
                        composite_toolbar_icon(dib, layout, icon, layout.appearance_rect);
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

/// Draws the toolbar's GDI text (the trailing gear glyph and the selector)
/// and fills every toolbar control rect: the gear and selector from the text
/// metrics, plus the rotate/appearance slots that `composite_toolbar_icon`
/// paints after the alpha fix.
fn draw_toolbar_text(dc: HDC, toolbar: &WindowsDeviceFrameToolbar, layout: &mut FrameLayout) {
    unsafe {
        SetBkMode(dc, TRANSPARENT);
    }
    let toolbar_rect = layout.toolbar;
    let center_y = (toolbar_rect.top + toolbar_rect.bottom) / 2;

    // Trailing gear glyph (Segoe MDL2 Assets "Settings", U+E713).
    if toolbar.action_command.is_some() {
        let glyph = to_wide("\u{E713}");
        let glyph_chars = &glyph[..glyph.len().saturating_sub(1)];
        let icon_font = unsafe {
            CreateFontW(
                -layout.px(16),
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
            let x = toolbar_rect.right - layout.px(TOOLBAR_SIDE_MARGIN) - extent.cx;
            let y = center_y - extent.cy / 2;
            let _ = TextOutW(dc, x, y, glyph_chars);
            layout.action_rect = RECT {
                left: x - layout.px(6),
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

    // Selector: "<label>  ⌄", centered when it fits. The device may shrink to
    // fit the work area while the controls keep monitor DPI, so only the label
    // gives way; the arrow is the selector's affordance and stays visible.
    let font = unsafe {
        CreateFontW(
            -layout.px(15),
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
        let text = SelectorText {
            label: text_width(dc, &toolbar.selector_label),
            spacing: text_width(dc, SELECTOR_SPACING),
            arrow: text_width(dc, SELECTOR_ARROW),
            ellipsis: text_width(dc, "\u{2026}"),
        };
        reserve_trailing_icons(toolbar, layout, text.arrow);
        layout.selector_rect =
            selector_rect(layout, text.label + text.spacing + text.arrow, text.arrow);
        let (label_rect, arrow_rect) =
            selector_text_rects(layout.selector_rect, layout.px(SELECTOR_PADDING), text);
        let line = DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX;
        if let Some(rect) = label_rect {
            // Right-aligned: an ellipsized label is shorter than its slot and
            // should stay next to the arrow. A label that fits fills the slot.
            let format = DT_RIGHT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS;
            draw_text_in(dc, &toolbar.selector_label, rect, format);
        }
        if let Some(rect) = arrow_rect {
            draw_text_in(dc, SELECTOR_ARROW, rect, line);
        }
        if !old_font.is_invalid() {
            let _ = SelectObject(dc, old_font);
        }
        let _ = DeleteObject(HGDIOBJ(font.0));
    }

    // The rotate/appearance controls are shared design icons
    // (design/icons/svg), composited after the GDI text pass in
    // `composite_toolbar_icon`.
}

/// Width/height of the trailing toolbar design icons (rotate, appearance).
const TOOLBAR_ICON_SIZE: i32 = 16;

/// Selector inner padding, also its gap to the neighbouring controls.
const SELECTOR_PADDING: i32 = 8;
const SELECTOR_SPACING: &str = "  ";
const SELECTOR_ARROW: &str = "\u{2304}";

/// Selector text widths in its font.
#[derive(Debug, Clone, Copy)]
struct SelectorText {
    label: i32,
    spacing: i32,
    arrow: i32,
    ellipsis: i32,
}

/// Width `DrawTextW` gives `text` in `dc`'s current font. Measured with the
/// renderer: its glyph padding can exceed GetTextExtentPoint32's advance
/// width and cause needless ellipsis.
fn text_width(dc: HDC, text: &str) -> i32 {
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    let mut measured = RECT::default();
    unsafe {
        let _ = DrawTextW(
            dc,
            &mut wide,
            &mut measured,
            DT_CALCRECT | DT_SINGLELINE | DT_NOPREFIX,
        );
    }
    measured.right - measured.left
}

fn draw_text_in(dc: HDC, text: &str, mut rect: RECT, format: DRAW_TEXT_FORMAT) {
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    unsafe {
        let _ = DrawTextW(dc, &mut wide, &mut rect, format);
    }
}

/// Reserves the rotate and appearance slots left of the gear. They have no
/// selector-menu fallback, but device switching outranks them: when the
/// selector would lose room for its arrow, appearance gives way, then rotate.
fn reserve_trailing_icons(
    toolbar: &WindowsDeviceFrameToolbar,
    layout: &mut FrameLayout,
    arrow_width: i32,
) {
    let mut right_anchor = if layout.action_rect.right > layout.action_rect.left {
        layout.action_rect.left - layout.px(4)
    } else {
        layout.toolbar.right - layout.px(TOOLBAR_SIDE_MARGIN)
    };
    if toolbar.rotate_command.is_some() {
        layout.rotate_rect = toolbar_icon_rect(layout, right_anchor);
        right_anchor = layout.rotate_rect.left;
    }
    if toolbar.appearance_command.is_some() {
        layout.appearance_rect = toolbar_icon_rect(layout, right_anchor);
    }
    let min_width = arrow_width + 2 * layout.px(SELECTOR_PADDING);
    let fits = |layout: &FrameLayout| {
        let (left, right) = selector_span(layout);
        right - left >= min_width
    };
    if !fits(layout) {
        layout.appearance_rect = RECT::default();
    }
    if !fits(layout) {
        layout.rotate_rect = RECT::default();
    }
}

/// Horizontal room between the window dots and the leftmost trailing
/// control, keeping the selector's gap to each.
fn selector_span(layout: &FrameLayout) -> (i32, i32) {
    let gap = layout.px(SELECTOR_PADDING);
    let mut left = layout.toolbar.left + layout.px(TOOLBAR_SIDE_MARGIN);
    let mut right = layout.toolbar.right - layout.px(TOOLBAR_SIDE_MARGIN);
    for rect in [layout.close_rect, layout.minimize_rect] {
        if rect.right > rect.left {
            left = left.max(rect.right + gap);
        }
    }
    for rect in [
        layout.action_rect,
        layout.rotate_rect,
        layout.appearance_rect,
    ] {
        if rect.right > rect.left {
            right = right.min(rect.left - gap);
        }
    }
    (left, right)
}

/// Centered when the text fits; otherwise the selector (drawing and click
/// target) takes the room between the controls. Empty only when not even
/// the arrow fits.
fn selector_rect(layout: &FrameLayout, text_width: i32, min_text_width: i32) -> RECT {
    let padding = layout.px(SELECTOR_PADDING);
    let (left, right) = selector_span(layout);
    if right - left < min_text_width + 2 * padding {
        return RECT::default();
    }
    let width = (text_width + 2 * padding).min(right - left);
    let x = ((layout.toolbar.left + layout.toolbar.right - width) / 2).clamp(left, right - width);
    RECT {
        left: x,
        top: layout.toolbar.top,
        right: x + width,
        bottom: layout.toolbar.bottom,
    }
}

/// Label and arrow rects inside `selector`. The arrow keeps its width at the
/// right end and the label takes what is left; once not even an ellipsis
/// fits, the label is dropped and the arrow centered.
fn selector_text_rects(
    selector: RECT,
    padding: i32,
    text: SelectorText,
) -> (Option<RECT>, Option<RECT>) {
    let inner = RECT {
        left: selector.left + padding,
        right: selector.right - padding,
        ..selector
    };
    if inner.right - inner.left < text.arrow {
        return (None, None);
    }
    let label_right = inner.right - text.arrow - text.spacing;
    if label_right - inner.left < text.ellipsis {
        let left = (inner.left + inner.right - text.arrow) / 2;
        let arrow = RECT {
            left,
            right: left + text.arrow,
            ..inner
        };
        return (None, Some(arrow));
    }
    let label = RECT {
        right: label_right,
        ..inner
    };
    let arrow = RECT {
        left: inner.right - text.arrow,
        ..inner
    };
    (Some(label), Some(arrow))
}

fn toolbar_icon_rect(layout: &FrameLayout, right: i32) -> RECT {
    RECT {
        left: right - layout.px(TOOLBAR_ICON_SIZE) - layout.px(6),
        top: layout.toolbar.top,
        right,
        bottom: layout.toolbar.bottom,
    }
}

/// Composites into the precomputed hit region after `fix_toolbar_alpha`, so
/// GDI cannot erase the design icon's premultiplied alpha.
fn composite_toolbar_icon(
    dib: &mut [u32],
    layout: &FrameLayout,
    design_icon: crate::WindowsDesignIcon,
    rect: RECT,
) {
    // A slot dropped for lack of room.
    if rect.right <= rect.left {
        return;
    }
    let center_y = (rect.top + rect.bottom) / 2;
    let size = layout.px(TOOLBAR_ICON_SIZE);
    let x = rect.right - size;
    let y = center_y - size / 2;
    let Some(icon) = crate::design_icons::design_icon_argb_premultiplied(
        design_icon,
        size as u32,
        Some(0x00B4B4B4),
    ) else {
        return;
    };
    for iy in 0..size {
        for ix in 0..size {
            let src = icon[(iy * size + ix) as usize];
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
    let radius = outer_corner_radius(spec) as f32;
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

    // Toolbar and shadow are host chrome at the layout's chrome scale; the
    // bezel geometry above is already fit-scaled.
    let chrome_scale = layout.chrome_scale as f32;
    let has_toolbar = spec.toolbar.is_some();
    let toolbar = layout.toolbar;
    let toolbar_radius = TOOLBAR_RADIUS as f32 * chrome_scale;
    let toolbar_center_x = (toolbar.left + toolbar.right) as f32 / 2.0;
    let toolbar_center_y = (toolbar.top + toolbar.bottom) as f32 / 2.0;
    let toolbar_half_x = (toolbar.right - toolbar.left) as f32 / 2.0 - toolbar_radius;
    let toolbar_half_y = (toolbar.bottom - toolbar.top) as f32 / 2.0 - toolbar_radius;
    let toolbar_distance = move |x: f32, y: f32| {
        rounded_distance(
            x - toolbar_center_x,
            y - toolbar_center_y,
            toolbar_half_x,
            toolbar_half_y,
            toolbar_radius,
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

    // A frameless screen (bezel 0) has no body ring; a hairline outline
    // hugging the silhouette takes its place. Painted mostly OUTSIDE the
    // edge: the content window's region cut covers the inner half, so the
    // visible ~1px line is this bitmap's anti-aliased pixels — smooth along
    // the full perimeter, status/nav strip included (surfaces only patch
    // their own corner arcs on the inside).
    let frameless = spec.bezel_width <= 0;
    let outline_alpha = ((FRAMELESS_OUTLINE_COLOR >> 24) & 0xff) as f32 / 255.0;
    let outline_red = ((FRAMELESS_OUTLINE_COLOR >> 16) & 0xff) as f32;
    let outline_green = ((FRAMELESS_OUTLINE_COLOR >> 8) & 0xff) as f32;
    let outline_blue = (FRAMELESS_OUTLINE_COLOR & 0xff) as f32;

    let shadow_reach = layout.px(FRAME_SHADOW_MARGIN) as f32;
    let shadow_offset = FRAME_SHADOW_OFFSET_Y * chrome_scale;
    let toolbar_shadow_offset = 2.0 * chrome_scale;
    let toolbar_shadow_reach = 8.0 * chrome_scale;
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
            let shadow_d = bezel_distance(px, py - shadow_offset).max(0.0);
            let falloff = (1.0 - shadow_d / shadow_reach).clamp(0.0, 1.0);
            let mut shadow = FRAME_SHADOW_ALPHA * falloff * falloff;
            if has_toolbar {
                let toolbar_shadow_d = toolbar_distance(px, py - toolbar_shadow_offset).max(0.0);
                let toolbar_falloff =
                    (1.0 - toolbar_shadow_d / toolbar_shadow_reach).clamp(0.0, 1.0);
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

            // Outline hairline: a solid ~1px band just outside the
            // silhouette edge with anti-aliased flanks (trapezoid profile —
            // a triangle profile reads fuzzy), composited over body +
            // shadow. The content region covers the inner flank.
            if frameless {
                let line = ((bezel_d + 0.5) * 2.0).clamp(0.0, 1.0)
                    * ((1.0 - bezel_d) * 2.0).clamp(0.0, 1.0)
                    * outline_alpha;
                if line > 0.0 {
                    red = outline_red * line + red * (1.0 - line);
                    green = outline_green * line + green * (1.0 - line);
                    blue = outline_blue * line + blue * (1.0 - line);
                    alpha = line + alpha * (1.0 - line);
                }
            }

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
                let radius = (rect.right - rect.left) as f32 / 2.0;
                let coverage = (radius + 0.5 - distance).clamp(0.0, 1.0);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn toolbar_layout(scale: f64, work: RECT, commands: u8, label: &str) -> FrameLayout {
        let mut spec = WindowsDeviceFrame {
            screen_width: 430,
            screen_height: 932,
            bezel_width: 4,
            outer_corner_radius: 60,
            screen_corner_radius: 56,
            cutout: None,
            status_bar: None,
            bezel_color: 0,
            screen_corner_color: 0,
            toolbar: Some(WindowsDeviceFrameToolbar {
                selector_label: label.to_string(),
                selector_items: Vec::new(),
                action_command: (commands & 1 != 0).then_some(1),
                rotate_command: (commands & 2 != 0).then_some(2),
                appearance_command: (commands & 4 != 0).then_some(3),
                appearance_dark: false,
                capsule_items: Vec::new(),
                capsule_close_command: None,
                window_dots: true,
            }),
        };
        let fit = fit_scale_in(work, &spec, scale);
        scale_spec(&mut spec, fit);
        let mut layout = compute_layout(&spec, scale);
        // Real GDI text metrics on a memory DC. The per-pixel frame bitmap
        // does not affect control placement and is slow in debug builds.
        unsafe {
            let screen = GetDC(None);
            let dc = CreateCompatibleDC(Some(screen));
            draw_toolbar_text(dc, spec.toolbar.as_ref().unwrap(), &mut layout);
            let _ = DeleteDC(dc);
            let _ = ReleaseDC(None, screen);
        }
        layout
    }

    fn assert_disjoint_controls(layout: &FrameLayout) {
        let controls = [
            layout.close_rect,
            layout.minimize_rect,
            layout.selector_rect,
            layout.action_rect,
            layout.rotate_rect,
            layout.appearance_rect,
        ];
        assert!(layout.selector_rect.right > layout.selector_rect.left);
        for (index, rect) in controls.iter().enumerate() {
            if rect.right <= rect.left {
                continue;
            }
            assert!(rect.left >= layout.toolbar.left && rect.right <= layout.toolbar.right);
            for other in &controls[index + 1..] {
                if other.right > other.left {
                    assert!(
                        rect.right <= other.left || other.right <= rect.left,
                        "overlapping controls at scale {}: {rect:?}, {other:?}",
                        layout.chrome_scale,
                    );
                }
            }
        }
    }

    #[test]
    fn fitted_toolbar_controls_do_not_overlap_at_high_dpi() {
        let work = RECT {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1040,
        };
        for scale in [1.0, 1.5, 2.25] {
            let layout = toolbar_layout(scale, work, 7, "iPhone 15 Pro Max");
            assert_disjoint_controls(&layout);
        }
    }

    #[test]
    fn long_selector_reserves_only_present_controls() {
        let work = RECT {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1040,
        };
        for commands in 0..8 {
            let layout = toolbar_layout(1.5, work, commands, &"很长的模拟设备名称".repeat(8));
            assert_disjoint_controls(&layout);
            assert_eq!(
                layout.action_rect.right > layout.action_rect.left,
                commands & 1 != 0
            );
            assert_eq!(
                layout.rotate_rect.right > layout.rotate_rect.left,
                commands & 2 != 0
            );
            assert_eq!(
                layout.appearance_rect.right > layout.appearance_rect.left,
                commands & 4 != 0
            );
        }
    }

    #[test]
    fn narrowest_toolbar_drops_toggles_before_the_selector_arrow() {
        // 1366x768 at 200%: with every icon kept, no room is left for the
        // selector at all.
        let work = RECT {
            left: 0,
            top: 0,
            right: 1366,
            bottom: 728,
        };
        let layout = toolbar_layout(2.0, work, 7, "iPhone 15 Pro Max");
        assert_disjoint_controls(&layout);
        assert!(layout.action_rect.right > layout.action_rect.left);
        assert_eq!(layout.appearance_rect, RECT::default());
        assert_eq!(layout.rotate_rect, RECT::default());
    }

    #[test]
    fn cut_label_keeps_the_arrow_at_the_end() {
        let text = SelectorText {
            label: 200,
            spacing: 10,
            arrow: 20,
            ellipsis: 12,
        };
        let selector = |width: i32| RECT {
            left: 100,
            top: 0,
            right: 100 + width,
            bottom: 40,
        };

        // Fits: label, spacing and arrow back to back.
        let (label, arrow) = selector_text_rects(selector(246), 8, text);
        let (label, arrow) = (label.unwrap(), arrow.unwrap());
        assert_eq!((label.left, label.right), (108, 308));
        assert_eq!((arrow.left, arrow.right), (318, 338));

        // Cut: the label shrinks, the arrow keeps its width at the end.
        let (label, arrow) = selector_text_rects(selector(100), 8, text);
        let (label, arrow) = (label.unwrap(), arrow.unwrap());
        assert_eq!((arrow.left, arrow.right), (172, 192));
        assert_eq!(label.right, arrow.left - 10);

        // Not even an ellipsis fits: the arrow alone, centered.
        let (label, arrow) = selector_text_rects(selector(56), 8, text);
        assert!(label.is_none());
        assert_eq!(
            arrow.map(|arrow| (arrow.left, arrow.right)),
            Some((118, 138))
        );
    }
}
