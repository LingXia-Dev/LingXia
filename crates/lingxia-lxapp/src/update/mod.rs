mod app;
mod lxapp;
pub(crate) mod state;

use crate::archive;
use crate::error::LxAppError;
use crate::lxapp::metadata::LxAppRecord;
use crate::lxapp::{
    self as lxapp_runtime, LINGXIA_DIR, LXAPPS_DIR, ReleaseType, STORAGE_DIR, USER_CACHE_DIR,
    USER_DATA_DIR, lxapp_fingermark, metadata, version::Version,
};
use crate::provider::provider_error_to_lxapp_error;
use crate::publish_app_event;
use lingxia_platform::Platform;
use lingxia_platform::traits::app_runtime::AppRuntime;
use lingxia_platform::traits::update::UpdateService;
use lingxia_update::{
    LxAppUpdateQuery, RuntimeCompatibilityError, SemanticVersion, UpdatePackageInfo, UpdateTarget,
};
use rong_rt::download::{self as service_executor, BodySink};
use rong_rt::http as host_http;
use serde_json::Value;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::timeout;

pub(crate) use self::lxapp::{ensure_first_install, ensure_target_version_ready};
pub use self::lxapp::{
    ensure_force_update_for_installed, prepare_lxapp_open, schedule_lxapp_update_check,
};
pub use self::state::is_force_update_downloading;

/// Tracks download progress and reports to the UI layer.
struct ProgressSink {
    total_bytes: u64,
    downloaded_bytes: u64,
    last_reported_progress: i32,
    runtime: Option<Arc<Platform>>,
}

impl ProgressSink {
    fn new(total_bytes: u64, runtime: Option<Arc<Platform>>) -> Self {
        Self {
            total_bytes,
            downloaded_bytes: 0,
            last_reported_progress: 0,
            runtime,
        }
    }
}

impl BodySink for ProgressSink {
    fn write(&mut self, chunk: &[u8]) -> Result<(), String> {
        self.downloaded_bytes += chunk.len() as u64;

        if self.total_bytes > 0 {
            let progress =
                ((self.downloaded_bytes as f64 / self.total_bytes as f64) * 100.0) as i32;
            let progress = progress.min(100);

            if progress > self.last_reported_progress {
                self.last_reported_progress = progress;
                if let Some(runtime) = &self.runtime {
                    let _ = runtime.update_download_progress(progress);
                }
            }
        }

        Ok(())
    }

    fn close(&mut self, result: &Result<(), String>) {
        if result.is_ok()
            && let Some(runtime) = &self.runtime
        {
            let _ = runtime.update_download_progress(100);
        }
    }
}

/// Coordinates update preparation, download, and installation for LxApps.
#[derive(Clone)]
pub struct UpdateManager {
    /// Bound app reference used to access platform runtime (paths, fs) and app context.
    lxapp: Arc<lxapp_runtime::LxApp>,
    /// Directory where archives are downloaded before installation.
    downloads_dir: PathBuf,
}

const FOREGROUND_UPDATE_CHECK_TIMEOUT: Duration = Duration::from_secs(3);

async fn with_foreground_update_timeout<T, F>(future: F, context: &str) -> Result<T, LxAppError>
where
    F: std::future::Future<Output = Result<T, LxAppError>>,
{
    match timeout(FOREGROUND_UPDATE_CHECK_TIMEOUT, future).await {
        Ok(result) => result,
        Err(_) => Err(LxAppError::Runtime(format!(
            "{} timed out after {}s",
            context,
            FOREGROUND_UPDATE_CHECK_TIMEOUT.as_secs()
        ))),
    }
}

#[derive(Clone, Debug)]
pub struct DownloadedUpdateInfo {
    pub version: String,
    pub archive_path: PathBuf,
}

/// OTA update target.
#[derive(Clone)]
pub enum OtaUpdateTarget {
    App {
        runtime: Arc<Platform>,
        current_version: Option<String>,
    },
    LxApp {
        target_appid: String,
    },
}

fn ensure_runtime_version_compatible(
    lxappid: &str,
    pkg: &UpdatePackageInfo,
) -> Result<(), LxAppError> {
    pkg.ensure_runtime_compatible(crate::SDK_RUNTIME_VERSION, lxappid)
        .map_err(runtime_compatibility_to_lxapp_error)
}

