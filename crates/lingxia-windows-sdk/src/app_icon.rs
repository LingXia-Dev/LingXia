//! Windows app icon ownership.
//!
//! The host SDK decides which icon represents the process. `lingxia-webview`
//! only exposes a host-window-created hook so this crate can apply the icon
//! to WebView host HWNDs as they appear.

use std::ffi::c_void;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Gdi::{CreateBitmap, DeleteObject, HGDIOBJ};
use windows::Win32::UI::WindowsAndMessaging::{
    self, GCLP_HICON, GCLP_HICONSM, HICON, ICON_BIG, ICON_SMALL, ICONINFO, WM_SETICON,
};
use windows::core::BOOL;

use lingxia_windows_contract::add_host_window_created_handler;

#[derive(Debug, Clone, Copy)]
struct AppIconHandles {
    small: isize,
    large: isize,
}

static APP_ICON_HANDLES: OnceLock<Mutex<Option<AppIconHandles>>> = OnceLock::new();
static APP_ICON_PATH: OnceLock<Mutex<Option<std::path::PathBuf>>> = OnceLock::new();
static APP_CHROME_ICON_PATH: OnceLock<Mutex<Option<std::path::PathBuf>>> = OnceLock::new();
static ICON_HOOK_INSTALLED: OnceLock<()> = OnceLock::new();

pub(crate) fn set_app_icon_from_path(path: &Path) -> Result<(), String> {
    install_icon_hook();
    // Decode + normalize once, then rasterize to each size. The large icon is
    // rendered at 48px (not 32px) so the taskbar/alt-tab downscale it crisply on
    // high-DPI displays instead of upscaling a 32px icon.
    let image = prepare_app_icon_image(path)?;
    let handles = AppIconHandles {
        small: create_icon_from_image(&image, 16, path)?,
        large: create_icon_from_image(&image, 48, path)?,
    };
    let icon_state = APP_ICON_HANDLES.get_or_init(|| Mutex::new(None));
    let mut icon_state = icon_state
        .lock()
        .map_err(|_| "Windows app icon state is poisoned".to_string())?;
    if let Some(old) = icon_state.replace(handles) {
        destroy_icon_handle(old.small);
        destroy_icon_handle(old.large);
    }
    // Remember the source PNG (the resolved product/launcher icon) so the
    // shell can render it in the top-bar app-menu button and the About box.
    if let Ok(mut slot) = APP_ICON_PATH.get_or_init(|| Mutex::new(None)).lock() {
        *slot = Some(path.to_path_buf());
    }
    if let Some(chrome) = resolve_host_chrome_icon(path) {
        if let Ok(mut slot) = APP_CHROME_ICON_PATH.get_or_init(|| Mutex::new(None)).lock() {
            *slot = Some(chrome);
        }
    }
    Ok(())
}

/// Full-bleed chrome tile (`icons/host-chrome.png`) when the CLI staged one
/// next to the launcher icon. Sidebar / Settings use this so they are not
/// Dock-inset.
pub(crate) fn current_chrome_icon_path() -> Option<std::path::PathBuf> {
    APP_CHROME_ICON_PATH
        .get()
        .and_then(|path| path.lock().ok())
        .and_then(|path| path.clone())
}

/// Dist keeps the tile beside the launcher icon; `lingxia dev` launches a
/// badged copy from `windows/overlay/<env>/`, whose tile is in `windows/assets/`.
fn resolve_host_chrome_icon(app_icon: &Path) -> Option<std::path::PathBuf> {
    app_icon.ancestors().take(6).find_map(|ancestor| {
        [
            ancestor.join("icons"),
            ancestor.join("assets").join("icons"),
        ]
        .into_iter()
        .map(|dir| dir.join("host-chrome.png"))
        .find(|candidate| candidate.is_file())
    })
}

/// The source PNG path of the applied product/app icon (the launcher icon
/// resolved at startup), if one was set. This is the application's icon, not
/// any single lxapp's icon. Chrome uses it for the About/Exit entry, the
/// About box, the tray icon, and built-in pages that have no favicon of
/// their own.
pub(crate) fn current_app_icon_path() -> Option<std::path::PathBuf> {
    APP_ICON_PATH
        .get()
        .and_then(|path| path.lock().ok())
        .and_then(|path| path.clone())
}

/// Creates a fresh `HICON` (as a raw handle) from a PNG file at `size`px, for
/// callers that need an owned icon to pass to Win32 dialogs (e.g. the shell's
/// About box). The caller owns the handle and must `DestroyIcon` it. Returns
/// `None` when the file cannot be decoded.
pub(crate) fn create_icon_handle_from_path(path: &Path, size: u32) -> Option<isize> {
    let image = prepare_app_icon_image(path).ok()?;
    create_icon_from_image(&image, size, path).ok()
}

