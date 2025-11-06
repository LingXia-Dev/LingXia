use crate::error::LxAppError;
use crate::lxapp::metadata::{LxAppRecord, SemanticVersion};
use crate::lxapp::{
    self, LINGXIA_DIR, LXAPPS_DIR, ReleaseType, STORAGE_DIR, USER_CACHE_DIR, USER_DATA_DIR,
    lxapp_fingermark, metadata, version, version::Version,
};
use lingxia_platform::{AppRuntime, Platform};
use ring::digest::{Context, SHA256};
use rong::service_executor;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use zip::read::ZipArchive;

/// Information about a downloadable update package from the cloud.
#[derive(Clone, Debug)]
pub struct UpdatePackageInfo {
    /// Semantic version string of the package.
    pub version: String,
    /// Download URL for the package.
    pub url: String,
    /// SHA-256 checksum (hex).
    pub checksum_sha256: String,
}

/// Result of an update check against the server.
#[derive(Clone, Debug)]
pub struct UpdateCheckResult {
    /// Whether a newer package is available.
    pub has_update: bool,
    /// When an update is available, contains package information.
    pub package: Option<UpdatePackageInfo>,
}

/// Coordinates update preparation, download, and installation for LxApps.
pub struct UpdateManager {
    /// Bound app reference used to access platform runtime (paths, fs) and app context.
    lxapp: Arc<lxapp::LxApp>,
    /// Directory where archives are downloaded before installation.
    downloads_dir: PathBuf,
}

#[derive(Clone, Debug)]
pub struct DownloadedUpdateInfo {
    pub version: String,
    pub zip_path: PathBuf,
}

