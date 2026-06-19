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

use lingxia_windows_host::add_host_window_created_handler;

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
    let handles = AppIconHandles {
        small: create_icon_from_png(path, 16)?,
        large: create_icon_from_png(path, 32)?,
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
/// any single lxapp's icon.
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
    create_icon_from_png(path, size).ok()
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

fn create_icon_from_png(path: &Path, size: u32) -> Result<isize, String> {
    let image = image::open(path)
        .map_err(|err| format!("Failed to load Windows app icon {}: {err}", path.display()))?;
    let image = image
        .resize_exact(size, size, image::imageops::FilterType::Lanczos3)
        .into_rgba8();

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
