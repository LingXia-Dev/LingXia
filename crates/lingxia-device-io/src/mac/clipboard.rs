//! Clipboard text get/set/clear and paste (Cmd+V) via `NSPasteboard`. Text-only
//! in v1, mirroring the Windows backend.

use crate::error::{Error, Result};
use crate::model::{Ack, Clipboard, Modifier};
use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
use objc2_foundation::NSString;

pub fn get() -> Result<Clipboard> {
    let pb = NSPasteboard::generalPasteboard();
    let mut formats = Vec::new();
    let text = pb
        .stringForType(unsafe { NSPasteboardTypeString })
        .map(|s| s.to_string());
    if text.is_some() {
        formats.push("text/plain".to_string());
    }
    Ok(Clipboard {
        available_formats: formats,
        text,
    })
}

pub fn set(text: &str) -> Result<Ack> {
    let pb = NSPasteboard::generalPasteboard();
    pb.clearContents();
    let ok = pb.setString_forType(&NSString::from_str(text), unsafe { NSPasteboardTypeString });
    if !ok {
        return Err(Error::Failed("could not write to the clipboard".into()));
    }
    Ok(Ack::new("clipboard.set"))
}

pub fn clear() -> Result<Ack> {
    NSPasteboard::generalPasteboard().clearContents();
    Ok(Ack::new("clipboard.clear"))
}

/// Paste into the focused control via Cmd+V.
pub fn paste() -> Result<Ack> {
    super::input::key_press("v", &[Modifier::Meta], None)?;
    Ok(Ack::new("clipboard.paste"))
}
