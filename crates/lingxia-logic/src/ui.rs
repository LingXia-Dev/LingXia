use rong::{JSContext, JSResult};

mod action_sheet;
mod appearance;
mod capsule;
mod modal;
mod more_actions;
mod navbar;
mod page_chrome_patch;
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
    appearance::init(ctx)?;
    capsule::init(ctx)?;
    navbar::init(ctx)?;
    tabbar::init(ctx)?;
    router::init(ctx)?;
    pull_to_refresh::init(ctx)?;
    more_actions::init(ctx)?;
    shell::init(ctx)?;
    tray::init(ctx)?;
    Ok(())
}
