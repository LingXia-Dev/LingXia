use rong::{JSContext, JSResult, RongJSError};
use std::sync::Arc;

use crate::miniapp::{LxApp, LxAppNavigator};

pub(crate) fn navigator_to_lxapp(ctx: JSContext, app: LxAppNavigator) -> JSResult<()> {
    let miniapp = ctx.get_user_data::<Arc<LxApp>>().unwrap();
    miniapp
        .navigator_to_lxapp(app)
        .map_err(|e| RongJSError::Error(format!("Failed to open miniapp: {}", e)))?;
    Ok(())
}
