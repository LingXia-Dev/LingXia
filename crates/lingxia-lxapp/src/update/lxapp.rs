use super::*;
use crate::update::error_bridge::lxapp_error_to_update_error;
use lingxia_provider::BoxFuture;
use lingxia_update::{LxAppUpdateHost, UpdateError};
use tokio::task::yield_now;

fn emit_update_ready_event(
    target_appid: &str,
    channel: ReleaseType,
    version: &str,
    is_force_update: bool,
) {
    let payload = serde_json::json!({
        "version": version,
        "isForceUpdate": is_force_update,
        "channel": channel.as_str(),
    });
    let _ = publish_app_event(target_appid, "UpdateReady", Some(payload.to_string()));
}

fn emit_update_failed_event(
    target_appid: &str,
    channel: ReleaseType,
    pkg: &UpdatePackageInfo,
    error: &str,
) {
    let payload = serde_json::json!({
        "version": pkg.version,
        "isForceUpdate": pkg.is_force_update,
        "channel": channel.as_str(),
        "minRuntimeVersion": pkg.required_runtime_version,
        "currentRuntimeVersion": crate::SDK_RUNTIME_VERSION,
        "error": error,
    });
    let _ = publish_app_event(target_appid, "UpdateFailed", Some(payload.to_string()));
}

#[derive(Clone)]
struct BoundLxAppUpdateHost {
    context_lxapp: Arc<lxapp_runtime::LxApp>,
    target_appid: String,
    release_type: ReleaseType,
    current_version_hint: Option<String>,
}

impl BoundLxAppUpdateHost {
    fn new(
        context_lxapp: Arc<lxapp_runtime::LxApp>,
        target_appid: String,
        release_type: ReleaseType,
        current_version_hint: Option<String>,
    ) -> Self {
        Self {
            context_lxapp,
            target_appid,
            release_type,
            current_version_hint,
        }
    }

    fn manager(&self) -> UpdateManager {
        UpdateManager::new(self.context_lxapp.clone())
    }
}

impl LxAppUpdateHost for BoundLxAppUpdateHost {
    fn spawn_detached(&self, task: BoxFuture<'static, ()>) {
        let _ = crate::executor::spawn(task);
    }

    fn target_appid(&self) -> &str {
        &self.target_appid
    }

    fn channel(&self) -> ReleaseType {
        self.release_type
    }

    fn runtime_version(&self) -> &str {
        crate::SDK_RUNTIME_VERSION
    }

    fn current_version_hint(&self) -> Option<String> {
        self.current_version_hint.clone()
    }

    fn installed_version<'a>(&'a self) -> BoxFuture<'a, Result<Option<String>, UpdateError>> {
        Box::pin(async move {
            self.manager()
                .installed_version(&self.target_appid, self.release_type)
                .map_err(lxapp_error_to_update_error)
        })
    }

    fn is_installed<'a>(&'a self) -> BoxFuture<'a, Result<bool, UpdateError>> {
        Box::pin(async move {
            self.manager()
                .is_installed(&self.target_appid, self.release_type)
                .map_err(lxapp_error_to_update_error)
        })
    }

    fn check_latest_update<'a>(
        &'a self,
        current_version: Option<&'a str>,
    ) -> BoxFuture<'a, Result<Option<UpdatePackageInfo>, UpdateError>> {
        Box::pin(async move {
            self.manager()
                .check_latest_update(&self.target_appid, self.release_type, current_version)
                .await
                .map_err(lxapp_error_to_update_error)
        })
    }

    fn check_exact_update<'a>(
        &'a self,
        target_version: &'a str,
    ) -> BoxFuture<'a, Result<Option<UpdatePackageInfo>, UpdateError>> {
        Box::pin(async move {
            self.manager()
                .check_exact_update(&self.target_appid, self.release_type, target_version)
                .await
                .map_err(lxapp_error_to_update_error)
        })
    }

    fn has_downloaded_update<'a>(
        &'a self,
        version: &'a str,
    ) -> BoxFuture<'a, Result<bool, UpdateError>> {
        Box::pin(async move {
            Ok(matches!(
                self.manager().has_downloaded_update(&self.target_appid, self.release_type),
                Ok(Some(info)) if info.version == version && info.archive_path.exists()
            ))
        })
    }

    fn download_update<'a>(
        &'a self,
        update: &'a UpdatePackageInfo,
    ) -> BoxFuture<'a, Result<(), UpdateError>> {
        Box::pin(async move {
            self.manager()
                .download_archive_with_checksum(
                    &self.target_appid,
                    self.release_type,
                    &update.url,
                    &update.checksum_sha256,
                    &update.version,
                )
                .await
                .map(|_| ())
                .map_err(lxapp_error_to_update_error)
        })
    }

    fn wait_for_or_start_force_download<'a>(
        &'a self,
        update: &'a UpdatePackageInfo,
    ) -> BoxFuture<'a, Result<(), UpdateError>> {
        Box::pin(async move {
            let manager = self.manager();
            let key = state::force_update_download_key(&self.target_appid, self.release_type);

            loop {
                if let Some(mut rx) =
                    state::force_update_tracker().try_start_download(&key, &update.version)
                {
                    let manager_bg = manager.clone();
                    let key_bg = key.clone();
                    let target_appid_bg = self.target_appid.clone();
                    let url_bg = update.url.clone();
                    let checksum_bg = update.checksum_sha256.clone();
                    let version_bg = update.version.clone();
                    let release_type = self.release_type;

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
                            Err(err) => {
                                state::force_update_tracker().mark_failed(&key_bg, err.to_string())
                            }
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
                                return Err(UpdateError::io(format!(
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
                                return Err(UpdateError::io(format!(
                                    "forced update package download failed: {}",
                                    error
                                )));
                            }
                        }
                    }
                }

                let prepared = matches!(
                    manager.has_downloaded_update(&self.target_appid, self.release_type),
                    Ok(Some(info)) if info.version == update.version && info.archive_path.exists()
                );
                if prepared {
                    return Ok(());
                }

                yield_now().await;
            }
        })
    }

    fn emit_update_ready(&self, version: &str, is_force_update: bool) -> Result<(), UpdateError> {
        emit_update_ready_event(
            &self.target_appid,
            self.release_type,
            version,
            is_force_update,
        );
        Ok(())
    }

    fn emit_update_failed(
        &self,
        update: &UpdatePackageInfo,
        error: &str,
    ) -> Result<(), UpdateError> {
        emit_update_failed_event(&self.target_appid, self.release_type, update, error);
        Ok(())
    }

    fn is_bundled_available(&self) -> bool {
        bundled_lxapp_available(&self.context_lxapp, &self.target_appid)
    }

    fn register_builtin_bundle(&self) -> Result<(), UpdateError> {
        lxapp_runtime::register_builtin_asset_bundle(self.target_appid.clone());
        Ok(())
    }

    fn has_update_provider(&self) -> bool {
        crate::provider::has_update_provider()
    }

    fn log_warning(&self, detail: &str) {
        crate::warn!("{}", detail).with_appid(self.target_appid.clone());
    }
}

