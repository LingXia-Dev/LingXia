use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::{
    ModalOptions, PickerType, ToastIcon, ToastOptions, ToastPosition, UserFeedback,
};

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

    fn show_action_sheet(
        &self,
        options: Vec<String>,
        cancel_text: String,
        item_color: String,
        callback_id: u64,
    ) -> Result<(), PlatformError> {
        // Convert options to JSON string
        let options_json = serde_json::to_string(&options)
            .map_err(|e| PlatformError::Platform(format!("Failed to serialize options: {}", e)))?;

        // Call ArkTS showActionSheet function via TSFN with individual parameters
        lingxia_webview::tsfn::call_arkts(
            "showActionSheet",
            &[
                &options_json,
                &cancel_text,
                &item_color,
                &callback_id.to_string(),
            ],
        )
        .map_err(|e| PlatformError::Platform(format!("Failed to show action sheet: {}", e)))?;

        Ok(())
    }

    fn show_picker(
        &self,
        picker_type: PickerType,
        cancel_text: String,
        cancel_button_color: String,
        cancel_text_color: String,
        confirm_text: String,
        confirm_button_color: String,
        confirm_text_color: String,
        callback_id: u64,
    ) -> Result<(), PlatformError> {
        // Convert picker type to columns JSON string
        let columns_json = match picker_type {
            PickerType::SingleColumn { items } => {
                serde_json::to_string(&vec![items]).map_err(|e| {
                    PlatformError::Platform(format!("Failed to serialize single column: {}", e))
                })?
            }
            PickerType::DualColumn {
                first_column,
                second_column,
            } => serde_json::to_string(&vec![first_column, second_column]).map_err(|e| {
                PlatformError::Platform(format!("Failed to serialize dual columns: {}", e))
            })?,
            PickerType::DualColumnCascading {
                first_column,
                cascading_data,
            } => {
                // For cascading, send first column and cascading data as JSON
                let cascading_structure = serde_json::json!([first_column, cascading_data]);
                serde_json::to_string(&cascading_structure).map_err(|e| {
                    PlatformError::Platform(format!("Failed to serialize cascading columns: {}", e))
                })?
            }
        };

        let callback_id_str = callback_id.to_string();

        // Call ArkTS showPicker function via TSFN with individual parameters
        lingxia_webview::tsfn::call_arkts(
            "showPicker",
            &[
                &columns_json,
                &cancel_text,
                &cancel_button_color,
                &cancel_text_color,
                &confirm_text,
                &confirm_button_color,
                &confirm_text_color,
                &callback_id_str,
            ],
        )
        .map_err(|e| PlatformError::Platform(format!("Failed to show picker: {}", e)))?;

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
