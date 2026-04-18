use super::await_or_cancel;
use crate::LxApp;
use crate::lxapp::ReleaseType;
use crate::startup::LxAppStartupOptions;
use crate::{LxAppError, UpdateManager};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

#[derive(Deserialize)]
struct NavigateToLxAppOptions {
    #[serde(rename = "appId")]
    appid: String,
    path: Option<String>,
    page: Option<String>,
    query: Option<Value>,
    #[serde(rename = "envVersion")]
    env_version: Option<String>,
    #[serde(rename = "targetVersion")]
    target_version: Option<String>,
}

fn build_startup_options(
    target: &LxApp,
    options: &NavigateToLxAppOptions,
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
    crate::parse_optional_env_release_type(env_version).map_err(LxAppError::InvalidParameter)
}

fn resolve_page_target(
    target: &LxApp,
    options: &NavigateToLxAppOptions,
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

fn append_query(path: String, query: Option<&Value>) -> Result<String, LxAppError> {
    let Some(query) = query else {
        return Ok(path);
    };
    crate::append_page_query(path, query).map_err(LxAppError::InvalidParameter)
}

fn should_navigate_to_lxapp(
    lxapp: &LxApp,
    options: &NavigateToLxAppOptions,
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
    options: NavigateToLxAppOptions,
    cancel: &mut super::HostCancel,
) -> Result<(), LxAppError> {
    let target_appid = options.appid.clone();
    let release_type = parse_env_version(options.env_version.as_deref())?;
    let target_version = options
        .target_version
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());

    if let Some(target_version) = target_version {
        await_or_cancel(
            cancel,
            crate::update::ensure_target_version_ready(
                &lxapp,
                &target_appid,
                release_type,
                target_version,
            ),
        )
        .await?;
    } else {
        await_or_cancel(
            cancel,
            crate::update::ensure_first_install(&lxapp, &target_appid, release_type),
        )
        .await?;
        await_or_cancel(
            cancel,
            crate::update::ensure_force_update_for_installed(&lxapp, &target_appid, release_type),
        )
        .await?;
    }

    let target_app = crate::ensure_lxapp(&target_appid, release_type)?;
    let (startup_options, _) = build_startup_options(&target_app, &options)?;

    lxapp.navigate_to(target_appid.clone(), startup_options)?;

    UpdateManager::spawn_release_lxapp_update_check(target_appid);
    Ok(())
}

host_api_async!(
    NavigateToLxApp,
    NavigateToLxAppOptions,
    (),
    |lxapp, options, cancel| async {
        if !should_navigate_to_lxapp(&lxapp, &options)? {
            return Ok(());
        }
        do_navigate_to_lxapp(lxapp, options, &mut cancel).await?;
        Ok(())
    }
);

host_api!(NavigateBackLxApp, (), |lxapp| {
    lxapp.navigate_back()?;
    Ok(())
});

pub(crate) fn register_all() {
    register_host_module!("navigator", {
        "navigateToLxApp" => Arc::new(NavigateToLxApp),
        "navigateBackLxApp" => Arc::new(NavigateBackLxApp)
    });
}
