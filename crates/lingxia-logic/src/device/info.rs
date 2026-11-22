use lingxia_platform::traits::device::Device;
use lingxia_platform::{DeviceInfo as PlatformDeviceInfo, ScreenInfo as PlatformScreenInfo};
use lxapp::LxApp;
use rong::{IntoJSObject, JSContext, JSResult};

/// What `lx.device.info` reports: the device and its OS.
#[derive(Debug, Clone, IntoJSObject)]
pub struct DeviceInfo {
    /// Manufacturer, e.g. `Apple`, `Xiaomi`.
    brand: String,
    /// Model identifier, e.g. `iPhone12,1`.
    model: String,
    /// Name the device is sold under, e.g. `iPhone 11`.
    #[js_name = "marketName"]
    market_name: String,
    /// Operating system family.
    #[js_name = "osName"]
    #[ts_type = "HostOs"]
    os_name: String,
    /// Operating system version, e.g. `18.2`.
    #[js_name = "osVersion"]
    os_version: String,
}

/// The screen's size in logical pixels and its density.
#[derive(Debug, Clone, IntoJSObject)]
pub struct ScreenInfo {
    /// Width in logical pixels.
    width: f64,
    /// Height in logical pixels.
    height: f64,
    /// Physical pixels per logical pixel.
    scale: f64,
}

impl From<PlatformDeviceInfo> for DeviceInfo {
    fn from(device_info: PlatformDeviceInfo) -> Self {
        DeviceInfo {
            brand: device_info.brand,
            model: device_info.model,
            market_name: device_info.market_name,
            os_name: lingxia_platform::os_id_for(&device_info.os_name).to_string(),
            os_version: device_info.os_version,
        }
    }
}

impl From<PlatformScreenInfo> for ScreenInfo {
    fn from(screen_info: PlatformScreenInfo) -> Self {
        ScreenInfo {
            width: screen_info.width,
            height: screen_info.height,
            scale: screen_info.scale,
        }
    }
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)?;
    register_device_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        const device: "DeviceApi" = crate::namespace::device(ctx)?;
    }
}

rong::js_api! {
    fn register_device_api(ctx) {
        namespace DeviceApi = crate::namespace::device(ctx)?;
        const info: "Readonly<DeviceInfo>" = device_info(ctx.clone())?;
        const orientation: "OrientationApi" = crate::namespace::orientation(ctx)?;
        fn screen(ts_return = "ScreenInfo") = screen_info;
    }
}

/// Read the device and OS facts this host reports.
fn device_info(ctx: JSContext) -> JSResult<DeviceInfo> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let device_info = lxapp.runtime.device_info();
    Ok(device_info.into())
}

/// Read the screen geometry and pixel ratio this host reports.
fn screen_info(ctx: JSContext) -> JSResult<ScreenInfo> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    Ok(lxapp.runtime.screen_info().into())
}
