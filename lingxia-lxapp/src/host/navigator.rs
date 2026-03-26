use super::{await_or_cancel, parse_release_type};
use crate::LxApp;
use crate::lxapp::ReleaseType;
use crate::startup::LxAppStartupOptions;
use crate::{LxAppError, UpdateManager};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
struct NavigateToLxAppOptions {
    #[serde(rename = "appId")]
    appid: String,
    path: Option<String>,
    #[serde(rename = "envVersion")]
    env_version: Option<String>,
    #[serde(rename = "targetVersion")]
    target_version: Option<String>,
}

fn build_startup_options(options: &NavigateToLxAppOptions) -> (LxAppStartupOptions, ReleaseType) {
    let path = options.path.as_deref().unwrap_or("");
    let mut startup_options = LxAppStartupOptions::new(path);

    let release_type = parse_release_type(options.env_version.as_deref());

    if options.env_version.is_some() {
        startup_options = startup_options.set_release_type(release_type);
    }

    (startup_options, release_type)
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
    let (startup_options, release_type) = build_startup_options(&options);
    let target_appid = options.appid.clone();
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
