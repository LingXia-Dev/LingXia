use rong::{JSContext, JSFunc, JSObject, JSResult};

pub mod extension;

pub use extension::{LxLogicExtension, register_logic_extension};

/// Register the global `lx` object in the JavaScript context.
/// This function must be called before using any other `lx` APIs.
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    let lx = JSObject::new(ctx);
    ctx.global().set("lx", lx)?;
    Ok(())
}

/// Register a JS function to the lx object.
/// `lx::init` must be called before this function.
pub fn register_js_api(ctx: &JSContext, name: &str, func: JSFunc) -> JSResult<()> {
    let lx = ctx.global().get::<_, JSObject>("lx")?;
    lx.set(name, func)?;
    Ok(())
}