/// Tray glyphs are often black-on-transparent (macOS template style). Windows
/// does not invert those, so a dark notification area would hide the icon.
pub(crate) fn create_tray_icon_handle_from_path(path: &Path, size: u32) -> Option<isize> {
    let mut image = prepare_app_icon_image(path).ok()?;
    invert_dark_tray_glyph(&mut image);
    create_icon_from_image(&image, size, path).ok()
}

/// The process's current large (32px) app-icon handle, if one has been
/// applied. A shared, caller-must-not-destroy handle usable as a fallback
/// when no app-specific icon path is available.
pub(crate) fn current_large_icon_handle() -> Option<isize> {
    current_app_icon_handles().map(|handles| handles.large)
}

fn install_icon_hook() {
    ICON_HOOK_INSTALLED.get_or_init(|| {
        add_host_window_created_handler(Arc::new(|window| {
            if let Some(handles) = current_app_icon_handles() {
                apply_window_icons(HWND(window as *mut c_void), handles);
            }
        }));
    });
}

fn current_app_icon_handles() -> Option<AppIconHandles> {
    APP_ICON_HANDLES
        .get()
        .and_then(|icons| icons.lock().ok().and_then(|icons| *icons))
}

/// Decodes the launcher PNG and Dock-normalizes it for the taskbar / alt-tab
/// cell: 73% visual ratio plus a 22% rounded plate, matching macOS `AppIcon`.
///
/// Keep the ratios in sync with `tools/lingxia-cli/src/gen/icons.rs`
/// (`png_to_ico_bytes`), or the embedded `.exe` icon and the live taskbar
/// drift apart.
pub(crate) fn prepare_app_icon_image(path: &Path) -> Result<image::RgbaImage, String> {
    let image = if path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"))
    {
        rasterize_svg_icon(path)?
    } else {
        image::open(path)
            .map_err(|err| format!("Failed to load Windows app icon {}: {err}", path.display()))?
            .into_rgba8()
    };
    Ok(dock_normalize_icon(image))
}

const TARGET_DOCK_VISUAL_RATIO: f32 = 0.73;
const APP_TILE_CORNER_RATIO: f32 = 0.22;

fn dock_normalize_icon(image: image::RgbaImage) -> image::RgbaImage {
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return image;
    }
    let canvas = width.max(height);
    let ratio = opaque_bounds_ratio(&image);
    let scale = (TARGET_DOCK_VISUAL_RATIO / ratio).clamp(0.60, 0.92);
    let mut icon_size = (canvas as f32 * scale).round().max(1.0) as u32;
    // Keep the plate's parity equal to the canvas's so the centering offset
    // is exact; a half-pixel shift reads as a lopsided tile at 16px.
    if (canvas - icon_size) % 2 == 1 {
        icon_size += 1;
    }
    let offset = (canvas - icon_size) / 2;
    let mut plate = image::imageops::resize(
        &image,
        icon_size,
        icon_size,
        image::imageops::FilterType::Lanczos3,
    );
    apply_rounded_corner_mask(&mut plate, icon_size as f32 * APP_TILE_CORNER_RATIO);
    let mut out = image::RgbaImage::new(canvas, canvas);
    image::imageops::overlay(&mut out, &plate, offset as i64, offset as i64);
    out
}

fn opaque_bounds_ratio(image: &image::RgbaImage) -> f32 {
    let (width, height) = image.dimensions();
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (width, height, 0u32, 0u32);
    let mut found = false;
    for (x, y, pixel) in image.enumerate_pixels() {
        if pixel.0[3] <= 12 {
            continue;
        }
        found = true;
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }
    if !found {
        return 1.0;
    }
    let bw = (max_x - min_x + 1) as f32 / width as f32;
    let bh = (max_y - min_y + 1) as f32 / height as f32;
    bw.max(bh).clamp(0.01, 1.0)
}

fn apply_rounded_corner_mask(image: &mut image::RgbaImage, radius: f32) {
    let (width, height) = image.dimensions();
    let radius = radius.clamp(1.0, width.min(height) as f32 * 0.5);
    let left = radius;
    let top = radius;
    let right = width as f32 - radius;
    let bottom = height as f32 - radius;
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        let xf = x as f32 + 0.5;
        let yf = y as f32 + 0.5;
        let cx = xf.clamp(left, right);
        let cy = yf.clamp(top, bottom);
        let dist = (xf - cx).hypot(yf - cy);
        if dist <= radius - 1.0 {
            continue;
        }
        if dist >= radius {
            pixel.0[3] = 0;
            continue;
        }
        let edge = ((radius - dist) * 255.0).clamp(0.0, 255.0) as u16;
        pixel.0[3] = ((u16::from(pixel.0[3]) * edge) / 255) as u8;
    }
}

