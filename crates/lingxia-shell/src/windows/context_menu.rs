//! Native context menu helper for shell chrome features
//! (e.g. the terminal panel's right-click menu).

use std::sync::Arc;

use lingxia_platform::windows::webview_host::post_to_window_thread;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, MF_STRING, SetForegroundWindow, TPM_NONOTIFY,
    TPM_RETURNCMD, TPM_TOPALIGN, TrackPopupMenu,
};
use windows::core::PCWSTR;

/// Shows a popup menu at `screen` (screen coordinates) owned by `window`
/// (an HWND handle as returned through the webview layer). Marshalled onto
/// the window's UI thread; `on_select` receives the zero-based index of the
/// chosen item, and is not called when the menu is dismissed.
pub fn show_context_menu(
    window: isize,
    screen: (i32, i32),
    items: Vec<String>,
    on_select: Arc<dyn Fn(usize) + Send + Sync>,
) {
    if items.is_empty() {
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
                for (index, item) in items.iter().enumerate() {
                    let mut text: Vec<u16> = item.encode_utf16().collect();
                    text.push(0);
                    // Command ids are 1-based: TrackPopupMenu returns 0 for
                    // "dismissed without a choice".
                    let _ = AppendMenuW(menu, MF_STRING, index + 1, PCWSTR(text.as_ptr()));
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
                let chosen = chosen.0 as usize;
                if chosen > 0 {
                    on_select(chosen - 1);
                }
            }
        }),
    );
}
