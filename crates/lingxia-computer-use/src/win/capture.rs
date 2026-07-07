//! Screen/window capture and pixel probe via GDI. Window capture uses
//! `PrintWindow(PW_RENDERFULLCONTENT)` so it works even when the window is
//! occluded; screen/display/region capture uses BitBlt from the screen DC.

use super::parse_hwnd;
use crate::error::{Error, Result};
use crate::model::{Capture, CaptureTarget, Pixel};
use windows::Win32::Foundation::{COLORREF, RECT};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC,
    DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, GetPixel, HBITMAP, HDC, ReleaseDC,
    SRCCOPY, SelectObject,
};
use windows::Win32::Storage::Xps::{PRINT_WINDOW_FLAGS, PrintWindow};
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, GetWindowRect, PW_RENDERFULLCONTENT, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
    SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};

pub fn pixel(x: i32, y: i32) -> Result<Pixel> {
    super::ensure_dpi_aware();
    unsafe {
        let hdc = GetDC(None);
        if hdc.is_invalid() {
            return Err(Error::Failed("could not get screen device context".into()));
        }
        let c: COLORREF = GetPixel(hdc, x, y);
        ReleaseDC(None, hdc);
        if c.0 == u32::MAX {
            return Err(Error::NotFound(format!("no pixel at {x},{y}")));
        }
        let r = (c.0 & 0xFF) as u8;
        let g = ((c.0 >> 8) & 0xFF) as u8;
        let b = ((c.0 >> 16) & 0xFF) as u8;
        Ok(Pixel {
            x,
            y,
            hex: format!("{r:02x}{g:02x}{b:02x}"),
            r,
            g,
            b,
        })
    }
}

/// Poll a pixel until it matches `hex` within `tolerance` per channel, or time
/// out (exit 5).
pub fn wait_pixel(x: i32, y: i32, hex: &str, tolerance: u8, timeout_ms: u64) -> Result<Pixel> {
    let want = parse_hex(hex)?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
    loop {
        if let Ok(p) = pixel(x, y) {
            let close = |a: u8, b: u8| a.abs_diff(b) <= tolerance;
            if close(p.r, want.0) && close(p.g, want.1) && close(p.b, want.2) {
                return Ok(p);
            }
        }
        if std::time::Instant::now() >= deadline {
            return Err(Error::Timeout(format!("timed out waiting for pixel {hex}")));
        }
        std::thread::sleep(std::time::Duration::from_millis(120));
    }
}

fn parse_hex(hex: &str) -> Result<(u8, u8, u8)> {
    let h = hex.trim_start_matches('#');
    if h.len() != 6 {
        return Err(Error::Usage(format!("color must be rrggbb, got '{hex}'")));
    }
    let byte = |i: usize| {
        u8::from_str_radix(&h[i..i + 2], 16)
            .map_err(|_| Error::Usage(format!("invalid color '{hex}'")))
    };
    Ok((byte(0)?, byte(2)?, byte(4)?))
}

pub fn screenshot(target: CaptureTarget) -> Result<Capture> {
    super::ensure_dpi_aware();
    match target {
        CaptureTarget::Window(id) => capture_window(&id),
        CaptureTarget::Screen => {
            let (x, y, w, h) = virtual_screen();
            capture_screen_rect(x, y, w, h)
        }
        CaptureTarget::Region { x, y, w, h } => {
            if w <= 0 || h <= 0 {
                return Err(Error::Usage("region width/height must be positive".into()));
            }
            if w > 32767 || h > 32767 {
                return Err(Error::Usage("region is unreasonably large".into()));
            }
            capture_screen_rect(x, y, w, h)
        }
        CaptureTarget::Display(n) => {
            let displays = super::displays()?;
            let d = displays
                .get(n.wrapping_sub(1))
                .ok_or_else(|| Error::NotFound(format!("no display {n}")))?;
            capture_screen_rect(d.bounds.x, d.bounds.y, d.bounds.w, d.bounds.h)
        }
    }
}

