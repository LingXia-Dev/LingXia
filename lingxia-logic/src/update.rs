use rong::{JSContext, JSFunc, JSObject, JSResult};

// Register Update-related JS bindings
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    // lx.getUpdateManager() -> returns JSUpdateManager instance
    fn get_update_manager(ctx: JSContext) -> JSResult<JSObject> {
        lingxia_lxapp::get_or_create_update_manager(&ctx)
    }
    let get_update_manager = JSFunc::new(ctx, get_update_manager)?;
    lingxia_lxapp::lx::register_js_api(ctx, "getUpdateManager", get_update_manager)?;
    Ok(())
}
