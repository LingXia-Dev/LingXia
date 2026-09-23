//! Host-owned data-root override for isolated test runs.
//!
//! A host automation run can point one lxapp's storage, `lx://userdata`,
//! `lx://usercache` and `lx://temp` at a throwaway directory (a *profile*).
//! Nothing is written into the app and the app has no way to tell: only the
//! roots `initialize_paths` resolves move. Paths are read once per `LxApp`
//! instance, so every change is close → change → reopen, which also means no
//! Logic context holds the previous storage file when its directory is copied
//! or swapped.
//!
//! The override lives only in this process's memory. A crash or kill falls
//! back to the app's normal data, and the next startup sweeps the profile
//! directories left behind.
// Only the `automation` build can switch profiles; without it the override
// is never set and these helpers stay unused.
#![cfg_attr(not(feature = "automation"), allow(dead_code))]

use super::*;

/// Directory under `<data>/lingxia` holding every profile of this process.
pub const PROFILES_DIR: &str = "test-profiles";
/// Snapshot archive layout version.
pub const PROFILE_FORMAT: u32 = 1;
/// On-disk format of `lx.getStorage()`. A snapshot from another major redb
/// is refused rather than handed to a library that may not read it.
pub const STORAGE_FORMAT: &str = "redb-4";
/// Largest profile a snapshot may carry, unpacked.
pub const MAX_PROFILE_BYTES: u64 = 256 * 1024 * 1024;

const LIVE_DIR: &str = "live";
const CHECKPOINTS_DIR: &str = "checkpoints";
const MANIFEST_FILE: &str = "manifest.json";
const DATA_PREFIX: &str = "data";
const STORAGE_FILE: &str = "storage.redb";
const USER_DATA: &str = "userdata";
const USER_CACHE: &str = "usercache";
const TEMP: &str = "temp";

fn overrides() -> &'static DashMap<String, PathBuf> {
    static OVERRIDES: OnceLock<DashMap<String, PathBuf>> = OnceLock::new();
    OVERRIDES.get_or_init(DashMap::new)
}

/// The profile root currently overriding `appid`'s data, if any.
pub fn get(appid: &str) -> Option<PathBuf> {
    overrides().get(appid).map(|entry| entry.value().clone())
}

pub(crate) fn set(appid: &str, root: PathBuf) {
    overrides().insert(appid.to_string(), root);
}

pub(crate) fn clear(appid: &str) {
    overrides().remove(appid);
}

/// Data paths of an lxapp whose data lives in a profile.
pub(crate) struct ProfilePaths {
    pub storage_file: PathBuf,
    pub user_data: PathBuf,
    pub user_cache: PathBuf,
    pub temp_base: PathBuf,
}

fn paths(root: &Path) -> ProfilePaths {
    ProfilePaths {
        storage_file: root.join(STORAGE_FILE),
        user_data: root.join(USER_DATA),
        user_cache: root.join(USER_CACHE),
        temp_base: root.join(TEMP),
    }
}

pub(crate) fn paths_for(appid: &str) -> Option<ProfilePaths> {
    get(appid).map(|root| paths(&root))
}

/// `<data>/lingxia/test-profiles`: outside the usercache and install roots,
/// so product cache cleanup never reaches it.
pub fn profiles_base(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join(LINGXIA_DIR).join(PROFILES_DIR)
}

/// [`profiles_base`] of the running host.
pub fn host_profiles_base() -> Result<PathBuf, LxAppError> {
    let platform = super::runtime_registry::get_platform()
        .ok_or_else(|| LxAppError::Runtime("LingXia runtime is not initialized".to_string()))?;
    Ok(profiles_base(&platform.app_data_dir()))
}

/// Remove every profile directory no live override points into.
pub fn sweep_stale(base: &Path) {
    let Ok(entries) = fs::read_dir(base) else {
        return;
    };
    let live: Vec<PathBuf> = overrides()
        .iter()
        .map(|entry| entry.value().clone())
        .collect();
    for entry in entries.flatten() {
        let path = entry.path();
        if live.iter().any(|root| root.starts_with(&path)) {
            continue;
        }
        let removed = if path.is_dir() {
            fs::remove_dir_all(&path)
        } else {
            fs::remove_file(&path)
        };
        if let Err(err) = removed {
            warn!("failed to sweep test profile {}: {}", path.display(), err);
        }
    }
}

