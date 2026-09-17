//! System clipboard for `lx.clipboard`: Unicode text and images.
//!
//! Images are published as both the registered `PNG` format and `CF_DIBV5`:
//! browsers and image editors take the PNG, while Office, Paint, Explorer and
//! most Win32 pasters only understand a DIB. Reads accept `PNG`, `CF_DIBV5` and
//! `CF_DIB` (which the system also synthesizes from `CF_BITMAP`, so a
//! screenshot or a Paint copy is visible too).
//!
//! Every Win32 call runs on the blocking pool: `OpenClipboard` spins while
//! another process holds the clipboard and must never stall the executor.

use std::fs;
use std::io::Cursor;
use std::path::Path;

use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{
    GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
};
use windows::Win32::System::Ole::{CF_DIB, CF_DIBV5, CF_UNICODETEXT};
use windows::core::w;

use crate::error::PlatformError;
use crate::traits::clipboard::{
    ClipboardContents, ClipboardKind, ClipboardReadRequest, ClipboardService, ClipboardTypes,
    ClipboardWrite,
};

use super::Platform;

const CLIPBOARD_RETRIES: u32 = 8;

const BI_RGB: u32 = 0;
const BI_BITFIELDS: u32 = 3;
const BITMAPINFOHEADER_SIZE: usize = 40;
const BITMAPV5HEADER_SIZE: usize = 124;
const LCS_SRGB: u32 = 0x7352_4742; // 'sRGB'
const LCS_GM_IMAGES: u32 = 4;

fn cf_unicode() -> u32 {
    CF_UNICODETEXT.0 as u32
}

fn cf_dib() -> u32 {
    CF_DIB.0 as u32
}

fn cf_dibv5() -> u32 {
    CF_DIBV5.0 as u32
}

fn cf_png() -> u32 {
    unsafe { RegisterClipboardFormatW(w!("PNG")) }
}

fn with_clipboard<T>(f: impl FnOnce() -> Result<T, PlatformError>) -> Result<T, PlatformError> {
    unsafe {
        let mut last = None;
        for _ in 0..CLIPBOARD_RETRIES {
            match OpenClipboard(None) {
                Ok(()) => {
                    let result = f();
                    let _ = CloseClipboard();
                    return result;
                }
                Err(e) => last = Some(e),
            }
            std::thread::sleep(std::time::Duration::from_millis(8));
        }
        Err(PlatformError::Platform(format!(
            "OpenClipboard failed: {}",
            last.map(|e| e.to_string())
                .unwrap_or_else(|| "unknown".into())
        )))
    }
}

/// Copies a clipboard global block out while it is locked. Returns `None`
/// when the format is absent or the block is unreadable.
fn read_global_bytes(format: u32) -> Option<Vec<u8>> {
    unsafe {
        if IsClipboardFormatAvailable(format).is_err() {
            return None;
        }
        let handle = GetClipboardData(format).ok()?;
        let hglobal = HGLOBAL(handle.0);
        let ptr = GlobalLock(hglobal) as *const u8;
        if ptr.is_null() {
            return None;
        }
        let size = GlobalSize(hglobal);
        let bytes = std::slice::from_raw_parts(ptr, size).to_vec();
        let _ = GlobalUnlock(hglobal);
        if bytes.is_empty() { None } else { Some(bytes) }
    }
}

/// Hands a byte block to the clipboard; ownership moves to the system on
/// success and is released here on failure.
fn set_global_bytes(format: u32, bytes: &[u8]) -> Result<(), PlatformError> {
    unsafe {
        let hmem: HGLOBAL = GlobalAlloc(GMEM_MOVEABLE, bytes.len())
            .map_err(|e| PlatformError::Platform(format!("GlobalAlloc failed: {e}")))?;
        let dst = GlobalLock(hmem) as *mut u8;
        if dst.is_null() {
            let _ = GlobalFree(Some(hmem));
            return Err(PlatformError::Platform("GlobalLock failed".into()));
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), dst, bytes.len());
        let _ = GlobalUnlock(hmem);
        if let Err(e) = SetClipboardData(format, Some(HANDLE(hmem.0))) {
            let _ = GlobalFree(Some(hmem));
            return Err(PlatformError::Platform(format!(
                "SetClipboardData failed: {e}"
            )));
        }
    }
    Ok(())
}

