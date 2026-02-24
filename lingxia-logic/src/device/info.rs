use lingxia_platform::traits::device::Device;
use lingxia_platform::{DeviceInfo, ScreenInfo};
use lxapp::LxApp;
use lxapp::lx;
use rong::{IntoJSObj, JSContext, JSFunc, JSResult};

#[derive(Debug, Clone, IntoJSObj)]
pub struct DevInfoObj {
    brand: String,
    model: String,
    #[rename = "marketName"]
    market_name: String,
    #[rename = "osName"]
    os_name: String,
    #[rename = "osVersion"]
    os_version: String,
}

#[derive(Debug, Clone, IntoJSObj)]
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

pub fn init(ctx: &JSContext) -> JSResult<()> {
    let device_info_func = JSFunc::new(ctx, device_info)?;
    lx::register_js_api(ctx, "getDeviceInfo", device_info_func)?;

    let screen_info_func = JSFunc::new(ctx, screen_info)?;
    lx::register_js_api(ctx, "getScreenInfo", screen_info_func)?;
    Ok(())
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
