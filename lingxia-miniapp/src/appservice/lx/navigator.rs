use rong::{FromJSObj, JSContext, JSResult, RongJSError};
use std::sync::{Mutex, Weak};

use crate::appservice::MiniAppServiceManager;
use crate::open_miniapp;

#[derive(FromJSObj)]
pub struct MiniAppNavigator {
    #[rename = "appId"]
    appid: String,
    path: String,
}

pub(crate) fn navigator_to_miniapp(ctx: JSContext, app: MiniAppNavigator) -> JSResult<()> {
    if let Some(manager_weak) = ctx.get_user_data::<Weak<Mutex<MiniAppServiceManager>>>() {
        if let Some(manager_arc) = manager_weak.upgrade() {
            if let Ok(manager) = manager_arc.lock() {
                let controller = manager.get_controller();
                open_miniapp(&*controller, &app.appid, &app.path)
                    .map_err(|e| RongJSError::Error(format!("Failed to open miniapp: {}", e)))?;
                return Ok(());
            }
        }
    }

    Err(RongJSError::Error("Controller not available".to_string()))
}
