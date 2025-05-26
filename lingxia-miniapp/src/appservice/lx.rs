use rong::{JSContext, JSFunc, JSObject, JSResult};

mod device;
mod navigator;

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    let lx = JSObject::new(ctx);
    ctx.global().set("lx", lx.clone())?;

    let navigator_miniapp = JSFunc::new(ctx, navigator::navigator_to_miniapp)?;
    lx.set("navigateToMiniProgram", navigator_miniapp)?;

    let device_info = JSFunc::new(ctx, device::derive_info)?;
    lx.set("getDeviceInfo", device_info)?;

    Ok(())
}
