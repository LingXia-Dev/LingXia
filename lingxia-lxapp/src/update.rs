use crate::archive;
use crate::emit_app_event;
use crate::error::LxAppError;
use crate::lxapp::metadata::{LxAppRecord, SemanticVersion};
use crate::lxapp::{
    self, LINGXIA_DIR, LXAPPS_DIR, ReleaseType, STORAGE_DIR, USER_CACHE_DIR, USER_DATA_DIR,
    lxapp_fingermark, metadata, version::Version,
};
use crate::provider::{UpdatePackageInfo, UpdateTarget};
use dashmap::DashMap;
use lingxia_messaging::{CallbackResult, get_callback, remove_callback};
use lingxia_platform::Platform;
use lingxia_platform::traits::app_runtime::AppRuntime;
use lingxia_platform::traits::update::UpdateService;
use rong_http::{self as service_executor, BodySink};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::watch;

/// Tracks download progress and reports to the UI layer
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

            // Only update UI if progress changed by at least 1%
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
        if result.is_ok() {
            if let Some(runtime) = &self.runtime {
                let _ = runtime.update_download_progress(100);
            }
        }
    }
}

/// Coordinates update preparation, download, and installation for LxApps.
#[derive(Clone)]
pub struct UpdateManager {
    /// Bound app reference used to access platform runtime (paths, fs) and app context.
    lxapp: Arc<lxapp::LxApp>,
    /// Directory where archives are downloaded before installation.
    downloads_dir: PathBuf,
}

#[derive(Clone, Debug)]
pub struct DownloadedUpdateInfo {
    pub version: String,
    pub archive_path: PathBuf,
}

/// Per-target forced-update package preparation state.
#[derive(Clone, Debug, PartialEq, Eq)]
enum ForceUpdateDownloadState {
    Downloading { version: String },
    Completed,
    Failed(String),
}

struct ForceUpdateDownloadTracker {
    downloads: DashMap<String, watch::Sender<ForceUpdateDownloadState>>,
}

impl ForceUpdateDownloadTracker {
    fn new() -> Self {
        Self {
            downloads: DashMap::new(),
        }
    }

    fn try_start_download(
        &self,
        key: &str,
        version: &str,
    ) -> Option<watch::Receiver<ForceUpdateDownloadState>> {
        use dashmap::mapref::entry::Entry;

        match self.downloads.entry(key.to_string()) {
            Entry::Occupied(_) => None,
            Entry::Vacant(entry) => {
                let initial = ForceUpdateDownloadState::Downloading {
                    version: version.to_string(),
                };
                let (tx, rx) = watch::channel(initial);
                entry.insert(tx);
                Some(rx)
            }
        }
    }

    fn mark_completed(&self, key: &str) {
        if let Some(entry) = self.downloads.get(key) {
            let _ = entry.send(ForceUpdateDownloadState::Completed);
        }
        self.downloads.remove(key);
    }

    fn mark_failed(&self, key: &str, error: String) {
        if let Some(entry) = self.downloads.get(key) {
            let _ = entry.send(ForceUpdateDownloadState::Failed(error));
        }
        self.downloads.remove(key);
    }

    fn wait_for_download(&self, key: &str) -> Option<watch::Receiver<ForceUpdateDownloadState>> {
        self.downloads.get(key).map(|entry| entry.subscribe())
    }

    fn state(&self, key: &str) -> Option<ForceUpdateDownloadState> {
        self.downloads.get(key).map(|entry| entry.borrow().clone())
    }
}

static FORCE_UPDATE_DOWNLOAD_TRACKER: OnceLock<ForceUpdateDownloadTracker> = OnceLock::new();

fn force_update_tracker() -> &'static ForceUpdateDownloadTracker {
    FORCE_UPDATE_DOWNLOAD_TRACKER.get_or_init(ForceUpdateDownloadTracker::new)
}

fn force_update_download_key(lxappid: &str, release_type: ReleaseType) -> String {
    format!("{}@{}", lxappid, release_type.as_str())
}

