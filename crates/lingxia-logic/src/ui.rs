use rong::{JSContext, JSResult};

mod action_sheet;
mod capsule;
mod modal;
mod navbar;
mod pull_to_refresh;
mod router;
mod shell;
mod tabbar;
mod toast;
mod tray;

pub(crate) use action_sheet::present_action_sheet;

/// Initialize UI module
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    toast::init(ctx)?;
    modal::init(ctx)?;
    action_sheet::init(ctx)?;
    navbar::init(ctx)?;
    tabbar::init(ctx)?;
    router::init(ctx)?;
    pull_to_refresh::init(ctx)?;
    capsule::init(ctx)?;
    shell::init(ctx)?;
    tray::init(ctx)?;
    Ok(())
}
