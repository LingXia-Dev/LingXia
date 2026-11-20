use rong::{JSContext, JSObject, JSResult};

mod action_sheet;
mod modal;
mod more_actions;
mod navbar;
mod page;
mod page_chrome_patch;
mod pull_to_refresh;
mod router;
mod shell;
mod tabbar;
mod toast;
mod tray;

pub(crate) use action_sheet::present_action_sheet;

/// Feedback shown over the page: toasts, dialogs and action sheets.
fn ui_namespace(ctx: &JSContext) -> JSResult<JSObject> {
    let lx = ctx.global().get::<_, JSObject>("lx")?;
    match lx.get::<_, JSObject>("ui") {
        Ok(namespace) => Ok(namespace),
        Err(_) => {
            let namespace = JSObject::new(ctx);
            lx.set("ui", namespace.clone())?;
            Ok(namespace)
        }
    }
}

/// `tone: 'danger'` marks a destructive choice; the host draws it in red.
fn tone_color(tone: Option<&str>) -> JSResult<Option<String>> {
    match tone {
        None | Some("default") => Ok(None),
        Some("danger") => Ok(Some("#FF3B30".to_string())),
        Some(other) => Err(crate::i18n::js_invalid_parameter_error(format!(
            "tone must be 'default' or 'danger', got '{other}'"
        ))),
    }
}

rong::js_api! {
    fn register_ui_property(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        const ui: "UiApi" = ui_namespace(ctx)?;
    }
}

/// Initialize UI module
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_ui_property(ctx)?;
    toast::init(ctx)?;
    modal::init(ctx)?;
    action_sheet::init(ctx)?;
    page::init(ctx)?;
    tabbar::init(ctx)?;
    router::init(ctx)?;
    more_actions::init(ctx)?;
    shell::init(ctx)?;
    tray::init(ctx)?;
    Ok(())
}
