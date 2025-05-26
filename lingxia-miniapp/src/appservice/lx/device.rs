use rong::{JSContext, JSResult, RongJSError};
use std::sync::{Mutex, Weak};

use crate::appservice::MiniAppServiceManager;
use crate::app::DeviceInfo;

pub(crate) fn derive_info(ctx: JSContext) -> JSResult<DeviceInfo> {
    if let Some(manager_weak) = ctx.get_user_data::<Weak<Mutex<MiniAppServiceManager>>>() {
        if let Some(manager_arc) = manager_weak.upgrade() {
            if let Ok(manager) = manager_arc.lock() {
                let controller = manager.get_controller();
                let device_info = controller.device_info();
                
                return Ok(device_info);
            }
        }
    }

    Err(RongJSError::Error("Controller not available".to_string()))
}
