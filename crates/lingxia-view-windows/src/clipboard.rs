//! Text onto the system clipboard.
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
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
