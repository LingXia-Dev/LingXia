use rong::{JSContext, JSFunc, JSObject, JSResult};

mod navigator;

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    let lx = JSObject::new(ctx);
    ctx.global().set("lx", lx.clone())?;

    let navigator_miniapp = JSFunc::new(ctx, navigator::navigator_to_miniapp)?;
    lx.set("navigateToMiniProgram", navigator_miniapp)?;

    Ok(())
}
