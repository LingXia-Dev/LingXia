//! HICON creation from PNGs plus the app/panel icon caches.

use super::*;

pub(crate) type IconCacheKey = (PathBuf, u32);

pub(crate) type IconHandleCache = HashMap<IconCacheKey, Option<isize>>;

#[derive(Debug, Clone, Copy)]
pub(crate) struct AppIconHandles {
    pub(crate) small: isize,
    pub(crate) large: isize,
}

pub(crate) static APP_ICON_HANDLES: OnceLock<Mutex<Option<AppIconHandles>>> = OnceLock::new();

pub(crate) static PANEL_ICON_HANDLES: OnceLock<Mutex<IconHandleCache>> = OnceLock::new();

pub fn set_app_icon_from_path(path: &Path) -> StdResult<()> {
    let handles = AppIconHandles {
        small: create_icon_from_png(path, 16)?,
        large: create_icon_from_png(path, 32)?,
    };
    let icon_state = APP_ICON_HANDLES.get_or_init(|| Mutex::new(None));
    let mut icon_state = icon_state
        .lock()
        .map_err(|_| WebViewError::WebView("Windows app icon state is poisoned".to_string()))?;
    if let Some(old) = icon_state.replace(handles) {
        destroy_icon_handle(old.small);
        destroy_icon_handle(old.large);
    }
    Ok(())
}

pub(crate) fn destroy_icon_handle(handle: isize) {
    if handle != 0 {
        unsafe {
            let _ = WindowsAndMessaging::DestroyIcon(hicon(handle));
        }
    }
}

pub(crate) fn current_app_icon_handles() -> Option<AppIconHandles> {
    APP_ICON_HANDLES
        .get()
        .and_then(|icons| icons.lock().ok().and_then(|icons| *icons))
}

pub(crate) fn create_icon_from_png(path: &Path, size: u32) -> StdResult<isize> {
    let image = image::open(path)
        .map_err(|err| {
            WebViewError::WebView(format!(
                "Failed to load Windows app icon {}: {}",
                path.display(),
                err
            ))
        })?
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
            return Err(WebViewError::WebView(format!(
                "Failed to create Windows app icon color bitmap from {}",
                path.display()
            )));
        }

        let mask = CreateBitmap(width, height, 1, 1, None);
        if mask.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(color.0));
            return Err(WebViewError::WebView(format!(
                "Failed to create Windows app icon mask bitmap from {}",
                path.display()
            )));
        }

        let info = ICONINFO {
            fIcon: BOOL(1),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color,
        };
        let icon = WindowsAndMessaging::CreateIconIndirect(&info).map_err(|err| {
            WebViewError::WebView(format!(
                "Failed to create Windows app icon from {}: {}",
                path.display(),
                err
            ))
        })?;
        let _ = DeleteObject(HGDIOBJ(color.0));
        let _ = DeleteObject(HGDIOBJ(mask.0));
        Ok(icon.0 as isize)
    }
}

pub(crate) fn hicon(handle: isize) -> HICON {
    HICON(handle as *mut c_void)
}

pub(crate) fn apply_window_icons(hwnd: HWND, icons: AppIconHandles) {
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

pub(crate) fn hide_titlebar_icon(hwnd: HWND) {
    unsafe {
        let _ = WindowsAndMessaging::SendMessageW(
            hwnd,
            WM_SETICON,
            Some(WPARAM(ICON_SMALL as usize)),
            Some(LPARAM(0)),
        );
        let _ = WindowsAndMessaging::SendMessageW(
            hwnd,
            WM_SETICON,
            Some(WPARAM(ICON_BIG as usize)),
            Some(LPARAM(0)),
        );
        let _ = WindowsAndMessaging::SetClassLongPtrW(hwnd, GCLP_HICONSM, 0);
        let _ = WindowsAndMessaging::SetClassLongPtrW(hwnd, GCLP_HICON, 0);
        let ex_style =
            WindowsAndMessaging::GetWindowLongPtrW(hwnd, WindowsAndMessaging::GWL_EXSTYLE) as u32;
        let _ = WindowsAndMessaging::SetWindowLongPtrW(
            hwnd,
            WindowsAndMessaging::GWL_EXSTYLE,
            (ex_style | WindowsAndMessaging::WS_EX_DLGMODALFRAME.0) as isize,
        );
        let _ = WindowsAndMessaging::SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            WindowsAndMessaging::SWP_NOMOVE
                | WindowsAndMessaging::SWP_NOSIZE
                | WindowsAndMessaging::SWP_NOZORDER
                | WindowsAndMessaging::SWP_NOACTIVATE
                | WindowsAndMessaging::SWP_FRAMECHANGED,
        );
    }
}

/// Returns a cached `HICON` handle (as `isize`) for a PNG file rendered at
/// `size` x `size` pixels, or `None` if the image cannot be loaded.
///
/// Part of the chrome-renderer seam: registered [`WindowsChromeRenderer`]s
/// use this to draw tab/activator icons without owning an icon cache. The
/// returned handle stays owned by the cache and must not be destroyed.
pub fn cached_png_icon_handle(path: &str, size: u32) -> Option<isize> {
    let path = PathBuf::from(path);
    let key = (path.clone(), size);
    let handles = PANEL_ICON_HANDLES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut handles) = handles.lock() {
        if let Some(handle) = handles.get(&key) {
            return *handle;
        }
        let handle = create_icon_from_png(&path, size).ok();
        if let Some(Some(previous)) = handles.insert(key, handle)
            && Some(previous) != handle
        {
            destroy_icon_handle(previous);
        }
        return handle;
    }
    create_icon_from_png(&path, size).ok()
}
