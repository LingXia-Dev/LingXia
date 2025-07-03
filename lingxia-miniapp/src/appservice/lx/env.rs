use crate::miniapp::MiniApp;
use rong::{JSContext, JSObject, JSResult};
use std::sync::Arc;

pub fn init(ctx: &JSContext) -> JSResult<JSObject> {
    let miniapp = ctx.get_user_data::<Arc<MiniApp>>().unwrap();

    let obj = JSObject::new(ctx);
    obj.set("USER_DATA_PATH", miniapp.user_data_dir.to_str().unwrap())?;
    obj.set("USER_CACHE_PATH", miniapp.user_cache_dir.to_str().unwrap())?;
    Ok(obj)
}
