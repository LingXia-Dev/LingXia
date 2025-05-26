use rong::{JSContext, JSResult, RongJSError};
use std::sync::Arc;

use crate::app::DeviceInfo;
use crate::miniapp::MiniApp;

pub(crate) fn derive_info(ctx: JSContext) -> JSResult<DeviceInfo> {
    if let Some(miniapp) = ctx.get_user_data::<Arc<MiniApp>>() {
        let device_info = miniapp.controller.device_info();
        return Ok(device_info);
    }

    Err(RongJSError::Error(
        "MiniApp not available in context".to_string(),
    ))
}
