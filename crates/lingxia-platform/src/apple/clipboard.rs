use crate::error::PlatformError;
use crate::traits::clipboard::{
    ClipboardContents, ClipboardKind, ClipboardReadRequest, ClipboardService, ClipboardTypes,
    ClipboardWrite, parse_native_reply,
};

use super::Platform;
use super::ffi;

impl ClipboardService for Platform {
    async fn clipboard_write(&self, item: ClipboardWrite) -> Result<(), PlatformError> {
        let payload = match &item {
            ClipboardWrite::Text(text) => ffi::clipboard_write("text", text),
            ClipboardWrite::Image { path } => ffi::clipboard_write("image", path),
        };
        parse_native_reply(&payload)?.into_unit()
    }

    async fn clipboard_read(
        &self,
        request: ClipboardReadRequest,
    ) -> Result<ClipboardContents, PlatformError> {
        let kind = request.kind.map(ClipboardKind::as_str).unwrap_or("");
        let dest = request.image_output_path.as_deref().unwrap_or("");
        let payload = ffi::clipboard_read(kind, dest);
        parse_native_reply(&payload)?.into_contents()
    }

    async fn clipboard_clear(&self) -> Result<(), PlatformError> {
        parse_native_reply(&ffi::clipboard_clear())?.into_unit()
    }

    async fn clipboard_types(&self) -> Result<ClipboardTypes, PlatformError> {
        parse_native_reply(&ffi::clipboard_types())?.into_types()
    }
}
