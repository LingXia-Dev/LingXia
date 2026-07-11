//! Native context menu helper for shell chrome features
//! (e.g. the terminal panel's right-click menu).
#![cfg_attr(not(feature = "terminal-runtime"), allow(dead_code))]

use std::sync::Arc;

use crate::{WindowsDesignIcon, design_icons::design_icon_argb_premultiplied};
use lingxia_windows_contract::post_to_window_thread;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateDIBSection, DIB_RGB_COLORS, DeleteObject, HBITMAP,
    HGDIOBJ,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, MENUITEMINFOW, MF_CHECKED, MF_DISABLED, MF_GRAYED,
    MF_SEPARATOR, MF_STRING, MIIM_BITMAP, SetForegroundWindow, SetMenuItemInfoW, TPM_NONOTIFY,
    TPM_RETURNCMD, TPM_TOPALIGN, TrackPopupMenu,
};
use windows::core::PCWSTR;

/// Shows a popup menu at `screen` (screen coordinates) owned by `window`
/// (an HWND handle as returned through the webview layer). Marshalled onto
/// the window's UI thread; `on_select` receives the zero-based index of the
/// chosen item, and is not called when the menu is dismissed.
/// Shows a popup menu; `checked[i] == true` draws item `i` with a checkmark
/// (a shorter `checked` slice leaves later items unchecked).
pub fn show_context_menu_checked(
    window: isize,
    screen: (i32, i32),
    items: Vec<String>,
    checked: Vec<bool>,
    on_select: Arc<dyn Fn(usize) + Send + Sync>,
) {
    let entries = items
        .into_iter()
        .enumerate()
        .map(|(index, label)| ContextMenuEntry {
            separator: label.is_empty(),
            label,
            enabled: true,
            checked: checked.get(index).copied().unwrap_or(false),
            icon: None,
        })
        .collect();
    show_context_menu_entries(window, screen, entries, on_select);
}

#[derive(Debug, Clone)]
pub struct ContextMenuEntry {
    pub label: String,
    pub enabled: bool,
    pub checked: bool,
    pub separator: bool,
    pub icon: Option<WindowsDesignIcon>,
}

impl ContextMenuEntry {
    pub fn item(label: String, enabled: bool, icon: WindowsDesignIcon) -> Self {
        Self {
            label,
            enabled,
            checked: false,
            separator: false,
            icon: Some(icon),
        }
    }

    pub fn separator() -> Self {
        Self {
            label: String::new(),
            enabled: false,
            checked: false,
            separator: true,
            icon: None,
        }
    }
}

pub fn show_context_menu_entries(
    window: isize,
    screen: (i32, i32),
    entries: Vec<ContextMenuEntry>,
    on_select: Arc<dyn Fn(usize) + Send + Sync>,
) {
    if entries.is_empty() {
        return;
    }
    post_to_window_thread(
        window,
        Box::new(move || {
            let hwnd = HWND(window as *mut core::ffi::c_void);
            unsafe {
                let Ok(menu) = CreatePopupMenu() else {
                    return;
                };
                let mut icon_bitmaps = Vec::new();
                for (index, entry) in entries.iter().enumerate() {
                    // Empty items render as separator lines; they keep their
                    // slot in the index space but are not selectable.
                    if entry.separator {
                        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
                        continue;
                    }
                    let mut text: Vec<u16> = entry.label.encode_utf16().collect();
                    text.push(0);
                    let mut flags = MF_STRING;
                    if entry.checked {
                        flags |= MF_CHECKED;
                    }
                    if !entry.enabled {
                        flags |= MF_DISABLED | MF_GRAYED;
                    }
                    // Command ids are 1-based: TrackPopupMenu returns 0 for
                    // "dismissed without a choice".
                    let _ = AppendMenuW(menu, flags, index + 1, PCWSTR(text.as_ptr()));
                    if let Some(bitmap) = entry.icon.and_then(create_menu_icon_bitmap) {
                        let info = MENUITEMINFOW {
                            cbSize: std::mem::size_of::<MENUITEMINFOW>() as u32,
                            fMask: MIIM_BITMAP,
                            hbmpItem: bitmap,
                            ..Default::default()
                        };
                        let _ = SetMenuItemInfoW(menu, (index + 1) as u32, false, &info);
                        icon_bitmaps.push(bitmap);
                    }
                }
                // Required for the menu to dismiss when clicking elsewhere.
                let _ = SetForegroundWindow(hwnd);
                let chosen = TrackPopupMenu(
                    menu,
                    TPM_RETURNCMD | TPM_NONOTIFY | TPM_TOPALIGN,
                    screen.0,
                    screen.1,
                    None,
                    hwnd,
                    None,
                );
                let _ = DestroyMenu(menu);
                for bitmap in icon_bitmaps {
                    let _ = DeleteObject(HGDIOBJ(bitmap.0));
                }
                let chosen = chosen.0 as usize;
                if chosen > 0 {
                    on_select(chosen - 1);
                }
            }
        }),
    );
}

fn create_menu_icon_bitmap(icon: WindowsDesignIcon) -> Option<HBITMAP> {
    const SIZE: i32 = 16;
    let pixels = design_icon_argb_premultiplied(icon, SIZE as u32, Some(0x333333))?;
    unsafe {
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: SIZE,
                biHeight: -SIZE,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        let bitmap = CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
        if bits.is_null() {
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            return None;
        }
        std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits.cast::<u32>(), pixels.len());
        Some(bitmap)
    }
}