fn runtime_compatibility_to_lxapp_error(error: RuntimeCompatibilityError) -> LxAppError {
    match error {
        RuntimeCompatibilityError::InvalidCurrentRuntimeVersion { .. } => {
            LxAppError::Runtime(error.to_string())
        }
        RuntimeCompatibilityError::InvalidRequiredRuntimeVersion { .. }
        | RuntimeCompatibilityError::RequiresRuntimeUpgrade { .. } => {
            LxAppError::UnsupportedOperation(error.to_string())
        }
    }
}

fn filename_from_url_or_hash(url: &str) -> String {
    let main = url.split(&['?', '#'][..]).next().unwrap_or(url);
    let seg = main.rsplit('/').next().unwrap_or(main);
    if !seg.is_empty() && seg.contains('.') {
        seg.to_string()
    } else {
        format!("{}.tar.zst", UpdateManager::hash_url(url))
    }
}

impl UpdateManager {
    /// Trigger OTA update flow.
    ///
    /// - App: prompt/download/install flow, immediate and cooldown-bypassed.
    /// - LxApp: immediate latest-package check flow, cooldown-bypassed.
    pub fn trigger_ota_update(target: OtaUpdateTarget) {
        match target {
            OtaUpdateTarget::App {
                runtime,
                current_version,
            } => {
                Self::spawn_app_update_flow_internal(runtime, current_version, Duration::ZERO, true)
            }
            OtaUpdateTarget::LxApp { target_appid } => {
                let release_type = ReleaseType::Release;
                let context_lxapp = lxapp_runtime::try_get(&target_appid)
                    .filter(|app| app.release_type == release_type);

                let Some(context_lxapp) = context_lxapp else {
                    crate::warn!(
                        "Target lxapp is not active for OTA-triggered update check: {}@{}",
                        target_appid,
                        release_type.as_str()
                    );
                    return;
                };

                let current_version = context_lxapp.current_version();

                Self::spawn_background_update_check_internal(
                    context_lxapp,
                    target_appid,
                    release_type,
                    Some(current_version),
                    true,
                );
            }
        }
    }

    /// Create a new UpdateManager bound to a specific LxApp.
    pub fn new(lxapp: Arc<lxapp_runtime::LxApp>) -> Self {
        let downloads_dir = lxapp
            .runtime
            .app_cache_dir()
            .join(LINGXIA_DIR)
            .join(LXAPPS_DIR)
            .join("download");
        let _ = fs::create_dir_all(&downloads_dir);

        Self {
            lxapp,
            downloads_dir,
        }
    }

    /// Decide whether we should download/apply the server version for this app variant.
    /// Policy: allow upgrade or downgrade; skip only when server_version equals installed.
    pub fn should_update(
        &self,
        lxappid: &str,
        release_type: ReleaseType,
        server_version: &str,
    ) -> bool {
        let installed = crate::lxapp::metadata::get(lxappid, release_type)
            .ok()
            .flatten()
            .map(|rec| rec.version_string());
        UpdatePackageInfo::should_replace_version(server_version, installed.as_deref())
    }

    /// Return path to a downloaded package if present for (lxappid, release_type).
    pub fn has_downloaded_update(
        &self,
        lxappid: &str,
        release_type: ReleaseType,
    ) -> Result<Option<DownloadedUpdateInfo>, LxAppError> {
        Ok(
            metadata::downloaded_get(lxappid, release_type)?.map(|rec| DownloadedUpdateInfo {
                version: rec.version.to_version_string(),
                archive_path: PathBuf::from(rec.zip_path),
            }),
        )
    }

    /// Return installed version for a given lxapp variant.
    pub fn installed_version(
        &self,
        lxappid: &str,
        release_type: ReleaseType,
    ) -> Result<Option<String>, LxAppError> {
        Ok(metadata::get(lxappid, release_type)?.map(|rec| rec.version_string()))
    }

    /// Returns whether the given lxappid+release_type is already installed.
    pub fn is_installed(
        &self,
        lxappid: &str,
        release_type: ReleaseType,
    ) -> Result<bool, LxAppError> {
        let Some(record) = metadata::get(lxappid, release_type)? else {
            return Ok(false);
        };

        let install_path_str = record.install_path.trim();
        let install_path = Path::new(install_path_str);
        let config_path = install_path.join("lxapp.json");

        let is_valid =
            !install_path_str.is_empty() && install_path.is_dir() && config_path.is_file();
        if is_valid {
            return Ok(true);
        }

        crate::warn!(
            "Stale installed metadata detected (release_type={}, install_path={}); treating as not installed",
            release_type,
            record.install_path
        )
        .with_appid(lxappid);
        let _ = metadata::remove(lxappid, release_type);
        Ok(false)
    }

