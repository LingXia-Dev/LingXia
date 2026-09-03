use super::await_or_cancel;
use crate::LxApp;
use crate::LxAppError;
use crate::lxapp::ReleaseType;
use crate::startup::LxAppStartupOptions;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

#[derive(Deserialize)]
struct NavigateToAppOptions {
    #[serde(rename = "appId")]
    appid: String,
    // Kept only so callers get a forward-only rejection for the removed
    // route-based input instead of having it ignored by serde.
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
    crate::parse_optional_env_release_type(env_version).map_err(LxAppError::InvalidParameter)
}

fn resolve_page_target(
    target: &LxApp,
    options: &NavigateToAppOptions,
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

fn append_query(path: String, query: Option<&Value>) -> Result<String, LxAppError> {
    let Some(query) = query else {
        return Ok(path);
    };
    crate::append_page_query(path, query).map_err(LxAppError::InvalidParameter)
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

async fn do_navigate_to_app(
    lxapp: Arc<LxApp>,
    options: NavigateToAppOptions,
    cancel: &mut super::HostCancel,
) -> Result<(), LxAppError> {
    validate_page_selector(&options)?;
    let target_appid = options.appid.clone();
    let release_type = parse_env_version(options.env_version.as_deref())?;
    let target_version = options
        .target_version
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());

    await_or_cancel(cancel, crate::ensure_open_allowed(&target_appid)).await?;

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
    let release_type = startup_options.release_type;

    lxapp.navigate_to(target_appid.clone(), startup_options)?;

    crate::schedule_lxapp_update_check(&target_appid, release_type);
    Ok(())
}

host_api_async!(
    NavigateToApp,
    NavigateToAppOptions,
    (),
    |lxapp, options, cancel| async {
        if !should_navigate_to_app(&lxapp, &options)? {
            return Ok(());
        }
        do_navigate_to_app(lxapp, options, &mut cancel).await?;
        Ok(())
    }
);

host_api!(NavigateBackApp, (), |lxapp| {
    lxapp.navigate_back()?;
    Ok(())
});

pub(crate) fn register_all() {
    register_host_module!("navigator", {
        "navigateToApp" => Arc::new(NavigateToApp),
        "navigateBackApp" => Arc::new(NavigateBackApp)
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn component_app_navigation_rejects_route_paths() {
        let options = NavigateToAppOptions {
            appid: "target".to_string(),
            path: Some("/pages/home/index".to_string()),
            page: None,
            query: None,
            env_version: None,
            target_version: None,
        };

        let error = validate_page_selector(&options).unwrap_err().to_string();
        assert!(error.contains("path is not supported"));
    }
}
