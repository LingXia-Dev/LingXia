use super::*;
use tokio::task::yield_now;

fn emit_update_ready_event(
    target_appid: &str,
    release_type: ReleaseType,
    version: &str,
    is_force_update: bool,
) {
    let payload = serde_json::json!({
        "version": version,
        "isForceUpdate": is_force_update,
        "releaseType": release_type.as_str(),
    });
    let _ = publish_app_event(target_appid, "UpdateReady", Some(payload.to_string()));
}

fn emit_update_failed_event(
    target_appid: &str,
    release_type: ReleaseType,
    pkg: &UpdatePackageInfo,
    error: &str,
) {
    let payload = serde_json::json!({
        "version": pkg.version,
        "isForceUpdate": pkg.is_force_update,
        "releaseType": release_type.as_str(),
        "minRuntimeVersion": pkg.required_runtime_version,
        "currentRuntimeVersion": crate::SDK_RUNTIME_VERSION,
        "error": error,
    });
    let _ = publish_app_event(target_appid, "UpdateFailed", Some(payload.to_string()));
}

fn is_same_downloaded_update(
    manager: &UpdateManager,
    target_appid: &str,
    release_type: ReleaseType,
    version: &str,
) -> bool {
    matches!(
        manager.has_downloaded_update(target_appid, release_type),
        Ok(Some(info)) if info.version == version && info.archive_path.exists()
    )
}

impl UpdateManager {
    pub(super) fn spawn_background_update_check_internal(
        context_lxapp: Arc<lxapp_runtime::LxApp>,
        target_appid: String,
        release_type: ReleaseType,
        current_version: Option<String>,
        bypass_cooldown: bool,
    ) {
        let update_check_target = UpdateTarget::lxapp(
            target_appid.clone(),
            release_type,
            LxAppUpdateQuery::latest(None::<String>),
        )
        .scope_key();
        let _ = crate::executor::spawn(async move {
            if !bypass_cooldown && !state::try_acquire_update_check_window(&update_check_target) {
                return;
            }

            let manager = UpdateManager::new(context_lxapp);
            let current_version = current_version.or_else(|| {
                manager
                    .installed_version(&target_appid, release_type)
                    .ok()
                    .flatten()
            });

            match manager
                .check_latest_update(&target_appid, release_type, current_version.as_deref())
                .await
            {
                Ok(Some(pkg)) => {
                    if !manager.should_update(&target_appid, release_type, &pkg.version) {
                        return;
                    }

                    if let Err(err) = ensure_runtime_version_compatible(&target_appid, &pkg) {
                        emit_update_failed_event(
                            &target_appid,
                            release_type,
                            &pkg,
                            &err.to_string(),
                        );
                        return;
                    }

                    let already_downloaded_same = is_same_downloaded_update(
                        &manager,
                        &target_appid,
                        release_type,
                        &pkg.version,
                    );

                    if already_downloaded_same {
                        crate::info!(
                            "Update package already downloaded; emitting UpdateReady directly (version={})",
                            pkg.version
                        )
                        .with_appid(target_appid.clone());
                        emit_update_ready_event(
                            &target_appid,
                            release_type,
                            &pkg.version,
                            pkg.is_force_update,
                        );
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
                        emit_update_ready_event(
                            &target_appid,
                            release_type,
                            &pkg.version,
                            pkg.is_force_update,
                        );
                    } else {
                        let error = download_res
                            .err()
                            .map(|e| e.to_string())
                            .unwrap_or_else(|| "download failed".to_string());
                        emit_update_failed_event(&target_appid, release_type, &pkg, &error);
                    }
                }
                Ok(None) => {}
                Err(_) => {}
            }
        });
    }

    /// Check for updates using the registered Provider.
    /// Returns no update if no provider is registered.
    pub async fn check_update(
        &self,
        lxappid: &str,
        release_type: ReleaseType,
        query: LxAppUpdateQuery,
    ) -> Result<Option<UpdatePackageInfo>, LxAppError> {
        let provider = crate::get_provider();
        let target = UpdateTarget::lxapp(lxappid, release_type, query);
        provider.check_update(target).await.map_err(|e| {
            crate::error!("check_update failed: {}", e).with_appid(lxappid);
            provider_error_to_lxapp_error(&e)
        })
    }

    async fn check_latest_update(
        &self,
        lxappid: &str,
        release_type: ReleaseType,
        current_version: Option<&str>,
    ) -> Result<Option<UpdatePackageInfo>, LxAppError> {
        self.check_update(
            lxappid,
            release_type,
            LxAppUpdateQuery::latest(current_version),
        )
        .await
    }

