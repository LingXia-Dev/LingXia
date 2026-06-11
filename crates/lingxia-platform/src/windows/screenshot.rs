use std::ffi::c_void;
use std::io::Cursor;

use async_trait::async_trait;
use lingxia_webview::runtime as webview_runtime;

use super::Platform;
use crate::error::PlatformError;
use crate::traits::screenshot::{AppScreenshot, WindowInfo};

#[async_trait]
impl AppScreenshot for Platform {
    async fn list_app_windows(&self) -> Result<Vec<WindowInfo>, PlatformError> {
        list_app_windows()
    }

    async fn take_app_screenshot(&self, window_id: Option<&str>) -> Result<Vec<u8>, PlatformError> {
        let hwnd = resolve_screenshot_window(window_id)?;
        capture_window_png(hwnd.0 as usize).await
    }
}

fn list_app_windows() -> Result<Vec<WindowInfo>, PlatformError> {
    use windows::Win32::Foundation::{HWND, LPARAM, RECT};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetForegroundWindow, GetWindowRect, GetWindowThreadProcessId, IsIconic,
        IsWindowVisible,
    };
    use windows::core::BOOL;

    struct EnumState {
        pid: u32,
        foreground: HWND,
        windows: Vec<WindowInfo>,
    }

    unsafe extern "system" fn enum_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let state = unsafe { &mut *(lparam.0 as *mut EnumState) };
        let mut owner_pid = 0u32;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut owner_pid));
        }
        if owner_pid != state.pid {
            return BOOL(1);
        }

        let mut rect = RECT::default();
        let has_rect = unsafe { GetWindowRect(hwnd, &mut rect).is_ok() };
        let visible = unsafe { IsWindowVisible(hwnd).as_bool() && !IsIconic(hwnd).as_bool() };
        let title = window_title(hwnd);
        let width = if has_rect {
            (rect.right - rect.left).max(0) as u32
        } else {
            0
        };
        let height = if has_rect {
            (rect.bottom - rect.top).max(0) as u32
        } else {
            0
        };

        state.windows.push(WindowInfo {
            id: (hwnd.0 as usize).to_string(),
            title,
            focused: hwnd == state.foreground,
            main: hwnd == state.foreground,
            visible,
            width,
            height,
        });
        BOOL(1)
    }

    let mut state = EnumState {
        pid: std::process::id(),
        foreground: unsafe { GetForegroundWindow() },
        windows: Vec::new(),
    };

    unsafe {
        EnumWindows(
            Some(enum_window),
            LPARAM((&mut state as *mut EnumState) as isize),
        )
    }
    .map_err(|err| PlatformError::Platform(format!("EnumWindows failed: {err}")))?;

    state.windows.sort_by(|a, b| {
        b.focused
            .cmp(&a.focused)
            .then_with(|| b.visible.cmp(&a.visible))
            .then_with(|| a.title.cmp(&b.title))
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(state.windows)
}

pub(super) fn resolve_screenshot_window(
    window_id: Option<&str>,
) -> Result<windows::Win32::Foundation::HWND, PlatformError> {
    if let Some(window_id) = window_id {
        return hwnd_from_window_id(window_id);
    }

    let windows = list_app_windows()?;
    let selected = windows
        .iter()
        .find(|window| window.focused && window.visible && window.width > 0 && window.height > 0)
        .or_else(|| {
            windows
                .iter()
                .find(|window| window.visible && window.width > 0 && window.height > 0)
        })
        .ok_or_else(|| {
            PlatformError::Platform("no visible Windows app window available to screenshot".into())
        })?;
    hwnd_from_window_id(&selected.id)
}

fn hwnd_from_window_id(raw: &str) -> Result<windows::Win32::Foundation::HWND, PlatformError> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowThreadProcessId, IsWindow};

    let id = raw.trim().parse::<usize>().map_err(|err| {
        PlatformError::InvalidParameter(format!(
            "window id must be a numeric HWND, got {raw}: {err}"
        ))
    })?;
    let hwnd = HWND(id as *mut c_void);
    if hwnd.0.is_null() || !unsafe { IsWindow(Some(hwnd)).as_bool() } {
        return Err(PlatformError::InvalidParameter(format!(
            "window id is not a live HWND: {raw}"
        )));
    }

    let mut owner_pid = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut owner_pid));
    }
    if owner_pid != std::process::id() {
        return Err(PlatformError::InvalidParameter(format!(
            "window id {raw} does not belong to this process"
        )));
    }
    Ok(hwnd)
}

