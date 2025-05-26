use rong::{FromJSObj, JSContext, JSResult, RongJSError};
use std::sync::Arc;

use crate::miniapp::MiniApp;
use crate::open_miniapp;

#[derive(FromJSObj)]
pub struct MiniAppNavigator {
    #[rename = "appId"]
    appid: String,
    path: String,
}

pub(crate) fn navigator_to_miniapp(ctx: JSContext, app: MiniAppNavigator) -> JSResult<()> {
    let miniapp = ctx.get_user_data::<Arc<MiniApp>>().unwrap();
    if miniapp.appid != app.appid {
        open_miniapp(&*miniapp.controller, &app.appid, &app.path)
            .map_err(|e| RongJSError::Error(format!("Failed to open miniapp: {}", e)))?;
    }
    Ok(())
}
