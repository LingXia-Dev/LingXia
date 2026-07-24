use crate::i18n::{js_internal_error, js_service_unavailable_error};
use lingxia_platform::traits::ui::UIUpdate;
use lxapp::LxApp;
use rong::{FromJSObject, JSContext, JSResult};
use std::sync::Arc;

fn update_current_navbar(
    ctx: JSContext,
    mutator: impl FnOnce(&Arc<LxApp>, &str) -> bool,
) -> JSResult<bool> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    let current_path = lxapp
        .peek_current_page()
        .ok_or_else(|| js_service_unavailable_error("No current page found"))?;

    let updated = mutator(&lxapp, &current_path);
    // Custom navigation still uses this state for the status bar.
    if updated && let Err(e) = lxapp.runtime.update_navbar_ui(lxapp.appid.clone()) {
        return Err(js_internal_error(format!(
            "Failed to update navbar UI: {}",
            e
        )));
    }
    Ok(updated)
}

/// Options for setNavigationBarTitle
#[derive(FromJSObject)]
struct SetNavigationBarTitleOptions {
    title: String,
}

/// Options for setNavigationBarColor
#[derive(FromJSObject)]
struct SetNavigationBarColorOptions {
    #[js_name = "frontColor"]
    front_color: String,
    #[js_name = "backgroundColor"]
    background_color: String,
}

/// Set navigation bar title
fn set_navigation_bar_title(
    ctx: JSContext,
    options: SetNavigationBarTitleOptions,
) -> JSResult<bool> {
    update_current_navbar(ctx, |lxapp, path| {
        lxapp
            .get_page(path)
            .and_then(|page| {
                page.get_navbar_state_mut(|navbar| navbar.set_title(options.title.clone()))
            })
            .is_some()
    })
}

/// Set navigation bar color
fn set_navigation_bar_color(
    ctx: JSContext,
    options: SetNavigationBarColorOptions,
) -> JSResult<bool> {
    update_current_navbar(ctx, |lxapp, path| {
        lxapp
            .get_page(path)
            .and_then(|page| {
                page.get_navbar_state_mut(|navbar| {
                    navbar.set_background_color(options.background_color.clone());

                    let style =
                        if options.front_color == "#000000" || options.front_color == "black" {
                            "black".to_string()
                        } else {
                            "white".to_string()
                        };
                    navbar.set_text_style(style);
                })
            })
            .is_some()
    })
}

/// Hide home button
fn hide_home_button(ctx: JSContext) -> JSResult<bool> {
    update_current_navbar(ctx, |lxapp, path| {
        lxapp
            .get_page(path)
            .and_then(|page| {
                page.get_navbar_state_mut(|navbar| navbar.set_home_button_visibility(false))
            })
            .is_some()
    })
}

/// Initialize NavigationBar module
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        fn setNavigationBarTitle(ts_params = "options: SetNavigationBarTitleOptions") = set_navigation_bar_title;
        fn setNavigationBarColor(ts_params = "options: SetNavigationBarColorOptions") = set_navigation_bar_color;
        fn hideHomeButton = hide_home_button;
    }
}
