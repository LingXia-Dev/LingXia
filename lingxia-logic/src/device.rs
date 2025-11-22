mod actions;
mod info;

use rong::{JSContext, JSResult};

pub fn init(ctx: &JSContext) -> JSResult<()> {
    info::init(ctx)?;
    actions::init(ctx)?;
    Ok(())
}
