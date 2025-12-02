use lingxia_platform::UIUpdate;
use lxapp::{LxApp, lx};
use rong::{FromJSObj, JSContext, JSFunc, JSResult, RongJSError};
use std::sync::Arc;

/// Check if NavigationBar is currently visible for the current page
fn is_navbar_visible(lxapp: &Arc<LxApp>, path: &str) -> bool {
    lxapp.get_navbar_state(path).show_navbar
}

/// Options for setNavigationBarTitle
#[derive(FromJSObj)]
struct SetNavigationBarTitleOptions {
    title: String,
}

/// Options for setNavigationBarColor
#[derive(FromJSObj)]
struct SetNavigationBarColorOptions {
    front_color: String,
    background_color: String,
}

/// Set navigation bar title
fn set_navigation_bar_title(
    ctx: JSContext,
    options: SetNavigationBarTitleOptions,
) -> JSResult<bool> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    // Get current page path
    let current_path = lxapp
        .peek_current_page()
        .ok_or_else(|| RongJSError::Error("No current page found".to_string()))?;

    // Update navbar state with new title
    let updated = lxapp.with_navbar_mut(&current_path, |navbar| {
        navbar.set_title(options.title);
    });

    if updated && is_navbar_visible(&lxapp, &current_path) {
        // Notify UI to update only if navbar is visible
        if let Err(e) = lxapp.runtime.update_navbar_ui(lxapp.appid.clone()) {
            eprintln!("Failed to update navbar UI: {}", e);
            return Ok(false);
        }
        Ok(true)
    } else {
        Ok(updated)
    }
}

/// Set navigation bar color
fn set_navigation_bar_color(
    ctx: JSContext,
    options: SetNavigationBarColorOptions,
) -> JSResult<bool> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    // Get current page path
    let current_path = lxapp
        .peek_current_page()
        .ok_or_else(|| RongJSError::Error("No current page found".to_string()))?;

    // Update navbar state with new colors
    let updated = lxapp.with_navbar_mut(&current_path, |navbar| {
        navbar.set_background_color(options.background_color);

        // Convert front_color to text_style
        let style = if options.front_color == "#000000" || options.front_color == "black" {
            "black".to_string()
        } else {
            "white".to_string()
        };
        navbar.set_text_style(style);
    });

    if updated && is_navbar_visible(&lxapp, &current_path) {
        // Notify UI to update only if navbar is visible
        if let Err(e) = lxapp.runtime.update_navbar_ui(lxapp.appid.clone()) {
            eprintln!("Failed to update navbar UI: {}", e);
            return Ok(false);
        }
        Ok(true)
    } else {
        Ok(updated)
    }
}

/// Hide home button
fn hide_home_button(ctx: JSContext) -> JSResult<bool> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    // Get current page path
    let current_path = lxapp
        .peek_current_page()
        .ok_or_else(|| RongJSError::Error("No current page found".to_string()))?;

    // Update navbar state to hide home button
    let updated = lxapp.with_navbar_mut(&current_path, |navbar| {
        navbar.set_home_button_visibility(false);
    });

    if updated && is_navbar_visible(&lxapp, &current_path) {
        // Notify UI to update only if navbar is visible
        if let Err(e) = lxapp.runtime.update_navbar_ui(lxapp.appid.clone()) {
            eprintln!("Failed to update navbar UI: {}", e);
            return Ok(false);
        }
        Ok(true)
    } else {
        Ok(updated)
    }
}

/// Initialize NavigationBar module
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    let set_title_func = JSFunc::new(ctx, set_navigation_bar_title)?;
    lx::register_js_api(ctx, "setNavigationBarTitle", set_title_func)?;

    let set_color_func = JSFunc::new(ctx, set_navigation_bar_color)?;
    lx::register_js_api(ctx, "setNavigationBarColor", set_color_func)?;

    let hide_home_func = JSFunc::new(ctx, hide_home_button)?;
    lx::register_js_api(ctx, "hideHomeButton", hide_home_func)?;

    Ok(())
}
