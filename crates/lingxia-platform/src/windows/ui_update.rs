use std::sync::{Arc, Mutex};

use super::Platform;
use crate::error::PlatformError;
use crate::traits::ui::UIUpdate;

type WindowsUiUpdateHandler = Arc<dyn Fn(String) + Send + Sync>;
static WINDOWS_UI_UPDATE_HANDLER: Mutex<Option<WindowsUiUpdateHandler>> = Mutex::new(None);

pub fn set_windows_ui_update_handler(handler: WindowsUiUpdateHandler) {
    if let Ok(mut slot) = WINDOWS_UI_UPDATE_HANDLER.lock() {
        *slot = Some(handler);
    }
}

fn invoke_windows_ui_update_handler(appid: String) {
    let handler = WINDOWS_UI_UPDATE_HANDLER
        .lock()
        .ok()
        .and_then(|slot| slot.clone());
    if let Some(handler) = handler {
        handler(appid);
    }
}

impl UIUpdate for Platform {
    fn update_navbar_ui(&self, appid: String) -> Result<(), PlatformError> {
        invoke_windows_ui_update_handler(appid);
        Ok(())
    }

    fn update_tabbar_ui(&self, appid: String) -> Result<(), PlatformError> {
        invoke_windows_ui_update_handler(appid);
        Ok(())
    }
}
