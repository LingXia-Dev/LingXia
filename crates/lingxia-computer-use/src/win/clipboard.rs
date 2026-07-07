//! Clipboard text get/set/clear and paste (Ctrl+V). Text-only in v1; the
//! reliable path for entering CJK/emoji/long text (`key type` bypasses the IME).

use crate::error::{Error, Result};
use crate::model::{Ack, Clipboard, Modifier};
use windows::Win32::Foundation::{HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Ole::CF_UNICODETEXT;

fn cf_unicode() -> u32 {
    CF_UNICODETEXT.0 as u32
}

pub fn get() -> Result<Clipboard> {
    unsafe {
        OpenClipboard(None).map_err(|e| Error::Failed(format!("OpenClipboard failed: {e}")))?;
        let mut formats = Vec::new();
        let mut text = None;
        if IsClipboardFormatAvailable(cf_unicode()).is_ok() {
            formats.push("text/plain".to_string());
            if let Ok(handle) = GetClipboardData(cf_unicode()) {
                let hglobal = HGLOBAL(handle.0);
                let ptr = GlobalLock(hglobal) as *const u16;
                if !ptr.is_null() {
                    let mut len = 0usize;
                    while *ptr.add(len) != 0 {
                        len += 1;
                    }
                    text = Some(String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len)));
                    let _ = GlobalUnlock(hglobal);
                }
            }
        }
        let _ = CloseClipboard();
        Ok(Clipboard {
            available_formats: formats,
            text,
        })
    }
}

pub fn set(text: &str) -> Result<Ack> {
    let utf16: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes = utf16.len() * std::mem::size_of::<u16>();
    unsafe {
        let hmem: HGLOBAL = GlobalAlloc(GMEM_MOVEABLE, bytes)
            .map_err(|e| Error::Failed(format!("GlobalAlloc failed: {e}")))?;
        let dst = GlobalLock(hmem) as *mut u16;
        if dst.is_null() {
            return Err(Error::Failed("GlobalLock failed".into()));
        }
        std::ptr::copy_nonoverlapping(utf16.as_ptr(), dst, utf16.len());
        let _ = GlobalUnlock(hmem);

        OpenClipboard(None).map_err(|e| Error::Failed(format!("OpenClipboard failed: {e}")))?;
        let _ = EmptyClipboard();
        // On success the clipboard takes ownership of hmem.
        let set = SetClipboardData(cf_unicode(), Some(HANDLE(hmem.0)));
        let _ = CloseClipboard();
        set.map_err(|e| Error::Failed(format!("SetClipboardData failed: {e}")))?;
    }
    Ok(Ack::new("clipboard.set"))
}

pub fn clear() -> Result<Ack> {
    unsafe {
        OpenClipboard(None).map_err(|e| Error::Failed(format!("OpenClipboard failed: {e}")))?;
        let _ = EmptyClipboard();
        let _ = CloseClipboard();
    }
    Ok(Ack::new("clipboard.clear"))
}

/// Paste into the focused control via Ctrl+V.
pub fn paste() -> Result<Ack> {
    super::input::key_press("v", &[Modifier::Ctrl])?;
    Ok(Ack::new("clipboard.paste"))
}