fn read_unicode_text() -> Option<String> {
    let bytes = read_global_bytes(cf_unicode())?;
    // The block is NUL-terminated but GlobalSize may round up, so bound the
    // scan by the block rather than trusting the terminator alone.
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|unit| *unit != 0)
        .collect();
    Some(String::from_utf16_lossy(&units))
}

fn set_unicode_text(text: &str) -> Result<(), PlatformError> {
    let utf16: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes: Vec<u8> = utf16.iter().flat_map(|unit| unit.to_le_bytes()).collect();
    set_global_bytes(cf_unicode(), &bytes)
}

fn decode_image(path: &str) -> Result<image::RgbaImage, PlatformError> {
    let bytes = fs::read(path).map_err(|e| {
        PlatformError::InvalidParameter(format!("clipboard image is not readable: {e}"))
    })?;
    image::load_from_memory(&bytes)
        .map(|decoded| decoded.to_rgba8())
        .map_err(|e| {
            PlatformError::InvalidParameter(format!("clipboard image is not an image: {e}"))
        })
}

fn encode_png(rgba: &image::RgbaImage) -> Result<Vec<u8>, PlatformError> {
    let mut png = Vec::new();
    rgba.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| PlatformError::Platform(format!("failed to encode clipboard PNG: {e}")))?;
    Ok(png)
}

/// Packs a 32bpp BGRA bottom-up `BITMAPV5HEADER` DIB with explicit masks, so
/// alpha survives in pasters that honor V5 and the image still renders in
/// those that only read the `BITMAPINFOHEADER` prefix.
fn encode_dibv5(rgba: &image::RgbaImage) -> Vec<u8> {
    let (width, height) = rgba.dimensions();
    let stride = width as usize * 4;
    let image_size = stride * height as usize;
    let mut out = Vec::with_capacity(BITMAPV5HEADER_SIZE + image_size);
    let push_u32 = |out: &mut Vec<u8>, v: u32| out.extend_from_slice(&v.to_le_bytes());
    let push_i32 = |out: &mut Vec<u8>, v: i32| out.extend_from_slice(&v.to_le_bytes());
    let push_u16 = |out: &mut Vec<u8>, v: u16| out.extend_from_slice(&v.to_le_bytes());

    push_u32(&mut out, BITMAPV5HEADER_SIZE as u32); // bV5Size
    push_i32(&mut out, width as i32); // bV5Width
    push_i32(&mut out, height as i32); // bV5Height (bottom-up)
    push_u16(&mut out, 1); // bV5Planes
    push_u16(&mut out, 32); // bV5BitCount
    push_u32(&mut out, BI_BITFIELDS); // bV5Compression
    push_u32(&mut out, image_size as u32); // bV5SizeImage
    push_i32(&mut out, 2835); // bV5XPelsPerMeter (72 dpi)
    push_i32(&mut out, 2835); // bV5YPelsPerMeter
    push_u32(&mut out, 0); // bV5ClrUsed
    push_u32(&mut out, 0); // bV5ClrImportant
    push_u32(&mut out, 0x00FF_0000); // bV5RedMask
    push_u32(&mut out, 0x0000_FF00); // bV5GreenMask
    push_u32(&mut out, 0x0000_00FF); // bV5BlueMask
    push_u32(&mut out, 0xFF00_0000); // bV5AlphaMask
    push_u32(&mut out, LCS_SRGB); // bV5CSType
    out.extend_from_slice(&[0u8; 36]); // bV5Endpoints
    push_u32(&mut out, 0); // bV5GammaRed
    push_u32(&mut out, 0); // bV5GammaGreen
    push_u32(&mut out, 0); // bV5GammaBlue
    push_u32(&mut out, LCS_GM_IMAGES); // bV5Intent
    push_u32(&mut out, 0); // bV5ProfileData
    push_u32(&mut out, 0); // bV5ProfileSize
    push_u32(&mut out, 0); // bV5Reserved
    debug_assert_eq!(out.len(), BITMAPV5HEADER_SIZE);

    for row in rgba.rows().rev() {
        for px in row {
            out.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
        }
    }
    out
}

