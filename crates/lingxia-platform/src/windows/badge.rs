//! Taskbar overlay badge.
//!
//! Windows has no badge label to hand a string to: the overlay is an icon, so
//! the count has to be drawn. The drawn icon replaces whatever overlay was
//! there, and a cleared badge removes it.

use windows::Win32::Foundation::{HWND, LPARAM, SIZE};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection,
    CreateFontIndirectW, DEFAULT_CHARSET, DIB_RGB_COLORS, DT_CENTER, DT_SINGLELINE, DT_VCENTER,
    DeleteDC, DeleteObject, DrawTextW, FF_DONTCARE, FW_BOLD, GetDC, GetTextExtentPoint32W, HDC,
    HGDIOBJ, LOGFONTW, ReleaseDC, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::Win32::UI::Shell::{ITaskbarList3, TaskbarList};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconIndirect, DestroyIcon, EnumWindows, GetWindowThreadProcessId, HICON, ICONINFO,
    IsWindowVisible,
};
use windows::core::{BOOL, PCWSTR};

use crate::error::PlatformError;

/// Overlay icons are small; anything wider than three glyphs is unreadable, so
/// a larger count becomes `99+` the way every other platform's badge does.
const MAX_GLYPHS: usize = 3;
const ICON_SIZE: i32 = 32;

/// `Ok(false)` when this process has no taskbar button to overlay yet.
pub(super) fn set_app_badge(text: &str) -> Result<bool, PlatformError> {
    let label = badge_label(text);
    let windows = top_level_windows();
    for hwnd in &windows {
        apply_overlay(*hwnd, label.as_deref())?;
    }
    Ok(!windows.is_empty())
}

/// `None` clears. A count over two digits becomes `99+`, and a non-count was
/// already rejected before it reached this platform.
fn badge_label(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    match text.parse::<i64>() {
        Ok(0) => None,
        Ok(count) if count > 99 => Some("99+".to_string()),
        Ok(count) if count < 0 => Some("0".to_string()),
        Ok(count) => Some(count.to_string()),
        // Not reachable through `lx.app.setBadge` (Logic rejects it), but a
        // truncated label beats a panic if some other caller appears.
        Err(_) => Some(text.chars().take(MAX_GLYPHS).collect()),
    }
}

fn apply_overlay(hwnd: HWND, label: Option<&str>) -> Result<(), PlatformError> {
    // Tokio's blocking workers do not inherit the UI thread's COM apartment.
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
        .ok()
        .map_err(|error| PlatformError::Platform(format!("badge COM initialization: {error}")))?;
    let _com = ComApartment;
    let taskbar: ITaskbarList3 = unsafe {
        CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER)
    }
    .map_err(|error| PlatformError::Platform(format!("taskbar overlay unavailable: {error}")))?;
    unsafe { taskbar.HrInit() }
        .map_err(|error| PlatformError::Platform(format!("taskbar init failed: {error}")))?;

    let Some(label) = label else {
        unsafe { taskbar.SetOverlayIcon(hwnd, HICON::default(), PCWSTR::null()) }
            .map_err(|error| PlatformError::Platform(format!("clearing the badge: {error}")))?;
        return Ok(());
    };

    let icon = draw_badge_icon(label)?;
    let description = wide(label);
    let outcome = unsafe { taskbar.SetOverlayIcon(hwnd, icon, PCWSTR(description.as_ptr())) };
    // The shell copies the icon, so it is ours to release either way.
    let _ = unsafe { DestroyIcon(icon) };
    outcome.map_err(|error| PlatformError::Platform(format!("painting the badge: {error}")))
}

struct ComApartment;

impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

/// A filled circle with the count centred in it, on a 32×32 ARGB surface.
fn draw_badge_icon(label: &str) -> Result<HICON, PlatformError> {
    unsafe {
        let screen = GetDC(None);
        let dc = CreateCompatibleDC(Some(screen));
        let header = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: ICON_SIZE,
                // Top-down, so the premultiplied pixels below are written in
                // the order they are read.
                biHeight: -ICON_SIZE,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let bitmap = CreateDIBSection(Some(dc), &header, DIB_RGB_COLORS, &mut bits, None, 0)
            .map_err(|error| PlatformError::Platform(format!("badge bitmap: {error}")))?;
        if bits.is_null() {
            cleanup(dc, screen, Some(bitmap.into()));
            return Err(PlatformError::Platform("badge bitmap has no pixels".into()));
        }

        let previous = SelectObject(dc, bitmap.into());
        fill_circle(bits.cast::<u32>());
        draw_label(dc, label);

        let mask = windows::Win32::Graphics::Gdi::CreateBitmap(ICON_SIZE, ICON_SIZE, 1, 1, None);
        let info = ICONINFO {
            fIcon: BOOL(1),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: bitmap,
        };
        let icon = CreateIconIndirect(&info);
        SelectObject(dc, previous);
        let _ = DeleteObject(mask.into());
        cleanup(dc, screen, Some(bitmap.into()));
        icon.map_err(|error| PlatformError::Platform(format!("badge icon: {error}")))
    }
}