/// Identity a snapshot carries, checked before it is ever opened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct ProfileManifest {
    pub format: u32,
    pub appid: String,
    pub fingermark: String,
    pub created_at_ms: u64,
    pub lingxia_version: String,
    pub storage_format: String,
}

impl ProfileManifest {
    /// Refuse a snapshot of another app, another channel/device, or a
    /// storage format this build does not write.
    pub fn check_matches(&self, expected: &ProfileManifest) -> Result<(), LxAppError> {
        if self.format != PROFILE_FORMAT {
            return Err(LxAppError::InvalidParameter(format!(
                "profile snapshot format {} is not supported (expected {PROFILE_FORMAT})",
                self.format
            )));
        }
        if self.storage_format != expected.storage_format {
            return Err(LxAppError::InvalidParameter(format!(
                "profile snapshot storage format {} does not match this host ({})",
                self.storage_format, expected.storage_format
            )));
        }
        if self.appid != expected.appid {
            return Err(LxAppError::InvalidParameter(format!(
                "profile snapshot belongs to lxapp {}, not {}",
                self.appid, expected.appid
            )));
        }
        if self.fingermark != expected.fingermark {
            return Err(LxAppError::InvalidParameter(format!(
                "profile snapshot of {} was taken for another channel or device; save it again \
                 on this host",
                self.appid
            )));
        }
        Ok(())
    }
}

/// The manifest a snapshot of `appid` taken on this host carries now.
pub fn manifest_for(appid: &str) -> ProfileManifest {
    let fingermark = super::runtime_registry::try_get(appid)
        .map(|app| app.fingermark.clone())
        .filter(|fingermark| !fingermark.is_empty())
        .or_else(|| {
            metadata::get(appid, crate::default_channel())
                .ok()
                .flatten()
                .map(|record| record.fingermark)
        })
        .unwrap_or_else(|| lxapp_fingermark(appid, crate::default_channel()));
    manifest_with(appid, fingermark)
}

fn manifest_with(appid: &str, fingermark: String) -> ProfileManifest {
    ProfileManifest {
        format: PROFILE_FORMAT,
        appid: appid.to_string(),
        fingermark,
        created_at_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis() as u64)
            .unwrap_or_default(),
        lingxia_version: crate::SDK_RUNTIME_VERSION.to_string(),
        storage_format: STORAGE_FORMAT.to_string(),
    }
}

/// One run's profile directory: `live/` is what the app sees,
/// `checkpoints/<id>/` are closed-app copies of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunProfile {
    dir: PathBuf,
}

impl RunProfile {
    /// Create a fresh, empty profile directory under `base`.
    pub fn create(base: &Path) -> Result<Self, LxAppError> {
        let dir = base.join(Uuid::new_v4().simple().to_string());
        create_private_dir(&dir)?;
        create_private_dir(&dir.join(LIVE_DIR))?;
        create_private_dir(&dir.join(CHECKPOINTS_DIR))?;
        Ok(Self { dir })
    }

