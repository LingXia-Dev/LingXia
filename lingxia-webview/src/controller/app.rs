use crate::{App, MiniAppPlatform};
use miniapp::{MiniAppCmd, MiniAppError};

/// MiniApp message handler responsible for processing MiniApp commands from the UI thread
pub(crate) fn handle_miniapp_cmd(platform: &App, cmd: MiniAppCmd) -> Result<(), MiniAppError> {
    match cmd {
        MiniAppCmd::OpenMiniApp {
            appid,
            path,
            responder,
        } => {
            let result = platform.open_miniapp(&appid, &path);

            let send_result = responder.send(result.clone());
            if send_result.is_err() {
                return Err(MiniAppError::WebView("Failed to send response".to_string()));
            }

            // Propagate any error that occurred during processing
            result
        }
        MiniAppCmd::SwitchPage {
            appid,
            path,
            responder,
        } => {
            let result = platform.switch_page(&appid, &path);

            let send_result = responder.send(result.clone());
            if send_result.is_err() {
                return Err(MiniAppError::WebView("Failed to send response".to_string()));
            }

            // Propagate any error that occurred during processing
            result
        }
    }
}
