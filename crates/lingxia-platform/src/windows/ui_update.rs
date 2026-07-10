use std::sync::{Arc, Mutex};

use super::Platform;
use crate::error::PlatformError;
use crate::traits::ui::UIUpdate;

type WindowsUiUpdateHandler = Arc<dyn Fn(String) + Send + Sync>;
static WINDOWS_UI_UPDATE_HANDLER: Mutex<Option<WindowsUiUpdateHandler>> = Mutex::new(None);

/// Async UI update: the handler receives the appid and a completion closure
/// it must call (with success) once the UI has actually applied the change.
type WindowsUiUpdateAsyncHandler = Arc<dyn Fn(String, Box<dyn FnOnce(bool) + Send>) + Send + Sync>;
static WINDOWS_UI_UPDATE_ASYNC_HANDLER: Mutex<Option<WindowsUiUpdateAsyncHandler>> =
    Mutex::new(None);

pub fn set_windows_ui_update_handler(handler: WindowsUiUpdateHandler) {
    if let Ok(mut slot) = WINDOWS_UI_UPDATE_HANDLER.lock() {
        *slot = Some(handler);
    }
}

pub fn set_windows_ui_update_async_handler(handler: WindowsUiUpdateAsyncHandler) {
    if let Ok(mut slot) = WINDOWS_UI_UPDATE_ASYNC_HANDLER.lock() {
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

pub fn sync_windows_ui(appid: &str) {
    invoke_windows_ui_update_handler(appid.to_string());
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

    async fn update_tabbar_ui_async(&self, appid: String) -> Result<(), PlatformError> {
        let handler = WINDOWS_UI_UPDATE_ASYNC_HANDLER
            .lock()
            .ok()
            .and_then(|slot| slot.clone());
        let Some(handler) = handler else {
            // No async handler registered (bare host apps): the sync handler
            // applies the update inline, so resolving afterwards is accurate.
            return self.update_tabbar_ui(appid);
        };
        crate::rt::native_call_ui(|callback_id| {
            handler(
                appid.clone(),
                Box::new(move |ok| {
                    let result = if ok { Ok("{}".to_string()) } else { Err(1000) };
                    lingxia_messaging::invoke_callback(callback_id, result);
                }),
            );
            Ok(())
        })
        .await
    }
}
