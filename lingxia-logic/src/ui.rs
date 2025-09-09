use rong::{JSContext, JSResult};

mod modal;
mod toast;

/// Initialize UI module
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    toast::init(ctx)?;
    modal::init(ctx)?;
    Ok(())
}