    async fn check_exact_update(
        &self,
        lxappid: &str,
        release_type: ReleaseType,
        target_version: &str,
    ) -> Result<Option<UpdatePackageInfo>, LxAppError> {
        self.check_update(
            lxappid,
            release_type,
            LxAppUpdateQuery::target_version(target_version),
        )
        .await
    }

    /// Spawn a release-channel background update check for a known lxapp.
    pub fn spawn_release_lxapp_update_check(target_appid: String) {
        let release_type = ReleaseType::Release;

        let Some(lxapp) = lxapp_runtime::try_get(&target_appid) else {
            crate::warn!(
                "LxApp '{}' not found for background update check",
                target_appid
            );
            return;
        };

        if lxapp.release_type != release_type {
            return;
        }

        UpdateManager::spawn_background_update_check_internal(
            lxapp.clone(),
            target_appid,
            release_type,
            Some(lxapp.current_version()),
            false,
        );
    }

    /// Spawn a background check to download newer packages for the given app.
    pub fn spawn_background_update_check(lxapp: Arc<lxapp_runtime::LxApp>) {
        if lxapp.release_type != ReleaseType::Release {
            return;
        }

        let target_appid = lxapp.appid.clone();
        let release_type = lxapp.release_type;
        UpdateManager::spawn_background_update_check_internal(
            lxapp.clone(),
            target_appid,
            release_type,
            Some(lxapp.current_version()),
            false,
        );
    }

    /// Apply a previously downloaded update without requiring an LxApp instance.
    /// Safe to call before the LxApp object exists (navigation startup).
    pub(crate) fn apply_downloaded_update(
        runtime: Arc<Platform>,
        lxappid: &str,
        release_type: ReleaseType,
    ) -> Result<(), LxAppError> {
        let downloaded = match metadata::downloaded_get(lxappid, release_type)? {
            Some(rec) => rec,
            None => return Ok(()),
        };

        let archive_path = PathBuf::from(&downloaded.zip_path);
        if !archive_path.exists() {
            metadata::downloaded_remove(lxappid, release_type)?;
            return Ok(());
        }

        let previous_path =
            metadata::get(lxappid, release_type)?.map(|rec| PathBuf::from(rec.install_path));

        let install_path =
            Self::install_archive_to_dir(&runtime, lxappid, release_type, &archive_path)?;

        Self::record_install_metadata(
            lxappid,
            release_type,
            &downloaded.version.to_string(),
            &install_path,
        )?;

        if let Some(prev) = previous_path
            && prev.exists()
            && prev != install_path
        {
            let _ = fs::remove_dir_all(&prev);
        }

        let _ = metadata::downloaded_remove(lxappid, release_type);
        Ok(())
    }
}

/// Ensure the target app is installed at least once (first-launch preparation).
///
/// If the target lxapp is not installed, this checks for an available package and downloads it.
/// Downloaded archives are recorded in metadata and applied when creating/opening the app
/// (see `LxApps::get_or_init_lxapp`).
pub(crate) async fn ensure_first_install(
    current_lxapp: &Arc<lxapp_runtime::LxApp>,
    target_appid: &str,
    release_type: ReleaseType,
) -> Result<(), LxAppError> {
    if release_type != ReleaseType::Release {
        return Ok(());
    }

    let manager = UpdateManager::new(current_lxapp.clone());
    if manager.is_installed(target_appid, release_type)? {
        return Ok(());
    }

    if bundled_lxapp_available(current_lxapp, target_appid) {
        lxapp_runtime::register_builtin_asset_bundle(
            target_appid.to_string(),
            target_appid.to_string(),
        );
        return Ok(());
    }

    if !crate::provider::has_update_provider() {
        crate::warn!(
            "Cannot first-install lxapp '{target_appid}': not installed, no bundled manifest, and no UpdateProvider"
        );
        return Err(LxAppError::UnsupportedOperation(format!(
            "lxapp '{target_appid}' is not installed; remote install unavailable"
        )));
    }

    let pkg = match with_foreground_update_timeout(
        manager.check_latest_update(target_appid, release_type, None),
        &format!("first install update check for {}", target_appid),
    )
    .await
    {
        Ok(Some(pkg)) => pkg,
        Ok(None) => {
            crate::warn!(
                "Cannot first-install lxapp '{target_appid}': UpdateProvider returned no package for {}",
                release_type.as_str()
            );
            return Err(LxAppError::ResourceNotFound(format!(
                "lxapp '{target_appid}' package not found ({})",
                release_type.as_str()
            )));
        }
        Err(err) => {
            return Err(LxAppError::Runtime(format!(
                "failed to query UpdateProvider for first install of '{target_appid}': {err}"
            )));
        }
    };

    ensure_runtime_version_compatible(target_appid, &pkg)?;

    let _archive = manager
        .download_archive_with_checksum(
            target_appid,
            release_type,
            &pkg.url,
            &pkg.checksum_sha256,
            &pkg.version,
        )
        .await?;

    Ok(())
}

