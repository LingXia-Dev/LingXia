//! PNG icon cache for Windows shell chrome drawing.

use std::collections::HashMap;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use windows::Win32::Graphics::Gdi::{CreateBitmap, DeleteObject, HGDIOBJ};
use windows::Win32::UI::WindowsAndMessaging::{self, HICON, ICONINFO};
use windows::core::BOOL;

type IconCacheKey = (PathBuf, u32);
/// (mtime, len) of the source file when the handle was created. Favicon
/// cache files are rewritten in place on refresh, so a path-only key would
/// serve the old icon (or a memoized failure) forever.
type IconStamp = (u64, u64);
type IconHandleCache = HashMap<IconCacheKey, (IconStamp, Option<isize>)>;
type BytesIconCache = HashMap<(String, u32), (u64, Option<isize>)>;

static PANEL_ICON_HANDLES: OnceLock<Mutex<IconHandleCache>> = OnceLock::new();
static BYTES_ICON_HANDLES: OnceLock<Mutex<BytesIconCache>> = OnceLock::new();

pub(super) fn cached_png_icon_handle(path: &str, size: u32) -> Option<isize> {
    let path = PathBuf::from(path);
    // A missing file (e.g. mid-replace during a favicon refresh) is never
    // memoized — the next repaint retries.
    let stamp = file_stamp(&path)?;
    let key = (path.clone(), size);
    let handles = PANEL_ICON_HANDLES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut handles) = handles.lock() {
        if let Some((cached_stamp, handle)) = handles.get(&key)
            && *cached_stamp == stamp
        {
            return *handle;
        }
        let handle = create_icon_from_path(&path, size).ok();
        if let Some((_, Some(previous))) = handles.insert(key, (stamp, handle))
            && Some(previous) != handle
        {
            destroy_icon_handle(previous);
        }
        return handle;
    }
    create_icon_from_path(&path, size).ok()
}

fn file_stamp(path: &Path) -> Option<IconStamp> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?;
    Some((modified.as_nanos() as u64, meta.len()))
}

pub(super) fn cached_png_bytes_icon_handle(
    cache_key: &str,
    png: &[u8],
    size: u32,
) -> Option<isize> {
    let hash = fnv1a64(png);
    let key = (cache_key.to_string(), size);
    let handles = BYTES_ICON_HANDLES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut handles) = handles.lock() {
        if let Some((cached_hash, handle)) = handles.get(&key)
            && *cached_hash == hash
        {
            return *handle;
        }
        let handle = create_icon_from_png_bytes(png, size).ok();
        if let Some((_, Some(previous))) = handles.insert(key, (hash, handle))
            && Some(previous) != handle
        {
            destroy_icon_handle(previous);
        }
        return handle;
    }
    create_icon_from_png_bytes(png, size).ok()
}

fn create_icon_from_path(path: &Path, size: u32) -> Result<isize, String> {
    let is_svg = path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"));
    if is_svg {
        let data = std::fs::read(path)
            .map_err(|err| format!("Failed to read SVG icon {}: {err}", path.display()))?;
        let image = rasterize_svg(&data, size)?;
        return create_icon_from_image(
            image::DynamicImage::ImageRgba8(image),
            size,
            &path.display().to_string(),
            false,
        );
    }
    let image = image::open(path).map_err(|err| {
        format!(
            "Failed to load Windows shell icon {}: {err}",
            path.display()
        )
    })?;
    // Raster tiles get an anti-aliased 22% corner in the bitmap. GDI
    // `CreateRoundRectRgn` clip aliases that arc into a staircase at 16px.
    create_icon_from_image(image, size, &path.display().to_string(), true)
}

/// Rasterizes an SVG (icons may be SVG, as on macOS) to an `size`x`size`
/// straight-alpha RGBA image, fit to the square and centered.
fn rasterize_svg(svg: &[u8], size: u32) -> Result<image::RgbaImage, String> {
    let svg = std::str::from_utf8(svg).map_err(|err| format!("SVG icon is not UTF-8: {err}"))?;
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default())
        .map_err(|err| format!("Failed to parse SVG icon: {err}"))?;
    let svg_size = tree.size();
    let max_side = svg_size.width().max(svg_size.height());
    if max_side <= 0.0 {
        return Err("SVG icon has an empty viewport".to_string());
    }
    let scale = size as f32 / max_side;
    let offset_x = (size as f32 - svg_size.width() * scale) / 2.0;
    let offset_y = (size as f32 - svg_size.height() * scale) / 2.0;
    let mut pixmap = tiny_skia::Pixmap::new(size, size)
        .ok_or_else(|| "Failed to allocate SVG pixmap".to_string())?;
    let transform = tiny_skia::Transform::from_row(scale, 0.0, 0.0, scale, offset_x, offset_y);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    let mut image = image::RgbaImage::new(size, size);
    for (pixel, out) in pixmap.pixels().iter().zip(image.pixels_mut()) {
        let color = pixel.demultiply();
        *out = image::Rgba([color.red(), color.green(), color.blue(), color.alpha()]);
    }
    Ok(image)
}

