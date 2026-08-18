//! Screen/window capture and pixel probe via GDI. Window capture uses
//! `PrintWindow(PW_RENDERFULLCONTENT)` so it works even when the window is
//! occluded; screen/display/region capture uses BitBlt from the screen DC.

use super::parse_hwnd;
use crate::error::{Error, Result};
use crate::model::CaptureTarget;
#[cfg(feature = "snapshot")]
use crate::model::{Capture, Pixel};
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

#[cfg(feature = "snapshot")]
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
#[cfg(feature = "snapshot")]
pub fn wait_pixel(x: i32, y: i32, hex: &str, tolerance: u8, timeout_ms: u64) -> Result<Pixel> {
    let want = parse_hex(hex)?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
    loop {
        let p = pixel(x, y)?;
        let close = |a: u8, b: u8| a.abs_diff(b) <= tolerance;
        if close(p.r, want.0) && close(p.g, want.1) && close(p.b, want.2) {
            return Ok(p);
        }
        if std::time::Instant::now() >= deadline {
            return Err(Error::Timeout(format!("timed out waiting for pixel {hex}")));
        }
        std::thread::sleep(std::time::Duration::from_millis(120));
    }
}

#[cfg(feature = "snapshot")]
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

pub(crate) fn capture_frame(target: &CaptureTarget) -> Result<crate::engine::EngineFrame> {
    super::ensure_dpi_aware();
    match target {
        CaptureTarget::Window(id) => capture_window_frame(id),
        CaptureTarget::Screen => {
            let (x, y, w, h) = virtual_screen();
            capture_screen_frame(x, y, w, h)
        }
        CaptureTarget::Region { x, y, w, h } => {
            if *w <= 0 || *h <= 0 {
                return Err(Error::Usage("region width/height must be positive".into()));
            }
            if *w > 32767 || *h > 32767 {
                return Err(Error::Usage("region is unreasonably large".into()));
            }
            capture_screen_frame(*x, *y, *w, *h)
        }
        CaptureTarget::Display(n) => {
            let displays = super::displays()?;
            let d = displays
                .get(n.wrapping_sub(1))
                .ok_or_else(|| Error::NotFound(format!("no display {n}")))?;
            capture_screen_frame(d.bounds.x, d.bounds.y, d.bounds.w, d.bounds.h)
        }
    }
}

#[cfg(feature = "snapshot")]
pub fn screenshot(target: CaptureTarget) -> Result<Capture> {
    let frame = crate::engine::capture_frame(&target)?;
    Ok(Capture {
        width: frame.width,
        height: frame.height,
        png: crate::engine::encode_png(frame.width, frame.height, frame.rgba)?,
        occlusion_independent: frame.occlusion_independent,
        backend: frame.backend.into(),
    })
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

fn capture_screen_frame(x: i32, y: i32, w: i32, h: i32) -> Result<crate::engine::EngineFrame> {
    unsafe {
        let screen = GetDC(None);
        if screen.is_invalid() {
            return Err(Error::Failed("could not get screen device context".into()));
        }
        let result = blit_rgba(screen, x, y, w, h);
        ReleaseDC(None, screen);
        Ok(crate::engine::EngineFrame {
            width: w as u32,
            height: h as u32,
            rgba: result?,
            source: crate::model::Rect { x, y, w, h },
            scale: 1.0,
            backend: "gdi_bitblt",
            occlusion_independent: false,
        })
    }
}

fn capture_window_frame(id: &str) -> Result<crate::engine::EngineFrame> {
    let hwnd = parse_hwnd(id)?;
    match super::wgc::capture_window(hwnd) {
        Ok(rgba) => {
            let source = window_source(hwnd, id)?;
            return Ok(crate::engine::EngineFrame {
                width: rgba.width,
                height: rgba.height,
                rgba: rgba.pixels,
                source,
                scale: 1.0,
                backend: "wgc",
                occlusion_independent: true,
            });
        }
        Err(e) => log::debug!("wgc capture failed, falling back to PrintWindow: {e}"),
    }
    capture_window_printwindow(id, hwnd)
}

fn window_source(hwnd: windows::Win32::Foundation::HWND, id: &str) -> Result<crate::model::Rect> {
    unsafe {
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return Err(Error::Stale(format!("window {id} is not available")));
        }
        Ok(crate::model::Rect {
            x: rect.left,
            y: rect.top,
            w: rect.right - rect.left,
            h: rect.bottom - rect.top,
        })
    }
}

fn capture_window_printwindow(
    id: &str,
    hwnd: windows::Win32::Foundation::HWND,
) -> Result<crate::engine::EngineFrame> {
    unsafe {
        let source = window_source(hwnd, id)?;
        if source.w <= 0 || source.h <= 0 {
            return Err(Error::Failed(format!("window {id} has zero size")));
        }

        let screen = GetDC(None);
        let memdc = CreateCompatibleDC(Some(screen));
        let bmp = CreateCompatibleBitmap(screen, source.w, source.h);
        let old = SelectObject(memdc, bmp.into());

        let ok = PrintWindow(hwnd, memdc, PRINT_WINDOW_FLAGS(PW_RENDERFULLCONTENT)).as_bool();
        let rgba = if ok {
            dib_to_rgba(memdc, bmp, source.w, source.h)
        } else {
            Err(Error::Failed(format!("PrintWindow failed for {id}")))
        };

        SelectObject(memdc, old);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(memdc);
        ReleaseDC(None, screen);

        Ok(crate::engine::EngineFrame {
            width: source.w as u32,
            height: source.h as u32,
            rgba: rgba?,
            source,
            scale: 1.0,
            backend: "print_window",
            occlusion_independent: true,
        })
    }
}

unsafe fn blit_rgba(screen: HDC, x: i32, y: i32, w: i32, h: i32) -> Result<Vec<u8>> {
    unsafe {
        let memdc = CreateCompatibleDC(Some(screen));
        let bmp = CreateCompatibleBitmap(screen, w, h);
        let old = SelectObject(memdc, bmp.into());
        let blt = BitBlt(memdc, 0, 0, w, h, Some(screen), x, y, SRCCOPY);
        let rgba = if blt.is_ok() {
            dib_to_rgba(memdc, bmp, w, h)
        } else {
            Err(Error::Failed("BitBlt failed".into()))
        };
        SelectObject(memdc, old);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(memdc);
        rgba
    }
}

unsafe fn dib_to_rgba(memdc: HDC, bmp: HBITMAP, w: i32, h: i32) -> Result<Vec<u8>> {
    unsafe {
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
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
        for px in buf.chunks_exact_mut(4) {
            px.swap(0, 2);
            px[3] = 255;
        }
        Ok(buf)
    }
}