fn bundled_lxapp_available(current_lxapp: &Arc<lxapp_runtime::LxApp>, target_appid: &str) -> bool {
    let manifest = format!("{target_appid}/lxapp.json");
    current_lxapp.runtime.read_asset(&manifest).is_ok()
}

/// Ensure a specific target version package is prepared before opening.
///
/// Policy:
/// - Already installed with the same version: no-op.
/// - Otherwise: resolve exact version metadata and ensure archive is downloaded.
/// - Downloaded archive is applied when app instance is (re)opened.
pub async fn ensure_target_version_ready(
    current_lxapp: &Arc<lxapp_runtime::LxApp>,
    target_appid: &str,
    release_type: ReleaseType,
    target_version: &str,
) -> Result<(), LxAppError> {
    let target_version = target_version.trim();
    if target_version.is_empty() {
        return Err(LxAppError::InvalidParameter(
            "targetVersion cannot be empty".to_string(),
        ));
    }

    let target_semver = Version::parse(target_version).map_err(|_| {
        LxAppError::InvalidParameter(format!(
            "targetVersion must be semantic version: {}",
            target_version
        ))
    })?;

    let manager = UpdateManager::new(current_lxapp.clone());
    let is_installed = manager.is_installed(target_appid, release_type)?;
    let current_version = if is_installed {
        manager.installed_version(target_appid, release_type)?
    } else {
        None
    };
    if release_type == ReleaseType::Release {
        match with_foreground_update_timeout(
            manager.check_latest_update(target_appid, release_type, current_version.as_deref()),
            &format!("force-update gate check for {}", target_appid),
        )
        .await
        {
            Ok(Some(pkg)) if pkg.is_force_update => {
                let force_version = Version::parse(&pkg.version).map_err(|_| {
                    LxAppError::UnsupportedOperation(format!(
                        "invalid forced update version '{}' for {}",
                        pkg.version, target_appid
                    ))
                })?;
                if target_semver < force_version {
                    return Err(LxAppError::UnsupportedOperation(format!(
                        "targetVersion {} is lower than required forced version {} for {} ({})",
                        target_version,
                        pkg.version,
                        target_appid,
                        release_type.as_str()
                    )));
                }
            }
            Ok(_) => {}
            Err(err) => {
                crate::warn!(
                    "targetVersion force-update check failed (fail-open): {}",
                    err
                )
                .with_appid(target_appid.to_string());
            }
        }
    }

    if current_version.as_deref() == Some(target_version) {
        return Ok(());
    }

    let pkg = with_foreground_update_timeout(
        manager.check_exact_update(target_appid, release_type, target_version),
        &format!(
            "exact version update check for {}@{}",
            target_appid, target_version
        ),
    )
    .await?
    .ok_or_else(|| {
        LxAppError::ResourceNotFound(format!(
            "No package available for {}@{} ({})",
            target_appid,
            target_version,
            release_type.as_str()
        ))
    })?;
    ensure_runtime_version_compatible(target_appid, &pkg)?;

    let already_downloaded_same = matches!(
        manager.has_downloaded_update(target_appid, release_type),
        Ok(Some(info)) if info.version == pkg.version && info.archive_path.exists()
    );
    if already_downloaded_same {
        return Ok(());
    }

    manager
        .download_archive_with_checksum(
            target_appid,
            release_type,
            &pkg.url,
            &pkg.checksum_sha256,
            &pkg.version,
        )
        .await?;

    Ok(())
}

