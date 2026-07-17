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

    async fn resolve_app_window(
        &self,
        window_id: Option<&str>,
    ) -> Result<WindowInfo, PlatformError> {
        let hwnd = resolve_screenshot_window(window_id)?;
        let id = (hwnd.0 as usize).to_string();
        list_app_windows()?
            .into_iter()
            .find(|window| window.id == id)
            .ok_or_else(|| PlatformError::Platform(format!("resolved window {id} disappeared")))
    }
}

fn list_app_windows() -> Result<Vec<WindowInfo>, PlatformError> {
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetForegroundWindow, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
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

        let rect = content_window_rect(hwnd).ok();
        let visible = unsafe { IsWindowVisible(hwnd).as_bool() && !IsIconic(hwnd).as_bool() };
        let title = window_title(hwnd);
        let width = if let Some(rect) = rect {
            (rect.right - rect.left).max(0) as u32
        } else {
            0
        };
        let height = if let Some(rect) = rect {
            (rect.bottom - rect.top).max(0) as u32
        } else {
            0
        };
        if !is_automation_window(&title, width, height) {
            return BOOL(1);
        }

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

fn is_automation_window(title: &str, width: u32, height: u32) -> bool {
    width > 0
        && height > 0
        && title != "Default IME"
        && title != "MSCTFIME UI"
        && !title.starts_with("GDI+ Window (")
}

pub(super) fn resolve_screenshot_window(
    window_id: Option<&str>,
) -> Result<windows::Win32::Foundation::HWND, PlatformError> {
    if let Some(window_id) = window_id {
        return hwnd_from_window_id(window_id);
    }

    let windows = list_app_windows()?;
    // Prefer the window actually presenting app content (a WebView host). The
    // device frame adds borderless companion windows (the bezel, overlays) that
    // are visible but untitled; one of those would otherwise win the default
    // pick and screenshot as an empty/black surface (no WebView2 to composite).
    let hosts = webview_host_window_ids();
    let selected = windows
        .iter()
        .find(|window| {
            window.visible
                && window.width > 0
                && window.height > 0
                && window
                    .id
                    .parse::<usize>()
                    .map(|id| hosts.contains(&id))
                    .unwrap_or(false)
        })
        .or_else(|| {
            windows.iter().find(|window| {
                window.focused && window.visible && window.width > 0 && window.height > 0
            })
        })
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

/// Window ids that currently host a visible WebView surface — the real app
/// content windows, as opposed to the device frame's companion windows.
fn webview_host_window_ids() -> std::collections::HashSet<usize> {
    let mut ids = std::collections::HashSet::new();
    for webtag in webview_runtime::list_webviews() {
        if let Ok(snapshot) = lingxia_windows_contract::webview_window_snapshot(&webtag)
            && snapshot.visible
            && snapshot.content_width > 0
            && snapshot.content_height > 0
        {
            ids.insert(snapshot.window_id);
        }
    }
    ids
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
    let (width, height, mut image) = capture_window_native_rgba(hwnd_from_usize(window_id))?;
    for (snapshot, webview_png) in visible_webview_screenshots_for_window(window_id).await {
        overlay_webview_screenshot(&mut image, &snapshot, &webview_png)?;
    }
    overlay_window_screenshots(&mut image, hwnd_from_usize(window_id))?;
    encode_rgba_png(width, height, image)
}

fn hwnd_from_usize(window_id: usize) -> windows::Win32::Foundation::HWND {
    windows::Win32::Foundation::HWND(window_id as *mut c_void)
}

fn capture_window_native_rgba(
    hwnd: windows::Win32::Foundation::HWND,
) -> Result<(u32, u32, image::RgbaImage), PlatformError> {
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleDC, CreateDIBSection,
        DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, HGDIOBJ, ReleaseDC, SRCCOPY, SelectObject,
    };

    let rect = content_window_rect(hwnd)?;
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    if width <= 0 || height <= 0 {
        return Err(PlatformError::Platform(format!(
            "window has empty bounds: {width}x{height}"
        )));
    }

    if is_screen_capture_window_class(&window_class_name(hwnd)) {
        return capture_screen_rect_rgba(rect).map(|image| (width as u32, height as u32, image));
    }

    unsafe {
        let window_dc = GetDC(Some(hwnd));
        if window_dc.is_invalid() {
            return Err(PlatformError::Platform("GetDC failed".to_string()));
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
    use windows::Win32::Graphics::Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
    use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;

    let mut rect = RECT::default();
    unsafe {
        if DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut rect as *mut _ as *mut c_void,
            std::mem::size_of::<RECT>() as u32,
        )
        .is_ok()
            && rect.right > rect.left
            && rect.bottom > rect.top
        {
            return Ok(rect);
        }
        GetWindowRect(hwnd, &mut rect)
            .map_err(|err| PlatformError::Platform(format!("GetWindowRect failed: {err}")))?;
    }
    Ok(rect)
}

/// Client bounds in screen coordinates. These dimensions and the native DC's
/// origin are the same coordinates accepted by app mouse messages.
fn content_window_rect(
    hwnd: windows::Win32::Foundation::HWND,
) -> Result<windows::Win32::Foundation::RECT, PlatformError> {
    use windows::Win32::Foundation::{POINT, RECT};
    use windows::Win32::Graphics::Gdi::ClientToScreen;
    use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

    let mut client = RECT::default();
    unsafe {
        GetClientRect(hwnd, &mut client)
            .map_err(|err| PlatformError::Platform(format!("GetClientRect failed: {err}")))?;
    }
    let mut origin = POINT {
        x: client.left,
        y: client.top,
    };
    if !unsafe { ClientToScreen(hwnd, &mut origin).as_bool() } {
        return Err(PlatformError::Platform("ClientToScreen failed".to_string()));
    }
    Ok(RECT {
        left: origin.x,
        top: origin.y,
        right: origin.x + (client.right - client.left),
        bottom: origin.y + (client.bottom - client.top),
    })
}

async fn visible_webview_screenshots_for_window(
    window_id: usize,
) -> Vec<(
    lingxia_windows_contract::WindowsWebViewWindowSnapshot,
    Vec<u8>,
)> {
    let mut captures = Vec::new();
    for webtag in webview_runtime::list_webviews() {
        let snapshot = match lingxia_windows_contract::webview_window_snapshot(&webtag) {
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
    snapshot: &lingxia_windows_contract::WindowsWebViewWindowSnapshot,
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
    let mut webview = image::imageops::resize(
        &webview,
        width,
        height,
        image::imageops::FilterType::Lanczos3,
    );
    // CapturePreview sees content pre-clip; reproduce the live surface's
    // composition-clip corners so screenshots match the screen.
    mask_corner_alpha(&mut webview, snapshot.content_corner_radii);
    image::imageops::overlay(base, &webview, i64::from(left), i64::from(top));
    Ok(())
}

/// Multiplies corner alpha by rounded coverage `[tl, tr, br, bl]` (the same
/// SDF the shell's corner masks use), leaving zero-radius corners untouched.
fn mask_corner_alpha(image: &mut image::RgbaImage, radii: [i32; 4]) {
    if radii == [0; 4] {
        return;
    }
    let (width, height) = (image.width() as i32, image.height() as i32);
    let [tl, tr, br, bl] = radii;
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        let (x, y) = (x as i32, y as i32);
        let (radius, corner_x, corner_y) = if x < tl && y < tl {
            (tl, tl, tl)
        } else if x >= width - tr && y < tr {
            (tr, width - tr, tr)
        } else if x >= width - br && y >= height - br {
            (br, width - br, height - br)
        } else if x < bl && y >= height - bl {
            (bl, bl, height - bl)
        } else {
            continue;
        };
        let dx = x as f32 + 0.5 - corner_x as f32;
        let dy = y as f32 + 0.5 - corner_y as f32;
        let coverage = (radius as f32 - (dx * dx + dy * dy).sqrt() + 0.5).clamp(0.0, 1.0);
        pixel.0[3] = (pixel.0[3] as f32 * coverage) as u8;
    }
}

fn overlay_window_screenshots(
    base: &mut image::RgbaImage,
    hwnd: windows::Win32::Foundation::HWND,
) -> Result<(), PlatformError> {
    let base_rect = content_window_rect(hwnd)?;
    let mut overlays = overlay_windows_for_window(hwnd, base_rect)?;
    overlays.reverse();
    for overlay in overlays {
        let Ok(overlay_image) = capture_screen_rect_rgba(overlay.rect) else {
            continue;
        };
        let x = i64::from(overlay.rect.left - base_rect.left);
        let y = i64::from(overlay.rect.top - base_rect.top);
        image::imageops::overlay(base, &overlay_image, x, y);
    }
    Ok(())
}

struct OverlayWindow {
    rect: windows::Win32::Foundation::RECT,
}

fn overlay_windows_for_window(
    base_window: windows::Win32::Foundation::HWND,
    base_rect: windows::Win32::Foundation::RECT,
) -> Result<Vec<OverlayWindow>, PlatformError> {
    use windows::Win32::Foundation::{HWND, LPARAM, RECT};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GW_OWNER, GetWindow, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
    };
    use windows::core::BOOL;

    struct EnumState {
        pid: u32,
        base_window: HWND,
        base_rect: RECT,
        overlays: Vec<OverlayWindow>,
    }

    unsafe extern "system" fn enum_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let state = unsafe { &mut *(lparam.0 as *mut EnumState) };
        let class_name = window_class_name(hwnd);
        if !is_screenshot_overlay_class(&class_name) {
            return BOOL(1);
        }

        let mut owner_pid = 0u32;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut owner_pid));
        }
        if owner_pid != state.pid {
            return BOOL(1);
        }
        let owner = unsafe { GetWindow(hwnd, GW_OWNER).unwrap_or_default() };
        if owner != state.base_window {
            return BOOL(1);
        }
        if unsafe { !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() } {
            return BOOL(1);
        }

        let Ok(rect) = visible_window_rect(hwnd) else {
            return BOOL(1);
        };
        if !rects_intersect(state.base_rect, rect) {
            return BOOL(1);
        }

        state.overlays.push(OverlayWindow { rect });
        BOOL(1)
    }

    let mut state = EnumState {
        pid: std::process::id(),
        base_window,
        base_rect,
        overlays: Vec::new(),
    };
    unsafe {
        EnumWindows(
            Some(enum_window),
            LPARAM((&mut state as *mut EnumState) as isize),
        )
    }
    .map_err(|err| PlatformError::Platform(format!("EnumWindows failed: {err}")))?;
    Ok(state.overlays)
}