fn create_icon_from_png_bytes(png: &[u8], size: u32) -> Result<isize, String> {
    let image = image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .map_err(|err| format!("Failed to decode PNG icon bytes: {err}"))?;
    create_icon_from_image(image, size, "<png bytes>", false)
}

/// Same ratio as macOS `appTileCornerRatio` / the old GDI clip.
const APP_TILE_CORNER_RATIO: f32 = 0.22;

/// Soft-masks a square tile to a rounded rect. Coverage uses a 1px SDF so
/// the corner blends instead of a hard GDI region edge.
fn apply_app_tile_corners(image: &mut image::RgbaImage) {
    let width = image.width();
    let height = image.height();
    if width == 0 || height == 0 {
        return;
    }
    let half_w = width as f32 * 0.5;
    let half_h = height as f32 * 0.5;
    let radius = (width.min(height) as f32 * APP_TILE_CORNER_RATIO).max(1.0);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        let px = x as f32 + 0.5 - half_w;
        let py = y as f32 + 0.5 - half_h;
        let qx = px.abs() - half_w + radius;
        let qy = py.abs() - half_h + radius;
        let outside = qx.max(0.0).hypot(qy.max(0.0));
        let inside = qx.max(qy).min(0.0);
        let coverage = (0.5 - (outside + inside - radius)).clamp(0.0, 1.0);
        if coverage <= 0.0 {
            *pixel = image::Rgba([0, 0, 0, 0]);
            continue;
        }
        if coverage < 1.0 {
            pixel.0[3] = (f32::from(pixel.0[3]) * coverage).round() as u8;
        }
    }
}

fn create_icon_from_image(
    image: image::DynamicImage,
    size: u32,
    source: &str,
    tile_corners: bool,
) -> Result<isize, String> {
    let mut image = image
        .resize_exact(size, size, image::imageops::FilterType::Lanczos3)
        .into_rgba8();
    if tile_corners {
        apply_app_tile_corners(&mut image);
    }

    let mut bgra = Vec::with_capacity(image.len());
    for pixel in image.pixels() {
        let [r, g, b, a] = pixel.0;
        bgra.extend_from_slice(&[b, g, r, a]);
    }

    unsafe {
        let width = size as i32;
        let height = size as i32;
        let color = CreateBitmap(width, height, 1, 32, Some(bgra.as_ptr().cast()));
        if color.is_invalid() {
            return Err(format!(
                "Failed to create Windows shell icon color bitmap from {source}"
            ));
        }

        let mask = CreateBitmap(width, height, 1, 1, None);
        if mask.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(color.0));
            return Err(format!(
                "Failed to create Windows shell icon mask bitmap from {source}"
            ));
        }

        let info = ICONINFO {
            fIcon: BOOL(1),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color,
        };
        let icon = WindowsAndMessaging::CreateIconIndirect(&info)
            .map_err(|err| format!("Failed to create Windows shell icon from {source}: {err}"))?;
        let _ = DeleteObject(HGDIOBJ(color.0));
        let _ = DeleteObject(HGDIOBJ(mask.0));
        Ok(icon.0 as isize)
    }
}

fn destroy_icon_handle(handle: isize) {
    if handle != 0 {
        unsafe {
            let _ = WindowsAndMessaging::DestroyIcon(HICON(handle as *mut c_void));
        }
    }
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_tile_corners_are_transparent_and_center_stays_opaque() {
        let mut image = image::RgbaImage::from_pixel(16, 16, image::Rgba([20, 40, 80, 255]));
        apply_app_tile_corners(&mut image);
        assert_eq!(image.get_pixel(0, 0).0[3], 0);
        assert_eq!(image.get_pixel(15, 0).0[3], 0);
        assert_eq!(image.get_pixel(0, 15).0[3], 0);
        assert_eq!(image.get_pixel(15, 15).0[3], 0);
        assert_eq!(image.get_pixel(8, 8).0[3], 255);
        let fringe = image.get_pixel(1, 0).0[3];
        assert!(
            fringe > 0 && fringe < 255,
            "corner fringe should be anti-aliased, got {fringe}"
        );
    }
}