    /// Adopt an existing profile directory. It must sit directly under
    /// `base`, so a caller-supplied path can never reach the app's real data.
    pub fn open(base: &Path, dir: &Path) -> Result<Self, LxAppError> {
        if dir.parent() != Some(base) || !dir.join(LIVE_DIR).is_dir() {
            return Err(LxAppError::InvalidParameter(format!(
                "{} is not a test profile",
                dir.display()
            )));
        }
        Ok(Self {
            dir: dir.to_path_buf(),
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The root the override points at.
    pub fn live(&self) -> PathBuf {
        self.dir.join(LIVE_DIR)
    }

    fn checkpoint(&self, id: &str) -> Result<PathBuf, LxAppError> {
        let valid = !id.is_empty()
            && id.len() <= 64
            && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
        if !valid {
            return Err(LxAppError::InvalidParameter(format!(
                "invalid profile checkpoint id: {id:?}"
            )));
        }
        Ok(self.dir.join(CHECKPOINTS_DIR).join(id))
    }

    /// Whether `appid` currently runs on this profile.
    pub fn is_active_for(&self, appid: &str) -> bool {
        get(appid).is_some_and(|root| root == self.live())
    }

    pub fn remove(&self) -> Result<(), LxAppError> {
        match fs::remove_dir_all(&self.dir) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err.into()),
        }
    }

    /// Discard a checkpoint. Unknown ids are not an error.
    pub fn drop_checkpoint(&self, id: &str) -> Result<(), LxAppError> {
        let dir = self.checkpoint(id)?;
        match fs::remove_dir_all(dir) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err.into()),
        }
    }

    fn copy_live_to_checkpoint(&self, id: &str) -> Result<(), LxAppError> {
        let target = self.checkpoint(id)?;
        copy_dir(&self.live(), &target)
    }

    fn replace_live_from_checkpoint(&self, id: &str) -> Result<(), LxAppError> {
        let source = self.checkpoint(id)?;
        if !source.is_dir() {
            return Err(LxAppError::ResourceNotFound(format!(
                "profile checkpoint {id}"
            )));
        }
        let live = self.live();
        // Stage next to live first, so a failed copy leaves live intact.
        let staged = self
            .dir
            .join(format!("restore-{}", Uuid::new_v4().simple()));
        copy_dir(&source, &staged)?;
        if live.exists() {
            fs::remove_dir_all(&live)?;
        }
        fs::rename(&staged, &live)?;
        Ok(())
    }
}

fn create_private_dir(dir: &Path) -> Result<(), LxAppError> {
    fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// Recursive copy of regular files and directories; symlinks are skipped.
/// `std::fs::copy` clones on APFS, so a large redb costs no extra space.
pub fn copy_dir(source: &Path, target: &Path) -> Result<(), LxAppError> {
    create_private_dir(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let to = target.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir(&entry.path(), &to)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), &to)?;
        }
    }
    Ok(())
}

fn dir_size(dir: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => dir_size(&entry.path()),
            Ok(kind) if kind.is_file() => entry.metadata().map(|meta| meta.len()).unwrap_or(0),
            _ => 0,
        })
        .sum()
}

/// Pack a closed profile as `tar.zst`: `manifest.json`, then `data/`.
/// Session temp files are left out; they are wiped on every open anyway.
pub fn pack(live: &Path, manifest: &ProfileManifest) -> Result<Vec<u8>, LxAppError> {
    let size = dir_size(live);
    if size > MAX_PROFILE_BYTES {
        return Err(LxAppError::InvalidParameter(format!(
            "profile is {size} bytes; a snapshot holds at most {MAX_PROFILE_BYTES}"
        )));
    }
    let io = |err: std::io::Error| LxAppError::IoError(format!("pack profile: {err}"));
    let encoder = zstd::stream::write::Encoder::new(Vec::new(), 3).map_err(io)?;
    let mut builder = tar::Builder::new(encoder);
    builder.follow_symlinks(false);
    let manifest = serde_json::to_vec_pretty(manifest)
        .map_err(|err| LxAppError::IoError(format!("encode profile manifest: {err}")))?;
    let mut header = tar::Header::new_gnu();
    header.set_size(manifest.len() as u64);
    header.set_mode(0o600);
    header.set_cksum();
    builder
        .append_data(&mut header, MANIFEST_FILE, manifest.as_slice())
        .map_err(io)?;
    for name in [STORAGE_FILE, USER_DATA, USER_CACHE] {
        let path = live.join(name);
        let entry = Path::new(DATA_PREFIX).join(name);
        if path.is_file() {
            builder.append_path_with_name(&path, &entry).map_err(io)?;
        } else if path.is_dir() {
            builder.append_dir_all(&entry, &path).map_err(io)?;
        }
    }
    builder.into_inner().map_err(io)?.finish().map_err(io)
}

/// Lowercase hex SHA-256 of a packed snapshot.
pub fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let digest = ring::digest::digest(&ring::digest::SHA256, bytes);
    digest
        .as_ref()
        .iter()
        .fold(String::with_capacity(64), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        })
}

fn decoder(bytes: &[u8]) -> Result<tar::Archive<impl Read + '_>, LxAppError> {
    let decoder = zstd::stream::read::Decoder::new(bytes)
        .map_err(|err| LxAppError::InvalidParameter(format!("not a profile snapshot: {err}")))?;
    Ok(tar::Archive::new(decoder))
}