impl UpdateManager {
    /// Download a package synchronously. When `version` is None, fetch from cloud to get latest.
    /// Returns the downloaded zip path and records it in `downloaded` table.
    /// Create a new UpdateManager bound to a specific LxApp.
    pub fn new(lxapp: Arc<lxapp::LxApp>) -> Self {
        let downloads_dir = lxapp
            .runtime
            .app_data_dir()
            .join(LINGXIA_DIR)
            .join(LXAPPS_DIR)
            .join("download");
        let _ = fs::create_dir_all(&downloads_dir);

        Self {
            lxapp,
            downloads_dir,
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
                zip_path: PathBuf::from(rec.zip_path),
            }),
        )
    }

    /// Returns whether the given lxappid+release_type is already installed
    pub fn is_installed(
        &self,
        lxappid: &str,
        release_type: ReleaseType,
    ) -> Result<bool, LxAppError> {
        metadata::exists(lxappid, release_type)
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
    /// Apply the given zip archive for `lxappid` with explicit release_type and version.
    pub fn apply_update_zip(
        &self,
        lxappid: &str,
        release_type: ReleaseType,
        _version: &str,
        zip_path: &Path,
    ) -> Result<(), LxAppError> {
        // Remember previous install path (if any)
        let previous_path =
            metadata::get(lxappid, release_type)?.map(|rec| PathBuf::from(rec.install_path));

        // Install into a new hashed directory for this version
        let install_path = self.install_from_zip(lxappid, release_type, _version, zip_path)?;

        // On successful install, remove previous package path if it differs
        if let Some(prev) = previous_path {
            if prev.exists() && prev != install_path {
                let _ = fs::remove_dir_all(prev);
            }
        }

        // Update metadata last, pointing to the new install path
        Self::record_install_metadata(lxappid, release_type, _version, &install_path)?;
        // clean downloaded entry and archive
        let _ = fs::remove_file(zip_path);
        let _ = metadata::downloaded_remove(lxappid, release_type);
        Ok(())
    }

    /// Install from a local zip package into the correct per-release path.
    pub fn install_from_zip(
        &self,
        lxappid: &str,
        release_type: ReleaseType,
        _version: &str,
        zip_path: &Path,
    ) -> Result<PathBuf, LxAppError> {
        let dir_name = lxapp_fingermark(lxappid, release_type);
        let destination = self
            .lxapp
            .runtime
            .app_data_dir()
            .join(LINGXIA_DIR)
            .join(LXAPPS_DIR)
            .join(dir_name);

        fs::create_dir_all(&destination)?;

        let file = File::open(zip_path)?;
        let mut archive = ZipArchive::new(file).map_err(|e| {
            LxAppError::IoError(format!(
                "Failed to read zip archive {}: {}",
                zip_path.display(),
                e
            ))
        })?;

        for index in 0..archive.len() {
            let mut entry = archive
                .by_index(index)
                .map_err(|e| LxAppError::IoError(format!("Cannot read zip entry #{index}: {e}")))?;

            let entry_path = match entry.enclosed_name() {
                Some(path) => destination.join(path),
                None => continue,
            };

            if entry.name().ends_with('/') {
                fs::create_dir_all(&entry_path)?;
                continue;
            }

            if let Some(parent) = entry_path.parent() {
                fs::create_dir_all(parent)?;
            }

            let mut outfile = File::create(&entry_path)?;
            io::copy(&mut entry, &mut outfile)?;
        }

        Ok(destination)
    }

    /// Uninstall on-disk contents for a specific (lxappid, release_type) and clear metadata.
    pub fn uninstall_installed(
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

    /// Download an archive to the internal downloads directory and return its path.
    /// If `checksum_sha256` is provided, verify the file before returning.
    pub fn download_archive_with_checksum(
        &self,
        url: &str,
        checksum_sha256: &str,
    ) -> Result<PathBuf, LxAppError> {
        let dest = self.dest_path_for_url(url);
        if dest.exists() {
            let _ = fs::remove_file(&dest);
        }
        let receiver =
            service_executor::request_download(url.to_string(), dest.clone(), None, None)
                .map_err(|e| LxAppError::IoError(format!("failed to start download: {}", e)))?;

        match receiver
            .blocking_recv()
            .map_err(|_| LxAppError::IoError("download task cancelled".to_string()))?
        {
            Ok(()) => {
                if !checksum_sha256.is_empty() {
                    if let Err(e) = maybe_verify_sha256(&dest, checksum_sha256) {
                        let _ = fs::remove_file(&dest);
                        return Err(e);
                    }
                }
                Ok(dest)
            }
            Err(err) => {
                let _ = fs::remove_file(&dest);
                Err(LxAppError::IoError(format!("download failed: {}", err)))
            }
        }
    }

    /// Async variant with checksum verification.
    pub async fn download_archive_with_checksum_async(
        &self,
        url: &str,
        checksum_sha256: &str,
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
                    if let Err(e) = maybe_verify_sha256(&dest, checksum_sha256) {
                        let _ = fs::remove_file(&dest);
                        return Err(e);
                    }
                }
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

        let parsed_version = Version::parse(version).map_err(|_| {
            LxAppError::InvalidParameter(format!("Invalid semantic version: {}", version))
        })?;

        let fingermark = lxapp_fingermark(lxappid, release_type);
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

    /// Check with the cloud whether a newer package is available.
    ///
    /// - `current_version`: None means client requests the latest package regardless of current state (first install).
    /// - Returns `UpdateCheckResult` with a package URL and checksum when an update is available.
    pub fn check_update(
        &self,
        _lxappid: &str,
        _release_type: ReleaseType,
        current_version: Option<&str>,
    ) -> Result<UpdateCheckResult, LxAppError> {
        let remote_version = "to do get".to_string();
        let newer_available = version::need_update(current_version, &remote_version);

        if !newer_available {
            return Ok(UpdateCheckResult {
                has_update: false,
                package: None,
            });
        }
        let url = "to get".to_string();

        Ok(UpdateCheckResult {
            has_update: true,
            package: Some(UpdatePackageInfo {
                version: remote_version,
                url,
                checksum_sha256: String::new(),
            }),
        })
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
        // default to hash.zip
        format!("{}.zip", UpdateManager::hash_url(url))
    }
}

/// Compute SHA-256 of a file and return lowercase hex string.
fn compute_sha256_hex(path: &Path) -> Result<String, LxAppError> {
    let mut file = File::open(path)?;
    let mut ctx = Context::new(&SHA256);
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        ctx.update(&buf[..n]);
    }
    let digest = ctx.finish();
    Ok(hex_lower(digest.as_ref()))
}

fn maybe_verify_sha256(path: &Path, expected_hex: &str) -> Result<(), LxAppError> {
    if expected_hex.is_empty() {
        return Ok(());
    }
    let actual = compute_sha256_hex(path)?;
    if actual.eq_ignore_ascii_case(expected_hex) {
        Ok(())
    } else {
        Err(LxAppError::IoError(format!(
            "checksum mismatch: expected {}, got {}",
            expected_hex, actual
        )))
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(hex_char((b >> 4) & 0x0f));
        s.push(hex_char(b & 0x0f));
    }
    s
}

#[inline]
fn hex_char(n: u8) -> char {
    match n {
        0..=9 => (b'0' + n) as char,
        _ => (b'a' + (n - 10)) as char,
    }
}
