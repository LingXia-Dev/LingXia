mod image;
mod picker;
mod preview;
mod save;
mod scan;

use rong::{JSContext, JSResult};

pub fn init(ctx: &JSContext) -> JSResult<()> {
    preview::init(ctx)?;
    save::init(ctx)?;
    image::init(ctx)?;
    picker::init(ctx)?;
    scan::init(ctx)?;
    Ok(())
}