fn capture_screen_rect_rgba(
    rect: windows::Win32::Foundation::RECT,
) -> Result<image::RgbaImage, PlatformError> {
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleDC, CreateDIBSection,
        DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, HGDIOBJ, ReleaseDC, SRCCOPY, SelectObject,
    };

    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    if width <= 0 || height <= 0 {
        return Err(PlatformError::Platform(format!(
            "screen rect has empty bounds: {width}x{height}"
        )));
    }

    unsafe {
        let screen_dc = GetDC(None);
        if screen_dc.is_invalid() {
            return Err(PlatformError::Platform("GetDC(None) failed".to_string()));
        }
        let memory_dc = CreateCompatibleDC(Some(screen_dc));
        if memory_dc.is_invalid() {
            let _ = ReleaseDC(None, screen_dc);
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
                biSizeImage: 0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits_ptr: *mut c_void = std::ptr::null_mut();
        let bitmap = match CreateDIBSection(
            Some(screen_dc),
            &bitmap_info,
            DIB_RGB_COLORS,
            &mut bits_ptr,
            None,
            0,
        ) {
            Ok(bitmap) => bitmap,
            Err(err) => {
                let _ = DeleteDC(memory_dc);
                let _ = ReleaseDC(None, screen_dc);
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
            Some(screen_dc),
            rect.left,
            rect.top,
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
        let _ = ReleaseDC(None, screen_dc);
        result
    }
}

fn is_screenshot_overlay_class(class_name: &str) -> bool {
    matches!(
        class_name,
        "LingXiaTransparentTabbarOverlay"
            | "LingXiaDeviceCapsule"
            | "LingXiaDeviceCutout"
            | "LingXiaDeviceCornerMask"
            | "LingXiaDeviceStatusBar"
            | "LingXiaDeviceAboutMask"
            | "LingXiaDeviceAboutSheet"
    )
}

fn is_screen_capture_window_class(class_name: &str) -> bool {
    class_name == "LingXiaDeviceFrame" || is_screenshot_overlay_class(class_name)
}

fn rects_intersect(
    a: windows::Win32::Foundation::RECT,
    b: windows::Win32::Foundation::RECT,
) -> bool {
    a.left < b.right && a.right > b.left && a.top < b.bottom && a.bottom > b.top
}

fn window_class_name(hwnd: windows::Win32::Foundation::HWND) -> String {
    use windows::Win32::UI::WindowsAndMessaging::GetClassNameW;

    let mut buffer = vec![0u16; 128];
    let copied = unsafe { GetClassNameW(hwnd, &mut buffer) };
    if copied <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buffer[..copied as usize])
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

#[cfg(test)]
mod tests {
    use super::is_automation_window;

    #[test]
    fn filters_non_content_helper_windows() {
        assert!(!is_automation_window("Default IME", 120, 40));
        assert!(!is_automation_window("GDI+ Window (LingXiaDemo.exe)", 1, 1));
        assert!(!is_automation_window("LingXia", 1010, 0));
        assert!(is_automation_window("LingXia", 1010, 754));
    }
}
