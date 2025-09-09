use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::{Modal, ModalOptions, ModalResult};

impl Modal for Platform {
    fn show_modal(&self, options: ModalOptions) -> Result<ModalResult, PlatformError> {
        // Convert ModalOptions to individual string parameters for TSFN call
        let title = &options.title;
        let content = &options.content;
        let show_cancel = if options.show_cancel { "true" } else { "false" };
        let cancel_text = &options.cancel_text;
        let confirm_text = &options.confirm_text;
        let editable = if options.editable { "true" } else { "false" };
        let placeholder_text = &options.placeholder_text;
        let confirm_color = options.confirm_color.as_deref().unwrap_or("");

        // Call ArkTS showModal function via TSFN with individual parameters
        lingxia_webview::tsfn::call_arkts(
            "showModal",
            &[
                title,
                content,
                show_cancel,
                cancel_text,
                confirm_text,
                editable,
                placeholder_text,
                confirm_color,
            ],
        )
        .map_err(|e| PlatformError::Platform(format!("Failed to show modal: {}", e)))?;

        // Return placeholder result - async implementation will be added later
        Ok(ModalResult {
            content: String::new(),
            cancel: true,
            confirm: false,
        })
    }
}
