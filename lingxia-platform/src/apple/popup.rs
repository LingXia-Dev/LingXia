use super::app::Platform;
use super::ffi::{PopupPositionBridge, hide_popup, show_popup};
use crate::error::PlatformError;
use crate::traits::{PopupPosition, PopupPresenter, PopupRequest};

impl PopupPresenter for Platform {
    fn show_popup(&self, request: PopupRequest) -> Result<(), PlatformError> {
        let position = match request.position {
            PopupPosition::Center => PopupPositionBridge::Center,
            PopupPosition::Bottom => PopupPositionBridge::Bottom,
            PopupPosition::Left => PopupPositionBridge::Left,
            PopupPosition::Right => PopupPositionBridge::Right,
        };

        if show_popup(
            &request.app_id,
            &request.path,
            request.width_ratio,
            request.height_ratio,
            position,
        ) {
            Ok(())
        } else {
            Err(PlatformError::Platform(
                "Failed to show popup on Apple platform".to_string(),
            ))
        }
    }

    fn hide_popup(&self, app_id: &str) -> Result<(), PlatformError> {
        if hide_popup(app_id) {
            Ok(())
        } else {
            Err(PlatformError::Platform(
                "Failed to hide popup on Apple platform".to_string(),
            ))
        }
    }
}
