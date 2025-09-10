use rong::{JSContext, JSResult};

mod modal;
mod router;
mod toast;

/// Initialize UI module
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    toast::init(ctx)?;
    modal::init(ctx)?;
    router::init(ctx)?;
    Ok(())
}
