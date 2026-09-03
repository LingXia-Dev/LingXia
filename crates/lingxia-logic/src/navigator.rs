use crate::I18nKey;
use crate::i18n::{js_error_from_lxapp_error, t};
use crate::update;
#[cfg(feature = "terminal")]
use lingxia_platform::traits::app_runtime::AppRuntime;
use lingxia_platform::traits::ui::{ToastIcon, ToastOptions, ToastPosition, UserFeedback};
use lxapp::{self, LxApp, LxAppError, LxAppStartupOptions, ReleaseType};
use rong::{FromJSObject, JSContext, JSObject, JSResult};
use serde_json::Value;
use std::sync::Arc;

#[derive(FromJSObject)]
#[ts_skip]
pub(crate) struct NavigateToAppOptions {
    #[js_name = "appId"]
    pub(crate) appid: String,
    // Kept only so the forward-only API rejects legacy route input instead of
    // silently ignoring an unknown object field.
    pub(crate) path: Option<String>,
    pub(crate) page: Option<String>,
    pub(crate) query: Option<JSObject>,
    #[js_name = "envVersion"]
    pub(crate) env_version: Option<String>,
    #[js_name = "targetVersion"]
    pub(crate) target_version: Option<String>,
}

fn build_startup_options(
    target: &LxApp,
    options: &NavigateToAppOptions,
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
    options: &'a NavigateToAppOptions,
) -> Result<String, LxAppError> {
    validate_page_selector(options)?;
    let path = if let Some(page) = options.page.as_deref().map(str::trim) {
        target
            .find_page_path_by_name(page)
            .ok_or_else(|| LxAppError::ResourceNotFound(format!("page name: {page}")))?
    } else {
        String::new()
    };
    append_query(path, options.query.as_ref())
}

fn validate_page_selector(options: &NavigateToAppOptions) -> Result<(), LxAppError> {
    if options.path.is_some() {
        return Err(LxAppError::InvalidParameter(
            "path is not supported; pass the configured page name in page".to_string(),
        ));
    }
    if options
        .page
        .as_deref()
        .is_some_and(|page| page.trim().is_empty())
    {
        return Err(LxAppError::InvalidParameter(
            "page must be a non-empty configured page name".to_string(),
        ));
    }
    Ok(())
}

fn append_query(path: String, query: Option<&JSObject>) -> Result<String, LxAppError> {
    let Some(query) = query else {
        return Ok(path);
    };
    let query_json = query.to_json_string().map_err(LxAppError::from)?;
    let query: Value = serde_json::from_str(&query_json)?;
    lxapp::append_page_query(path, &query).map_err(LxAppError::InvalidParameter)
}

fn should_navigate_to_app(
    lxapp: &LxApp,
    options: &NavigateToAppOptions,
) -> Result<bool, LxAppError> {
    validate_page_selector(options)?;
    if options.appid.is_empty() {
        return Err(LxAppError::InvalidParameter(
            "navigateToApp requires appId".to_string(),
        ));
    }

    if lxapp.appid == options.appid {
        return Ok(false);
    }

    Ok(true)
}

pub(crate) async fn prepare_app_open(
    lxapp: &Arc<LxApp>,
    options: &NavigateToAppOptions,
) -> JSResult<(LxAppStartupOptions, ReleaseType)> {
    validate_page_selector(options).map_err(|e| js_error_from_lxapp_error(&e))?;
    let target_appid = options.appid.clone();
    let release_type = parse_env_version(options.env_version.as_deref())
        .map_err(|e| js_error_from_lxapp_error(&e))?;
    let target_version = options
        .target_version
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let host_terminal_settings = register_host_terminal_settings_bundle(lxapp, &target_appid)?;
    if host_terminal_settings && target_version.is_some() {
        return Err(js_error_from_lxapp_error(&LxAppError::InvalidParameter(
            "the host-bundled Terminal Settings app does not support targetVersion".to_string(),
        )));
    }

    lxapp::ensure_open_allowed(&target_appid)
        .await
        .map_err(|e| js_error_from_lxapp_error(&e))?;

    if !host_terminal_settings {
        if let Some(target_version) = target_version {
            lxapp::ensure_target_version_ready(lxapp, &target_appid, release_type, target_version)
                .await
                .map_err(|e| js_error_from_lxapp_error(&e))?;
        } else {
            update::ensure_first_install(lxapp, &target_appid, release_type).await?;
            if lxapp::is_force_update_downloading(&target_appid, release_type) {
                show_force_update_downloading_toast(lxapp);
            }
            lxapp::ensure_force_update_for_installed(lxapp, &target_appid, release_type)
                .await
                .map_err(|e| js_error_from_lxapp_error(&e))?;
        }
    }

    let target_app = lxapp::ensure_lxapp(&target_appid, release_type)
        .map_err(|e| js_error_from_lxapp_error(&e))?;
    let (startup_options, _) =
        build_startup_options(&target_app, options).map_err(|e| js_error_from_lxapp_error(&e))?;

    Ok((startup_options, release_type))
}

