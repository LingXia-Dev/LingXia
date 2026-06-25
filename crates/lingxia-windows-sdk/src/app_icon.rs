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
static ICON_HOOK_INSTALLED: OnceLock<()> = OnceLock::new();

pub(crate) fn set_app_icon_from_path(path: &Path) -> Result<(), String> {
    install_icon_hook();
    // Decode + tighten once, then rasterize to each size. The large icon is
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
    Ok(())
}

/// The source PNG path of the applied product/app icon (the launcher icon
/// resolved at startup), if one was set. This is the application's icon, not
/// any single lxapp's icon. Only the product shell's About box and tray icon
/// read it (the app-menu button draws the brand glyph), so it is gated to
/// `browser-shell`.
#[cfg(feature = "browser-shell")]
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
// Only the product shell (tray icon, About box) needs an owned dialog icon;
// `browser-runtime`-only hosts (the dev runner) build without those.
#[cfg(feature = "browser-runtime")]
#[cfg_attr(not(feature = "browser-shell"), allow(dead_code))]
pub(crate) fn create_icon_handle_from_path(path: &Path, size: u32) -> Option<isize> {
    let image = prepare_app_icon_image(path).ok()?;
    create_icon_from_image(&image, size, path).ok()
}

/// The process's current large (32px) app-icon handle, if one has been
/// applied. A shared, caller-must-not-destroy handle usable as a fallback
/// when no app-specific icon path is available.
#[cfg(feature = "browser-runtime")]
#[cfg_attr(not(feature = "browser-shell"), allow(dead_code))]
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

/// Decodes the app-icon PNG and tightens it for the small Windows taskbar /
/// alt-tab cell: a mobile launcher icon centers its glyph inside a wide safe-
/// area margin, which reads as a tiny logo lost in padding once scaled to
/// 16-48px. When the icon has a uniform background (the four corners agree), the
/// padding is cropped to the glyph plus a small margin (kept square so the logo
/// is never stretched), so it fills the cell. Icons without a uniform border are
/// returned unchanged.
fn prepare_app_icon_image(path: &Path) -> Result<image::RgbaImage, String> {
    let image = image::open(path)
        .map_err(|err| format!("Failed to load Windows app icon {}: {err}", path.display()))?
        .into_rgba8();
    Ok(tighten_icon(image))
}

fn tighten_icon(image: image::RgbaImage) -> image::RgbaImage {
    let (w, h) = image.dimensions();
    if w == 0 || h == 0 {
        return image;
    }
    // Only trim when the border is a single flat color (the corners agree); a
    // full-bleed / photographic icon must be left as-is.
    let bg = image.get_pixel(0, 0).0;
    let corners = [
        image.get_pixel(w - 1, 0).0,
        image.get_pixel(0, h - 1).0,
        image.get_pixel(w - 1, h - 1).0,
    ];
    if corners.iter().any(|c| !color_close(*c, bg, 12)) {
        return image;
    }
    // Bounding box of everything that isn't the background (transparent counts
    // as background too).
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (w, h, 0u32, 0u32);
    let mut found = false;
    for (x, y, pixel) in image.enumerate_pixels() {
        let p = pixel.0;
        if p[3] < 16 || color_close(p, bg, 32) {
            continue;
        }
        found = true;
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }
    if !found {
        return image;
    }
    // Square crop centered on the glyph with ~12% breathing room, clamped to the
    // image and back-filled with the background where it would overrun an edge.
    let content = (max_x - min_x + 1).max(max_y - min_y + 1);
    let side = content + content / 4; // glyph + ~12% margin each side
    let cx = (min_x + max_x) / 2;
    let cy = (min_y + max_y) / 2;
    let half = (side / 2) as i64;
    let mut out = image::RgbaImage::from_pixel(side, side, image::Rgba(bg));
    for oy in 0..side {
        for ox in 0..side {
            let sx = cx as i64 - half + ox as i64;
            let sy = cy as i64 - half + oy as i64;
            if sx >= 0 && sy >= 0 && (sx as u32) < w && (sy as u32) < h {
                out.put_pixel(ox, oy, *image.get_pixel(sx as u32, sy as u32));
            }
        }
    }
    out
}

/// Whether two RGBA colors are within `tol` per channel (ignoring alpha).
fn color_close(a: [u8; 4], b: [u8; 4], tol: u8) -> bool {
    a[0].abs_diff(b[0]) <= tol && a[1].abs_diff(b[1]) <= tol && a[2].abs_diff(b[2]) <= tol
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
