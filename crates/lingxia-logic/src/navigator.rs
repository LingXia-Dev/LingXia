use crate::I18nKey;
use crate::i18n::{js_error_from_lxapp_error, t};
use crate::update;
use lingxia_platform::traits::ui::{ToastIcon, ToastOptions, ToastPosition, UserFeedback};
use lxapp::lx;
use lxapp::{self, LxApp, LxAppError, LxAppStartupOptions, ReleaseType, UpdateManager};
use rong::{FromJSObj, JSContext, JSFunc, JSObject, JSResult};
use serde_json::Value;
use std::sync::Arc;

#[derive(FromJSObj)]
struct NavigateToOptions {
    #[rename = "appId"]
    appid: String,
    path: Option<String>,
    page: Option<String>,
    query: Option<JSObject>,
    #[rename = "envVersion"]
    env_version: Option<String>,
    #[rename = "targetVersion"]
    target_version: Option<String>,
}

fn build_startup_options(
    target: &LxApp,
    options: &NavigateToOptions,
) -> Result<(LxAppStartupOptions, ReleaseType), LxAppError> {
    let path = resolve_page_target(target, options)?;
    let mut startup_options = LxAppStartupOptions::new(&path);

    let release_type = parse_env_version(options.env_version.as_deref())?;

    if options.env_version.is_some() {
        startup_options = startup_options.set_release_type(release_type);
    }

    Ok((startup_options, release_type))
}

fn parse_env_version(env_version: Option<&str>) -> Result<ReleaseType, LxAppError> {
    lxapp::parse_optional_env_release_type(env_version).map_err(LxAppError::InvalidParameter)
}

fn resolve_page_target<'a>(
    target: &'a LxApp,
    options: &'a NavigateToOptions,
) -> Result<String, LxAppError> {
    let has_page = options
        .page
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());
    let has_path = options
        .path
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());
    if has_page && has_path {
        return Err(LxAppError::InvalidParameter(
            "pass either page or path, not both".to_string(),
        ));
    }
    let path = if let Some(page) = options
        .page
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        target
            .find_page_path_by_name(page)
            .ok_or_else(|| LxAppError::ResourceNotFound(format!("page name: {page}")))?
    } else {
        options
            .path
            .as_deref()
            .map(str::trim)
            .unwrap_or_default()
            .to_string()
    };
    append_query(path, options.query.as_ref())
}

fn append_query(path: String, query: Option<&JSObject>) -> Result<String, LxAppError> {
    let Some(query) = query else {
        return Ok(path);
    };
    let query_json = query.to_json_string().map_err(LxAppError::from)?;
    let query: Value = serde_json::from_str(&query_json)?;
    lxapp::append_page_query(path, &query).map_err(LxAppError::InvalidParameter)
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

async fn do_navigate_to_lxapp(lxapp: Arc<LxApp>, options: NavigateToOptions) -> JSResult<()> {
    let target_appid = options.appid.clone();
    let release_type = parse_env_version(options.env_version.as_deref())
        .map_err(|e| js_error_from_lxapp_error(&e))?;
    let target_version = options
        .target_version
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if let Some(target_version) = target_version {
        lxapp::ensure_target_version_ready(&lxapp, &target_appid, release_type, target_version)
            .await
            .map_err(|e| js_error_from_lxapp_error(&e))?;
    } else {
        update::ensure_first_install(&lxapp, &target_appid, release_type).await?;
        if lxapp::is_force_update_downloading(&target_appid, release_type) {
            show_force_update_downloading_toast(&lxapp);
        }
        lxapp::ensure_force_update_for_installed(&lxapp, &target_appid, release_type)
            .await
            .map_err(|e| js_error_from_lxapp_error(&e))?;
    }

    let target_app = lxapp::ensure_lxapp(&target_appid, release_type)
        .map_err(|e| js_error_from_lxapp_error(&e))?;
    let (startup_options, _) =
        build_startup_options(&target_app, &options).map_err(|e| js_error_from_lxapp_error(&e))?;

    lxapp
        .navigate_to(target_appid.clone(), startup_options)
        .map_err(|e| js_error_from_lxapp_error(&e))?;

    UpdateManager::spawn_release_lxapp_update_check(target_appid);
    Ok(())
}

fn show_force_update_downloading_toast(lxapp: &Arc<LxApp>) {
    let title = t(I18nKey::UpdateDownloading);
    let _ = lxapp.runtime.show_toast(ToastOptions {
        title,
        icon: ToastIcon::Loading,
        image: None,
        duration: 1.5,
        mask: false,
        position: ToastPosition::Center,
    });
}

fn do_navigate_back_lxapp(lxapp: &LxApp) -> Result<(), LxAppError> {
    lxapp.navigate_back()?;
    Ok(())
}

async fn navigate_to_lxapp(ctx: JSContext, options: NavigateToOptions) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    if !should_navigate_to_lxapp(&lxapp, &options).map_err(|e| js_error_from_lxapp_error(&e))? {
        return Ok(());
    }

    do_navigate_to_lxapp(lxapp, options).await?;
    Ok(())
}

async fn navigate_back_lxapp(ctx: JSContext) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    do_navigate_back_lxapp(&lxapp).map_err(|e| js_error_from_lxapp_error(&e))?;
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
