use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::{ModalOptions, ToastIcon, ToastOptions, ToastPosition, UserFeedback};

impl UserFeedback for Platform {
    fn show_toast(&self, options: ToastOptions) -> Result<(), PlatformError> {
        let icon_str = convert_toast_icon_to_string(options.icon);
        let position_str = convert_toast_position_to_string(options.position);
        let image_str = options.image.unwrap_or_default();
        let duration_str = options.duration.to_string();
        let mask_str = options.mask.to_string();

        // Call ArkTS showToast function via TSFN
        lingxia_webview::tsfn::call_arkts(
            "showToast",
            &[
                &options.title,
                &icon_str,
                &image_str,
                &duration_str,
                &mask_str,
                &position_str,
            ],
        )
        .map_err(|e| PlatformError::Platform(format!("Failed to show toast: {}", e)))
    }

    fn hide_toast(&self) -> Result<(), PlatformError> {
        // Call ArkTS hideToast function via TSFN
        lingxia_webview::tsfn::call_arkts("hideToast", &[])
            .map_err(|e| PlatformError::Platform(format!("Failed to hide toast: {}", e)))
    }

    fn show_modal(&self, options: ModalOptions, callback_id: u64) -> Result<(), PlatformError> {
        // Convert ModalOptions to individual string parameters for TSFN call
        let title = &options.title;
        let content = &options.content;
        let show_cancel = if options.show_cancel { "true" } else { "false" };
        let cancel_text = &options.cancel_text;
        let confirm_text = &options.confirm_text;
        let confirm_color = options.confirm_color.as_deref().unwrap_or("");
        let callback_id_str = callback_id.to_string();

        // Call ArkTS showModal function via TSFN with individual parameters
        lingxia_webview::tsfn::call_arkts(
            "showModal",
            &[
                title,
                content,
                show_cancel,
                cancel_text,
                confirm_text,
                confirm_color,
                &callback_id_str,
            ],
        )
        .map_err(|e| PlatformError::Platform(format!("Failed to show modal: {}", e)))?;

        Ok(())
    }
}

/// Convert ToastIcon to string representation for HarmonyOS ArkTS
fn convert_toast_icon_to_string(icon: ToastIcon) -> String {
    match icon {
        ToastIcon::Success => "success".to_string(),
        ToastIcon::Error => "error".to_string(),
        ToastIcon::Loading => "loading".to_string(),
        ToastIcon::None => "none".to_string(),
    }
}

/// Convert ToastPosition to string representation for HarmonyOS ArkTS
fn convert_toast_position_to_string(position: ToastPosition) -> String {
    match position {
        ToastPosition::Top => "top".to_string(),
        ToastPosition::Center => "center".to_string(),
        ToastPosition::Bottom => "bottom".to_string(),
    }
}
