use crate::update;
use lxapp::{self, LxApp, LxAppStartupOptions, ReleaseType, lx};
use rong::{FromJSObj, JSContext, JSFunc, JSResult, RongJSError};

#[derive(FromJSObj)]
struct NavigateToOptions {
    #[rename = "appId"]
    appid: String,
    path: Option<String>,
    #[rename = "envVersion"]
    env_version: Option<String>,
}

async fn navigate_to_lxapp(ctx: JSContext, options: NavigateToOptions) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    if lxapp.appid == options.appid {
        return Ok(());
    }

    let path = options.path.as_deref().unwrap_or("");
    let mut startup_options = LxAppStartupOptions::new(path);

    let release_type = options
        .env_version
        .as_deref()
        .map(lxapp::parse_env_release_type)
        .unwrap_or(ReleaseType::Release);

    if options.env_version.is_some() {
        startup_options = startup_options.set_release_type(release_type);
    }

    update::ensure_first_install(&lxapp, &options.appid, release_type).await?;

    let target_appid = options.appid.clone();
    lxapp
        .navigate_to(target_appid.clone(), startup_options)
        .map_err(|e| RongJSError::Error(format!("Failed to navigate to lxapp: {}", e)))?;

    update::spawn_background_update_check(target_appid.clone(), release_type);
    Ok(())
}

async fn navigate_back_lxapp(ctx: JSContext) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
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
