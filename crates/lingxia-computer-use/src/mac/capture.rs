// The Quartz capture entry points (CGDisplayCreateImage, CGWindowListCreateImage)
// are deprecated in favor of the async ScreenCaptureKit; they remain the right
// tool for a synchronous dev-time screenshot and still work with the Screen
// Recording permission, so we opt out of the deprecation lint here.
#![allow(deprecated)]

//! Screen/window/region capture and pixel probe via Quartz. Screen and region
//! captures composite the on-screen content (`CGWindowListCreateImage`);
//! per-window capture composites the target window's own backing store, so it
//! survives occlusion. Every capture is decoded into top-down RGBA by redrawing
//! the `CGImage` into a bitmap context we own, then PNG-encoded.

use super::{displays, parse_window_id};
use crate::error::{Error, Result};
use crate::model::{Capture, CaptureTarget, Pixel};
use objc2_core_foundation::CGRect;
use objc2_core_graphics::{
    CGColorSpace, CGContext, CGDisplayCreateImage, CGImage, CGImageAlphaInfo,
    CGPreflightScreenCaptureAccess, CGWindowImageOption, CGWindowListCreateImage,
    CGWindowListOption,
};
use std::ffi::c_void;

unsafe extern "C-unwind" {
    fn CGBitmapContextCreate(
        data: *mut c_void,
        width: usize,
        height: usize,
        bits_per_component: usize,
        bytes_per_row: usize,
        space: *const CGColorSpace,
        bitmap_info: u32,
    ) -> *mut CGContext;
    static CGRectNull: CGRect;
}

/// Turn a screen-recording failure into the right error: a `None` image with the
/// permission absent is a permission problem, not a generic failure.
fn capture_error(what: &str) -> Error {
    if CGPreflightScreenCaptureAccess() {
        Error::Failed(format!("could not capture {what}"))
    } else {
        Error::Permission(format!(
            "screen capture denied: grant Screen Recording to this terminal in System Settings › Privacy & Security (needed to capture {what})"
        ))
    }
}

pub fn screenshot(target: CaptureTarget) -> Result<Capture> {
    match target {
        CaptureTarget::Window(id) => {
            let wid = parse_window_id(&id)?;
            let img = unsafe {
                CGWindowListCreateImage(
                    CGRectNull,
                    CGWindowListOption::OptionIncludingWindow,
                    wid,
                    CGWindowImageOption::BoundsIgnoreFraming,
                )
            }
            .ok_or_else(|| capture_error(&format!("window {id}")))?;
            let (width, height, png) = encode(&img)?;
            Ok(Capture {
                width,
                height,
                png,
                occlusion_independent: true,
                backend: "cgwindow".into(),
            })
        }
        CaptureTarget::Display(n) => {
            let ds = displays()?;
            let d = ds
                .get(n.wrapping_sub(1))
                .ok_or_else(|| Error::NotFound(format!("no display {n}")))?;
            // Re-derive the CGDirectDisplayID by index the same way displays() did.
            let id = display_id_at(n.wrapping_sub(1))?;
            let img = CGDisplayCreateImage(id)
                .ok_or_else(|| capture_error(&format!("display {}", d.id)))?;
            let (width, height, png) = encode(&img)?;
            Ok(Capture {
                width,
                height,
                png,
                occlusion_independent: false,
                backend: "cgdisplay".into(),
            })
        }
        CaptureTarget::Screen => {
            let bounds = virtual_bounds()?;
            capture_screen_rect(bounds)
        }
        CaptureTarget::Region { x, y, w, h } => {
            if w <= 0 || h <= 0 {
                return Err(Error::Usage("region width/height must be positive".into()));
            }
            capture_screen_rect(CGRect::new(
                objc2_core_foundation::CGPoint::new(x as f64, y as f64),
                objc2_core_foundation::CGSize::new(w as f64, h as f64),
            ))
        }
    }
}

fn capture_screen_rect(rect: CGRect) -> Result<Capture> {
    let img = CGWindowListCreateImage(
        rect,
        CGWindowListOption::OptionOnScreenOnly,
        0,
        CGWindowImageOption::Default,
    )
    .ok_or_else(|| capture_error("screen"))?;
    let (width, height, png) = encode(&img)?;
    Ok(Capture {
        width,
        height,
        png,
        occlusion_independent: false,
        backend: "cgwindow".into(),
    })
}

