use lingxia_lxapp::{LxApp, LxAppMode, LxAppStartupOptions, lx};
use rong::{FromJSObj, JSContext, JSFunc, JSResult, RongJSError};
use std::sync::Arc;

#[derive(FromJSObj)]
struct NavigateToOptions {
    #[rename = "appId"]
    appid: String,
    path: String,
    #[rename = "envVersion"]
    env_version: Option<String>,
}

fn navigate_to_lxapp(ctx: JSContext, options: NavigateToOptions) -> JSResult<()> {
    let mut startup_options = LxAppStartupOptions::new(&options.path);

    if let Some(env) = options.env_version {
        startup_options = startup_options.set_mode(LxAppMode::from(env.as_str()));
    }

    let lxapp = ctx.get_user_data::<Arc<LxApp>>().unwrap();
    lxapp
        .navigate_to(options.appid, startup_options)
        .map_err(|e| RongJSError::Error(format!("Failed to navigate to lxapp: {}", e)))?;
    Ok(())
}

fn navigate_back_lxapp(ctx: JSContext) -> JSResult<()> {
    let lxapp = ctx.get_user_data::<Arc<LxApp>>().unwrap();
    lxapp
        .navigate_back()
        .map_err(|e| RongJSError::Error(format!("Failed to navigate back: {}", e)))?;
    Ok(())
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    // Register navigator
    let navigate_to_lxapp = JSFunc::new(ctx, navigate_to_lxapp)?;
    lx::register_js_api(ctx, "navigateToLxApp", navigate_to_lxapp)?;

    let navigate_back_lxapp = JSFunc::new(ctx, navigate_back_lxapp)?;
    lx::register_js_api(ctx, "navigateBackLxApp", navigate_back_lxapp)?;

    Ok(())
}
