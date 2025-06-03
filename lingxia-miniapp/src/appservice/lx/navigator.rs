use rong::{JSContext, JSResult, RongJSError};
use std::sync::Arc;

use crate::miniapp::{MiniApp, MiniAppNavigator};

pub(crate) fn navigator_to_miniapp(ctx: JSContext, app: MiniAppNavigator) -> JSResult<()> {
    let miniapp = ctx.get_user_data::<Arc<MiniApp>>().unwrap();
    miniapp
        .navigator_to_miniapp(app)
        .map_err(|e| RongJSError::Error(format!("Failed to open miniapp: {}", e)))?;
    Ok(())
}
