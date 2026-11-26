use super::app::Platform;
use super::ffi;
use crate::traits::ui::{ModalOptions, ToastIcon, ToastOptions, ToastPosition, UserFeedback};
use lingxia_error::Error;

impl UserFeedback for Platform {
    fn show_toast(&self, options: ToastOptions) -> Result<(), Error> {
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

    fn dismiss_dialog(&self) -> Result<(), Error> {
        // macOS dialogs are View-drawn; Logic closes them with `ui.dismissDialog`.
        #[cfg(target_os = "macos")]
        return Err(Error::not_supported("dismiss_dialog".to_string()));
        #[cfg(not(target_os = "macos"))]
        {
            ffi::dismiss_dialog();
            Ok(())
        }
    }

    fn hide_toast(&self) -> Result<(), Error> {
        // Call the Swift FFI function
        ffi::hide_toast();
        Ok(())
    }

    async fn show_modal(&self, options: ModalOptions) -> Result<String, Error> {
        crate::rt::native_call(|callback_id| {
            // Convert our ModalOptions to the FFI ModalOptions
            let ffi_options = ffi::ModalOptions {
                title: options.title,
                content: options.content,
                show_cancel: options.show_cancel,
                cancel_text: options.cancel_text,
                cancel_color: options.cancel_color.unwrap_or_default(),
                confirm_text: options.confirm_text,
                confirm_color: options.confirm_color.unwrap_or_default(),
            };

            // Call the Swift FFI function with callback ID
            ffi::show_modal(ffi_options, callback_id);
            Ok(())
        })
        .await
    }

    async fn show_action_sheet(
        &self,
        options: Vec<String>,
        cancel_text: String,
        item_color: String,
    ) -> Result<String, Error> {
        crate::rt::native_call(|callback_id| {
            // Convert our options to the FFI ActionSheetOptions
            let ffi_options = ffi::ActionSheetOptions {
                options,
                cancel_text,
                item_color,
            };

            // Call the Swift FFI function with callback ID
            ffi::show_action_sheet(ffi_options, callback_id);
            Ok(())
        })
        .await
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