    /// Install an app from pre-bundled assets (used for home app bootstrap).
    pub fn install_from_assets(
        runtime: Arc<Platform>,
        lxappid: &str,
        version: &str,
    ) -> Result<PathBuf, LxAppError> {
        let dir_name = lxapp_fingermark(lxappid, ReleaseType::Release);
        let destination = runtime
            .app_data_dir()
            .join(LINGXIA_DIR)
            .join(LXAPPS_DIR)
            .join(&dir_name);

        if destination.exists() {
            fs::remove_dir_all(&destination)?;
        }
        fs::create_dir_all(&destination)?;

        for entry in runtime.asset_dir_iter(lxappid) {
            let entry = entry?;
            let rel_path = entry
                .path
                .strip_prefix(&format!("{}/", lxappid))
                .unwrap_or(&entry.path);
            let target = destination.join(rel_path);

            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }

            let mut reader = entry.reader;
            let mut buffer = Vec::new();
            reader.read_to_end(&mut buffer)?;
            fs::write(&target, buffer)?;
        }

        Self::record_install_metadata(lxappid, ReleaseType::Release, version, &destination)?;
        Ok(destination)
    }

    /// Apply the given tar.zst archive for `lxappid` with explicit release_type and version.
    pub fn apply_update_archive(
        &self,
        lxappid: &str,
        release_type: ReleaseType,
        version: &str,
        archive_path: &Path,
    ) -> Result<(), LxAppError> {
        let previous_path =
            metadata::get(lxappid, release_type)?.map(|rec| PathBuf::from(rec.install_path));

        let install_path =
            Self::install_archive_to_dir(&self.lxapp.runtime, lxappid, release_type, archive_path)?;

        if let Err(e) = Self::record_install_metadata(lxappid, release_type, version, &install_path)
        {
            if let Err(cleanup_err) = fs::remove_dir_all(&install_path) {
                crate::error!(
                    "Failed to rollback new installation at {}: {}",
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
            && let Err(e) = fs::remove_dir_all(&prev)
        {
            crate::warn!(
                "Failed to remove old installation at {}: {}. Manual cleanup may be needed.",
                prev.display(),
                e
            )
            .with_appid(lxappid);
        }

        if let Err(e) = metadata::downloaded_remove(lxappid, release_type) {
            crate::warn!(
                "Failed to clean up download metadata and archive for {}:{:?}: {}",
                lxappid,
                release_type,
                e
            )
            .with_appid(lxappid);
        }

        Ok(())
    }

    /// Core install helper shared by instance and static paths.
    fn install_archive_to_dir(
        runtime: &Arc<Platform>,
        lxappid: &str,
        release_type: ReleaseType,
        archive_path: &Path,
    ) -> Result<PathBuf, LxAppError> {
        let dir_name = lxapp_fingermark(lxappid, release_type);
        let destination = runtime
            .app_data_dir()
            .join(LINGXIA_DIR)
            .join(LXAPPS_DIR)
            .join(dir_name);

        archive::extract_tar_zst(archive_path, &destination)?;
        Ok(destination)
    }

    /// Uninstall on-disk contents for a specific (lxappid, release_type) and clear metadata.
    fn uninstall_installed(
        &self,
        lxappid: &str,
        release_type: ReleaseType,
    ) -> Result<(), LxAppError> {
        if crate::lxapp::is_lxapp_open(lxappid) {
            return Err(LxAppError::UnsupportedOperation(
                "cannot uninstall an opened app".to_string(),
            ));
        }

        if let Some(rec) = metadata::get(lxappid, release_type)? {
            let dir_name = rec.fingermark;
            let pkg_dir = self
                .lxapp
                .runtime
                .app_data_dir()
                .join(LINGXIA_DIR)
                .join(LXAPPS_DIR)
                .join(&dir_name);
            if pkg_dir.exists() {
                fs::remove_dir_all(&pkg_dir)?;
            }

            let data_dir = self
                .lxapp
                .runtime
                .app_data_dir()
                .join(LINGXIA_DIR)
                .join(USER_DATA_DIR)
                .join(&dir_name);
            if data_dir.exists() {
                let _ = fs::remove_dir_all(&data_dir);
            }

            let cache_dir = self
                .lxapp
                .runtime
                .app_cache_dir()
                .join(LINGXIA_DIR)
                .join(LXAPPS_DIR)
                .join(USER_CACHE_DIR)
                .join(&dir_name);
            if cache_dir.exists() {
                let _ = fs::remove_dir_all(&cache_dir);
            }
        }

        if let Some(rec) = metadata::get(lxappid, release_type)? {
            let storage_file = self
                .lxapp
                .runtime
                .app_data_dir()
                .join(LINGXIA_DIR)
                .join(STORAGE_DIR)
                .join(format!("{}.redb", rec.fingermark));
            if storage_file.exists() {
                let _ = fs::remove_file(&storage_file);
            }
        }

        Ok(())
    }

    /// Uninstall all releases and all per-app data for the given lxappid.
    pub fn uninstall_all(&self, lxappid: &str) -> Result<(), LxAppError> {
        if crate::lxapp::is_lxapp_open(lxappid) {
            return Err(LxAppError::UnsupportedOperation(
                "cannot uninstall an opened app".to_string(),
            ));
        }

        let _ = self.uninstall_installed(lxappid, ReleaseType::Release);
        let _ = self.uninstall_installed(lxappid, ReleaseType::Preview);
        let _ = self.uninstall_installed(lxappid, ReleaseType::Developer);
        let _ = metadata::remove_all(lxappid);
        Ok(())
    }

    pub async fn download_archive_with_checksum(
        &self,
        lxappid: &str,
        release_type: ReleaseType,
        url: &str,
        checksum_sha256: &str,
        version: &str,
    ) -> Result<PathBuf, LxAppError> {
        let dest = self.dest_path_for_url(url);
        if dest.exists() {
            let _ = fs::remove_file(&dest);
        }

        let receiver =
            service_executor::request_download(url.to_string(), dest.clone(), None, None)
                .map_err(|e| LxAppError::IoError(format!("failed to start download: {}", e)))?;

        match receiver
            .await
            .map_err(|_| LxAppError::IoError("download task cancelled".to_string()))?
        {
            Ok(()) => {
                if !checksum_sha256.is_empty()
                    && let Err(e) = archive::verify_sha256(&dest, checksum_sha256)
                {
                    let _ = fs::remove_file(&dest);
                    return Err(e);
                }

                if let Err(e) = metadata::downloaded_upsert(lxappid, release_type, version, &dest) {
                    let _ = fs::remove_file(&dest);
                    return Err(LxAppError::IoError(format!(
                        "failed to record downloaded update: {}",
                        e
                    )));
                }

                crate::info!(
                    "Recorded downloaded update: appid={}, release_type={}, version={}, archive={}",
                    lxappid,
                    release_type,
                    version,
                    dest.display()
                )
                .with_appid(lxappid);
                Ok(dest)
            }
            Err(err) => {
                let _ = fs::remove_file(&dest);
                Err(LxAppError::IoError(format!("download failed: {}", err)))
            }
        }
    }

    /// Compute a destination path for the provided URL inside the downloads directory.
    fn dest_path_for_url(&self, url: &str) -> PathBuf {
        let name = filename_from_url_or_hash(url);
        self.downloads_dir.join(name)
    }

    /// Utility: hash url to a deterministic short hex string.
    fn hash_url(url: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        url.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }

    /// Persist the installation metadata in redb (current installed version only).
    fn record_install_metadata(
        lxappid: &str,
        release_type: ReleaseType,
        version: &str,
        install_path: &Path,
    ) -> Result<(), LxAppError> {
        let installed_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or_default();

        let fingermark = lxapp_fingermark(lxappid, release_type);
        let parsed_version = Version::parse(version).map_err(|_| {
            LxAppError::InvalidParameter(format!("Invalid semantic version: {}", version))
        })?;
        let record = LxAppRecord::new(
            lxappid,
            release_type,
            SemanticVersion::from_version(&parsed_version),
            fingermark,
            install_path.to_string_lossy().to_string(),
            installed_at,
        );

        metadata::upsert(&record)
    }
}