/// Read a snapshot's manifest without unpacking it.
pub fn read_manifest(bytes: &[u8]) -> Result<ProfileManifest, LxAppError> {
    let mut archive = decoder(bytes)?;
    let invalid = |err: std::io::Error| {
        LxAppError::InvalidParameter(format!("not a profile snapshot: {err}"))
    };
    for entry in archive.entries().map_err(invalid)? {
        let mut entry = entry.map_err(invalid)?;
        if entry.path().map_err(invalid)?.as_ref() == Path::new(MANIFEST_FILE) {
            let mut raw = Vec::new();
            entry
                .by_ref()
                .take(64 * 1024)
                .read_to_end(&mut raw)
                .map_err(invalid)?;
            return serde_json::from_slice(&raw).map_err(|err| {
                LxAppError::InvalidParameter(format!("invalid profile manifest: {err}"))
            });
        }
    }
    Err(LxAppError::InvalidParameter(
        "profile snapshot has no manifest".to_string(),
    ))
}

/// Unpack a snapshot into `live`, which must be empty, after checking it
/// against `expected`. Only regular files and directories under `data/` are
/// written; anything else is ignored.
pub fn unpack(
    bytes: &[u8],
    live: &Path,
    expected: &ProfileManifest,
) -> Result<ProfileManifest, LxAppError> {
    let manifest = read_manifest(bytes)?;
    manifest.check_matches(expected)?;
    let staging = live.with_file_name(format!("unpack-{}", Uuid::new_v4().simple()));
    create_private_dir(&staging)?;
    let result = unpack_data(bytes, &staging);
    if let Err(err) = result {
        let _ = fs::remove_dir_all(&staging);
        return Err(err);
    }
    let data = staging.join(DATA_PREFIX);
    create_private_dir(&data)?;
    if live.exists() {
        fs::remove_dir_all(live)?;
    }
    fs::rename(&data, live)?;
    let _ = fs::remove_dir_all(&staging);
    Ok(manifest)
}

fn unpack_data(bytes: &[u8], staging: &Path) -> Result<(), LxAppError> {
    let invalid = |err: std::io::Error| {
        LxAppError::InvalidParameter(format!("invalid profile snapshot: {err}"))
    };
    let mut archive = decoder(bytes)?;
    let mut total = 0u64;
    for entry in archive.entries().map_err(invalid)? {
        let mut entry = entry.map_err(invalid)?;
        let kind = entry.header().entry_type();
        if !(kind.is_file() || kind.is_dir()) {
            continue;
        }
        if !entry
            .path()
            .map_err(invalid)?
            .starts_with(Path::new(DATA_PREFIX))
        {
            continue;
        }
        total = total.saturating_add(entry.size());
        if total > MAX_PROFILE_BYTES {
            return Err(LxAppError::InvalidParameter(format!(
                "profile snapshot unpacks to more than {MAX_PROFILE_BYTES} bytes"
            )));
        }
        entry.set_preserve_permissions(false);
        entry.set_unpack_xattrs(false);
        // `unpack_in` refuses any path that would land outside `staging`.
        if !entry.unpack_in(staging).map_err(invalid)? {
            return Err(LxAppError::InvalidParameter(
                "profile snapshot entry escapes its directory".to_string(),
            ));
        }
    }
    Ok(())
}

#[cfg(feature = "automation")]
pub use switching::*;

#[cfg(feature = "automation")]
mod switching {
    use super::*;

    /// How long a closed app may take to drop its Logic contexts, and a
    /// reopened one to become ready.
    const CLOSE_TIMEOUT: Duration = Duration::from_secs(10);
    const REOPEN_TIMEOUT: Duration = Duration::from_secs(20);