fn virtual_screen() -> (i32, i32, i32, i32) {
    unsafe {
        (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    }
}

fn capture_screen_rect(x: i32, y: i32, w: i32, h: i32) -> Result<Capture> {
    unsafe {
        let screen = GetDC(None);
        if screen.is_invalid() {
            return Err(Error::Failed("could not get screen device context".into()));
        }
        let result = blit_and_encode(screen, x, y, w, h);
        ReleaseDC(None, screen);
        let png = result?;
        Ok(Capture {
            width: w as u32,
            height: h as u32,
            png,
            occlusion_independent: false,
            backend: "gdi_bitblt".into(),
        })
    }
}

fn capture_window(id: &str) -> Result<Capture> {
    let hwnd = parse_hwnd(id)?;
    // Prefer WGC: it captures the real composited output, so GPU/hardware
    // surfaces (WebView2, Direct3D apps) that PrintWindow renders black come
    // through correctly. Fall back to PrintWindow if WGC is unavailable.
    match super::wgc::capture_window(hwnd) {
        Ok(rgba) => match rgba_to_png(rgba.width, rgba.height, rgba.pixels) {
            Ok(png) => {
                return Ok(Capture {
                    width: rgba.width,
                    height: rgba.height,
                    png,
                    occlusion_independent: true,
                    backend: "wgc".into(),
                });
            }
            Err(e) => log::debug!("wgc png encode failed, falling back to PrintWindow: {e}"),
        },
        Err(e) => log::debug!("wgc capture failed, falling back to PrintWindow: {e}"),
    }
    capture_window_printwindow(id, hwnd)
}

fn capture_window_printwindow(id: &str, hwnd: windows::Win32::Foundation::HWND) -> Result<Capture> {
    unsafe {
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return Err(Error::Stale(format!("window {id} is not available")));
        }
        let w = rect.right - rect.left;
        let h = rect.bottom - rect.top;
        if w <= 0 || h <= 0 {
            return Err(Error::Failed(format!("window {id} has zero size")));
        }

        let screen = GetDC(None);
        let memdc = CreateCompatibleDC(Some(screen));
        let bmp = CreateCompatibleBitmap(screen, w, h);
        let old = SelectObject(memdc, bmp.into());

        let ok = PrintWindow(hwnd, memdc, PRINT_WINDOW_FLAGS(PW_RENDERFULLCONTENT)).as_bool();

        let png = if ok {
            dib_to_png(memdc, bmp, w, h)
        } else {
            Err(Error::Failed(format!("PrintWindow failed for {id}")))
        };

        SelectObject(memdc, old);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(memdc);
        ReleaseDC(None, screen);

        Ok(Capture {
            width: w as u32,
            height: h as u32,
            png: png?,
            occlusion_independent: true,
            backend: "print_window".into(),
        })
    }
}

/// BitBlt a screen rect into a fresh bitmap and PNG-encode it.
unsafe fn blit_and_encode(screen: HDC, x: i32, y: i32, w: i32, h: i32) -> Result<Vec<u8>> {
    unsafe {
        let memdc = CreateCompatibleDC(Some(screen));
        let bmp = CreateCompatibleBitmap(screen, w, h);
        let old = SelectObject(memdc, bmp.into());
        let blt = BitBlt(memdc, 0, 0, w, h, Some(screen), x, y, SRCCOPY);
        let png = if blt.is_ok() {
            dib_to_png(memdc, bmp, w, h)
        } else {
            Err(Error::Failed("BitBlt failed".into()))
        };
        SelectObject(memdc, old);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(memdc);
        png
    }
}

/// Pull 32-bpp top-down BGRA pixels out of a bitmap and encode PNG.
unsafe fn dib_to_png(memdc: HDC, bmp: HBITMAP, w: i32, h: i32) -> Result<Vec<u8>> {
    unsafe {
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                // Negative height => top-down rows.
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut buf = vec![0u8; w as usize * h as usize * 4];
        let lines = GetDIBits(
            memdc,
            bmp,
            0,
            h as u32,
            Some(buf.as_mut_ptr() as *mut _),
            &mut info,
            DIB_RGB_COLORS,
        );
        if lines == 0 {
            return Err(Error::Failed("GetDIBits returned no scanlines".into()));
        }
        // BGRA -> RGBA, force opaque alpha (screen captures have undefined A).
        for px in buf.chunks_exact_mut(4) {
            px.swap(0, 2);
            px[3] = 255;
        }
        rgba_to_png(w as u32, h as u32, buf)
    }
}

/// Encode top-down RGBA pixels to PNG bytes.
fn rgba_to_png(w: u32, h: u32, buf: Vec<u8>) -> Result<Vec<u8>> {
    let img = image::RgbaImage::from_raw(w, h, buf)
        .ok_or_else(|| Error::Failed("bitmap buffer size mismatch".into()))?;
    let mut png = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| Error::Failed(format!("PNG encode failed: {e}")))?;
    Ok(png)
}
