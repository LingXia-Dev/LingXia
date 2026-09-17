use crate::error::PlatformError;
use crate::traits::clipboard::{
    ClipboardContents, ClipboardKind, ClipboardReadRequest, ClipboardService, ClipboardTypes,
    ClipboardWrite, parse_native_reply,
};

use super::Platform;

fn call_clipboard(
    method: &str,
    args: &[&str],
) -> impl std::future::Future<Output = Result<String, PlatformError>> {
    let method = method.to_string();
    let args: Vec<String> = args.iter().map(|value| value.to_string()).collect();
    async move {
        let payload = crate::rt::native_call(|callback_id| {
            let callback_id = callback_id.to_string();
            let mut forwarded: Vec<&str> = args.iter().map(String::as_str).collect();
            forwarded.push(&callback_id);
            lingxia_webview::platform::harmony::tsfn::call_arkts(&method, &forwarded)
                .map_err(|e| PlatformError::Platform(format!("Failed to call clipboard: {e}")))
        })
        .await?;
        Ok(payload)
    }
}

impl ClipboardService for Platform {
    async fn clipboard_write(&self, item: ClipboardWrite) -> Result<(), PlatformError> {
        let payload = match &item {
            ClipboardWrite::Text(text) => call_clipboard("clipboardWrite", &["text", text]).await?,
            ClipboardWrite::Image { path } => {
                call_clipboard("clipboardWrite", &["image", path]).await?
            }
        };
        parse_native_reply(&payload)?.into_unit()
    }

    async fn clipboard_read(
        &self,
        request: ClipboardReadRequest,
    ) -> Result<ClipboardContents, PlatformError> {
        let kind = request.kind.map(ClipboardKind::as_str).unwrap_or("");
        let dest = request.image_output_path.as_deref().unwrap_or("");
        let payload = call_clipboard("clipboardRead", &[kind, dest]).await?;
        parse_native_reply(&payload)?.into_contents()
    }

    async fn clipboard_clear(&self) -> Result<(), PlatformError> {
        parse_native_reply(&call_clipboard("clipboardClear", &[]).await?)?.into_unit()
    }

    async fn clipboard_types(&self) -> Result<ClipboardTypes, PlatformError> {
        parse_native_reply(&call_clipboard("clipboardTypes", &[]).await?)?.into_types()
    }
}
