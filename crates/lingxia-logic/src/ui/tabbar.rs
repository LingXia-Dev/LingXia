use crate::i18n::js_internal_error;
use lingxia_platform::traits::ui::UIUpdate;
use lxapp::LxApp;
use rong::{FromJSObject, JSContext, JSResult};

/// Options for showing TabBar red dot
#[derive(FromJSObject)]
#[ts_skip]
struct ShowTabBarRedDotOptions {
    index: i32,
}

/// Options for hiding TabBar red dot
#[derive(FromJSObject)]
#[ts_skip]
struct HideTabBarRedDotOptions {
    index: i32,
}

/// Options for setting TabBar badge
#[derive(FromJSObject)]
struct SetTabBarBadgeOptions {
    index: i32,
    text: String,
}

/// Options for removing TabBar badge
#[derive(FromJSObject)]
struct RemoveTabBarBadgeOptions {
    index: i32,
}

/// Options for setting TabBar style
#[derive(FromJSObject)]
struct SetTabBarStyleOptions {
    color: Option<String>,
    #[js_name = "selectedColor"]
    selected_color: Option<String>,
    #[js_name = "backgroundColor"]
    background_color: Option<String>,
    #[js_name = "borderStyle"]
    border_style: Option<String>,
}

/// Options for setting TabBar item
#[derive(FromJSObject)]
struct SetTabBarItemOptions {
    index: i32,
    text: Option<String>,
    #[js_name = "iconPath"]
    icon_path: Option<String>,
    #[js_name = "selectedIconPath"]
    selected_icon_path: Option<String>,
}

/// Check if TabBar is currently visible
/// Show TabBar red dot
fn show_tabbar_red_dot(ctx: JSContext, options: ShowTabBarRedDotOptions) -> JSResult<bool> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    // Update TabBar item red dot state
    let updated = lxapp
        .with_tabbar_mut(|tabbar| tabbar.set_red_dot(options.index, true))
        .unwrap_or(false);

    if updated {
        // Notify UI to update only if TabBar is visible
        if let Err(e) = lxapp.runtime.update_tabbar_ui(lxapp.appid.clone()) {
            return Err(js_internal_error(format!(
                "Failed to update TabBar UI: {}",
                e
            )));
        }
        Ok(true)
    } else {
        Ok(updated)
    }
}

/// Hide TabBar red dot
fn hide_tabbar_red_dot(ctx: JSContext, options: HideTabBarRedDotOptions) -> JSResult<bool> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    // Update TabBar item red dot state
    let updated = lxapp
        .with_tabbar_mut(|tabbar| tabbar.set_red_dot(options.index, false))
        .unwrap_or(false);

    if updated {
        // Notify UI to update only if TabBar is visible
        if let Err(e) = lxapp.runtime.update_tabbar_ui(lxapp.appid.clone()) {
            return Err(js_internal_error(format!(
                "Failed to update TabBar UI: {}",
                e
            )));
        }
        Ok(true)
    } else {
        Ok(updated)
    }
}

/// Set TabBar badge
fn set_tabbar_badge(ctx: JSContext, options: SetTabBarBadgeOptions) -> JSResult<bool> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    // Update TabBar item badge
    let updated = lxapp
        .with_tabbar_mut(|tabbar| tabbar.set_badge(options.index, &options.text))
        .unwrap_or(false);

    if updated {
        // Notify UI to update only if TabBar is visible
        if let Err(e) = lxapp.runtime.update_tabbar_ui(lxapp.appid.clone()) {
            return Err(js_internal_error(format!(
                "Failed to update TabBar UI: {}",
                e
            )));
        }
        Ok(true)
    } else {
        Ok(updated)
    }
}

/// Remove TabBar badge
fn remove_tabbar_badge(ctx: JSContext, options: RemoveTabBarBadgeOptions) -> JSResult<bool> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    // Update TabBar item badge
    let updated = lxapp
        .with_tabbar_mut(|tabbar| tabbar.remove_badge(options.index))
        .unwrap_or(false);

    if updated {
        // Notify UI to update only if TabBar is visible
        if let Err(e) = lxapp.runtime.update_tabbar_ui(lxapp.appid.clone()) {
            return Err(js_internal_error(format!(
                "Failed to update TabBar UI: {}",
                e
            )));
        }
        Ok(true)
    } else {
        Ok(updated)
    }
}

