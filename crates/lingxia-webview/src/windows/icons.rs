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
    let image = image::open(path).map_err(|err| {
        WebViewError::WebView(format!(
            "Failed to load Windows app icon {}: {}",
            path.display(),
            err
        ))
    })?;
    create_icon_from_image(image, size, &path.display().to_string())
}

pub(crate) fn create_icon_from_png_bytes(png: &[u8], size: u32) -> StdResult<isize> {
    let image = image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .map_err(|err| WebViewError::WebView(format!("Failed to decode PNG icon bytes: {err}")))?;
    create_icon_from_image(image, size, "<png bytes>")
}

fn create_icon_from_image(image: image::DynamicImage, size: u32, source: &str) -> StdResult<isize> {
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
            return Err(WebViewError::WebView(format!(
                "Failed to create Windows app icon color bitmap from {source}"
            )));
        }

        let mask = CreateBitmap(width, height, 1, 1, None);
        if mask.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(color.0));
            return Err(WebViewError::WebView(format!(
                "Failed to create Windows app icon mask bitmap from {source}"
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
                "Failed to create Windows app icon from {source}: {err}"
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

/// Per-key cache of HICONs decoded from in-memory PNG bytes: the cached
/// content hash detects when the bytes behind a key changed (e.g. a tab
/// navigated to a site with a different favicon) and the stale handle is
/// destroyed and replaced.
pub(crate) type BytesIconCache = HashMap<(String, u32), (u64, Option<isize>)>;

pub(crate) static BYTES_ICON_HANDLES: OnceLock<Mutex<BytesIconCache>> = OnceLock::new();

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

/// Returns a cached `HICON` handle (as `isize`) for PNG-encoded bytes
/// rendered at `size` x `size` pixels, or `None` if the bytes cannot be
/// decoded. `cache_key` identifies the icon slot (e.g. a browser tab id);
/// when the bytes behind a key change, the cached handle is replaced.
///
/// Part of the chrome-renderer seam (the bytes sibling of
/// [`cached_png_icon_handle`]): registered [`WindowsChromeRenderer`]s use
/// this to draw favicons without owning an icon cache. The returned handle
/// stays owned by the cache and must not be destroyed.
pub fn cached_png_bytes_icon_handle(cache_key: &str, png: &[u8], size: u32) -> Option<isize> {
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
