use lingxia_lxapp::lx::fast_api;
use lingxia_lxapp::{LxApp, LxAppError, lx};
use lingxia_platform::{AppRuntime, DeviceInfo};
use rong::{IntoJSObj, JSContext, JSResult};
use serde::Serialize;
use std::sync::Arc;

/// Device information
#[derive(Debug, Clone, IntoJSObj, Serialize)]
pub struct DevInfoObj {
    pub brand: String,
    pub model: String,
    pub system: String, // Operating system version
}

impl From<DeviceInfo> for DevInfoObj {
    fn from(device_info: DeviceInfo) -> Self {
        DevInfoObj {
            brand: device_info.brand,
            model: device_info.model,
            system: device_info.system,
        }
    }
}

pub(crate) fn device_info(ctx: JSContext) -> JSResult<DevInfoObj> {
    let lxapp = ctx.get_user_data::<Arc<LxApp>>().unwrap();
    let device_info = lxapp.runtime.device_info();
    Ok(device_info.into())
}

fast_api!(GetDeviceInfo, DevInfoObj, |lxapp: Arc<LxApp>| -> Result<
    DevInfoObj,
    LxAppError,
> {
    Ok(lxapp.runtime.device_info().into())
});

pub fn init(ctx: &JSContext) -> JSResult<()> {
    // Register JS function to lx object
    let js_func = rong::JSFunc::new(ctx, device_info)?;
    lx::register_js_api(ctx, "getDeviceInfo", js_func)?;

    // Register FastAPI handler
    lx::register_fast_api("getDeviceInfo", Arc::new(GetDeviceInfo));

    Ok(())
}
