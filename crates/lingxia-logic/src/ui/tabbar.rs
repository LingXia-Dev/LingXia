use crate::i18n::js_internal_error;
use lingxia_platform::traits::ui::UIUpdate;
use lxapp::{LxApp, lx};
use rong::{FromJSObj, JSContext, JSFunc, JSResult};
use std::sync::Arc;

/// Options for showing TabBar red dot
#[derive(FromJSObj)]
struct ShowTabBarRedDotOptions {
    index: i32,
}

/// Options for hiding TabBar red dot
#[derive(FromJSObj)]
struct HideTabBarRedDotOptions {
    index: i32,
}

/// Options for setting TabBar badge
#[derive(FromJSObj)]
struct SetTabBarBadgeOptions {
    index: i32,
    text: String,
}

/// Options for removing TabBar badge
#[derive(FromJSObj)]
struct RemoveTabBarBadgeOptions {
    index: i32,
}

/// Options for setting TabBar style
#[derive(FromJSObj)]
struct SetTabBarStyleOptions {
    color: Option<String>,
    #[rename = "selectedColor"]
    selected_color: Option<String>,
    #[rename = "backgroundColor"]
    background_color: Option<String>,
    #[rename = "borderStyle"]
    border_style: Option<String>,
}

/// Options for setting TabBar item
#[derive(FromJSObj)]
struct SetTabBarItemOptions {
    index: i32,
    text: Option<String>,
    #[rename = "iconPath"]
    icon_path: Option<String>,
    #[rename = "selectedIconPath"]
    selected_icon_path: Option<String>,
}

/// Check if TabBar is currently visible
fn is_tabbar_visible(lxapp: &Arc<LxApp>) -> bool {
    lxapp
        .get_tabbar()
        .map(|tabbar| tabbar.is_visible)
        .unwrap_or(false)
}

/// Show TabBar red dot
fn show_tabbar_red_dot(ctx: JSContext, options: ShowTabBarRedDotOptions) -> JSResult<bool> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    // Update TabBar item red dot state
    let updated = lxapp
        .with_tabbar_mut(|tabbar| tabbar.set_red_dot(options.index, true))
        .unwrap_or(false);

    if updated && is_tabbar_visible(&lxapp) {
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

    if updated && is_tabbar_visible(&lxapp) {
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

    if updated && is_tabbar_visible(&lxapp) {
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

    if updated && is_tabbar_visible(&lxapp) {
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

    if updated && is_tabbar_visible(&lxapp) {
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

    if updated && is_tabbar_visible(&lxapp) {
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
    let show_red_dot_func = JSFunc::new(ctx, show_tabbar_red_dot)?;
    lx::register_js_api(ctx, "showTabBarRedDot", show_red_dot_func)?;

    let hide_red_dot_func = JSFunc::new(ctx, hide_tabbar_red_dot)?;
    lx::register_js_api(ctx, "hideTabBarRedDot", hide_red_dot_func)?;

    let set_badge_func = JSFunc::new(ctx, set_tabbar_badge)?;
    lx::register_js_api(ctx, "setTabBarBadge", set_badge_func)?;

    let remove_badge_func = JSFunc::new(ctx, remove_tabbar_badge)?;
    lx::register_js_api(ctx, "removeTabBarBadge", remove_badge_func)?;

    let show_tabbar_func = JSFunc::new(ctx, show_tabbar)?;
    lx::register_js_api(ctx, "showTabBar", show_tabbar_func)?;

    let hide_tabbar_func = JSFunc::new(ctx, hide_tabbar)?;
    lx::register_js_api(ctx, "hideTabBar", hide_tabbar_func)?;

    let set_style_func = JSFunc::new(ctx, set_tabbar_style)?;
    lx::register_js_api(ctx, "setTabBarStyle", set_style_func)?;

    let set_item_func = JSFunc::new(ctx, set_tabbar_item)?;
    lx::register_js_api(ctx, "setTabBarItem", set_item_func)?;

    Ok(())
}