fn rasterize_svg_icon(path: &Path) -> Result<image::RgbaImage, String> {
    let svg = std::fs::read_to_string(path)
        .map_err(|err| format!("Failed to read Windows SVG icon {}: {err}", path.display()))?;
    let tree = usvg::Tree::from_str(&svg, &usvg::Options::default())
        .map_err(|err| format!("Failed to parse Windows SVG icon {}: {err}", path.display()))?;
    let size = tree.size();
    let width = size.width().max(1.0).round() as u32;
    let height = size.height().max(1.0).round() as u32;
    let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or_else(|| {
        format!(
            "Failed to allocate pixmap for Windows SVG icon {}",
            path.display()
        )
    })?;
    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
    image::RgbaImage::from_raw(width, height, pixmap.take()).ok_or_else(|| {
        format!(
            "Failed to decode rasterized Windows SVG icon {}",
            path.display()
        )
    })
}

fn taskbar_is_dark() -> bool {
    use windows::Win32::System::Registry::{
        HKEY_CURRENT_USER, KEY_READ, REG_DWORD, RegOpenKeyExW, RegQueryValueExW,
    };
    let mut key = windows::Win32::System::Registry::HKEY::default();
    let path =
        windows::core::w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
    let opened = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, path, Some(0), KEY_READ, &mut key) };
    if opened.is_err() {
        return true;
    }
    let mut value = 1u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    let mut kind = REG_DWORD;
    let status = unsafe {
        RegQueryValueExW(
            key,
            windows::core::w!("SystemUsesLightTheme"),
            None,
            Some(&mut kind),
            Some((&mut value as *mut u32).cast()),
            Some(&mut size),
        )
    };
    let _ = unsafe { windows::Win32::System::Registry::RegCloseKey(key) };
    status.is_err() || value == 0
}

fn invert_dark_tray_glyph(image: &mut image::RgbaImage) {
    // macOS treats the glyph as a template and inverts for the menu bar.
    // Windows has no template icons: a light taskbar needs the original
    // black strokes, a dark one needs the invert. Always-white was the
    // white blob on Windows 11's default light notification area.
    if !taskbar_is_dark() {
        return;
    }
    invert_dark_pixels(image);
}

fn invert_dark_pixels(image: &mut image::RgbaImage) {
    let mut lum_sum = 0u64;
    let mut count = 0u64;
    for pixel in image.pixels() {
        let [r, g, b, a] = pixel.0;
        if a < 32 {
            continue;
        }
        lum_sum += (u64::from(r) * 3 + u64::from(g) * 6 + u64::from(b)) / 10;
        count += 1;
    }
    if count == 0 || lum_sum / count > 140 {
        return;
    }
    for pixel in image.pixels_mut() {
        let [r, g, b, a] = pixel.0;
        if a < 16 {
            continue;
        }
        pixel.0 = [255 - r, 255 - g, 255 - b, a];
    }
}

fn create_icon_from_image(
    source: &image::RgbaImage,
    size: u32,
    path: &Path,
) -> Result<isize, String> {
    let image = image::imageops::resize(source, size, size, image::imageops::FilterType::Lanczos3);

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
                "Failed to create Windows app icon color bitmap from {}",
                path.display()
            ));
        }

        let mask = CreateBitmap(width, height, 1, 1, None);
        if mask.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(color.0));
            return Err(format!(
                "Failed to create Windows app icon mask bitmap from {}",
                path.display()
            ));
        }

        let info = ICONINFO {
            fIcon: BOOL(1),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color,
        };
        let icon = WindowsAndMessaging::CreateIconIndirect(&info).map_err(|err| {
            format!(
                "Failed to create Windows app icon from {}: {err}",
                path.display()
            )
        })?;
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

fn apply_window_icons(hwnd: HWND, icons: AppIconHandles) {
    unsafe {
        let _ = WindowsAndMessaging::SendMessageW(
            hwnd,
            WM_SETICON,
            Some(WPARAM(ICON_SMALL as usize)),
            Some(LPARAM(icons.small)),
        );
        let _ = WindowsAndMessaging::SendMessageW(
            hwnd,
            WM_SETICON,
            Some(WPARAM(ICON_BIG as usize)),
            Some(LPARAM(icons.large)),
        );
        let _ = WindowsAndMessaging::SetClassLongPtrW(hwnd, GCLP_HICONSM, icons.small);
        let _ = WindowsAndMessaging::SetClassLongPtrW(hwnd, GCLP_HICON, icons.large);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_tray_glyph_is_inverted_for_the_notification_area() {
        let mut source = image::RgbaImage::new(8, 8);
        source.put_pixel(3, 3, image::Rgba([0, 0, 0, 255]));
        invert_dark_pixels(&mut source);
        assert_eq!(source.get_pixel(3, 3).0, [255, 255, 255, 255]);
    }

    #[test]
    fn dock_normalize_insets_a_full_bleed_plate_and_rounds_it() {
        let source = image::RgbaImage::from_pixel(20, 20, image::Rgba([20, 80, 200, 255]));
        let normalized = dock_normalize_icon(source);
        assert_eq!(normalized.dimensions(), (20, 20));
        assert_eq!(normalized.get_pixel(0, 0).0[3], 0);
        assert_eq!(normalized.get_pixel(10, 10).0[3], 255);
    }
}
