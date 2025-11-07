use lingxia_lxapp::{self, LxApp, LxAppStartupOptions, ReleaseType, UpdateManager, lx};
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

async fn navigate_to_lxapp(ctx: JSContext, options: NavigateToOptions) -> JSResult<()> {
    let mut startup_options = LxAppStartupOptions::new(&options.path);

    let release_type = options
        .env_version
        .as_deref()
        .map(lingxia_lxapp::parse_env_release_type)
        .unwrap_or(ReleaseType::Release);

    if options.env_version.is_some() {
        startup_options = startup_options.set_release_type(release_type);
    }

    let lxapp = ctx.get_user_data::<Arc<LxApp>>().unwrap();

    ensure_app_package(&lxapp, &options.appid, release_type)
        .await
        .map_err(|e| RongJSError::Error(format!("Failed to prepare lxapp: {}", e)))?;

    lxapp
        .navigate_to(options.appid, startup_options)
        .map_err(|e| RongJSError::Error(format!("Failed to navigate to lxapp: {}", e)))?;
    Ok(())
}

async fn navigate_back_lxapp(ctx: JSContext) -> JSResult<()> {
    let lxapp = ctx.get_user_data::<Arc<LxApp>>().unwrap();
    lxapp
        .navigate_back()
        .map_err(|e| RongJSError::Error(format!("Failed to navigate back: {}", e)))?;
    Ok(())
}

// Ensures the target app package is installed or updated to latest.
// Only handles download/apply of the package; other dirs may be initialized later.
async fn ensure_app_package(
    current_lxapp: &Arc<LxApp>,
    target_appid: &str,
    release_type: ReleaseType,
) -> Result<(), String> {
    let manager = UpdateManager::new(current_lxapp.clone());

    // Not installed: check latest, download and apply synchronously.
    if !manager
        .is_installed(target_appid, release_type)
        .map_err(|e| e.to_string())?
    {
        let check = manager
            .check_update(target_appid, release_type, None)
            .await
            .map_err(|e| e.to_string())?;
        let pkg = check
            .package
            .ok_or_else(|| format!("No package available for first install of {}", target_appid))?;
        let zip_path = manager
            .download_archive_with_checksum(&pkg.url, &pkg.checksum_sha256)
            .await
            .map_err(|e| e.to_string())?;
        return manager
            .apply_update_zip(target_appid, release_type, &pkg.version, &zip_path)
            .map_err(|e| e.to_string());
    }

    // Installed: if a downloaded package exists, apply it now.
    if let Some(info) = manager
        .has_downloaded_update(target_appid, release_type)
        .map_err(|e| e.to_string())?
    {
        manager
            .apply_update_zip(target_appid, release_type, &info.version, &info.zip_path)
            .map_err(|e| e.to_string())?;
    }
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