/// Premultiplied ARGB, written straight into the DIB: GDI text drawing leaves
/// alpha alone, so the circle has to carry it.
unsafe fn fill_circle(pixels: *mut u32) {
    const RED: (u32, u32, u32) = (0xE8, 0x1A, 0x1A);
    let radius = (ICON_SIZE / 2) as f32;
    for y in 0..ICON_SIZE {
        for x in 0..ICON_SIZE {
            let dx = x as f32 - radius + 0.5;
            let dy = y as f32 - radius + 0.5;
            let distance = (dx * dx + dy * dy).sqrt();
            // One pixel of feathering, so the circle is not stair-stepped.
            let coverage = (radius - distance).clamp(0.0, 1.0);
            let alpha = (coverage * 255.0) as u32;
            let premultiply = |channel: u32| (channel * alpha) / 255;
            let color = (alpha << 24)
                | (premultiply(RED.0) << 16)
                | (premultiply(RED.1) << 8)
                | premultiply(RED.2);
            unsafe { pixels.offset((y * ICON_SIZE + x) as isize).write(color) };
        }
    }
}

unsafe fn draw_label(dc: HDC, label: &str) {
    unsafe {
        let mut text = wide(label);
        let font = CreateFontIndirectW(&badge_font(dc, &text));
        // `DrawTextW` takes the glyphs without the terminator.
        let glyphs = text.len() - 1;
        let previous = SelectObject(dc, font.into());
        SetBkMode(dc, TRANSPARENT);
        // The circle is opaque where the glyphs land, so plain white text
        // keeps its alpha without a second compositing pass.
        SetTextColor(dc, windows::Win32::Foundation::COLORREF(0x00FF_FFFF));
        let mut bounds = windows::Win32::Foundation::RECT {
            left: 0,
            top: 0,
            right: ICON_SIZE,
            bottom: ICON_SIZE,
        };
        DrawTextW(
            dc,
            &mut text[..glyphs],
            &mut bounds,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
        SelectObject(dc, previous);
        let _ = DeleteObject(font.into());
    }
}

/// Shrinks until the label fits, so `99+` is not clipped by the circle.
unsafe fn badge_font(dc: HDC, text: &[u16]) -> LOGFONTW {
    let mut font = LOGFONTW {
        lfWeight: FW_BOLD.0 as i32,
        lfCharSet: DEFAULT_CHARSET,
        lfPitchAndFamily: FF_DONTCARE.0,
        ..Default::default()
    };
    let name: Vec<u16> = "Segoe UI".encode_utf16().collect();
    font.lfFaceName[..name.len()].copy_from_slice(&name);

    let glyphs = text.len().saturating_sub(1).max(1);
    for height in [-22, -18, -15, -12] {
        font.lfHeight = height;
        let measured = unsafe {
            let candidate = CreateFontIndirectW(&font);
            let previous = SelectObject(dc, candidate.into());
            let mut size = SIZE::default();
            let fits = GetTextExtentPoint32W(dc, &text[..glyphs], &mut size).as_bool()
                && size.cx <= ICON_SIZE - 6;
            SelectObject(dc, previous);
            let _ = DeleteObject(candidate.into());
            fits
        };
        if measured {
            break;
        }
    }
    font
}

unsafe fn cleanup(dc: HDC, screen: HDC, bitmap: Option<HGDIOBJ>) {
    unsafe {
        if let Some(bitmap) = bitmap {
            let _ = DeleteObject(bitmap);
        }
        let _ = DeleteDC(dc);
        ReleaseDC(None, screen);
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Every visible top-level window this process owns: the taskbar button
/// belongs to the window, not to the process, so a multi-window host badges
/// all of them.
fn top_level_windows() -> Vec<HWND> {
    let mut windows: Vec<HWND> = Vec::new();
    unsafe {
        let _ = EnumWindows(
            Some(collect_window),
            LPARAM(&mut windows as *mut Vec<HWND> as isize),
        );
    }
    windows
}

unsafe extern "system" fn collect_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == windows::Win32::System::Threading::GetCurrentProcessId()
            && IsWindowVisible(hwnd).as_bool()
        {
            let windows = &mut *(lparam.0 as *mut Vec<HWND>);
            windows.push(hwnd);
        }
    }
    BOOL(1)
}

#[cfg(test)]
mod tests {
    use super::badge_label;

    #[test]
    fn a_cleared_badge_has_no_label() {
        assert_eq!(badge_label(""), None);
        assert_eq!(badge_label("  "), None);
        assert_eq!(badge_label("0"), None);
    }

    #[test]
    fn a_count_over_two_digits_is_summarised() {
        assert_eq!(badge_label("7").as_deref(), Some("7"));
        assert_eq!(badge_label("99").as_deref(), Some("99"));
        assert_eq!(badge_label("100").as_deref(), Some("99+"));
        assert_eq!(badge_label("-3").as_deref(), Some("0"));
    }
}
