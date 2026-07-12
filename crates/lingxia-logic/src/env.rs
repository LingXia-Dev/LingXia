use rong::{JSContext, JSObject, JSResult};

fn env_namespace(ctx: &JSContext) -> JSResult<JSObject> {
    let lx = ctx.global().get::<_, JSObject>("lx")?;
    if let Ok(env) = lx.get::<_, JSObject>("env") {
        return Ok(env);
    }
    let obj = JSObject::new(ctx);
    lx.set("env", obj.clone())?;
    Ok(obj)
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_env_api(ctx)?;
    register_env_property(ctx)
}

rong::js_api! {
    fn register_env_api(ctx) {
        namespace LxEnv = env_namespace(ctx)?;
        const USER_DATA_PATH: "'lx://userdata'" = "lx://userdata";
        const USER_CACHE_PATH: "'lx://usercache'" = "lx://usercache";
    }
}

rong::js_api! {
    fn register_env_property(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        const env: "LxEnv" = env_namespace(ctx)?;
    }
}
