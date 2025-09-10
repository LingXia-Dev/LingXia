use super::app::Platform;
use super::ffi;
use crate::error::PlatformError;
use crate::traits::{
    ModalOptions, ModalResult, ToastIcon, ToastOptions, ToastPosition, UserFeedback,
};

impl UserFeedback for Platform {
    fn show_toast(&self, options: ToastOptions) -> Result<(), PlatformError> {
        // Convert our ToastOptions to the FFI ToastOptions
        let ffi_options = ffi::ToastOptions {
            title: options.title,
            icon: convert_toast_icon(options.icon),
            image: options.image.unwrap_or_default(),
            duration: options.duration,
            mask: options.mask,
            position: convert_toast_position(options.position),
        };

        // Call the Swift FFI function
        ffi::show_toast(ffi_options);
        Ok(())
    }

    fn hide_toast(&self) -> Result<(), PlatformError> {
        // Call the Swift FFI function
        ffi::hide_toast();
        Ok(())
    }

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

/// Convert our ToastIcon to the FFI ToastIcon
fn convert_toast_icon(icon: ToastIcon) -> ffi::ToastIcon {
    match icon {
        ToastIcon::Success => ffi::ToastIcon::Success,
        ToastIcon::Error => ffi::ToastIcon::Error,
        ToastIcon::Loading => ffi::ToastIcon::Loading,
        ToastIcon::None => ffi::ToastIcon::None,
    }
}

/// Convert our ToastPosition to the FFI ToastPosition
fn convert_toast_position(position: ToastPosition) -> ffi::ToastPosition {
    match position {
        ToastPosition::Top => ffi::ToastPosition::Top,
        ToastPosition::Center => ffi::ToastPosition::Center,
        ToastPosition::Bottom => ffi::ToastPosition::Bottom,
    }
}