async fn capture_window_png(window_id: usize) -> Result<Vec<u8>, PlatformError> {
    let hwnd = windows::Win32::Foundation::HWND(window_id as *mut c_void);
    let (width, height, mut image) = capture_window_native_rgba(hwnd)?;
    for (snapshot, webview_png) in visible_webview_screenshots_for_window(window_id).await {
        overlay_webview_screenshot(&mut image, &snapshot, &webview_png)?;
    }
    encode_rgba_png(width, height, image)
}

fn capture_window_native_rgba(
    hwnd: windows::Win32::Foundation::HWND,
) -> Result<(u32, u32, image::RgbaImage), PlatformError> {
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleDC, CreateDIBSection,
        DIB_RGB_COLORS, DeleteDC, DeleteObject, GetWindowDC, HGDIOBJ, ReleaseDC, SRCCOPY,
        SelectObject,
    };

    let rect = visible_window_rect(hwnd)?;
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    if width <= 0 || height <= 0 {
        return Err(PlatformError::Platform(format!(
            "window has empty bounds: {width}x{height}"
        )));
    }

    unsafe {
        let window_dc = GetWindowDC(Some(hwnd));
        if window_dc.is_invalid() {
            return Err(PlatformError::Platform("GetWindowDC failed".to_string()));
        }

        let memory_dc = CreateCompatibleDC(Some(window_dc));
        if memory_dc.is_invalid() {
            let _ = ReleaseDC(Some(hwnd), window_dc);
            return Err(PlatformError::Platform(
                "CreateCompatibleDC failed".to_string(),
            ));
        }

        let bitmap_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                // 0 is valid for BI_RGB and avoids an overflowing multiply.
                biSizeImage: 0,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut bits_ptr: *mut c_void = std::ptr::null_mut();
        let bitmap = match CreateDIBSection(
            Some(window_dc),
            &bitmap_info,
            DIB_RGB_COLORS,
            &mut bits_ptr,
            None,
            0,
        ) {
            Ok(bitmap) => bitmap,
            Err(err) => {
                let _ = DeleteDC(memory_dc);
                let _ = ReleaseDC(Some(hwnd), window_dc);
                return Err(PlatformError::Platform(format!(
                    "CreateDIBSection failed: {err}"
                )));
            }
        };

        let old_bitmap = SelectObject(memory_dc, HGDIOBJ(bitmap.0));
        let copied = BitBlt(
            memory_dc,
            0,
            0,
            width,
            height,
            Some(window_dc),
            0,
            0,
            SRCCOPY,
        )
        .is_ok();

        let result = if !copied || bits_ptr.is_null() {
            Err(PlatformError::Platform("BitBlt failed".to_string()))
        } else {
            let byte_len = (width as usize)
                .saturating_mul(height as usize)
                .saturating_mul(4);
            let bgra = std::slice::from_raw_parts(bits_ptr.cast::<u8>(), byte_len);
            bgra_top_down_image(width as u32, height as u32, bgra)
        };

        if !old_bitmap.is_invalid() {
            let _ = SelectObject(memory_dc, old_bitmap);
        }
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(memory_dc);
        let _ = ReleaseDC(Some(hwnd), window_dc);

        result.map(|image| (width as u32, height as u32, image))
    }
}

