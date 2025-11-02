use super::app::Platform;
use crate::error::PlatformError;
use crate::traits::{PopupPosition, PopupPresenter, PopupRequest};

impl PopupPresenter for Platform {
    fn show_popup(&self, request: PopupRequest) -> Result<(), PlatformError> {
        let width_ratio = if request.width_ratio.is_nan() {
            "NaN".to_string()
        } else {
            request.width_ratio.to_string()
        };
        let height_ratio = if request.height_ratio.is_nan() {
            "NaN".to_string()
        } else {
            request.height_ratio.to_string()
        };
        let position = match request.position {
            PopupPosition::Center => "center",
            PopupPosition::Bottom => "bottom",
            PopupPosition::Left => "left",
            PopupPosition::Right => "right",
        };

        let args = [
            request.app_id.as_str(),
            request.path.as_str(),
            width_ratio.as_str(),
            height_ratio.as_str(),
            position,
        ];

        lingxia_webview::tsfn::call_arkts("showPopup", &args)
            .map_err(|e| PlatformError::Platform(format!("Failed to show popup: {}", e)))
    }

    fn hide_popup(&self, app_id: &str) -> Result<(), PlatformError> {
        lingxia_webview::tsfn::call_arkts("hidePopup", &[app_id])
            .map_err(|e| PlatformError::Platform(format!("Failed to hide popup: {}", e)))
    }
}
