use crate::update;
use lxapp::lx;
use lxapp::{self, LxApp, LxAppError, LxAppStartupOptions, ReleaseType, UpdateManager};
use rong::{FromJSObj, HostError, JSContext, JSFunc, JSResult};
use serde::Deserialize;
use std::sync::Arc;

#[derive(FromJSObj, Deserialize)]
struct NavigateToOptions {
    #[serde(rename = "appId")]
    #[rename = "appId"]
    appid: String,
    path: Option<String>,
    #[serde(rename = "envVersion")]
    #[rename = "envVersion"]
    env_version: Option<String>,
}

fn build_startup_options(options: &NavigateToOptions) -> (LxAppStartupOptions, ReleaseType) {
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

    (startup_options, release_type)
}

fn should_navigate_to_lxapp(
    lxapp: &LxApp,
    options: &NavigateToOptions,
) -> Result<bool, LxAppError> {
    if options.appid.is_empty() {
        return Err(LxAppError::InvalidParameter(
            "navigateToLxApp requires appId".to_string(),
        ));
    }

    if lxapp.appid == options.appid {
        return Ok(false);
    }

    Ok(true)
}

async fn do_navigate_to_lxapp(
    lxapp: Arc<LxApp>,
    options: NavigateToOptions,
) -> Result<(), LxAppError> {
    let (startup_options, release_type) = build_startup_options(&options);
    let target_appid = options.appid.clone();

    update::ensure_first_install(&lxapp, &target_appid, release_type).await?;

    lxapp.navigate_to(target_appid.clone(), startup_options)?;

    UpdateManager::spawn_background_update_check_for(target_appid, release_type);
    Ok(())
}

fn do_navigate_back_lxapp(lxapp: &LxApp) -> Result<(), LxAppError> {
    lxapp.navigate_back()?;
    Ok(())
}

async fn navigate_to_lxapp(ctx: JSContext, options: NavigateToOptions) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    if !should_navigate_to_lxapp(&lxapp, &options).map_err(|e| {
        HostError::new(
            rong::error::E_INTERNAL,
            format!("Failed to navigate to lxapp: {}", e),
        )
    })? {
        return Ok(());
    }

    do_navigate_to_lxapp(lxapp, options).await.map_err(|e| {
        HostError::new(
            rong::error::E_INTERNAL,
            format!("Failed to navigate to lxapp: {}", e),
        )
    })?;
    Ok(())
}

async fn navigate_back_lxapp(ctx: JSContext) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    do_navigate_back_lxapp(&lxapp).map_err(|e| {
        HostError::new(
            rong::error::E_INTERNAL,
            format!("Failed to navigate back: {}", e),
        )
    })?;
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
