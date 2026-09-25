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
use std::collections::BTreeMap;

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

    #[cfg(test)]
    fn replace_live_from_checkpoint(&self, id: &str) -> Result<(), LxAppError> {
        self.replace_live_keeping(id, &KeepKeys::default())
            .map(|_| ())
    }

    /// Replace live with checkpoint `id`, carrying the current value (or
    /// absence) of every storage key `keep` matches into the restored data.
    /// Resolves the keys whose current values were carried over. The merge
    /// happens on the staged copy, so live is either untouched or fully
    /// replaced: the reopened app never sees the checkpoint's values of a
    /// kept key.
    fn replace_live_keeping(&self, id: &str, keep: &KeepKeys) -> Result<Vec<String>, LxAppError> {
        let source = self.checkpoint(id)?;
        if !source.is_dir() {
            return Err(LxAppError::ResourceNotFound(format!(
                "profile checkpoint {id}"
            )));
        }
        let live = self.live();
        let kept = if keep.is_empty() {
            BTreeMap::new()
        } else {
            read_kept(&live.join(STORAGE_FILE), keep)?
        };
        // Stage next to live first, so a failed copy or merge leaves live
        // intact.
        let staged = self
            .dir
            .join(format!("restore-{}", Uuid::new_v4().simple()));
        let prepared = copy_dir(&source, &staged).and_then(|()| {
            if keep.is_empty() {
                Ok(())
            } else {
                apply_kept(&staged.join(STORAGE_FILE), keep, &kept)
            }
        });
        if let Err(err) = prepared {
            let _ = fs::remove_dir_all(&staged);
            return Err(err);
        }
        if live.exists() {
            fs::remove_dir_all(&live)?;
        }
        fs::rename(&staged, &live)?;
        Ok(kept.into_keys().collect())
    }
}

/// Most patterns one rollback may keep.
pub const MAX_KEEP_PATTERNS: usize = 64;
/// Longest keep pattern, the same bound `lx.getStorage()` puts on a key.
pub const MAX_KEEP_PATTERN_BYTES: usize = 1024;

/// `lx.getStorage()` keys a rollback keeps at their current values: globs
/// where `*` matches any run of characters (dots included) and `?` exactly
/// one; everything else is literal. Only storage keys are kept, never files.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeepKeys {
    patterns: Vec<String>,
}

impl KeepKeys {
    pub fn new(patterns: Vec<String>) -> Result<Self, LxAppError> {
        if patterns.len() > MAX_KEEP_PATTERNS {
            return Err(LxAppError::InvalidParameter(format!(
                "a rollback keeps at most {MAX_KEEP_PATTERNS} key patterns, got {}",
                patterns.len()
            )));
        }
        for pattern in &patterns {
            if pattern.is_empty() || pattern.len() > MAX_KEEP_PATTERN_BYTES {
                return Err(LxAppError::InvalidParameter(format!(
                    "keep pattern must be 1..={MAX_KEEP_PATTERN_BYTES} bytes, got {:?}",
                    pattern
                )));
            }
        }
        Ok(Self { patterns })
    }

    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    pub fn matches(&self, key: &str) -> bool {
        self.patterns
            .iter()
            .any(|pattern| glob_matches(pattern, key))
    }
}

/// `*` any run of characters, `?` one character, the rest literal.
fn glob_matches(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = text.chars().collect();
    let (mut p, mut t) = (0, 0);
    // Where the last `*` was and how much text it had absorbed.
    let mut star: Option<(usize, usize)> = None;
    while t < text.len() {
        match pattern.get(p) {
            Some('*') => {
                star = Some((p, t));
                p += 1;
            }
            Some(&c) if c == '?' || c == text[t] => {
                p += 1;
                t += 1;
            }
            _ => match star {
                Some((star_p, star_t)) => {
                    p = star_p + 1;
                    t = star_t + 1;
                    star = Some((star_p, star_t + 1));
                }
                None => return false,
            },
        }
    }
    pattern[p..].iter().all(|c| *c == '*')
}

/// The table `lx.getStorage()` (rong_storage, [`STORAGE_FORMAT`]) keeps its
/// entries in: string keys, encoded values copied as opaque bytes.
const STORAGE_TABLE: redb::TableDefinition<&str, &[u8]> = redb::TableDefinition::new("storage");

