use crate::App;
use crate::webview::MiniAppCmd;
use miniapp::MiniAppError;

/// MiniApp message handler responsible for processing MiniApp commands from the UI thread
pub(crate) fn handle_miniapp_cmd(app: &App, cmd: MiniAppCmd) -> Result<(), MiniAppError> {
    match cmd {
        MiniAppCmd::OpenMiniApp {
            appid,
            path,
            responder,
        } => {
            let result = app.open_miniapp(&appid, &path);
            let _ = responder.send(result);
        }
        MiniAppCmd::CloseMiniApp { appid, responder } => {
            let result = app.close_miniapp(&appid);
            let _ = responder.send(result);
        }
        MiniAppCmd::SwitchPage {
            appid,
            path,
            responder,
        } => {
            let result = app.switch_page(&appid, &path);
            let _ = responder.send(result);
        }
    }
    Ok(())
}

