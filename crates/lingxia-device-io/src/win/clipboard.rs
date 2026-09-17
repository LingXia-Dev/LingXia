//! Clipboard text get/set/clear and paste (Ctrl+V). Text-only in v1; the
//! reliable path for entering CJK/emoji/long text (`key type` bypasses the IME).
//!
//! `OpenClipboard` fails with ACCESS_DENIED while another window (often
//! WebView2 or clipboard history) holds the clipboard. The product
//! `lx.clipboard` path already spins; this driver must too.

use crate::error::{Error, Result};
use crate::model::{Ack, Clipboard, Modifier};
use std::time::Duration;
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::CF_UNICODETEXT;

const CLIPBOARD_RETRIES: u32 = 16;

fn cf_unicode() -> u32 {
    CF_UNICODETEXT.0 as u32
}

fn with_clipboard<T>(f: impl FnOnce() -> Result<T>) -> Result<T> {
    unsafe {
        let mut last = None;
        for attempt in 0..CLIPBOARD_RETRIES {
            match OpenClipboard(None) {
                Ok(()) => {
                    let result = f();
                    let _ = CloseClipboard();
                    return result;
                }
                Err(e) => last = Some(e),
            }
            if attempt + 1 < CLIPBOARD_RETRIES {
                std::thread::sleep(Duration::from_millis(8 + u64::from(attempt) * 4));
            }
        }
        Err(Error::Failed(format!(
            "OpenClipboard failed: {}",
            last.map(|e| e.to_string())
                .unwrap_or_else(|| "unknown".into())
        )))
    }
}

pub fn get() -> Result<Clipboard> {
    with_clipboard(|| unsafe {
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
                    text = Some(String::from_utf16_lossy(std::slice::from_raw_parts(
                        ptr, len,
                    )));
                    let _ = GlobalUnlock(hglobal);
                }
            }
        }
        Ok(Clipboard {
            available_formats: formats,
            text,
        })
    })
}

pub fn set(text: &str) -> Result<Ack> {
    let utf16: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes = utf16.len() * std::mem::size_of::<u16>();
    let hmem: HGLOBAL = unsafe {
        GlobalAlloc(GMEM_MOVEABLE, bytes)
            .map_err(|e| Error::Failed(format!("GlobalAlloc failed: {e}")))?
    };
    unsafe {
        let dst = GlobalLock(hmem) as *mut u16;
        if dst.is_null() {
            let _ = GlobalFree(Some(hmem));
            return Err(Error::Failed("GlobalLock failed".into()));
        }
        std::ptr::copy_nonoverlapping(utf16.as_ptr(), dst, utf16.len());
        let _ = GlobalUnlock(hmem);
    }

    let result = with_clipboard(|| unsafe {
        let _ = EmptyClipboard();
        // SetClipboardData transfers ownership of hmem to the clipboard only on
        // success; free it ourselves on failure.
        SetClipboardData(cf_unicode(), Some(HANDLE(hmem.0)))
            .map_err(|e| Error::Failed(format!("SetClipboardData failed: {e}")))?;
        Ok(Ack::new("clipboard.set"))
    });
    if result.is_err() {
        unsafe {
            let _ = GlobalFree(Some(hmem));
        }
    }
    result
}

pub fn clear() -> Result<Ack> {
    with_clipboard(|| unsafe {
        let _ = EmptyClipboard();
        Ok(Ack::new("clipboard.clear"))
    })
}

/// Paste into the focused control via Ctrl+V.
pub fn paste() -> Result<Ack> {
    super::input::key_press("v", &[Modifier::Ctrl], None)?;
    Ok(Ack::new("clipboard.paste"))
}