fn storage_err(context: &str, err: impl std::fmt::Display) -> LxAppError {
    LxAppError::IoError(format!("{context} profile storage: {err}"))
}

/// Current entries of the closed storage file whose keys `keep` matches.
fn read_kept(storage: &Path, keep: &KeepKeys) -> Result<BTreeMap<String, Vec<u8>>, LxAppError> {
    use redb::{ReadableDatabase, ReadableTable};
    let mut kept = BTreeMap::new();
    if !storage.is_file() {
        return Ok(kept);
    }
    let db = redb::Database::open(storage).map_err(|err| storage_err("open", err))?;
    let txn = db.begin_read().map_err(|err| storage_err("read", err))?;
    let table = match txn.open_table(STORAGE_TABLE) {
        Ok(table) => table,
        Err(redb::TableError::TableDoesNotExist(_)) => return Ok(kept),
        Err(err) => return Err(storage_err("read", err)),
    };
    for entry in table.iter().map_err(|err| storage_err("read", err))? {
        let (key, value) = entry.map_err(|err| storage_err("read", err))?;
        if keep.matches(key.value()) {
            kept.insert(key.value().to_string(), value.value().to_vec());
        }
    }
    Ok(kept)
}

/// Make the keys `keep` matches in the closed storage file exactly `kept`:
/// matching keys absent from `kept` are removed, the others written.
fn apply_kept(
    storage: &Path,
    keep: &KeepKeys,
    kept: &BTreeMap<String, Vec<u8>>,
) -> Result<(), LxAppError> {
    use redb::ReadableTable;
    if !storage.is_file() && kept.is_empty() {
        return Ok(());
    }
    let db = redb::Database::create(storage).map_err(|err| storage_err("open", err))?;
    let txn = db.begin_write().map_err(|err| storage_err("write", err))?;
    {
        let mut table = txn
            .open_table(STORAGE_TABLE)
            .map_err(|err| storage_err("write", err))?;
        let mut stale = Vec::new();
        for entry in table.iter().map_err(|err| storage_err("read", err))? {
            let (key, _) = entry.map_err(|err| storage_err("read", err))?;
            let key = key.value();
            if keep.matches(key) && !kept.contains_key(key) {
                stale.push(key.to_string());
            }
        }
        for key in stale {
            table
                .remove(key.as_str())
                .map_err(|err| storage_err("write", err))?;
        }
        for (key, value) in kept {
            table
                .insert(key.as_str(), value.as_slice())
                .map_err(|err| storage_err("write", err))?;
        }
    }
    txn.commit().map_err(|err| storage_err("write", err))
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
    /// How long a reopened app, once a page is ready, gets to finish its
    /// start-up work before the caller moves on anyway.
    const SETTLE_TIMEOUT: Duration = Duration::from_secs(5);
    /// How long the current page must stay the same, and ready, to count as
    /// settled.
    const SETTLE_QUIET: Duration = Duration::from_millis(300);

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
        restore_keeping(appid, profile, id, &KeepKeys::default())
            .await
            .map(|_| ())
    }

    /// [`restore`], carrying the current values of the storage keys `keep`
    /// matches into the restored profile while the app is closed. Resolves
    /// the keys carried over.
    pub async fn restore_keeping(
        appid: &str,
        profile: &RunProfile,
        id: &str,
        keep: &KeepKeys,
    ) -> Result<Vec<String>, LxAppError> {
        require_active(appid, profile)?;
        profile.checkpoint(id)?;
        let profile_owned = profile.clone();
        let checkpoint = id.to_string();
        let keep = keep.clone();
        with_app_closed(&manager()?, appid, move || {
            profile_owned.replace_live_keeping(&checkpoint, &keep)
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
        // A development bundle is read straight from the project's build
        // output, which a rebuild empties and refills: a reopen that lands
        // in between finds no `lxapp.json`. Wait the rebuild out instead of
        // leaving the app closed. Installed bundles fail for good.
        let retryable = !super::super::is_ota_managed_appid(appid);
        let app = retry_while(retryable, REOPEN_TIMEOUT, REBUILD_RETRY, appid, || {
            let app = manager.ensure_lxapp(appid.to_string(), reopen.release_type)?;
            app.open(
                LxAppStartupOptions::new(&reopen.path)
                    .set_release_type(reopen.release_type)
                    .set_open_mode(reopen.open_mode)
                    .set_panel_id(reopen.panel_id.clone()),
            )?;
            Ok(app)
        })
        .await?;
        wait_settled(&app).await
    }

    /// How often a reopen retries while a development bundle is rebuilt.
    const REBUILD_RETRY: Duration = Duration::from_millis(250);

    /// Run `attempt` until it succeeds; when `retryable`, a failure is
    /// retried every `interval` for up to `timeout`, then the last error is
    /// returned.
    pub(super) async fn retry_while<T>(
        retryable: bool,
        timeout: Duration,
        interval: Duration,
        appid: &str,
        mut attempt: impl FnMut() -> Result<T, LxAppError>,
    ) -> Result<T, LxAppError> {
        let deadline = tokio::time::Instant::now() + timeout;
        let mut warned = false;
        loop {
            match attempt() {
                Ok(value) => return Ok(value),
                Err(err) if retryable && tokio::time::Instant::now() < deadline => {
                    if !warned {
                        warn!(
                            "reopening {appid} failed ({err}); retrying while its bundle is rebuilt"
                        );
                        warned = true;
                    }
                    tokio::time::sleep(interval).await;
                }
                Err(err) => return Err(err),
            }
        }
    }

    /// Wait for a reopened app to settle: `App.onLaunch` has finished, a page
    /// is ready, and the current page has not changed for [`SETTLE_QUIET`].
    /// Start-up work the app kicks off itself (a session probe that
    /// redirects) then lands before the caller moves on, instead of under
    /// whatever runs next. Failing to get a ready page within
    /// [`REOPEN_TIMEOUT`] is an error; start-up work still running
    /// [`SETTLE_TIMEOUT`] after that is logged and left running.
    async fn wait_settled(app: &Arc<LxApp>) -> Result<(), LxAppError> {
        let launched = app.launch_settled.subscribe();
        let mut settle = Settle::new(tokio::time::Instant::now());
        loop {
            let current = app.current_page().ok().map(|page| {
                let state = page.automation_state();
                SettleView {
                    page: page.instance_id_string(),
                    ready: state.ready,
                    error: state.webview_error,
                }
            });
            let now = tokio::time::Instant::now();
            match settle.observe(now, *launched.borrow(), current) {
                SettleStep::Wait => {}
                SettleStep::Settled => return Ok(()),
                SettleStep::Unsettled(what) => {
                    warn!(
                        "{} still {what} {}ms after reopening; not waiting longer",
                        app.appid,
                        SETTLE_TIMEOUT.as_millis()
                    );
                    return Ok(());
                }
                SettleStep::Failed(message) => {
                    return Err(LxAppError::Runtime(format!("{}: {message}", app.appid)));
                }
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    /// What [`wait_settled`] sees of the current page on one poll.
    pub(super) struct SettleView {
        pub page: String,
        pub ready: bool,
        pub error: Option<String>,
    }

    #[derive(Debug, PartialEq, Eq)]
    pub(super) enum SettleStep {
        Wait,
        Settled,
        /// A page is ready but start-up work is still going; the string says what.
        Unsettled(&'static str),
        Failed(String),
    }

    /// The settle decision, kept apart from the clock and the app so it can
    /// be driven step by step.
    pub(super) struct Settle {
        ready_deadline: tokio::time::Instant,
        settle_deadline: Option<tokio::time::Instant>,
        page: Option<String>,
        since: tokio::time::Instant,
    }

    impl Settle {
        pub(super) fn new(now: tokio::time::Instant) -> Self {
            Self {
                ready_deadline: now + REOPEN_TIMEOUT,
                settle_deadline: None,
                page: None,
                since: now,
            }
        }

        pub(super) fn observe(
            &mut self,
            now: tokio::time::Instant,
            launched: bool,
            current: Option<SettleView>,
        ) -> SettleStep {
            if let Some(error) = current.as_ref().and_then(|view| view.error.clone()) {
                return SettleStep::Failed(format!("page WebView failed after reopening: {error}"));
            }
            let page = current.as_ref().map(|view| view.page.clone());
            if page != self.page {
                self.page = page;
                self.since = now;
            }
            let ready = current.as_ref().is_some_and(|view| view.ready);
            if ready && self.settle_deadline.is_none() {
                self.settle_deadline = Some(now + SETTLE_TIMEOUT);
            }
            if ready && launched && now.duration_since(self.since) >= SETTLE_QUIET {
                return SettleStep::Settled;
            }
            match self.settle_deadline {
                Some(deadline) if now >= deadline => SettleStep::Unsettled(if launched {
                    "navigating"
                } else {
                    "running App.onLaunch"
                }),
                None if now >= self.ready_deadline => {
                    SettleStep::Failed("timed out waiting for a page after reopening".into())
                }
                _ => SettleStep::Wait,
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::appservice::LxAppWorkers;

        fn view(page: &str, ready: bool) -> Option<SettleView> {
            Some(SettleView {
                page: page.into(),
                ready,
                error: None,
            })
        }

        #[tokio::test]
        async fn a_reopen_waits_out_a_rebuild_of_a_dev_bundle() {
            let interval = Duration::from_millis(5);
            let mut attempts = 0;
            // The bundle is back on the third attempt.
            let reopened = retry_while(true, Duration::from_secs(5), interval, "app", || {
                attempts += 1;
                if attempts < 3 {
                    Err(LxAppError::ResourceNotFound("lxapp.json".into()))
                } else {
                    Ok(attempts)
                }
            })
            .await;
            assert_eq!(reopened.unwrap(), 3);

            // An installed bundle does not come back by waiting.
            let mut attempts = 0;
            let failed = retry_while(false, Duration::from_secs(5), interval, "app", || {
                attempts += 1;
                Err::<(), _>(LxAppError::ResourceNotFound("lxapp.json".into()))
            })
            .await;
            assert!(failed.is_err());
            assert_eq!(attempts, 1);

            // A rebuild that never finishes gives up with the last error.
            let gave_up = retry_while(true, Duration::from_millis(30), interval, "app", || {
                Err::<(), _>(LxAppError::ResourceNotFound("lxapp.json".into()))
            })
            .await
            .unwrap_err();
            assert!(gave_up.to_string().contains("lxapp.json"), "{gave_up}");
        }

        #[test]
        fn a_reopened_app_settles_after_launch_and_a_quiet_page() {
            let start = tokio::time::Instant::now();
            let at = |ms: u64| start + Duration::from_millis(ms);
            let mut settle = Settle::new(start);
            assert_eq!(settle.observe(at(0), false, None), SettleStep::Wait);
            // The initial page is ready, but onLaunch is still probing.
            assert_eq!(
                settle.observe(at(100), false, view("home", true)),
                SettleStep::Wait
            );
            assert_eq!(
                settle.observe(at(900), false, view("home", true)),
                SettleStep::Wait
            );
            // onLaunch redirects and returns: the new page must be ready and
            // stay current for the quiet period.
            assert_eq!(
                settle.observe(at(1000), true, view("login", false)),
                SettleStep::Wait
            );
            assert_eq!(
                settle.observe(at(1100), true, view("login", true)),
                SettleStep::Wait
            );
            assert_eq!(
                settle.observe(at(1200), true, view("login", true)),
                SettleStep::Wait
            );
            assert_eq!(
                settle.observe(at(1000) + SETTLE_QUIET, true, view("login", true)),
                SettleStep::Settled
            );
        }

        #[test]
        fn settling_is_bounded() {
            let start = tokio::time::Instant::now();
            let mut settle = Settle::new(start);
            // onLaunch never returns.
            assert_eq!(
                settle.observe(start, false, view("home", true)),
                SettleStep::Wait
            );
            assert_eq!(
                settle.observe(start + SETTLE_TIMEOUT, false, view("home", true)),
                SettleStep::Unsettled("running App.onLaunch")
            );

            // No page ever becomes ready: that is still a failed reopen.
            let mut settle = Settle::new(start);
            assert_eq!(
                settle.observe(start, true, view("home", false)),
                SettleStep::Wait
            );
            assert!(matches!(
                settle.observe(start + REOPEN_TIMEOUT, true, view("home", false)),
                SettleStep::Failed(_)
            ));

            let mut settle = Settle::new(start);
            let broken = Some(SettleView {
                page: "home".into(),
                ready: false,
                error: Some("crashed".into()),
            });
            assert!(matches!(
                settle.observe(start, true, broken),
                SettleStep::Failed(_)
            ));
        }

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

    /// Hundreds of `restoreProfile` specs each take a checkpoint, roll back
    /// to it and drop it; the profile must not grow with them.
    #[test]
    fn checkpoint_rollback_cycles_leave_nothing_behind() {
        let base = scratch("cycles");
        let profile = RunProfile::create(&base).unwrap();
        seed(&profile.live());
        for round in 0..200 {
            let id = format!("cp-{round}");
            profile.copy_live_to_checkpoint(&id).unwrap();
            fs::write(
                profile.live().join(USER_DATA).join(format!("spec-{round}")),
                b"x",
            )
            .unwrap();
            profile.replace_live_from_checkpoint(&id).unwrap();
            profile.drop_checkpoint(&id).unwrap();
        }
        let entries = |dir: &Path| fs::read_dir(dir).unwrap().count();
        assert_eq!(
            entries(&profile.dir().join(CHECKPOINTS_DIR)),
            0,
            "no checkpoint left"
        );
        assert_eq!(
            entries(profile.dir()),
            2,
            "only live/ and checkpoints/, no staged copies"
        );
        assert_eq!(
            dir_size(&profile.live()),
            10 + 5 + 7,
            "live is the seed again"
        );
        let _ = fs::remove_dir_all(base);
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

    fn write_storage(path: &Path, entries: &[(&str, &[u8])], removed: &[&str]) {
        let db = redb::Database::create(path).unwrap();
        let txn = db.begin_write().unwrap();
        {
            let mut table = txn.open_table(STORAGE_TABLE).unwrap();
            for (key, value) in entries {
                table.insert(*key, *value).unwrap();
            }
            for key in removed {
                table.remove(*key).unwrap();
            }
        }
        txn.commit().unwrap();
    }

    fn read_storage(path: &Path) -> BTreeMap<String, Vec<u8>> {
        read_kept(path, &KeepKeys::new(vec!["*".into()]).unwrap()).unwrap()
    }

    fn keep(patterns: &[&str]) -> KeepKeys {
        KeepKeys::new(patterns.iter().map(|p| p.to_string()).collect()).unwrap()
    }

    #[test]
    fn keep_patterns_glob_over_whole_keys() {
        let auth = keep(&["auth.*"]);
        assert!(auth.matches("auth.token"));
        assert!(auth.matches("auth."));
        assert!(auth.matches("auth.session.refresh"));
        assert!(!auth.matches("auth"));
        assert!(!auth.matches("my.auth.token"));
        let mixed = keep(&["*token", "user.?d", "exact"]);
        assert!(mixed.matches("refresh_token"));
        assert!(mixed.matches("token"));
        assert!(mixed.matches("user.id"));
        assert!(!mixed.matches("user.idx"));
        assert!(mixed.matches("exact"));
        assert!(!mixed.matches("exactly"));
        assert!(keep(&["a*b*c"]).matches("a-b-b-c"));
        assert!(!keep(&["a*b*c"]).matches("a-c-b"));
        assert!(keep(&["令牌.*"]).matches("令牌.刷新"));
        assert!(KeepKeys::default().is_empty());
        assert!(!KeepKeys::default().matches("anything"));
        assert!(KeepKeys::new(vec![String::new()]).is_err());
        assert!(KeepKeys::new(vec!["x".repeat(MAX_KEEP_PATTERN_BYTES + 1)]).is_err());
        assert!(KeepKeys::new(vec!["k".into(); MAX_KEEP_PATTERNS + 1]).is_err());
    }

    #[test]
    fn restore_keeping_carries_current_matching_keys_over_the_checkpoint() {
        let base = scratch("keep");
        let profile = RunProfile::create(&base).unwrap();
        let storage = profile.live().join(STORAGE_FILE);
        write_storage(
            &storage,
            &[
                ("auth.token", b"t1"),
                ("auth.refresh", b"r1"),
                ("draft", b"d1"),
            ],
            &[],
        );
        fs::create_dir_all(profile.live().join(USER_DATA)).unwrap();
        fs::write(profile.live().join(USER_DATA).join("a.txt"), b"old").unwrap();
        profile.copy_live_to_checkpoint("cp").unwrap();

        // The spec rotates the token, signs a new device in, drops the
        // refresh token and edits app data.
        write_storage(
            &storage,
            &[
                ("auth.token", b"t2"),
                ("auth.device", b"dev"),
                ("draft", b"d2"),
            ],
            &["auth.refresh"],
        );
        fs::write(profile.live().join(USER_DATA).join("a.txt"), b"new").unwrap();

        let kept = profile
            .replace_live_keeping("cp", &keep(&["auth.*"]))
            .unwrap();
        assert_eq!(
            kept,
            vec!["auth.device".to_string(), "auth.token".to_string()]
        );
        let restored = read_storage(&storage);
        assert_eq!(
            restored.get("auth.token").map(Vec::as_slice),
            Some(&b"t2"[..])
        );
        assert_eq!(
            restored.get("auth.device").map(Vec::as_slice),
            Some(&b"dev"[..])
        );
        assert!(
            !restored.contains_key("auth.refresh"),
            "a deleted key stays deleted"
        );
        assert_eq!(restored.get("draft").map(Vec::as_slice), Some(&b"d1"[..]));
        assert_eq!(
            fs::read(profile.live().join(USER_DATA).join("a.txt")).unwrap(),
            b"old",
            "files roll back"
        );
        // The checkpoint itself is untouched.
        let checkpoint = read_storage(&profile.checkpoint("cp").unwrap().join(STORAGE_FILE));
        assert_eq!(
            checkpoint.get("auth.token").map(Vec::as_slice),
            Some(&b"t1"[..])
        );
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn restore_keeping_handles_missing_storage_on_either_side() {
        let base = scratch("keep-missing");
        let profile = RunProfile::create(&base).unwrap();
        // Checkpoint taken before the app ever opened its storage.
        profile.copy_live_to_checkpoint("empty").unwrap();
        let storage = profile.live().join(STORAGE_FILE);
        write_storage(&storage, &[("auth.token", b"t"), ("cache", b"c")], &[]);
        profile
            .replace_live_keeping("empty", &keep(&["auth.*"]))
            .unwrap();
        let restored = read_storage(&storage);
        assert_eq!(restored.len(), 1);
        assert_eq!(
            restored.get("auth.token").map(Vec::as_slice),
            Some(&b"t"[..])
        );

        // Nothing current to keep, nothing in the checkpoint: no file appears.
        let fresh = RunProfile::create(&base).unwrap();
        fresh.copy_live_to_checkpoint("none").unwrap();
        fresh
            .replace_live_keeping("none", &keep(&["auth.*"]))
            .unwrap();
        assert!(!fresh.live().join(STORAGE_FILE).exists());

        // A checkpoint with a matching key the current data no longer has.
        let signed_out = RunProfile::create(&base).unwrap();
        let path = signed_out.live().join(STORAGE_FILE);
        write_storage(&path, &[("auth.token", b"old")], &[]);
        signed_out.copy_live_to_checkpoint("cp").unwrap();
        write_storage(&path, &[], &["auth.token"]);
        signed_out
            .replace_live_keeping("cp", &keep(&["auth.*"]))
            .unwrap();
        assert!(
            read_storage(&path).is_empty(),
            "a sign-out survives the rollback"
        );
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn a_failed_keep_merge_leaves_live_untouched() {
        let base = scratch("keep-fail");
        let profile = RunProfile::create(&base).unwrap();
        profile.copy_live_to_checkpoint("cp").unwrap();
        // A checkpoint whose storage is not a database.
        fs::write(
            profile.checkpoint("cp").unwrap().join(STORAGE_FILE),
            b"not redb",
        )
        .unwrap();
        let storage = profile.live().join(STORAGE_FILE);
        write_storage(&storage, &[("auth.token", b"t")], &[]);
        assert!(
            profile
                .replace_live_keeping("cp", &keep(&["auth.*"]))
                .is_err()
        );
        assert_eq!(read_storage(&storage).len(), 1, "live is intact");
        let leftovers: Vec<_> = fs::read_dir(profile.dir())
            .unwrap()
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("restore-"))
            .collect();
        assert!(leftovers.is_empty(), "the staged copy is removed");
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
