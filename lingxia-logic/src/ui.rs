use rong::{JSContext, JSResult};

mod modal;
mod navbar;
mod router;
mod toast;

/// Initialize UI module
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    toast::init(ctx)?;
    modal::init(ctx)?;
    navbar::init(ctx)?;
    router::init(ctx)?;
    Ok(())
}