/// Returns whether a forced-update package is currently being prepared.
pub fn is_force_update_downloading(lxappid: &str, release_type: ReleaseType) -> bool {
    matches!(
        force_update_tracker().state(&force_update_download_key(lxappid, release_type)),
        Some(ForceUpdateDownloadState::Downloading { .. })
    )
}

impl UpdateManager {
    /// Download a package synchronously. When `version` is None, fetch from cloud to get latest.
    /// Returns the downloaded archive path and records it in `downloaded` table.
    /// Create a new UpdateManager bound to a specific LxApp.
    pub fn new(lxapp: Arc<lxapp::LxApp>) -> Self {
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

        // Remember previous install path (if any)
        let previous_path =
            metadata::get(lxappid, release_type)?.map(|rec| PathBuf::from(rec.install_path));

        // Install archive using the shared helper
        let install_path =
            Self::install_archive_to_dir(&runtime, lxappid, release_type, &archive_path)?;

        // Record install metadata
        Self::record_install_metadata(
            lxappid,
            release_type,
            &downloaded.version.to_string(),
            &install_path,
        )?;

        // Remove previous install if different
        if let Some(prev) = previous_path
            && prev.exists()
            && prev != install_path
        {
            let _ = fs::remove_dir_all(&prev);
        }

        // Clean up downloaded record + archive
        let _ = metadata::downloaded_remove(lxappid, release_type);

        Ok(())
    }

    /// Check for updates using the registered Provider.
    /// Returns no update if no provider is registered.
    pub async fn check_update(
        &self,
        lxappid: &str,
        _release_type: ReleaseType,
        current_version: Option<&str>,
    ) -> Result<Option<UpdatePackageInfo>, LxAppError> {
        let provider = crate::get_provider();
        let target = UpdateTarget::LxApp {
            id: lxappid.to_string(),
            current_version: current_version.map(|v| v.to_string()),
        };

        provider.check_update(target).await.map_err(|e| {
            crate::error!("check_update failed: {}", e).with_appid(lxappid);
            e.to_lxapp_error()
        })
    }

    /// Check for host app updates via the registered Provider.
    /// Returns no update if no provider is registered.
    pub async fn check_app_update(
        current_version: Option<&str>,
    ) -> Result<Option<UpdatePackageInfo>, LxAppError> {
        let provider = crate::get_provider();
        let target = UpdateTarget::App {
            current_version: current_version.map(|v| v.to_string()),
        };

        provider.check_update(target).await.map_err(|e| {
            crate::error!("check_app_update failed: {}", e);
            e.to_lxapp_error()
        })
    }

    /// Spawn async flow: check -> prompt -> download -> install for host app updates.
    pub fn spawn_app_update_flow(runtime: Arc<Platform>, current_version: Option<String>) {
        let _ = rong::bg::spawn(async move {
            if let Err(err) =
                UpdateManager::check_and_install_app_update(runtime, current_version.as_deref())
                    .await
            {
                crate::warn!("App update flow failed: {}", err);
            }
        });
    }

    /// Spawn a background update check for a known appid.
    pub fn spawn_background_update_check_for(target_appid: String, release_type: ReleaseType) {
        let Some(lxapp) = lxapp::try_get(&target_appid) else {
            crate::warn!(
                "LxApp '{}' not found for background update check",
                target_appid
            );
            return;
        };

        if lxapp.release_type != release_type {
            return;
        }

        UpdateManager::spawn_background_update_check(lxapp);
    }

