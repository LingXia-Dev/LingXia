use rong::{JSContext, JSFunc, JSObject, JSResult};

mod env;
mod fastapi;
mod navigator;
mod system;

pub(crate) use fastapi::get_fast_api;

/// Register a JS function to the lx object
pub fn register_js_api(ctx: &JSContext, name: &str, func: JSFunc) -> JSResult<()> {
    let lx = ctx.global().get::<_, JSObject>("lx")?;
    lx.set(name, func)?;
    Ok(())
}

/// Register a FastAPI handler
pub use fastapi::{FastApiHandler, register_fast_api};

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    let lx = JSObject::new(ctx);
    ctx.global().set("lx", lx.clone())?;

    lx.set("env", env::init(ctx))?;

    // Register navigator
    let navigator_lxapp = JSFunc::new(ctx, navigator::navigator_to_lxapp)?;
    register_js_api(ctx, "navigateToMiniProgram", navigator_lxapp)?;

    system::init(ctx)?;

    Ok(())
}
