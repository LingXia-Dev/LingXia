use lingxia_lxapp::LxApp;
use rong::{JSContext, JSObject, JSResult};

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(ctx)?;

    let obj = JSObject::new(ctx);
    obj.set("USER_DATA_PATH", lxapp.user_data_dir.to_str().unwrap())?;
    obj.set("USER_CACHE_PATH", lxapp.user_cache_dir.to_str().unwrap())?;

    let lx = ctx.global().get::<_, JSObject>("lx")?;
    lx.set("env", obj)?;
    Ok(())
}