fn visible_window_rect(
    hwnd: windows::Win32::Foundation::HWND,
) -> Result<windows::Win32::Foundation::RECT, PlatformError> {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;

    let mut rect = RECT::default();
    unsafe {
        GetWindowRect(hwnd, &mut rect)
            .map_err(|err| PlatformError::Platform(format!("GetWindowRect failed: {err}")))?;
    }
    Ok(rect)
}

async fn visible_webview_screenshots_for_window(
    window_id: usize,
) -> Vec<(
    lingxia_webview::platform::windows::WindowsWebViewWindowSnapshot,
    Vec<u8>,
)> {
    let mut captures = Vec::new();
    for webtag in webview_runtime::list_webviews() {
        let snapshot = match lingxia_webview::platform::windows::webview_window_snapshot(&webtag) {
            Ok(snapshot)
                if snapshot.window_id == window_id
                    && snapshot.visible
                    && snapshot.content_width > 0
                    && snapshot.content_height > 0 =>
            {
                snapshot
            }
            _ => continue,
        };

        let Some(webview) = webview_runtime::find_webview(&webtag) else {
            continue;
        };
        match webview.take_screenshot().await {
            Ok(bytes) => captures.push((snapshot, bytes)),
            Err(err) => log::warn!(
                "failed to capture Windows WebView screenshot for {}: {}",
                webtag.key(),
                err
            ),
        }
    }
    captures.sort_by_key(|(snapshot, _)| {
        std::cmp::Reverse(
            snapshot
                .content_width
                .saturating_mul(snapshot.content_height),
        )
    });
    captures
}

fn overlay_webview_screenshot(
    base: &mut image::RgbaImage,
    snapshot: &lingxia_webview::platform::windows::WindowsWebViewWindowSnapshot,
    webview_png: &[u8],
) -> Result<(), PlatformError> {
    if snapshot.content_left < 0 || snapshot.content_top < 0 {
        return Ok(());
    }
    let left = snapshot.content_left as u32;
    let top = snapshot.content_top as u32;
    if left >= base.width() || top >= base.height() {
        return Ok(());
    }
    let width = snapshot.content_width.min(base.width() - left);
    let height = snapshot.content_height.min(base.height() - top);
    if width == 0 || height == 0 {
        return Ok(());
    }

    let webview = image::load_from_memory(webview_png)
        .map_err(|err| PlatformError::Platform(format!("failed to decode WebView PNG: {err}")))?
        .into_rgba8();
    let webview = image::imageops::resize(
        &webview,
        width,
        height,
        image::imageops::FilterType::Lanczos3,
    );
    image::imageops::overlay(base, &webview, i64::from(left), i64::from(top));
    Ok(())
}

fn bgra_top_down_image(
    width: u32,
    height: u32,
    bgra: &[u8],
) -> Result<image::RgbaImage, PlatformError> {
    let mut rgba = Vec::with_capacity(bgra.len());
    for pixel in bgra.chunks_exact(4) {
        rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], 255]);
    }

    image::ImageBuffer::<image::Rgba<u8>, Vec<u8>>::from_raw(width, height, rgba)
        .ok_or_else(|| PlatformError::Platform("failed to build screenshot image".to_string()))
}

fn encode_rgba_png(
    width: u32,
    height: u32,
    image: image::RgbaImage,
) -> Result<Vec<u8>, PlatformError> {
    if image.width() != width || image.height() != height {
        return Err(PlatformError::Platform(format!(
            "screenshot image dimensions changed during composition: expected {width}x{height}, got {}x{}",
            image.width(),
            image.height()
        )));
    }
    let mut out = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut out, image::ImageFormat::Png)
        .map_err(|err| PlatformError::Platform(format!("failed to encode PNG: {err}")))?;
    Ok(out.into_inner())
}

fn window_title(hwnd: windows::Win32::Foundation::HWND) -> String {
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowTextLengthW, GetWindowTextW};

    let len = unsafe { GetWindowTextLengthW(hwnd) };
    if len <= 0 {
        return String::new();
    }
    let mut buffer = vec![0u16; len as usize + 1];
    let copied = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    if copied <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buffer[..copied as usize])
}