/// Ensure forced update package is prepared before opening an already-installed lxapp.
///
/// Policy:
/// - Not installed: no-op (handled by `ensure_first_install`).
/// - Installed + no update or non-forced update: no-op.
/// - Installed + forced update available: ensure target package is downloaded before opening.
///
/// Note: update-check network/provider failures are fail-open here to avoid blocking app open
/// on transient backend issues. Only confirmed forced-package download failures block navigation.
pub async fn ensure_force_update_for_installed(
    current_lxapp: &Arc<lxapp_runtime::LxApp>,
    target_appid: &str,
    release_type: ReleaseType,
) -> Result<(), LxAppError> {
    if release_type != ReleaseType::Release {
        return Ok(());
    }

    let manager = UpdateManager::new(current_lxapp.clone());
    if !manager.is_installed(target_appid, release_type)? {
        return Ok(());
    }

    let current_version = manager.installed_version(target_appid, release_type)?;
    let Some(current_version) = current_version else {
        crate::warn!("Installed lxapp has no recorded version; skip force-update gating")
            .with_appid(target_appid.to_string());
        return Ok(());
    };

    let update = match with_foreground_update_timeout(
        manager.check_latest_update(target_appid, release_type, Some(current_version.as_str())),
        &format!("installed app force-update check for {}", target_appid),
    )
    .await
    {
        Ok(update) => update,
        Err(err) => {
            crate::warn!("force-update check failed (fail-open): {}", err)
                .with_appid(target_appid.to_string());
            return Ok(());
        }
    };

    let Some(pkg) = update else {
        return Ok(());
    };

    if let Err(err) = ensure_runtime_version_compatible(target_appid, &pkg) {
        if pkg.is_force_update {
            return Err(err);
        }
        crate::warn!("optional update blocked by runtime version gate: {}", err)
            .with_appid(target_appid.to_string());
        return Ok(());
    }

    if !pkg.is_force_update || pkg.version == current_version {
        return Ok(());
    }

    let already_downloaded_same = matches!(
        manager.has_downloaded_update(target_appid, release_type),
        Ok(Some(info)) if info.version == pkg.version && info.archive_path.exists()
    );
    if already_downloaded_same {
        return Ok(());
    }

    let key = state::force_update_download_key(target_appid, release_type);
    loop {
        if let Some(mut rx) = state::force_update_tracker().try_start_download(&key, &pkg.version) {
            let manager_bg = manager.clone();
            let key_bg = key.clone();
            let target_appid_bg = target_appid.to_string();
            let url_bg = pkg.url.clone();
            let checksum_bg = pkg.checksum_sha256.clone();
            let version_bg = pkg.version.clone();

            let _ = crate::executor::spawn(async move {
                let result = manager_bg
                    .download_archive_with_checksum(
                        &target_appid_bg,
                        release_type,
                        &url_bg,
                        &checksum_bg,
                        &version_bg,
                    )
                    .await;

                match result {
                    Ok(_) => state::force_update_tracker().mark_completed(&key_bg),
                    Err(err) => state::force_update_tracker().mark_failed(&key_bg, err.to_string()),
                }
            });

            loop {
                let state = { rx.borrow().clone() };
                match state {
                    state::ForceUpdateDownloadState::Downloading { .. } => {
                        if rx.changed().await.is_err() {
                            break;
                        }
                    }
                    state::ForceUpdateDownloadState::Completed => return Ok(()),
                    state::ForceUpdateDownloadState::Failed(error) => {
                        return Err(LxAppError::IoError(format!(
                            "forced update package download failed: {}",
                            error
                        )));
                    }
                }
            }
        }

        if let Some(mut rx) = state::force_update_tracker().wait_for_download(&key) {
            loop {
                let state = { rx.borrow().clone() };
                match state {
                    state::ForceUpdateDownloadState::Downloading { .. } => {
                        if rx.changed().await.is_err() {
                            break;
                        }
                    }
                    state::ForceUpdateDownloadState::Completed => return Ok(()),
                    state::ForceUpdateDownloadState::Failed(error) => {
                        return Err(LxAppError::IoError(format!(
                            "forced update package download failed: {}",
                            error
                        )));
                    }
                }
            }
        }

        let prepared = matches!(
            manager.has_downloaded_update(target_appid, release_type),
            Ok(Some(info)) if info.version == pkg.version && info.archive_path.exists()
        );
        if prepared {
            return Ok(());
        }

        yield_now().await;
    }
}

/// Prepare an lxapp package so shell surfaces can open it immediately.
pub async fn prepare_lxapp_open(
    target_appid: &str,
    release_type: ReleaseType,
) -> Result<(), LxAppError> {
    let home_appid = crate::app::home_appid()
        .map(str::to_string)
        .ok_or_else(|| LxAppError::ResourceNotFound("app not initialized".to_string()))?;

    let home_lxapp = lxapp_runtime::try_get(&home_appid).ok_or_else(|| {
        LxAppError::ResourceNotFound(format!("home lxapp '{home_appid}' not found"))
    })?;

    ensure_first_install(&home_lxapp, target_appid, release_type).await?;
    ensure_force_update_for_installed(&home_lxapp, target_appid, release_type).await?;
    Ok(())
}

pub fn schedule_lxapp_update_check(target_appid: &str, release_type: ReleaseType) {
    if release_type != ReleaseType::Release {
        return;
    }
    UpdateManager::spawn_release_lxapp_update_check(target_appid.to_string());
}
