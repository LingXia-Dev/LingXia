use rong::{JSContext, JSResult};
use std::sync::Arc;

use crate::app::DeviceInfo;
use crate::miniapp::MiniApp;

pub(crate) fn device_info(ctx: JSContext) -> JSResult<DeviceInfo> {
    let miniapp = ctx.get_user_data::<Arc<MiniApp>>().unwrap();
    let device_info = miniapp.controller.device_info();
    Ok(device_info)
}