    /// Spawn a background check to download newer packages for the given app.
    pub fn spawn_background_update_check(lxapp: Arc<lxapp::LxApp>) {
        let target_appid = lxapp.appid.clone();
        let release_type = lxapp.release_type;
        let current_version = lxapp.current_version();

        let _ = rong::bg::spawn(async move {
            let manager = UpdateManager::new(lxapp);

            match manager
                .check_update(&target_appid, release_type, Some(current_version.as_str()))
                .await
            {
                Ok(Some(pkg)) => {
                    if !manager.should_update(&pkg.version) {
                        return;
                    }

                    let already_downloaded_same = matches!(
                        manager.has_downloaded_update(&target_appid, release_type),
                        Ok(Some(info)) if info.version == pkg.version && info.archive_path.exists()
                    );

                    if already_downloaded_same {
                        crate::info!(
                            "Update package already downloaded; emitting UpdateReady directly (version={})",
                            pkg.version
                        )
                        .with_appid(target_appid.clone());
                        let payload = serde_json::json!({
                            "version": pkg.version,
                            "isForceUpdate": pkg.is_force_update,
                            "releaseType": release_type.as_str(),
                        });
                        let _ =
                            emit_app_event(&target_appid, "UpdateReady", Some(payload.to_string()));
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
                        let payload = serde_json::json!({
                            "version": pkg.version,
                            "isForceUpdate": pkg.is_force_update,
                            "releaseType": release_type.as_str(),
                        });
                        let _ =
                            emit_app_event(&target_appid, "UpdateReady", Some(payload.to_string()));
                    } else {
                        let payload = serde_json::json!({
                            "version": pkg.version,
                            "isForceUpdate": pkg.is_force_update,
                            "releaseType": release_type.as_str(),
                            "error": download_res.err().map(|e| e.to_string()).unwrap_or_else(|| "download failed".to_string()),
                        });
                        let _ = emit_app_event(
                            &target_appid,
                            "UpdateFailed",
                            Some(payload.to_string()),
                        );
                    }
                }
                Ok(None) => {}
                Err(_) => {}
            }
        });
    }

    /// Check for host app updates and install when user confirms.
    /// Forced updates are non-skippable from UI perspective.
    pub async fn check_and_install_app_update(
        runtime: Arc<Platform>,
        current_version: Option<&str>,
    ) -> Result<(), LxAppError> {
        crate::info!(
            "App update flow start: current_version={:?}",
            current_version
        );
        let update = UpdateManager::check_app_update(current_version).await?;
        let Some(pkg) = update else {
            crate::info!("No app update available");
            return Ok(());
        };
        crate::info!(
            "App update available: version={} url={}",
            pkg.version,
            pkg.url
        );

        // Build update info JSON for the UI.
        // `isForceUpdate` controls whether the dialog is dismissible on the SDK side.
        let update_info_json = {
            let mut json_obj = serde_json::Map::new();
            json_obj.insert("version".to_string(), serde_json::json!(&pkg.version));
            json_obj.insert(
                "isForceUpdate".to_string(),
                serde_json::json!(pkg.is_force_update),
            );
            if let Some(size) = pkg.size {
                json_obj.insert("size".to_string(), serde_json::json!(size));
            }
            if let Some(notes) = &pkg.release_notes {
                json_obj.insert("releaseNotes".to_string(), serde_json::json!(notes));
            }
            Some(serde_json::to_string(&json_obj).unwrap_or_default())
        };

        let (callback_id, receiver) = get_callback();
        if let Err(e) = runtime.show_update_prompt(callback_id, update_info_json.as_deref()) {
            let _ = remove_callback(callback_id);
            return Err(LxAppError::Runtime(format!(
                "Failed to show update prompt: {}",
                e
            )));
        }

        let confirmed = match receiver.await {
            Ok(CallbackResult::Success(data)) => serde_json::from_str::<Value>(&data)
                .ok()
                .and_then(|json| json.get("confirm").and_then(|v| v.as_bool()))
                .unwrap_or(false),
            Ok(CallbackResult::Error(_)) => false,
            Err(_) => false,
        };

        if !confirmed && pkg.is_force_update {
            return Err(LxAppError::Runtime(
                "Forced app update was not confirmed".to_string(),
            ));
        }

        if !confirmed {
            crate::info!("App update cancelled or deferred");
            return Ok(());
        }
        crate::info!("App update confirmed, starting download");

        let path = UpdateManager::download_app_update_with_checksum(
            runtime.clone(),
            &pkg.url,
            &pkg.checksum_sha256,
            &pkg.version,
        )
        .await?;
        crate::info!("App update downloaded: {}", path.display());

        runtime.install_update(&path).map_err(|e| {
            LxAppError::Runtime(format!("Failed to request app update install: {}", e))
        })?;
        crate::info!("App update install requested");

        Ok(())
    }

