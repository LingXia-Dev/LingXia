mod actions;
mod info;
mod network;
mod wifi;

use rong::{JSContext, JSResult};

pub fn init(ctx: &JSContext) -> JSResult<()> {
    info::init(ctx)?;
    actions::init(ctx)?;
    network::init(ctx)?;
    wifi::init(ctx)?;
    Ok(())
}
