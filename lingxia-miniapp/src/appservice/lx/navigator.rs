use rong::{JSContext, JSResult, RongJSError};
use std::sync::Arc;

use crate::miniapp::{LxApp, LxAppNavigator};

pub(crate) fn navigator_to_lxapp(ctx: JSContext, app: LxAppNavigator) -> JSResult<()> {
    let lxapp = ctx.get_user_data::<Arc<LxApp>>().unwrap();
    lxapp
        .navigator_to_lxapp(app)
        .map_err(|e| RongJSError::Error(format!("Failed to open lxapp: {}", e)))?;
    Ok(())
}
