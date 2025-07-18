use rong::{JSContext, JSResult};
use std::sync::Arc;

use crate::app::DeviceInfo;
use crate::error::LxAppError;
use crate::fast_api;
use crate::miniapp::LxApp;

pub(crate) fn device_info(ctx: JSContext) -> JSResult<DeviceInfo> {
    let lxapp = ctx.get_user_data::<Arc<LxApp>>().unwrap();
    let device_info = lxapp.runtime.device_info();
    Ok(device_info)
}

fast_api!(GetDeviceInfo, DeviceInfo, |lxapp: Arc<LxApp>| -> Result<
    DeviceInfo,
    LxAppError,
> {
    Ok(lxapp.runtime.device_info())
});

pub fn init(ctx: &JSContext) -> JSResult<()> {
    // Register JS function to lx object
    let js_func = rong::JSFunc::new(ctx, device_info)?;
    super::register_js_api(ctx, "getDeviceInfo", js_func)?;

    // Register FastAPI handler
    super::register_fast_api("getDeviceInfo", Arc::new(GetDeviceInfo));

    Ok(())
}