#[cfg(feature = "terminal")]
fn register_host_terminal_settings_bundle(lxapp: &LxApp, target_appid: &str) -> JSResult<bool> {
    if target_appid != lingxia_terminal_config::SETTINGS_APP_ID {
        return Ok(false);
    }
    let manifest = format!("{target_appid}/lxapp.json");
    lxapp.runtime.read_asset(&manifest).map_err(|_| {
        js_error_from_lxapp_error(&LxAppError::ResourceNotFound(
            "Terminal Settings is not bundled by this host".to_string(),
        ))
    })?;
    lxapp::register_builtin_asset_bundle(target_appid.to_string());
    Ok(true)
}

#[cfg(not(feature = "terminal"))]
fn register_host_terminal_settings_bundle(_lxapp: &LxApp, _target_appid: &str) -> JSResult<bool> {
    Ok(false)
}

async fn do_navigate_to_app(lxapp: Arc<LxApp>, options: NavigateToAppOptions) -> JSResult<()> {
    let target_appid = options.appid.clone();
    let (startup_options, _) = prepare_app_open(&lxapp, &options).await?;
    let release_type = startup_options.release_type;

    lxapp
        .navigate_to(target_appid.clone(), startup_options)
        .map_err(|e| js_error_from_lxapp_error(&e))?;

    lxapp::schedule_lxapp_update_check(&target_appid, release_type);
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

/// Open another lxapp, optionally at one of its pages.
///
/// Navigating to the lxapp already running is a no-op. Rejects with
/// `E_SURFACE_CONFLICT` when the target is currently docked as an aside —
/// close that aside before opening it as a main.
async fn navigate_to_app(ctx: JSContext, options: NavigateToAppOptions) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;

    if !should_navigate_to_app(&lxapp, &options).map_err(|e| js_error_from_lxapp_error(&e))? {
        return Ok(());
    }

    // One lxapp, one region: navigating (main) to an lxapp that is currently
    // docked as an aside must not silently move it — the caller closes the
    // aside first.
    if lxapp::open_region(&options.appid) == Some(lxapp::LxAppOpenRegion::Aside) {
        return Err(rong::HostError::new(
            "E_SURFACE_CONFLICT",
            format!(
                "lxapp '{}' is already open as an aside; close it before navigating to it as a main",
                options.appid
            ),
        )
        .into());
    }

    do_navigate_to_app(lxapp, options).await?;
    Ok(())
}

/// Leave this lxapp and reveal the one that opened it.
async fn navigate_back_app(ctx: JSContext) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    do_navigate_back_lxapp(&lxapp).map_err(|e| js_error_from_lxapp_error(&e))?;
    Ok(())
}

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_api(ctx)
}

rong::js_api! {
    fn register_api(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;
        fn navigateToApp(ts_params = "options: NavigateToAppOptions") = navigate_to_app;
        fn navigateBackApp = navigate_back_app;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(page: Option<&str>, path: Option<&str>) -> NavigateToAppOptions {
        NavigateToAppOptions {
            appid: "target".to_string(),
            path: path.map(str::to_string),
            page: page.map(str::to_string),
            query: None,
            env_version: None,
            target_version: None,
        }
    }

    #[test]
    fn app_navigation_rejects_route_paths() {
        let error = validate_page_selector(&options(None, Some("/pages/home/index")))
            .unwrap_err()
            .to_string();
        assert!(error.contains("path is not supported"));
    }

    #[test]
    fn app_navigation_rejects_an_empty_page_name() {
        let error = validate_page_selector(&options(Some("  "), None))
            .unwrap_err()
            .to_string();
        assert!(error.contains("page must be a non-empty configured page name"));
    }
}
