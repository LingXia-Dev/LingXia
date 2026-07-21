//! Grayscale text-mask compositing for per-pixel-alpha windows.

use std::ffi::c_void;

use windows::Win32::Foundation::{COLORREF, RECT};
use windows::Win32::Graphics::Gdi::{
    ANTIALIASED_QUALITY, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CLIP_DEFAULT_PRECIS,
    CreateCompatibleDC, CreateDIBSection, CreateFontW, DEFAULT_CHARSET, DEFAULT_PITCH,
    DIB_RGB_COLORS, DT_CENTER, DT_END_ELLIPSIS, DT_LEFT, DT_SINGLELINE, DT_VCENTER, DeleteDC,
    DeleteObject, DrawTextW, FF_SWISS, HDC, HGDIOBJ, OUT_DEFAULT_PRECIS, SelectObject, SetBkMode,
    SetTextColor, TRANSPARENT,
};
use windows::core::w;

const TEXT_MASK_SCALE: i32 = 4;

/// Draws text into a 4x white grayscale mask, downsamples its coverage, and
/// composites premultiplied foreground pixels into a top-down layered DIB.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_supersampled_text_mask(
    reference_dc: HDC,
    pixels: &mut [u32],
    canvas_width: i32,
    canvas_height: i32,
    text: &str,
    rect: RECT,
    foreground: u32,
    font_height: i32,
    font_weight: i32,
    centered: bool,
) {
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    if text.is_empty() || width <= 0 || height <= 0 || canvas_width <= 0 || canvas_height <= 0 {
        return;
    }
    let Some(mask_width) = width.checked_mul(TEXT_MASK_SCALE) else {
        draw_text_mask_fallback(
            reference_dc,
            pixels,
            canvas_width,
            canvas_height,
            text,
            rect,
            foreground,
            font_height,
            font_weight,
            centered,
        );
        return;
    };
    let Some(mask_height) = height.checked_mul(TEXT_MASK_SCALE) else {
        draw_text_mask_fallback(
            reference_dc,
            pixels,
            canvas_width,
            canvas_height,
            text,
            rect,
            foreground,
            font_height,
            font_weight,
            centered,
        );
        return;
    };
    let Some(pixel_count) = (mask_width as usize).checked_mul(mask_height as usize) else {
        return;
    };

    unsafe {
        let mask_dc = CreateCompatibleDC(Some(reference_dc));
        if mask_dc.is_invalid() {
            draw_text_mask_fallback(
                reference_dc,
                pixels,
                canvas_width,
                canvas_height,
                text,
                rect,
                foreground,
                font_height,
                font_weight,
                centered,
            );
            return;
        }
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: mask_width,
                biHeight: -mask_height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut c_void = std::ptr::null_mut();
        let Ok(bitmap) = CreateDIBSection(
            Some(reference_dc),
            &info,
            DIB_RGB_COLORS,
            &mut bits,
            None,
            0,
        ) else {
            let _ = DeleteDC(mask_dc);
            draw_text_mask_fallback(
                reference_dc,
                pixels,
                canvas_width,
                canvas_height,
                text,
                rect,
                foreground,
                font_height,
                font_weight,
                centered,
            );
            return;
        };
        if bits.is_null() {
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = DeleteDC(mask_dc);
            draw_text_mask_fallback(
                reference_dc,
                pixels,
                canvas_width,
                canvas_height,
                text,
                rect,
                foreground,
                font_height,
                font_weight,
                centered,
            );
            return;
        }
        let old_bitmap = SelectObject(mask_dc, HGDIOBJ(bitmap.0));
        std::ptr::write_bytes(
            bits.cast::<u8>(),
            0,
            pixel_count * std::mem::size_of::<u32>(),
        );
        paint_white_text(
            mask_dc,
            text,
            RECT {
                left: 0,
                top: 0,
                right: mask_width,
                bottom: mask_height,
            },
            font_height * TEXT_MASK_SCALE,
            font_weight,
            centered,
        );

        let mask = std::slice::from_raw_parts(bits.cast::<u32>(), pixel_count);
        for local_y in 0..height {
            let target_y = rect.top + local_y;
            if !(0..canvas_height).contains(&target_y) {
                continue;
            }
            for local_x in 0..width {
                let target_x = rect.left + local_x;
                if !(0..canvas_width).contains(&target_x) {
                    continue;
                }
                let coverage = downsample_coverage(
                    mask,
                    mask_width,
                    local_x * TEXT_MASK_SCALE,
                    local_y * TEXT_MASK_SCALE,
                );
                if coverage == 0 {
                    continue;
                }
                let target = &mut pixels[(target_y * canvas_width + target_x) as usize];
                if (*target >> 24) == 0 {
                    *target = premultiplied_foreground(foreground, coverage);
                }
            }
        }

        if !old_bitmap.is_invalid() {
            let _ = SelectObject(mask_dc, old_bitmap);
        }
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(mask_dc);
    }
}

