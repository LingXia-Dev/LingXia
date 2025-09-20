use rong::{JSContext, JSResult};

mod action_sheet;
mod modal;
mod navbar;
mod picker;
mod router;
mod tabbar;
mod toast;

/// Initialize UI module
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    toast::init(ctx)?;
    modal::init(ctx)?;
    action_sheet::init(ctx)?;
    navbar::init(ctx)?;
    tabbar::init(ctx)?;
    router::init(ctx)?;
    picker::init(ctx)?;
    Ok(())
}