/// A channel mask that is one contiguous byte; anything else falls back to the
/// conventional BGRA order.
fn mask_shift(mask: u32) -> Option<u32> {
    let shift = mask.trailing_zeros();
    (mask != 0 && mask == 0xFF << shift).then_some(shift)
}

/// Decodes the uncompressed 24/32bpp DIBs that clipboard producers emit.
/// Paletted and RLE DIBs are rare on the clipboard and are reported absent.
fn decode_dib(bytes: &[u8]) -> Option<image::RgbaImage> {
    let u32_at = |offset: usize| -> Option<u32> {
        bytes
            .get(offset..offset + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let header_size = u32_at(0)? as usize;
    if header_size < BITMAPINFOHEADER_SIZE || header_size > bytes.len() {
        return None;
    }
    let width = u32_at(4)? as i32;
    let raw_height = u32_at(8)? as i32;
    let bit_count = u16::from_le_bytes([bytes[14], bytes[15]]) as u32;
    let compression = u32_at(16)?;
    let clr_used = u32_at(32)? as usize;
    if width <= 0 || raw_height == 0 || raw_height == i32::MIN {
        return None;
    }
    let top_down = raw_height < 0;
    let height = raw_height.unsigned_abs() as usize;
    let width = width as usize;

    let (mut masks, mut pixel_offset) =
        ([0x00FF_0000u32, 0x0000_FF00, 0x0000_00FF, 0], header_size);
    match (bit_count, compression) {
        (24, BI_RGB) => {}
        (32, BI_RGB) => {}
        (32, BI_BITFIELDS) => {
            let mask_base = if header_size == BITMAPINFOHEADER_SIZE {
                pixel_offset += 12;
                BITMAPINFOHEADER_SIZE
            } else {
                40
            };
            masks[0] = u32_at(mask_base)?;
            masks[1] = u32_at(mask_base + 4)?;
            masks[2] = u32_at(mask_base + 8)?;
            if header_size >= 56 {
                masks[3] = u32_at(mask_base + 12)?;
            }
        }
        _ => return None,
    }
    // Some producers store a (useless) palette even at 32bpp.
    pixel_offset += clr_used.saturating_mul(4);

    let bytes_per_px = (bit_count / 8) as usize;
    let stride = (width * bytes_per_px).div_ceil(4) * 4;
    let pixels = bytes.get(pixel_offset..pixel_offset + stride * height)?;

    let shifts = [
        mask_shift(masks[0]).unwrap_or(16),
        mask_shift(masks[1]).unwrap_or(8),
        mask_shift(masks[2]).unwrap_or(0),
    ];
    let alpha_shift = mask_shift(masks[3]);

    let mut rgba = image::RgbaImage::new(width as u32, height as u32);
    let mut saw_alpha = false;
    for y in 0..height {
        let src_row = if top_down { y } else { height - 1 - y };
        let row = &pixels[src_row * stride..src_row * stride + width * bytes_per_px];
        for x in 0..width {
            let px = &row[x * bytes_per_px..(x + 1) * bytes_per_px];
            let (r, g, b, a) = if bytes_per_px == 3 {
                (px[2], px[1], px[0], 255)
            } else {
                let v = u32::from_le_bytes([px[0], px[1], px[2], px[3]]);
                let a = match alpha_shift {
                    Some(shift) => ((v >> shift) & 0xFF) as u8,
                    None => px[3],
                };
                (
                    ((v >> shifts[0]) & 0xFF) as u8,
                    ((v >> shifts[1]) & 0xFF) as u8,
                    ((v >> shifts[2]) & 0xFF) as u8,
                    a,
                )
            };
            saw_alpha |= a != 0;
            rgba.put_pixel(x as u32, y as u32, image::Rgba([r, g, b, a]));
        }
    }
    // BI_RGB 32bpp leaves the fourth byte undefined; an all-zero channel is a
    // producer that never wrote alpha, not a fully transparent image.
    if bytes_per_px == 4 && alpha_shift.is_none() && !saw_alpha {
        for px in rgba.pixels_mut() {
            px[3] = 255;
        }
    }
    Some(rgba)
}

fn read_image_png() -> Result<Option<Vec<u8>>, PlatformError> {
    let png_format = cf_png();
    if png_format != 0
        && let Some(png) = read_global_bytes(png_format)
    {
        return Ok(Some(png));
    }
    for format in [cf_dibv5(), cf_dib()] {
        if let Some(dib) = read_global_bytes(format)
            && let Some(rgba) = decode_dib(&dib)
        {
            return encode_png(&rgba).map(Some);
        }
    }
    Ok(None)
}

fn image_available() -> bool {
    unsafe {
        let png_format = cf_png();
        (png_format != 0 && IsClipboardFormatAvailable(png_format).is_ok())
            || IsClipboardFormatAvailable(cf_dibv5()).is_ok()
            || IsClipboardFormatAvailable(cf_dib()).is_ok()
    }
}

fn write_png_file(dest: &str, png: &[u8]) -> Result<String, PlatformError> {
    if let Some(parent) = Path::new(dest).parent() {
        fs::create_dir_all(parent).map_err(|e| {
            PlatformError::Platform(format!("failed to prepare clipboard image dir: {e}"))
        })?;
    }
    fs::write(dest, png)
        .map_err(|e| PlatformError::Platform(format!("failed to write clipboard image: {e}")))?;
    Ok(dest.to_string())
}

fn write_blocking(item: ClipboardWrite) -> Result<(), PlatformError> {
    match item {
        ClipboardWrite::Text(text) => with_clipboard(|| {
            unsafe {
                let _ = EmptyClipboard();
            }
            set_unicode_text(&text)
        }),
        ClipboardWrite::Image { path } => {
            let rgba = decode_image(&path)?;
            let png = encode_png(&rgba)?;
            let dib = encode_dibv5(&rgba);
            let png_format = cf_png();
            with_clipboard(|| {
                unsafe {
                    let _ = EmptyClipboard();
                }
                set_global_bytes(cf_dibv5(), &dib)?;
                if png_format != 0 {
                    set_global_bytes(png_format, &png)?;
                }
                Ok(())
            })
        }
    }
}

fn read_blocking(request: ClipboardReadRequest) -> Result<ClipboardContents, PlatformError> {
    with_clipboard(|| {
        let want_text = request.kind.is_none() || request.kind == Some(ClipboardKind::Text);
        let want_image = request.kind.is_none() || request.kind == Some(ClipboardKind::Image);
        let text = if want_text { read_unicode_text() } else { None };
        let image = if want_image {
            match read_image_png()? {
                Some(png) => {
                    let dest = request.image_output_path.as_deref().ok_or_else(|| {
                        PlatformError::InvalidParameter(
                            "clipboard image read requires an output path".into(),
                        )
                    })?;
                    Some((write_png_file(dest, &png)?, "image/png".to_string()))
                }
                None => None,
            }
        } else {
            None
        };
        Ok(ClipboardContents {
            canceled: false,
            text,
            image_path: image.as_ref().map(|(path, _)| path.clone()),
            image_mime: image.map(|(_, mime)| mime),
        })
    })
}

fn types_blocking() -> Result<ClipboardTypes, PlatformError> {
    with_clipboard(|| {
        let mut kinds = Vec::new();
        if unsafe { IsClipboardFormatAvailable(cf_unicode()) }.is_ok() {
            kinds.push(ClipboardKind::Text);
        }
        if image_available() {
            kinds.push(ClipboardKind::Image);
        }
        Ok(ClipboardTypes {
            canceled: false,
            kinds,
        })
    })
}

impl ClipboardService for Platform {
    async fn clipboard_write(&self, item: ClipboardWrite) -> Result<(), PlatformError> {
        crate::rt::blocking(move || write_blocking(item)).await
    }

    async fn clipboard_read(
        &self,
        request: ClipboardReadRequest,
    ) -> Result<ClipboardContents, PlatformError> {
        crate::rt::blocking(move || read_blocking(request)).await
    }

    async fn clipboard_clear(&self) -> Result<(), PlatformError> {
        crate::rt::blocking(|| {
            with_clipboard(|| {
                unsafe {
                    let _ = EmptyClipboard();
                }
                Ok(())
            })
        })
        .await
    }

    async fn clipboard_types(&self) -> Result<ClipboardTypes, PlatformError> {
        crate::rt::blocking(types_blocking).await
    }
}

#[cfg(test)]
mod tests {
    use super::{decode_dib, encode_dibv5};

    fn sample() -> image::RgbaImage {
        let mut img = image::RgbaImage::new(3, 2);
        img.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
        img.put_pixel(1, 0, image::Rgba([0, 255, 0, 128]));
        img.put_pixel(2, 0, image::Rgba([0, 0, 255, 0]));
        img.put_pixel(0, 1, image::Rgba([1, 2, 3, 4]));
        img.put_pixel(1, 1, image::Rgba([9, 8, 7, 6]));
        img.put_pixel(2, 1, image::Rgba([250, 251, 252, 253]));
        img
    }

    #[test]
    fn dibv5_round_trips_with_alpha() {
        let img = sample();
        let dib = encode_dibv5(&img);
        assert_eq!(dib.len(), 124 + 3 * 2 * 4);
        let decoded = decode_dib(&dib).expect("decodes");
        assert_eq!(decoded.dimensions(), (3, 2));
        assert_eq!(decoded.as_raw(), img.as_raw());
    }

    #[test]
    fn plain_bitmapinfoheader_24bpp_bottom_up() {
        // 2x2, rows padded to 8 bytes (2 px * 3 B = 6, round up to 8).
        let mut dib = Vec::new();
        dib.extend_from_slice(&40u32.to_le_bytes());
        dib.extend_from_slice(&2i32.to_le_bytes());
        dib.extend_from_slice(&2i32.to_le_bytes());
        dib.extend_from_slice(&1u16.to_le_bytes());
        dib.extend_from_slice(&24u16.to_le_bytes());
        dib.extend_from_slice(&[0u8; 24]);
        // bottom row first: (B,G,R)
        dib.extend_from_slice(&[0, 0, 255, 0, 255, 0, 0, 0]); // red, green
        dib.extend_from_slice(&[255, 0, 0, 9, 8, 7, 0, 0]); // blue, (7,8,9)
        let decoded = decode_dib(&dib).expect("decodes");
        // The second stored row is the top row of the image.
        assert_eq!(decoded.get_pixel(0, 0).0, [0, 0, 255, 255]);
        assert_eq!(decoded.get_pixel(1, 0).0, [7, 8, 9, 255]);
        assert_eq!(decoded.get_pixel(0, 1).0, [255, 0, 0, 255]);
        assert_eq!(decoded.get_pixel(1, 1).0, [0, 255, 0, 255]);
    }

    #[test]
    fn bi_rgb_32bpp_without_alpha_is_opaque() {
        let mut dib = Vec::new();
        dib.extend_from_slice(&40u32.to_le_bytes());
        dib.extend_from_slice(&1i32.to_le_bytes());
        dib.extend_from_slice(&1i32.to_le_bytes());
        dib.extend_from_slice(&1u16.to_le_bytes());
        dib.extend_from_slice(&32u16.to_le_bytes());
        dib.extend_from_slice(&[0u8; 24]);
        dib.extend_from_slice(&[10, 20, 30, 0]);
        let decoded = decode_dib(&dib).expect("decodes");
        assert_eq!(decoded.get_pixel(0, 0).0, [30, 20, 10, 255]);
    }

    #[test]
    fn rejects_paletted_or_truncated() {
        let mut dib = Vec::new();
        dib.extend_from_slice(&40u32.to_le_bytes());
        dib.extend_from_slice(&1i32.to_le_bytes());
        dib.extend_from_slice(&1i32.to_le_bytes());
        dib.extend_from_slice(&1u16.to_le_bytes());
        dib.extend_from_slice(&8u16.to_le_bytes());
        dib.extend_from_slice(&[0u8; 24]);
        assert!(decode_dib(&dib).is_none());
        let truncated = &encode_dibv5(&sample())[..130];
        assert!(decode_dib(truncated).is_none());
    }
}
