//! PNG icon cache for Windows shell chrome drawing.

use std::collections::HashMap;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use windows::Win32::Graphics::Gdi::{CreateBitmap, DeleteObject, HGDIOBJ};
use windows::Win32::UI::WindowsAndMessaging::{self, HICON, ICONINFO};
use windows::core::BOOL;

type IconCacheKey = (PathBuf, u32);
type IconHandleCache = HashMap<IconCacheKey, Option<isize>>;
type BytesIconCache = HashMap<(String, u32), (u64, Option<isize>)>;

static PANEL_ICON_HANDLES: OnceLock<Mutex<IconHandleCache>> = OnceLock::new();
static BYTES_ICON_HANDLES: OnceLock<Mutex<BytesIconCache>> = OnceLock::new();

pub(super) fn cached_png_icon_handle(path: &str, size: u32) -> Option<isize> {
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

fn create_icon_from_png(path: &Path, size: u32) -> Result<isize, String> {
    let image = image::open(path).map_err(|err| {
        format!(
            "Failed to load Windows shell icon {}: {err}",
            path.display()
        )
    })?;
    create_icon_from_image(image, size, &path.display().to_string())
}

fn create_icon_from_png_bytes(png: &[u8], size: u32) -> Result<isize, String> {
    let image = image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .map_err(|err| format!("Failed to decode PNG icon bytes: {err}"))?;
    create_icon_from_image(image, size, "<png bytes>")
}

fn create_icon_from_image(
    image: image::DynamicImage,
    size: u32,
    source: &str,
) -> Result<isize, String> {
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