    /// Decide whether we should download/apply the server version for this app variant.
    /// Policy: allow upgrade or downgrade; skip only when server_version equals installed.
    pub fn should_update(&self, server_version: &str) -> bool {
        let installed = crate::lxapp::metadata::get(&self.lxapp.appid, self.lxapp.release_type)
            .ok()
            .flatten()
            .map(|rec| rec.version_string());
        match installed {
            Some(v) => v != server_version,
            None => true,
        }
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

    /// Returns whether the given lxappid+release_type is already installed
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
        _version: &str,
    ) -> Result<PathBuf, LxAppError> {
        // Determine hashed install directory consistent with zip installs
        let dir_name = lxapp_fingermark(lxappid, ReleaseType::Release);
        let destination = runtime
            .app_data_dir()
            .join(LINGXIA_DIR)
            .join(LXAPPS_DIR)
            .join(dir_name);

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

        Self::record_install_metadata(lxappid, ReleaseType::Release, _version, &destination)?;
        Ok(destination)
    }

    /// Prepare an update or first-time install.
    ///
    /// Not installed: downloads, verifies, installs synchronously, and removes the archive.
    /// Installed and newer available: downloads+verifies and saves a pending record to redb (no auto-apply).
    /// Apply the given tar.zst archive for `lxappid` with explicit release_type and version.
    pub fn apply_update_archive(
        &self,
        lxappid: &str,
        release_type: ReleaseType,
        _version: &str,
        archive_path: &Path,
    ) -> Result<(), LxAppError> {
        // Remember previous install path (if any)
        let previous_path =
            metadata::get(lxappid, release_type)?.map(|rec| PathBuf::from(rec.install_path));

        // Install into a new hashed directory for this version
        let install_path =
            Self::install_archive_to_dir(&self.lxapp.runtime, lxappid, release_type, archive_path)?;

        // Write metadata first to allow rollback
        if let Err(e) =
            Self::record_install_metadata(lxappid, release_type, _version, &install_path)
        {
            // Rollback: remove the new installation since we couldn't commit it
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

        // Safe to remove previous version
        if let Some(prev) = previous_path
            && prev.exists()
            && prev != install_path
            && let Err(e) = fs::remove_dir_all(&prev)
        {
            // Log warning but don't fail - new version is already committed
            crate::warn!(
                "Failed to remove old installation at {}: {}. Manual cleanup may be needed.",
                prev.display(),
                e
            )
            .with_appid(lxappid);
        }

        // Remove download metadata and archive
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
        // Reject uninstall when app is currently opened
        if crate::lxapp::is_lxapp_open(lxappid) {
            return Err(LxAppError::UnsupportedOperation(
                "cannot uninstall an opened app".to_string(),
            ));
        }

        // Remove installed package directory and per-app data using recorded fingermark
        if let Some(rec) = metadata::get(lxappid, release_type)? {
            let dir_name = rec.fingermark;
            // package dir
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
            // user data dir
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
            // cache dir
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
        // Remove per-app storage file for this variant if present (hashed name)
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
        // reject when opened
        if crate::lxapp::is_lxapp_open(lxappid) {
            return Err(LxAppError::UnsupportedOperation(
                "cannot uninstall an opened app".to_string(),
            ));
        }
        // per-release dirs
        let _ = self.uninstall_installed(lxappid, ReleaseType::Release);
        let _ = self.uninstall_installed(lxappid, ReleaseType::Preview);
        let _ = self.uninstall_installed(lxappid, ReleaseType::Developer);

        // remove installed metadata entries for all releases
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
                if !checksum_sha256.is_empty() {
                    if let Err(e) = archive::verify_sha256(&dest, checksum_sha256) {
                        let _ = fs::remove_file(&dest);
                        return Err(e);
                    }
                }
                // Persist pending downloaded update so it can be applied later.
                // Uses current app context (appid + release_type) and explicit version.
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

    /// Download a host app update package and verify checksum when provided.
    pub async fn download_app_update_with_checksum(
        runtime: Arc<Platform>,
        url: &str,
        checksum_sha256: &str,
        version: &str,
    ) -> Result<PathBuf, LxAppError> {
        crate::info!("App update download start: url={} version={}", url, version);
        let dest_dir = runtime
            .app_cache_dir()
            .join(LINGXIA_DIR)
            .join("app_updates");
        let _ = fs::create_dir_all(&dest_dir);

        let dest = dest_dir.join(app_update_filename(url, version));
        crate::info!("App update download dest: {}", dest.display());

        // Check if file already exists and is valid
        if dest.exists() {
            if checksum_sha256.is_empty() {
                if dest.metadata().map(|m| m.len()).unwrap_or(0) > 0 {
                    crate::info!("App update package already downloaded: {}", dest.display());
                    let _ = runtime.dismiss_download_progress();
                    return Ok(dest);
                }
                let _ = fs::remove_file(&dest);
            }
            if archive::verify_sha256(&dest, checksum_sha256).is_ok() {
                crate::info!(
                    "App update package already downloaded and verified: {}",
                    dest.display()
                );
                let _ = runtime.dismiss_download_progress();
                return Ok(dest);
            }
            // File exists but checksum failed, remove it
            let _ = fs::remove_file(&dest);
        }

        // Get file size for progress tracking
        let file_size = get_content_length(url).await.unwrap_or(0);

        // Show progress dialog before starting download
        if let Err(e) = runtime.show_download_progress() {
            crate::warn!("Failed to show download progress: {}", e);
        }

        // Create progress sink if we have file size
        let sink: Option<Box<dyn BodySink>> = if file_size > 0 {
            Some(Box::new(ProgressSink::new(
                file_size,
                Some(runtime.clone()),
            )))
        } else {
            None
        };

        let receiver =
            match service_executor::request_download(url.to_string(), dest.clone(), None, sink) {
                Ok(receiver) => receiver,
                Err(e) => {
                    let _ = runtime.dismiss_download_progress();
                    return Err(LxAppError::IoError(format!(
                        "failed to start download: {}",
                        e
                    )));
                }
            };

        let result = match receiver
            .await
            .map_err(|_| LxAppError::IoError("download task cancelled".to_string()))?
        {
            Ok(()) => {
                if !checksum_sha256.is_empty() {
                    if let Err(e) = archive::verify_sha256(&dest, checksum_sha256) {
                        let _ = fs::remove_file(&dest);
                        Err(e)
                    } else {
                        Ok(dest)
                    }
                } else {
                    Ok(dest)
                }
            }
            Err(err) => {
                let _ = fs::remove_file(&dest);
                Err(LxAppError::IoError(format!("download failed: {}", err)))
            }
        };

        // Dismiss progress dialog
        let _ = runtime.dismiss_download_progress();

        result
    }

    /// Utility: hash url to a deterministic short hex string
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

/// Ensure the target app is installed at least once (first-launch preparation).
///
/// If the target lxapp is not installed, this checks for an available package and downloads it.
/// Downloaded archives are recorded in metadata and applied when creating/opening the app
/// (see `LxApps::get_or_init_lxapp`).
pub(crate) async fn ensure_first_install(
    current_lxapp: &Arc<lxapp::LxApp>,
    target_appid: &str,
    release_type: ReleaseType,
) -> Result<(), LxAppError> {
    let manager = UpdateManager::new(current_lxapp.clone());
    if manager.is_installed(target_appid, release_type)? {
        return Ok(());
    }

    let pkg = manager
        .check_update(target_appid, release_type, None)
        .await?
        .ok_or_else(|| {
            LxAppError::ResourceNotFound(format!(
                "No package available for first install of {}",
                target_appid
            ))
        })?;

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
    current_lxapp: &Arc<lxapp::LxApp>,
    target_appid: &str,
    release_type: ReleaseType,
) -> Result<(), LxAppError> {
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

    let update = match manager
        .check_update(target_appid, release_type, Some(current_version.as_str()))
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

    let key = force_update_download_key(target_appid, release_type);
    loop {
        // Try to become the single downloader for this target.
        if let Some(mut rx) = force_update_tracker().try_start_download(&key, &pkg.version) {
            let manager_bg = manager.clone();
            let key_bg = key.clone();
            let target_appid_bg = target_appid.to_string();
            let url_bg = pkg.url.clone();
            let checksum_bg = pkg.checksum_sha256.clone();
            let version_bg = pkg.version.clone();

            let _ = rong::bg::spawn(async move {
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
                    Ok(_) => force_update_tracker().mark_completed(&key_bg),
                    Err(err) => force_update_tracker().mark_failed(&key_bg, err.to_string()),
                }
            });

            loop {
                let state = { rx.borrow().clone() };
                match state {
                    ForceUpdateDownloadState::Downloading { .. } => {
                        if rx.changed().await.is_err() {
                            break;
                        }
                    }
                    ForceUpdateDownloadState::Completed => return Ok(()),
                    ForceUpdateDownloadState::Failed(error) => {
                        return Err(LxAppError::IoError(format!(
                            "forced update package download failed: {}",
                            error
                        )));
                    }
                }
            }
        }

        // Another task is downloading; subscribe and wait for terminal state.
        if let Some(mut rx) = force_update_tracker().wait_for_download(&key) {
            loop {
                let state = { rx.borrow().clone() };
                match state {
                    ForceUpdateDownloadState::Downloading { .. } => {
                        if rx.changed().await.is_err() {
                            break;
                        }
                    }
                    ForceUpdateDownloadState::Completed => return Ok(()),
                    ForceUpdateDownloadState::Failed(error) => {
                        return Err(LxAppError::IoError(format!(
                            "forced update package download failed: {}",
                            error
                        )));
                    }
                }
            }
        }

        // No active tracker entry visible. If package is already prepared, we're done.
        let prepared = matches!(
            manager.has_downloaded_update(target_appid, release_type),
            Ok(Some(info)) if info.version == pkg.version && info.archive_path.exists()
        );
        if prepared {
            return Ok(());
        }

        // Allow scheduler to make progress before retrying to acquire the downloader slot.
        tokio::task::yield_now().await;
    }
}

// Hashing for app data separation is provided by lxapp::lxapp_fingermark

fn filename_from_url_or_hash(url: &str) -> String {
    // naive parse: take last path segment before query/fragment
    let main = url.split(&['?', '#'][..]).next().unwrap_or(url);
    let seg = main.rsplit('/').next().unwrap_or(main);
    if !seg.is_empty() && seg.contains('.') {
        seg.to_string()
    } else {
        // default to hash.tar.zst
        format!("{}.tar.zst", UpdateManager::hash_url(url))
    }
}

fn app_update_filename(url: &str, version: &str) -> String {
    let safe_version = version.replace(['/', '\\'], "_");
    let main = url.split(&['?', '#'][..]).next().unwrap_or(url);
    let seg = main.rsplit('/').next().unwrap_or(main);
    if !seg.is_empty() && seg.contains('.') {
        format!("app_{}_{}", safe_version, seg)
    } else {
        format!("app_{}_{}.apk", safe_version, UpdateManager::hash_url(url))
    }
}

/// Get content length from URL via HEAD request
async fn get_content_length(url: &str) -> Result<u64, String> {
    use http::Request;
    use http_body_util::{BodyExt, Empty};
    use std::io::Error;

    let request = Request::builder()
        .method("HEAD")
        .uri(url)
        .body(
            Empty::<bytes::Bytes>::new()
                .map_err(|_| Error::new(std::io::ErrorKind::Other, "body error"))
                .boxed(),
        )
        .map_err(|e| format!("Failed to build HEAD request: {}", e))?;

    let response = service_executor::send_request(request, 1024, None)
        .await
        .map_err(|e| format!("HEAD request failed: {}", e))?;

    if let Some(content_length) = response
        .headers
        .get(http::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
    {
        Ok(content_length)
    } else {
        Err("No Content-Length header".to_string())
    }
}
