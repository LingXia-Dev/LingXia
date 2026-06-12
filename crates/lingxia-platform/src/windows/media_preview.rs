//! Media-preview routing for Windows.
//!
//! `lx.previewMedia` sessions are owned by the UI layer (a dedicated
//! preview window in `lingxia-windows`); the platform layer only routes
//! the request and the cancel through a registered host — the same
//! inversion as the video-command and app-exit handlers.

use std::sync::{Arc, OnceLock};

use crate::traits::media_interaction::PreviewMediaRequest;

/// Opens a preview session; errors are human-readable.
pub type WindowsMediaPreviewOpen =
    Arc<dyn Fn(PreviewMediaRequest) -> Result<(), String> + Send + Sync>;
/// Cancels the session identified by its completion callback id.
pub type WindowsMediaPreviewCancel = Arc<dyn Fn(u64) -> Result<(), String> + Send + Sync>;

static HOST: OnceLock<(WindowsMediaPreviewOpen, WindowsMediaPreviewCancel)> = OnceLock::new();

/// Registers the preview host. Called once by the UI layer at install.
pub fn register_windows_media_preview_host(
    open: WindowsMediaPreviewOpen,
    cancel: WindowsMediaPreviewCancel,
) {
    if HOST.set((open, cancel)).is_err() {
        log::warn!("a Windows media preview host is already registered; ignoring");
    }
}

pub(crate) fn open_preview(request: PreviewMediaRequest) -> Result<(), String> {
    let (open, _) = HOST
        .get()
        .ok_or_else(|| "no media preview host is registered on Windows".to_string())?;
    open(request)
}

pub(crate) fn cancel_preview(callback_id: u64) -> Result<(), String> {
    let (_, cancel) = HOST
        .get()
        .ok_or_else(|| "no media preview host is registered on Windows".to_string())?;
    cancel(callback_id)
}