    fn switch_lock() -> &'static tokio::sync::Mutex<()> {
        static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
        &LOCK
    }

    struct Reopen {
        release_type: Channel,
        path: String,
        open_mode: lingxia_platform::traits::app_runtime::LxAppOpenMode,
        panel_id: String,
    }

    fn manager() -> Result<Arc<LxApps>, LxAppError> {
        super::super::runtime_registry::get_lxapps_manager()
            .ok_or_else(|| LxAppError::Runtime("LxApps manager not initialized".to_string()))
    }

    /// Point `appid` at `profile` (entering an isolated run), closing and
    /// reopening it if it is live. The override is set only once the app's
    /// Logic has stopped, so the previous data is never open twice.
    pub async fn enter(appid: &str, profile: &RunProfile) -> Result<(), LxAppError> {
        let live = profile.live();
        let appid_owned = appid.to_string();
        with_app_closed(&manager()?, appid, move || {
            set(&appid_owned, live);
            Ok(())
        })
        .await
    }

    /// Return `appid` to its normal data. The override is cleared first, so
    /// whatever happens to the close and reopen, the next instance of the app
    /// uses its own data again.
    pub async fn leave(appid: &str) -> Result<(), LxAppError> {
        let previous = get(appid);
        clear(appid);
        if let Some(root) = previous {
            cancel_downloads_under(&root);
        }
        with_app_closed(&manager()?, appid, || Ok(())).await
    }

    /// Copy the closed profile into a new checkpoint and return its id.
    /// Refused unless `appid` currently runs on `profile`.
    pub async fn checkpoint(appid: &str, profile: &RunProfile) -> Result<String, LxAppError> {
        require_active(appid, profile)?;
        let id = Uuid::new_v4().simple().to_string();
        let profile_owned = profile.clone();
        let checkpoint = id.clone();
        with_app_closed(&manager()?, appid, move || {
            profile_owned.copy_live_to_checkpoint(&checkpoint)
        })
        .await?;
        Ok(id)
    }

    /// Replace the profile with checkpoint `id`. Refused unless `appid`
    /// currently runs on `profile`, so it can never touch the app's real data.
    pub async fn restore(appid: &str, profile: &RunProfile, id: &str) -> Result<(), LxAppError> {
        require_active(appid, profile)?;
        profile.checkpoint(id)?;
        let profile_owned = profile.clone();
        let checkpoint = id.to_string();
        with_app_closed(&manager()?, appid, move || {
            profile_owned.replace_live_from_checkpoint(&checkpoint)
        })
        .await
    }

    fn require_active(appid: &str, profile: &RunProfile) -> Result<(), LxAppError> {
        if profile.is_active_for(appid) {
            Ok(())
        } else {
            Err(LxAppError::UnsupportedOperation(format!(
                "lxapp {appid} is not running on an isolated test profile"
            )))
        }
    }

    /// Stop downloads still writing into a profile the app is leaving.
    fn cancel_downloads_under(root: &Path) {
        let Some(platform) = super::super::runtime_registry::get_platform() else {
            return;
        };
        let data_dir = platform.app_data_dir();
        let Ok(snapshot) = lingxia_transfer::snapshot(&data_dir) else {
            return;
        };
        for record in snapshot.downloads {
            let active = matches!(
                record.status,
                lingxia_transfer::DownloadStatus::Downloading
                    | lingxia_transfer::DownloadStatus::Paused
            );
            if active && Path::new(&record.target_path).starts_with(root) {
                let _ = lingxia_transfer::cancel(&data_dir, &record.task_id);
            }
        }
    }

    /// Close `appid` (waiting until its Logic contexts are gone), run `op`,
    /// then reopen it where it was and wait for its page. An app that is not
    /// live only runs `op`.
    pub(crate) async fn with_app_closed<T>(
        manager: &Arc<LxApps>,
        appid: &str,
        op: impl FnOnce() -> Result<T, LxAppError>,
    ) -> Result<T, LxAppError> {
        let _serial = switch_lock().lock().await;
        let Some(app) = manager.lxapps.get(appid).map(|entry| entry.value().clone()) else {
            return op();
        };
        let reopen = matches!(
            app.status(),
            LxAppSessionStatus::Opened | LxAppSessionStatus::Opening
        )
        .then(|| {
            let state = app.state.lock().unwrap_or_else(|err| err.into_inner());
            Reopen {
                release_type: app.release_type,
                path: app.config().get_initial_route(),
                open_mode: state.startup_options.open_mode,
                panel_id: state.startup_options.panel_id.clone(),
            }
        });
        let mut contexts = app.logic_contexts.subscribe();
        manager.retire_lxapp(appid)?;
        drop(app);
        tokio::time::timeout(CLOSE_TIMEOUT, contexts.wait_for(|count| *count == 0))
            .await
            .map_err(|_| LxAppError::Runtime(format!("timed out waiting for {appid} to close")))?
            .map_err(|_| LxAppError::Runtime(format!("{appid} Logic observer closed")))?;

        let value = op()?;
        if let Some(reopen) = reopen {
            reopen_and_wait(manager, appid, reopen).await?;
        }
        Ok(value)
    }

    async fn reopen_and_wait(
        manager: &Arc<LxApps>,
        appid: &str,
        reopen: Reopen,
    ) -> Result<(), LxAppError> {
        let app = manager.ensure_lxapp(appid.to_string(), reopen.release_type)?;
        app.open(
            LxAppStartupOptions::new(&reopen.path)
                .set_release_type(reopen.release_type)
                .set_open_mode(reopen.open_mode)
                .set_panel_id(reopen.panel_id),
        )?;
        let deadline = tokio::time::Instant::now() + REOPEN_TIMEOUT;
        // Native containers create the first page asynchronously.
        let page = loop {
            if let Ok(page) = app.current_page() {
                break page;
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(LxAppError::Runtime(format!(
                    "timed out waiting for {appid} to reopen"
                )));
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        };
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        crate::automation::wait_page_runtime_ready(&app, &page, remaining)
            .await
            .map_err(LxAppError::Runtime)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::appservice::LxAppWorkers;

        #[test]
        fn switching_waits_for_logic_to_stop_before_touching_data() {
            #[cfg(target_vendor = "apple")]
            let _host = crate::apple_host_stubs::headless_lifecycle();
            let appid = format!("app.lingxia.profile-switch.{}", Uuid::new_v4());
            register_synthetic_lxapp(appid.clone());
            let root =
                std::env::temp_dir().join(format!("lingxia-profile-switch-{}", Uuid::new_v4()));
            let runtime = Platform::new(
                root.join("data").display().to_string(),
                root.join("cache").display().to_string(),
                "en-US".to_string(),
            )
            .expect("test platform");
            let manager = Arc::new(LxApps::new(runtime, LxAppWorkers::init(1), 2));
            let app = manager
                .ensure_lxapp(appid.clone(), Channel::Release)
                .unwrap();
            // A Logic context that has not shut down yet.
            app.logic_contexts.send_replace(1);
            let contexts = app.logic_contexts.clone();

            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let ran = Arc::new(AtomicBool::new(false));
                    let ran_in_op = ran.clone();
                    let switch = with_app_closed(&manager, &appid, move || {
                        ran_in_op.store(true, Ordering::SeqCst);
                        Ok(())
                    });
                    tokio::pin!(switch);
                    let early = tokio::time::timeout(Duration::from_millis(200), &mut switch).await;
                    assert!(early.is_err(), "must wait for Logic to stop");
                    assert!(!ran.load(Ordering::SeqCst), "data changed under live Logic");
                    assert!(!manager.lxapps.contains_key(&appid), "the app was closed");

                    contexts.send_replace(0);
                    switch.await.unwrap();
                    assert!(ran.load(Ordering::SeqCst));
                });
            let _ = fs::remove_dir_all(root);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lingxia-profile-{name}-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn manifest(appid: &str) -> ProfileManifest {
        manifest_with(appid, "fm-1".to_string())
    }

    fn seed(live: &Path) {
        fs::write(live.join(STORAGE_FILE), b"redb-bytes").unwrap();
        fs::create_dir_all(live.join(USER_DATA).join("nested")).unwrap();
        fs::write(live.join(USER_DATA).join("nested").join("a.txt"), b"hello").unwrap();
        fs::create_dir_all(live.join(TEMP)).unwrap();
        fs::write(live.join(TEMP).join("scratch"), b"dropped").unwrap();
    }

    #[test]
    fn override_moves_every_data_root_under_the_profile() {
        let appid = format!("app.lingxia.profile.{}", Uuid::new_v4());
        assert!(paths_for(&appid).is_none());
        let root = PathBuf::from("/profiles/run/live");
        set(&appid, root.clone());
        let paths = paths_for(&appid).expect("override");
        assert_eq!(paths.storage_file, root.join("storage.redb"));
        assert_eq!(paths.user_data, root.join("userdata"));
        assert_eq!(paths.user_cache, root.join("usercache"));
        assert_eq!(paths.temp_base, root.join("temp"));
        clear(&appid);
        assert!(paths_for(&appid).is_none());
    }

    #[test]
    fn pack_and_unpack_round_trip_without_temp_files() {
        let base = scratch("pack");
        let source = RunProfile::create(&base).unwrap();
        seed(&source.live());
        let bytes = pack(&source.live(), &manifest("app.one")).unwrap();
        assert_eq!(read_manifest(&bytes).unwrap().appid, "app.one");

        let target = RunProfile::create(&base).unwrap();
        unpack(&bytes, &target.live(), &manifest("app.one")).unwrap();
        let live = target.live();
        assert_eq!(fs::read(live.join(STORAGE_FILE)).unwrap(), b"redb-bytes");
        assert_eq!(
            fs::read(live.join(USER_DATA).join("nested").join("a.txt")).unwrap(),
            b"hello"
        );
        assert!(!live.join(TEMP).exists(), "session temp is not snapshotted");
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn unpack_refuses_another_app_or_device() {
        let base = scratch("mismatch");
        let source = RunProfile::create(&base).unwrap();
        seed(&source.live());
        let bytes = pack(&source.live(), &manifest("app.one")).unwrap();
        let target = RunProfile::create(&base).unwrap();

        let other_app = unpack(&bytes, &target.live(), &manifest("app.two")).unwrap_err();
        assert!(other_app.to_string().contains("belongs to lxapp app.one"));
        let other_device = manifest_with("app.one", "fm-2".to_string());
        assert!(unpack(&bytes, &target.live(), &other_device).is_err());
        let mut newer = manifest("app.one");
        newer.storage_format = "redb-5".to_string();
        assert!(unpack(&bytes, &target.live(), &newer).is_err());
        assert!(
            fs::read_dir(target.live()).unwrap().next().is_none(),
            "a refused seed writes nothing"
        );
        assert!(read_manifest(b"not zstd").is_err());
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn checkpoint_restore_replaces_the_live_profile() {
        let base = scratch("checkpoint");
        let profile = RunProfile::create(&base).unwrap();
        seed(&profile.live());
        profile.copy_live_to_checkpoint("cp-1").unwrap();
        fs::write(profile.live().join(STORAGE_FILE), b"changed").unwrap();
        fs::write(profile.live().join(USER_DATA).join("new.txt"), b"x").unwrap();

        profile.replace_live_from_checkpoint("cp-1").unwrap();
        assert_eq!(
            fs::read(profile.live().join(STORAGE_FILE)).unwrap(),
            b"redb-bytes"
        );
        assert!(!profile.live().join(USER_DATA).join("new.txt").exists());
        profile.drop_checkpoint("cp-1").unwrap();
        assert!(profile.replace_live_from_checkpoint("cp-1").is_err());
        assert!(profile.checkpoint("../escape").is_err());
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn open_only_adopts_a_profile_directly_under_the_base() {
        let base = scratch("open");
        let profile = RunProfile::create(&base).unwrap();
        assert_eq!(RunProfile::open(&base, profile.dir()).unwrap(), profile);
        assert!(RunProfile::open(&base, &base).is_err());
        assert!(RunProfile::open(&base, &profile.live()).is_err());
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn sweep_keeps_only_profiles_an_override_points_into() {
        let base = scratch("sweep");
        let kept = RunProfile::create(&base).unwrap();
        let stale = RunProfile::create(&base).unwrap();
        fs::write(base.join("staged-upload.part"), b"x").unwrap();
        let appid = format!("app.lingxia.sweep.{}", Uuid::new_v4());
        set(&appid, kept.live());

        sweep_stale(&base);
        assert!(kept.dir().exists());
        assert!(!stale.dir().exists());
        assert!(!base.join("staged-upload.part").exists());

        clear(&appid);
        sweep_stale(&base);
        assert!(!kept.dir().exists());
        let _ = fs::remove_dir_all(base);
    }
}
