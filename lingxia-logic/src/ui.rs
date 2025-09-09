use rong::{JSContext, JSResult};

mod toast;

/// Initialize UI module
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    toast::init(ctx)?;
    Ok(())
}