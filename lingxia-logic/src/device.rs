mod actions;
mod info;
mod wifi;

use rong::{JSContext, JSResult};

pub fn init(ctx: &JSContext) -> JSResult<()> {
    info::init(ctx)?;
    actions::init(ctx)?;
    wifi::init(ctx)?;
    Ok(())
}
