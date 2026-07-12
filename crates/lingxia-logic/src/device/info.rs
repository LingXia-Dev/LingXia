use lingxia_platform::traits::device::Device;
use lingxia_platform::{DeviceInfo, ScreenInfo};
use lxapp::LxApp;
use rong::{IntoJSObject, JSContext, JSResult};

#[derive(Debug, Clone, IntoJSObject)]
pub struct DevInfoObj {
    brand: String,
    model: String,
    #[js_name = "marketName"]
    market_name: String,
    #[js_name = "osName"]
    os_name: String,
    #[js_name = "osVersion"]
    os_version: String,
}

#[derive(Debug, Clone, IntoJSObject)]
pub struct ScreenInfoObj {
    width: f64,
    height: f64,
    scale: f64,
}

impl From<DeviceInfo> for DevInfoObj {
    fn from(device_info: DeviceInfo) -> Self {
        DevInfoObj {
            brand: device_info.brand,
            model: device_info.model,
            market_name: device_info.market_name,
            os_name: device_info.os_name,
            os_version: device_info.os_version,
        }
    }
}

impl From<ScreenInfo> for ScreenInfoObj {
    fn from(screen_info: ScreenInfo) -> Self {
        ScreenInfoObj {
            width: screen_info.width,
            height: screen_info.height,
            scale: screen_info.scale,
        }
    }
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        fn getDeviceInfo(ts_return = "DeviceInfo") = device_info;
        fn getScreenInfo(ts_return = "ScreenInfo") = screen_info;
    }
}

fn device_info(ctx: JSContext) -> JSResult<DevInfoObj> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let device_info = lxapp.runtime.device_info();
    Ok(device_info.into())
}

fn screen_info(ctx: JSContext) -> JSResult<ScreenInfoObj> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    Ok(lxapp.runtime.screen_info().into())
}
