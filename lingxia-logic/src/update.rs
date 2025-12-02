use lxapp::{AppServiceEvent, LxApp, ReleaseType, UpdateManager, warn};
use rong::{JSContext, JSFunc, JSObject, JSResult, RongJSError, service_executor};
use std::sync::Arc;

// Register Update-related JS bindings
pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    // lx.getUpdateManager() -> returns JSUpdateManager instance
    fn get_update_manager(ctx: JSContext) -> JSResult<JSObject> {
        lxapp::get_or_create_update_manager(&ctx)
    }
    let get_update_manager = JSFunc::new(ctx, get_update_manager)?;
    lxapp::lx::register_js_api(ctx, "getUpdateManager", get_update_manager)?;
    Ok(())
}

/// Ensure the target app is installed at least once (first-launch preparation).
pub async fn ensure_first_install(
    current_lxapp: &Arc<LxApp>,
    target_appid: &str,
    release_type: ReleaseType,
) -> JSResult<()> {
    let manager = UpdateManager::new(current_lxapp.clone());

    if manager
        .is_installed(target_appid, release_type)
        .map_err(|e| RongJSError::Error(e.to_string()))?
    {
        return Ok(());
    }

    let check = manager
        .check_update(target_appid, release_type, None)
        .await
        .map_err(|e| RongJSError::Error(e.to_string()))?;

    let pkg = check.package.ok_or_else(|| {
        RongJSError::Error(format!(
            "No package available for first install of {}",
            target_appid
        ))
    })?;

    manager
        .download_archive_with_checksum(
            target_appid,
            release_type,
            &pkg.url,
            &pkg.checksum_sha256,
            &pkg.version,
        )
        .await
        .map_err(|e| RongJSError::Error(e.to_string()))?;

    Ok(())
}

/// Spawn a background task to check cloud updates for the given app and pre-download newer packages.
pub fn spawn_background_update_check(target_appid: String, release_type: ReleaseType) {
    let _ = service_executor::spawn_async(async move {
        let Some(lxapp) = lxapp::try_get(&target_appid) else {
            warn!(
                "LxApp '{}' not found for background update check",
                target_appid
            );
            return;
        };
        let manager = UpdateManager::new(lxapp.clone());

        let current_version = lxapp.current_version();
        match manager
            .check_update(&target_appid, release_type, Some(current_version.as_str()))
            .await
        {
            Ok(check) => {
                if let Some(pkg) = check.package {
                    if !manager.should_update(&pkg.version) {
                        return;
                    }

                    let already_downloaded_same = matches!(
                        manager.has_downloaded_update(&target_appid, release_type),
                        Ok(Some(info)) if info.version == pkg.version
                    );

                    if already_downloaded_same {
                        return;
                    }

                    let download_res = manager
                        .download_archive_with_checksum(
                            &target_appid,
                            release_type,
                            &pkg.url,
                            &pkg.checksum_sha256,
                            &pkg.version,
                        )
                        .await;

                    if download_res.is_ok() {
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
}