impl UpdateManager {
    pub(super) fn spawn_background_update_check_internal(
        context_lxapp: Arc<lxapp_runtime::LxApp>,
        target_appid: String,
        release_type: ReleaseType,
        current_version: Option<String>,
    ) {
        lingxia_update::spawn_lxapp_background_update_check(
            BoundLxAppUpdateHost::new(
                context_lxapp,
                target_appid,
                release_type,
                current_version.clone(),
            ),
            current_version,
        );
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

        let version = downloaded.version.to_version_string();
        let install_path =
            Self::install_archive_to_dir(&runtime, lxappid, release_type, &version, &archive_path)?;

        if let Err(e) = Self::validate_installed_lxapp_manifest(&install_path) {
            if let Err(cleanup_err) = fs::remove_dir_all(&install_path) {
                crate::error!(
                    "Failed to rollback invalid downloaded update at {}: {}",
                    install_path.display(),
                    cleanup_err
                )
                .with_appid(lxappid);
            }
            let _ = metadata::downloaded_remove(lxappid, release_type);
            return Err(e);
        }

        if let Err(e) =
            Self::record_install_metadata(lxappid, release_type, &version, &install_path)
        {
            if let Err(cleanup_err) = fs::remove_dir_all(&install_path) {
                crate::error!(
                    "Failed to rollback downloaded update at {}: {}",
                    install_path.display(),
                    cleanup_err
                )
                .with_appid(lxappid);
            }
            return Err(e);
        }

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
    lingxia_update::ensure_lxapp_first_install(&BoundLxAppUpdateHost::new(
        current_lxapp.clone(),
        target_appid.to_string(),
        release_type,
        None,
    ))
    .await
    .map_err(Into::into)
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
    lingxia_update::ensure_lxapp_target_version_ready(
        &BoundLxAppUpdateHost::new(
            current_lxapp.clone(),
            target_appid.to_string(),
            release_type,
            None,
        ),
        target_version,
    )
    .await
    .map_err(Into::into)
}

/// Ensure forced update package is prepared before opening an already-installed lxapp.
pub async fn ensure_force_update_for_installed(
    current_lxapp: &Arc<lxapp_runtime::LxApp>,
    target_appid: &str,
    release_type: ReleaseType,
) -> Result<(), LxAppError> {
    lingxia_update::ensure_lxapp_force_update_for_installed(&BoundLxAppUpdateHost::new(
        current_lxapp.clone(),
        target_appid.to_string(),
        release_type,
        None,
    ))
    .await
    .map_err(Into::into)
}

/// Prepare an lxapp package so shell surfaces can open it immediately.
pub async fn prepare_lxapp_open(
    target_appid: &str,
    release_type: ReleaseType,
) -> Result<(), LxAppError> {
    let home_appid = lingxia_app_context::home_app_id()
        .map(str::to_string)
        .ok_or_else(|| LxAppError::Runtime("host app config is not initialized".to_string()))?;

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
