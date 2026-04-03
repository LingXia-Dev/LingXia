mod image;
mod picker;
mod preview;
mod save;
mod scan;
mod video;
mod video_file;

use rong::{JSContext, JSResult};

pub fn init(ctx: &JSContext) -> JSResult<()> {
    preview::init(ctx)?;
    save::init(ctx)?;
    image::init(ctx)?;
    video_file::init(ctx)?;
    picker::init(ctx)?;
    scan::init(ctx)?;
    video::init(ctx)?;
    Ok(())
}
