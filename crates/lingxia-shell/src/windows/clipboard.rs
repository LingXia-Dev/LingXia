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
