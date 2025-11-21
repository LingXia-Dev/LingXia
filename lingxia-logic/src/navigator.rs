use lingxia_lxapp::AppServiceEvent;
use lingxia_lxapp::{self, LxApp, LxAppStartupOptions, ReleaseType, UpdateManager, lx};
use rong::service_executor;
use rong::{FromJSObj, JSContext, JSFunc, JSResult, RongJSError};
use std::sync::Arc;

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
        .map(lingxia_lxapp::parse_env_release_type)
        .unwrap_or(ReleaseType::Release);

    if options.env_version.is_some() {
        startup_options = startup_options.set_release_type(release_type);
    }

    ensure_first_install(&lxapp, &options.appid, release_type)
        .await
        .map_err(|e| RongJSError::Error(format!("Failed to prepare lxapp: {}", e)))?;

    let target_appid = options.appid.clone();
    lxapp
        .navigate_to(target_appid.clone(), startup_options)
        .map_err(|e| RongJSError::Error(format!("Failed to navigate to lxapp: {}", e)))?;

    // After navigation, spawn a background task to check cloud for newer updates.
    // If a newer package is available, download it and record in metadata, then notify UpdateReady.
    let _ = service_executor::spawn_async(async move {
        let lxapp = lingxia_lxapp::get(target_appid.clone());
        let manager = UpdateManager::new(lxapp.clone());

        let current_version = lxapp.current_version();
        match manager
            .check_update(&target_appid, release_type, Some(current_version.as_str()))
            .await
        {
            Ok(check) => {
                if let Some(pkg) = check.package {
                    // Use UpdateManager policy (allow downgrade; skip only same version)
                    if !manager.should_update(&pkg.version) {
                        return;
                    }
                    // If the same version is already downloaded, skip re-downloading
                    let already_downloaded_same =
                        match manager.has_downloaded_update(&target_appid, release_type) {
                            Ok(Some(info)) if info.version == pkg.version => true,
                            _ => false,
                        };

                    if already_downloaded_same {
                        return;
                    }
                    if manager
                        .download_archive_with_checksum(
                            &target_appid,
                            release_type,
                            &pkg.url,
                            &pkg.checksum_sha256,
                            &pkg.version,
                        )
                        .await
                        .is_ok()
                    {
                        let _ = lxapp.appservice_notify(AppServiceEvent::UpdateReady, None);
                    } else {
                        let _ = lxapp.appservice_notify(AppServiceEvent::UpdateFailed, None);
                    }
                }
            }
            Err(_) => {
                // Ignore check errors in background
            }
        }
    });
    Ok(())
}

async fn navigate_back_lxapp(ctx: JSContext) -> JSResult<()> {
    let lxapp = LxApp::from_ctx(&ctx)?;
    lxapp
        .navigate_back()
        .map_err(|e| RongJSError::Error(format!("Failed to navigate back: {}", e)))?;
    Ok(())
}

// Ensures first-time install only. Installed apps are handled by the caller.
async fn ensure_first_install(
    current_lxapp: &Arc<LxApp>,
    target_appid: &str,
    release_type: ReleaseType,
) -> Result<(), String> {
    // Use UpdateManager bound to the current app's runtime; pass target appid for metadata
    let manager = UpdateManager::new(current_lxapp.clone());

    // Not installed: check latest from cloud, download and apply synchronously.
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
        let _zip_path = manager
            .download_archive_with_checksum(
                target_appid,
                release_type,
                &pkg.url,
                &pkg.checksum_sha256,
                &pkg.version,
            )
            .await
            .map_err(|e| e.to_string())?;

        // Do not apply here; LxApp.navigate_to will apply any downloaded package before opening
        return Ok(());
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