/// Show TabBar
async fn show_tabbar(ctx: JSContext) -> JSResult<bool> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    // Update TabBar visibility
    let updated = lxapp
        .with_tabbar_mut(|tabbar| {
            tabbar.set_visible(true);
            tabbar.set_api_hidden(false);
            true
        })
        .unwrap_or(false);

    if updated {
        // Always update UI for show/hide operations
        if let Err(e) = lxapp
            .runtime
            .update_tabbar_ui_async(lxapp.appid.clone())
            .await
        {
            return Err(js_internal_error(format!(
                "Failed to update TabBar UI: {}",
                e
            )));
        }
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Hide TabBar
async fn hide_tabbar(ctx: JSContext) -> JSResult<bool> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    // Update TabBar visibility
    let updated = lxapp
        .with_tabbar_mut(|tabbar| {
            tabbar.set_visible(false);
            tabbar.set_api_hidden(true);
            true
        })
        .unwrap_or(false);

    if updated {
        // Always update UI for show/hide operations
        if let Err(e) = lxapp
            .runtime
            .update_tabbar_ui_async(lxapp.appid.clone())
            .await
        {
            return Err(js_internal_error(format!(
                "Failed to update TabBar UI: {}",
                e
            )));
        }
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Set TabBar style
fn set_tabbar_style(ctx: JSContext, options: SetTabBarStyleOptions) -> JSResult<bool> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    // Update TabBar style
    let updated = lxapp
        .with_tabbar_mut(|tabbar| {
            if let Some(color) = options.color {
                tabbar.set_color(&color);
            }
            if let Some(selected_color) = options.selected_color {
                tabbar.set_selected_color(&selected_color);
            }
            if let Some(background_color) = options.background_color {
                tabbar.set_background_color(&background_color);
            }
            if let Some(border_style) = options.border_style {
                tabbar.set_border_style(&border_style);
            }
            true
        })
        .unwrap_or(false);

    if updated {
        // Notify UI to update only if TabBar is visible
        if let Err(e) = lxapp.runtime.update_tabbar_ui(lxapp.appid.clone()) {
            return Err(js_internal_error(format!(
                "Failed to update TabBar UI: {}",
                e
            )));
        }
        Ok(true)
    } else {
        Ok(updated)
    }
}

/// Set TabBar item
fn set_tabbar_item(ctx: JSContext, options: SetTabBarItemOptions) -> JSResult<bool> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    // Update TabBar item
    let updated = lxapp
        .with_tabbar_mut(|tabbar| {
            let mut changed = false;
            if let Some(text) = options.text {
                tabbar.set_item_text(options.index, &text);
                changed = true;
            }
            if let Some(icon_path) = options.icon_path {
                tabbar.set_item_icon(options.index, &icon_path);
                changed = true;
            }
            if let Some(selected_icon_path) = options.selected_icon_path {
                tabbar.set_item_selected_icon(options.index, &selected_icon_path);
                changed = true;
            }
            changed
        })
        .unwrap_or(false);

    if updated {
        // Notify UI to update only if TabBar is visible
        if let Err(e) = lxapp.runtime.update_tabbar_ui(lxapp.appid.clone()) {
            return Err(js_internal_error(format!(
                "Failed to update TabBar UI: {}",
                e
            )));
        }
        Ok(true)
    } else {
        Ok(updated)
    }
}

/// Initialize TabBar module
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        fn showTabBarRedDot(ts_params = "options: TabBarRedDotOptions") = show_tabbar_red_dot;
        fn hideTabBarRedDot(ts_params = "options: TabBarRedDotOptions") = hide_tabbar_red_dot;
        fn setTabBarBadge(ts_params = "options: SetTabBarBadgeOptions") = set_tabbar_badge;
        fn removeTabBarBadge(ts_params = "options: RemoveTabBarBadgeOptions") = remove_tabbar_badge;
        fn showTabBar = show_tabbar;
        fn hideTabBar = hide_tabbar;
        fn setTabBarStyle(ts_params = "options: SetTabBarStyleOptions") = set_tabbar_style;
        fn setTabBarItem(ts_params = "options: SetTabBarItemOptions") = set_tabbar_item;
    }
}
