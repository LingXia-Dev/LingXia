use rong::{JSContext, JSObject, JSResult};

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    let obj = JSObject::new(ctx);
    // Use abstract URIs to hide absolute system paths
    obj.set("USER_DATA_PATH", "lx://userdata")?;
    obj.set("USER_CACHE_PATH", "lx://usercache")?;

    let lx = ctx.global().get::<_, JSObject>("lx")?;
    lx.set("env", obj)?;
    Ok(())
}
