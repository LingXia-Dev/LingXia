use super::app::Platform;
use super::ffi;
use crate::error::PlatformError;
use crate::traits::{Modal, ModalOptions, ModalResult};

impl Modal for Platform {
    fn show_modal(&self, options: ModalOptions) -> Result<ModalResult, PlatformError> {
        // Convert our ModalOptions to the FFI ModalOptions
        let ffi_options = ffi::ModalOptions {
            title: options.title,
            content: options.content,
            show_cancel: options.show_cancel,
            cancel_text: options.cancel_text,
            cancel_color: options.cancel_color.unwrap_or_default(),
            confirm_text: options.confirm_text,
            confirm_color: options.confirm_color.unwrap_or_default(),
            editable: options.editable,
            placeholder_text: options.placeholder_text,
        };

        // Call the Swift FFI function
        let ffi_result = ffi::show_modal(ffi_options);

        // Convert FFI result to our ModalResult
        Ok(ModalResult {
            confirm: ffi_result.confirm,
            cancel: ffi_result.cancel,
            content: ffi_result.content,
        })
    }
}