/// The bounding rect (points) that covers every active display.
fn virtual_bounds() -> Result<CGRect> {
    let ds = displays()?;
    let first = ds
        .first()
        .ok_or_else(|| Error::Unavailable("no active displays".into()))?;
    let (mut minx, mut miny) = (first.bounds.x, first.bounds.y);
    let (mut maxx, mut maxy) = (
        first.bounds.x + first.bounds.w,
        first.bounds.y + first.bounds.h,
    );
    for d in &ds[1..] {
        minx = minx.min(d.bounds.x);
        miny = miny.min(d.bounds.y);
        maxx = maxx.max(d.bounds.x + d.bounds.w);
        maxy = maxy.max(d.bounds.y + d.bounds.h);
    }
    Ok(CGRect::new(
        objc2_core_foundation::CGPoint::new(minx as f64, miny as f64),
        objc2_core_foundation::CGSize::new((maxx - minx) as f64, (maxy - miny) as f64),
    ))
}

/// Resolve the Nth (0-based) active display's `CGDirectDisplayID`.
fn display_id_at(index: usize) -> Result<objc2_core_graphics::CGDirectDisplayID> {
    let mut ids = [0 as objc2_core_graphics::CGDirectDisplayID; 16];
    let mut count: u32 = 0;
    let err = unsafe {
        objc2_core_graphics::CGGetActiveDisplayList(ids.len() as u32, ids.as_mut_ptr(), &mut count)
    };
    if err.0 != 0 {
        return Err(Error::Unavailable("CGGetActiveDisplayList failed".into()));
    }
    ids.get(index)
        .filter(|_| index < count as usize)
        .copied()
        .ok_or_else(|| Error::NotFound(format!("no display index {index}")))
}

pub fn pixel(x: i32, y: i32) -> Result<Pixel> {
    let rect = CGRect::new(
        objc2_core_foundation::CGPoint::new(x as f64, y as f64),
        objc2_core_foundation::CGSize::new(1.0, 1.0),
    );
    let img = CGWindowListCreateImage(
        rect,
        CGWindowListOption::OptionOnScreenOnly,
        0,
        CGWindowImageOption::Default,
    )
    .ok_or_else(|| capture_error(&format!("pixel at {x},{y}")))?;
    let (_, _, rgba) = image_to_rgba(&img)?;
    let (r, g, b) = (rgba[0], rgba[1], rgba[2]);
    Ok(Pixel {
        x,
        y,
        hex: format!("{r:02x}{g:02x}{b:02x}"),
        r,
        g,
        b,
    })
}

/// Poll a pixel until it matches `hex` within `tolerance` per channel, or time
/// out (exit 5).
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

fn encode(img: &CGImage) -> Result<(u32, u32, Vec<u8>)> {
    let (w, h, rgba) = image_to_rgba(img)?;
    let image = image::RgbaImage::from_raw(w, h, rgba)
        .ok_or_else(|| Error::Failed("bitmap buffer size mismatch".into()))?;
    let mut png = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| Error::Failed(format!("PNG encode failed: {e}")))?;
    Ok((w, h, png))
}

/// Redraw a `CGImage` into a device-RGB bitmap we own and read it back top-down.
fn image_to_rgba(img: &CGImage) -> Result<(u32, u32, Vec<u8>)> {
    let w = CGImage::width(Some(img));
    let h = CGImage::height(Some(img));
    if w == 0 || h == 0 {
        return Err(Error::Failed("captured image has zero size".into()));
    }
    let space = CGColorSpace::new_device_rgb()
        .ok_or_else(|| Error::Failed("could not create RGB color space".into()))?;
    let bytes_per_row = w * 4;
    let mut buf = vec![0u8; bytes_per_row * h];
    let ctx = unsafe {
        CGBitmapContextCreate(
            buf.as_mut_ptr() as *mut c_void,
            w,
            h,
            8,
            bytes_per_row,
            &*space,
            CGImageAlphaInfo::PremultipliedLast.0,
        )
    };
    if ctx.is_null() {
        return Err(Error::Failed("could not create bitmap context".into()));
    }
    // Take ownership so the context is released when this scope ends.
    let ctx = unsafe {
        objc2_core_foundation::CFRetained::from_raw(std::ptr::NonNull::new_unchecked(ctx))
    };
    let rect = CGRect::new(
        objc2_core_foundation::CGPoint::new(0.0, 0.0),
        objc2_core_foundation::CGSize::new(w as f64, h as f64),
    );
    CGContext::draw_image(Some(&ctx), rect, Some(img));

    // A CGBitmapContext stores its rows top-down in memory, so the buffer is
    // already in PNG row order. Just force alpha opaque (screen captures leave
    // it undefined).
    for px in buf.chunks_exact_mut(4) {
        px[3] = 255;
    }
    Ok((w as u32, h as u32, buf))
}
