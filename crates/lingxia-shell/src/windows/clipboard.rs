//! Minimal Win32 clipboard access for shell chrome features
//! (e.g. terminal right-click paste).

use windows::Win32::Foundation::{HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{CloseClipboard, GetClipboardData, OpenClipboard};
use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};

/// `CF_UNICODETEXT`; the `windows` crate gates the constant behind an
/// unrelated feature, and the value is contractual.
const CF_UNICODETEXT: u32 = 13;

/// Returns the clipboard's current Unicode text, or `None` when the
/// clipboard is unavailable, empty, or holds no text.
pub fn clipboard_text() -> Option<String> {
    unsafe {
        // Retried once: another process may hold the clipboard briefly.
        if OpenClipboard(None).is_err() {
            std::thread::sleep(std::time::Duration::from_millis(10));
            OpenClipboard(None).ok()?;
        }
        let text = GetClipboardData(CF_UNICODETEXT)
            .ok()
            .and_then(|handle| read_global_wide(handle));
        let _ = CloseClipboard();
        text
    }
}

/// Replaces the clipboard content with `text` (CF_UNICODETEXT). Returns
/// `false` when the clipboard is unavailable or the allocation fails.
pub fn set_clipboard_text(text: &str) -> bool {
    use windows::Win32::System::DataExchange::{EmptyClipboard, SetClipboardData};
    use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc};

    let mut wide: Vec<u16> = text.encode_utf16().collect();
    wide.push(0);
    unsafe {
        if OpenClipboard(None).is_err() {
            std::thread::sleep(std::time::Duration::from_millis(10));
            if OpenClipboard(None).is_err() {
                return false;
            }
        }
        let _ = EmptyClipboard();
        let bytes = wide.len() * size_of::<u16>();
        let Ok(global) = GlobalAlloc(GMEM_MOVEABLE, bytes) else {
            let _ = CloseClipboard();
            return false;
        };
        let data = GlobalLock(global) as *mut u16;
        if data.is_null() {
            let _ = CloseClipboard();
            return false;
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr(), data, wide.len());
        let _ = GlobalUnlock(global);
        // On success the clipboard owns the allocation.
        let stored = SetClipboardData(CF_UNICODETEXT, Some(HANDLE(global.0))).is_ok();
        let _ = CloseClipboard();
        stored
    }
}

unsafe fn read_global_wide(handle: HANDLE) -> Option<String> {
    let global = HGLOBAL(handle.0);
    let data = unsafe { GlobalLock(global) } as *const u16;
    if data.is_null() {
        return None;
    }
    let mut len = 0usize;
    while unsafe { *data.add(len) } != 0 {
        len += 1;
    }
    let text = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(data, len) });
    let _ = unsafe { GlobalUnlock(global) };
    (!text.is_empty()).then_some(text)
}