fn downsample_coverage(mask: &[u32], mask_width: i32, left: i32, top: i32) -> u32 {
    let mut coverage = 0;
    for y in top..top + TEXT_MASK_SCALE {
        let row = y as usize * mask_width as usize;
        for x in left..left + TEXT_MASK_SCALE {
            let pixel = mask[row + x as usize];
            coverage += ((pixel >> 16) & 0xff)
                .max((pixel >> 8) & 0xff)
                .max(pixel & 0xff);
        }
    }
    let samples = (TEXT_MASK_SCALE * TEXT_MASK_SCALE) as u32;
    (coverage + samples / 2) / samples
}

#[allow(clippy::too_many_arguments)]
fn draw_text_mask_fallback(
    dc: HDC,
    pixels: &mut [u32],
    canvas_width: i32,
    canvas_height: i32,
    text: &str,
    rect: RECT,
    foreground: u32,
    font_height: i32,
    font_weight: i32,
    centered: bool,
) {
    unsafe {
        paint_white_text(dc, text, rect, font_height, font_weight, centered);
    }
    premultiply_grayscale_rect(pixels, canvas_width, canvas_height, rect, foreground);
}

unsafe fn paint_white_text(
    dc: HDC,
    text: &str,
    rect: RECT,
    font_height: i32,
    font_weight: i32,
    centered: bool,
) {
    let font = unsafe {
        CreateFontW(
            -font_height.max(1),
            0,
            0,
            0,
            font_weight,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            ANTIALIASED_QUALITY,
            DEFAULT_PITCH.0 as u32 | FF_SWISS.0 as u32,
            w!("Segoe UI"),
        )
    };
    if font.is_invalid() {
        return;
    }
    let mut wide = text.encode_utf16().collect::<Vec<_>>();
    let mut rect = rect;
    unsafe {
        let old_font = SelectObject(dc, HGDIOBJ(font.0));
        let _ = SetBkMode(dc, TRANSPARENT);
        let _ = SetTextColor(dc, COLORREF(0x00ff_ffff));
        let alignment = if centered { DT_CENTER } else { DT_LEFT };
        let _ = DrawTextW(
            dc,
            &mut wide,
            &mut rect,
            alignment | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
        );
        if !old_font.is_invalid() {
            let _ = SelectObject(dc, old_font);
        }
        let _ = DeleteObject(HGDIOBJ(font.0));
    }
}

/// Recolors only the clipped mask rectangle in a top-down `width` by `height`
/// DIB. This lets a layered surface carry labels with different colors.
pub(crate) fn premultiply_grayscale_rect(
    pixels: &mut [u32],
    width: i32,
    height: i32,
    rect: RECT,
    foreground: u32,
) {
    if width <= 0 || height <= 0 {
        return;
    }
    let left = rect.left.clamp(0, width);
    let top = rect.top.clamp(0, height);
    let right = rect.right.clamp(left, width);
    let bottom = rect.bottom.clamp(top, height);
    for y in top..bottom {
        let row = y as usize * width as usize;
        for x in left..right {
            if let Some(pixel) = pixels.get_mut(row + x as usize) {
                premultiply_grayscale_pixel(pixel, foreground);
            }
        }
    }
}

fn premultiply_grayscale_pixel(pixel: &mut u32, foreground: u32) {
    if (*pixel >> 24) != 0 {
        return;
    }
    // White glyphs drawn by GDI over a zeroed DIB encode coverage in RGB.
    let coverage = ((*pixel >> 16) & 0xff)
        .max((*pixel >> 8) & 0xff)
        .max(*pixel & 0xff);
    if coverage == 0 {
        return;
    }
    *pixel = premultiplied_foreground(foreground, coverage);
}

fn premultiplied_foreground(foreground: u32, coverage: u32) -> u32 {
    let premultiply = |channel: u32| (channel * coverage + 127) / 255;
    let red = premultiply((foreground >> 16) & 0xff);
    let green = premultiply((foreground >> 8) & 0xff);
    let blue = premultiply(foreground & 0xff);
    (coverage << 24) | (red << 16) | (green << 8) | blue
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grayscale_coverage_becomes_premultiplied_foreground() {
        let mut pixels = [0x0080_8080];

        premultiply_grayscale_pixel(&mut pixels[0], 0x16_77ff);

        assert_eq!(pixels, [0x800b_3c80]);
    }

    #[test]
    fn transparent_and_existing_alpha_pixels_are_preserved() {
        let mut pixels = [0, 0xff12_3456];

        for pixel in &mut pixels {
            premultiply_grayscale_pixel(pixel, 0xff_0000);
        }

        assert_eq!(pixels, [0, 0xff12_3456]);
    }

    #[test]
    fn rect_compositing_is_clipped_to_the_dib() {
        let mut pixels = [0x0040_4040; 6];

        premultiply_grayscale_rect(
            &mut pixels,
            3,
            2,
            RECT {
                left: -1,
                top: 0,
                right: 2,
                bottom: 1,
            },
            0xff_ffff,
        );

        assert_eq!(pixels[..2], [0x4040_4040; 2]);
        assert_eq!(pixels[2..], [0x0040_4040; 4]);
    }

    #[test]
    fn supersampled_coverage_is_box_downsampled() {
        let mut mask = [0u32; 16];
        mask[..8].fill(0x00ff_ffff);

        assert_eq!(downsample_coverage(&mask, 4, 0, 0), 128);
    }
}
