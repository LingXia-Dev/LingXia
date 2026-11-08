//! Text on the system clipboard.
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{
    GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
};
use windows::Win32::System::Ole::CF_UNICODETEXT;

pub(crate) fn set_text(text: &str) -> Result<(), String> {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: the clipboard owns the global once SetClipboardData succeeds; otherwise it is
    // freed here. The copy stays inside the locked allocation of `wide.len()` units.
    unsafe {
        OpenClipboard(None).map_err(|e| format!("open clipboard: {e}"))?;
        let result = (|| {
            EmptyClipboard().map_err(|e| e.to_string())?;
            let global = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2).map_err(|e| e.to_string())?;
            let target = GlobalLock(global).cast::<u16>();
            if target.is_null() {
                let _ = GlobalFree(Some(global));
                return Err("lock clipboard memory".to_string());
            }
            std::ptr::copy_nonoverlapping(wide.as_ptr(), target, wide.len());
            let _ = GlobalUnlock(global);
            if let Err(error) = SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(global.0))) {
                let _ = GlobalFree(Some(HGLOBAL(global.0)));
                return Err(error.to_string());
            }
            Ok(())
        })();
        let _ = CloseClipboard();
        result
    }
}

pub(crate) fn text() -> Option<String> {
    // SAFETY: reads a locked clipboard global up to its terminating NUL, then releases both.
    unsafe {
        OpenClipboard(None).ok()?;
        let result = (|| {
            let handle = GetClipboardData(CF_UNICODETEXT.0 as u32).ok()?;
            let global = HGLOBAL(handle.0);
            let source = GlobalLock(global).cast::<u16>();
            if source.is_null() {
                return None;
            }
            let units = GlobalSize(global) / 2;
            let slice = std::slice::from_raw_parts(source, units);
            let end = slice.iter().position(|&unit| unit == 0).unwrap_or(units);
            let text = String::from_utf16_lossy(&slice[..end]);
            let _ = GlobalUnlock(global);
            Some(text)
        })();
        let _ = CloseClipboard();
        result
    }
}
