use lingxia_update::{Channel, SemanticVersion};
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use super::version::Version;
use crate::LxAppError;

const INSTALLED_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("installed");
const DOWNLOADED_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("downloaded");
const REGISTRY_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("registry");

static DATABASE: OnceLock<Arc<Database>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct LxAppRecord {
    pub lxappid: String,
    pub release_type: Channel,
    pub version: SemanticVersion,
    pub fingermark: String,
    pub install_path: String,
    pub last_open_at: i64,
    /// SHA-256 of the package that produced this install. Absent on bundled
    /// first-installs that never went through OTA.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksum_sha256: Option<String>,
}

impl LxAppRecord {
    pub fn new(
        lxappid: &str,
        release_type: Channel,
        version: SemanticVersion,
        fingermark: String,
        install_path: String,
        last_open_at: i64,
    ) -> Self {
        Self {
            lxappid: lxappid.to_string(),
            release_type,
            version,
            fingermark,
            install_path,
            last_open_at,
            checksum_sha256: None,
        }
    }

    pub fn version_string(&self) -> String {
        self.version.to_version_string()
    }
}

pub(crate) fn init(db_path: PathBuf) -> Result<(), LxAppError> {
    if DATABASE.get().is_some() {
        return Ok(());
    }

    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let db = if db_path.exists() {
        Database::open(&db_path).map_err(|e| metadata_error("open database", e))?
    } else {
        Database::create(&db_path).map_err(|e| metadata_error("create database", e))?
    };

    let write_txn = db
        .begin_write()
        .map_err(|e| metadata_error("begin write transaction", e))?;
    {
        let _installed = write_txn
            .open_table(INSTALLED_TABLE)
            .map_err(|e| metadata_error("open installed table", e))?;
        let _downloaded = write_txn
            .open_table(DOWNLOADED_TABLE)
            .map_err(|e| metadata_error("open downloaded table", e))?;
        let _registry = write_txn
            .open_table(REGISTRY_TABLE)
            .map_err(|e| metadata_error("open registry table", e))?;
    }
    write_txn
        .commit()
        .map_err(|e| metadata_error("commit table creation", e))?;

    let _ = DATABASE.set(Arc::new(db));
    Ok(())
}

pub(crate) fn get(lxappid: &str, release_type: Channel) -> Result<Option<LxAppRecord>, LxAppError> {
    let key = key_for(lxappid, release_type);
    let db = database()?;
    let txn = db
        .begin_read()
        .map_err(|e| metadata_error("begin read transaction", e))?;
    let table = txn
        .open_table(INSTALLED_TABLE)
        .map_err(|e| metadata_error("open installed table", e))?;
    if let Some(value) = table
        .get(key.as_str())
        .map_err(|e| metadata_error("read record", e))?
    {
        let record: LxAppRecord = serde_json::from_slice(value.value())?;
        Ok(Some(record))
    } else {
        Ok(None)
    }
}

pub(crate) fn upsert(record: &LxAppRecord) -> Result<(), LxAppError> {
    let key = key_for(&record.lxappid, record.release_type);
    let db = database()?;
    let txn = db
        .begin_write()
        .map_err(|e| metadata_error("begin write transaction", e))?;
    {
        let mut table = txn
            .open_table(INSTALLED_TABLE)
            .map_err(|e| metadata_error("open installed table", e))?;
        let serialized = serde_json::to_vec(record)?;
        table
            .insert(key.as_str(), serialized.as_slice())
            .map_err(|e| metadata_error("write installed record", e))?;
    }
    txn.commit()
        .map_err(|e| metadata_error("commit installed write", e))?;
    Ok(())
}

/// Drop every channel's record of `lxappid` under the running service env;
/// another env's records still back that env's installs and data.
pub(crate) fn remove_all(lxappid: &str) -> Result<(), LxAppError> {
    remove(lxappid, Channel::Release)?;
    remove(lxappid, Channel::Draft)
}

pub(crate) fn remove(lxappid: &str, release_type: Channel) -> Result<(), LxAppError> {
    let key = key_for(lxappid, release_type);
    let db = database()?;
    let txn = db
        .begin_write()
        .map_err(|e| metadata_error("begin write transaction", e))?;
    {
        let mut table = txn
            .open_table(INSTALLED_TABLE)
            .map_err(|e| metadata_error("open installed table", e))?;
        table
            .remove(key.as_str())
            .map_err(|e| metadata_error("delete installed record", e))?;
    }
    txn.commit()
        .map_err(|e| metadata_error("commit installed delete", e))?;
    Ok(())
}

/// Keyed by service env so one env's lookup never selects the other's install:
/// a dev-service package may be unsigned.
fn key_for(lxappid: &str, release_type: Channel) -> String {
    format!(
        "{}::{}::{}",
        lxappid,
        release_type.as_str(),
        super::service_env_scope().as_str()
    )
}

/// Registry answers come from the service, so each env caches its own.
fn registry_key(appid: &str) -> String {
    format!("{appid}::{}", super::service_env_scope().as_str())
}

pub(crate) fn database() -> Result<Arc<Database>, LxAppError> {
    DATABASE
        .get()
        .cloned()
        .ok_or_else(|| LxAppError::Runtime("metadata database not initialized".to_string()))
}

pub(crate) fn metadata_error(action: &str, err: impl fmt::Display) -> LxAppError {
    LxAppError::Runtime(format!("metadata database {} failed: {}", action, err))
}

// Update last open time for an installed app
pub(crate) fn touch_last_open(
    lxappid: &str,
    release_type: Channel,
    ts: i64,
) -> Result<(), LxAppError> {
    if let Some(mut record) = get(lxappid, release_type)? {
        record.last_open_at = ts;
        upsert(&record)?;
    }
    Ok(())
}

// Downloaded updates API

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PendingUpdateRecord {
    pub lxappid: String,
    pub release_type: Channel,
    pub version: SemanticVersion,
    pub zip_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksum_sha256: Option<String>,
}

pub(crate) fn downloaded_get(
    lxappid: &str,
    release_type: Channel,
) -> Result<Option<PendingUpdateRecord>, LxAppError> {
    let key = key_for(lxappid, release_type);
    let db = database()?;
    let txn = db
        .begin_read()
        .map_err(|e| metadata_error("begin read transaction", e))?;
    let table = txn
        .open_table(DOWNLOADED_TABLE)
        .map_err(|e| metadata_error("open downloaded table", e))?;
    if let Some(value) = table
        .get(key.as_str())
        .map_err(|e| metadata_error("read downloaded record", e))?
    {
        let record: PendingUpdateRecord = serde_json::from_slice(value.value())?;
        Ok(Some(record))
    } else {
        Ok(None)
    }
}

pub(crate) fn downloaded_remove(lxappid: &str, release_type: Channel) -> Result<(), LxAppError> {
    // Fetch record for archive path
    let record = downloaded_get(lxappid, release_type)?;

    // Best-effort delete archive file
    if let Some(rec) = record {
        let archive_path = std::path::PathBuf::from(&rec.zip_path);
        if archive_path.exists()
            && let Err(e) = std::fs::remove_file(&archive_path)
        {
            crate::warn!(
                "Failed to remove archive file at {}: {}. Disk space may be wasted.",
                archive_path.display(),
                e
            );
        }
    }

    // Remove metadata entry
    let key = key_for(lxappid, release_type);
    let db = database()?;
    let txn = db
        .begin_write()
        .map_err(|e| metadata_error("begin write transaction", e))?;
    {
        let mut table = txn
            .open_table(DOWNLOADED_TABLE)
            .map_err(|e| metadata_error("open downloaded table", e))?;
        table
            .remove(key.as_str())
            .map_err(|e| metadata_error("delete downloaded record", e))?;
    }
    txn.commit()
        .map_err(|e| metadata_error("commit downloaded delete", e))?;
    Ok(())
}

pub(crate) fn downloaded_upsert(
    lxappid: &str,
    release_type: Channel,
    version: &str,
    zip_path: &std::path::Path,
    checksum_sha256: Option<&str>,
) -> Result<(), LxAppError> {
    let parsed_version = Version::parse(version).map_err(|_| {
        LxAppError::InvalidParameter(format!("Invalid semantic version: {}", version))
    })?;
    let record = PendingUpdateRecord {
        lxappid: lxappid.to_string(),
        release_type,
        version: SemanticVersion::from_version(&parsed_version),
        zip_path: zip_path.to_string_lossy().to_string(),
        checksum_sha256: checksum_sha256
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_ascii_lowercase()),
    };

    let key = key_for(lxappid, release_type);
    let db = database()?;
    let txn = db
        .begin_write()
        .map_err(|e| metadata_error("begin write transaction", e))?;
    {
        let mut table = txn
            .open_table(DOWNLOADED_TABLE)
            .map_err(|e| metadata_error("open downloaded table", e))?;
        let serialized = serde_json::to_vec(&record)?;
        table
            .insert(key.as_str(), serialized.as_slice())
            .map_err(|e| metadata_error("write downloaded record", e))?;
    }
    txn.commit()
        .map_err(|e| metadata_error("commit downloaded write", e))?;
    Ok(())
}

/// One cached registry answer for an app.
///
/// `icon_file` names a content-addressed file in the icon cache, so two apps
/// that resolved to the same artwork share one file on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RegistryRecord {
    pub appid: String,
    pub name: Option<String>,
    pub description: Option<String>,
    /// Where the artwork came from. The cache key: a record whose URL still
    /// matches the registry's needs no request at all.
    pub icon_url: Option<String>,
    pub icon_file: Option<String>,
    /// `LxAppStatus::as_str`; stored as text so the contract crate stays free of
    /// serde and an unknown value from a newer server degrades instead of failing.
    pub status: String,
    /// Grants by publish channel. A grant written for `release` says nothing
    /// about a draft of the same app id, and the listing refresh and
    /// an instance's own lookup can run on different channels — one slot would
    /// let either erase the other's answer.
    #[serde(default)]
    pub grants: BTreeMap<String, StoredGrant>,
    pub fetched_at: i64,
}

/// One channel's answer.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct StoredGrant {
    /// `None` is "no constraint" — what the registry says about an app it has
    /// no policy for.
    #[serde(default)]
    pub permissions: Option<StoredPermissions>,
    /// When this channel's answer arrived. Separate from the record's
    /// `fetched_at`, which a refresh on any channel advances.
    pub fetched_at: i64,
}

/// A grant as stored. Each half is independent: `None` does not constrain it,
/// an empty list denies it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct StoredPermissions {
    #[serde(default)]
    pub domains: Option<Vec<String>>,
    #[serde(default)]
    pub privileges: Option<Vec<String>>,
}

pub(crate) fn registry_get(appid: &str) -> Result<Option<RegistryRecord>, LxAppError> {
    let db = database()?;
    let txn = db
        .begin_read()
        .map_err(|e| metadata_error("begin read transaction", e))?;
    let table = txn
        .open_table(REGISTRY_TABLE)
        .map_err(|e| metadata_error("open registry table", e))?;
    let Some(value) = table
        .get(registry_key(appid).as_str())
        .map_err(|e| metadata_error("read registry record", e))?
    else {
        return Ok(None);
    };
    Ok(Some(serde_json::from_slice(value.value())?))
}

pub(crate) fn registry_upsert(record: &RegistryRecord) -> Result<(), LxAppError> {
    let key = registry_key(&record.appid);
    let key = key.as_str();
    let db = database()?;
    let txn = db
        .begin_write()
        .map_err(|e| metadata_error("begin write transaction", e))?;
    {
        let mut table = txn
            .open_table(REGISTRY_TABLE)
            .map_err(|e| metadata_error("open registry table", e))?;
        let serialized = serde_json::to_vec(record)?;
        table
            .insert(key, serialized.as_slice())
            .map_err(|e| metadata_error("write registry record", e))?;
    }
    txn.commit()
        .map_err(|e| metadata_error("commit registry write", e))?;
    Ok(())
}

/// Drop this app's record under the running service env, returning the icon
/// file it referenced so the caller can delete artwork it was the last user of.
pub(crate) fn registry_remove(appid: &str) -> Result<Option<String>, LxAppError> {
    let key = registry_key(appid);
    let db = database()?;
    let txn = db
        .begin_write()
        .map_err(|e| metadata_error("begin write transaction", e))?;
    let icon_file = {
        let mut table = txn
            .open_table(REGISTRY_TABLE)
            .map_err(|e| metadata_error("open registry table", e))?;
        table
            .remove(key.as_str())
            .map_err(|e| metadata_error("delete registry record", e))?
            .and_then(|value| serde_json::from_slice::<RegistryRecord>(value.value()).ok())
            .and_then(|record| record.icon_file)
    };
    txn.commit()
        .map_err(|e| metadata_error("commit registry delete", e))?;
    Ok(icon_file)
}

/// Every icon file still referenced by some app's records — the survivor set
/// when deciding which artwork an uninstall may delete.
pub(crate) fn registry_referenced_icon_files() -> Result<BTreeSet<String>, LxAppError> {
    let db = database()?;
    let txn = db
        .begin_read()
        .map_err(|e| metadata_error("begin read transaction", e))?;
    let table = txn
        .open_table(REGISTRY_TABLE)
        .map_err(|e| metadata_error("open registry table", e))?;
    let mut files = BTreeSet::new();
    for entry in table
        .iter()
        .map_err(|e| metadata_error("iterate registry records", e))?
    {
        let (_, value) = entry.map_err(|e| metadata_error("read registry record", e))?;
        if let Ok(record) = serde_json::from_slice::<RegistryRecord>(value.value())
            && let Some(file) = record.icon_file
        {
            files.insert(file);
        }
    }
    Ok(files)
}

/// Every install directory some record still points at. The survivor set for
/// sweeping orphaned installs: an extraction that outlived its record — a
/// crash between unpack and upsert, or an old version whose best-effort removal
/// failed — is referenced by nothing and is pure garbage.
pub(crate) fn installed_paths() -> Result<BTreeSet<String>, LxAppError> {
    let db = database()?;
    let txn = db
        .begin_read()
        .map_err(|e| metadata_error("begin read transaction", e))?;
    let table = txn
        .open_table(INSTALLED_TABLE)
        .map_err(|e| metadata_error("open installed table", e))?;
    let mut paths = BTreeSet::new();
    for entry in table
        .iter()
        .map_err(|e| metadata_error("iterate installed records", e))?
    {
        let (_, value) = entry.map_err(|e| metadata_error("read installed record", e))?;
        let record: LxAppRecord = serde_json::from_slice(value.value())?;
        if !record.install_path.trim().is_empty() {
            paths.insert(record.install_path);
        }
    }
    Ok(paths)
}

/// Every staged update archive a pending record still points at.
pub(crate) fn downloaded_archives() -> Result<BTreeSet<String>, LxAppError> {
    let db = database()?;
    let txn = db
        .begin_read()
        .map_err(|e| metadata_error("begin read transaction", e))?;
    let table = txn
        .open_table(DOWNLOADED_TABLE)
        .map_err(|e| metadata_error("open downloaded table", e))?;
    let mut paths = BTreeSet::new();
    for entry in table
        .iter()
        .map_err(|e| metadata_error("iterate downloaded records", e))?
    {
        let (_, value) = entry.map_err(|e| metadata_error("read downloaded record", e))?;
        let record: PendingUpdateRecord = serde_json::from_slice(value.value())?;
        if !record.zip_path.trim().is_empty() {
            paths.insert(record.zip_path);
        }
    }
    Ok(paths)
}

#[cfg(test)]
pub(crate) fn init_for_tests() {
    static ROOT: OnceLock<tempfile::TempDir> = OnceLock::new();
    ROOT.get_or_init(|| {
        let root = tempfile::tempdir().unwrap();
        init(root.path().join("metadata.redb")).unwrap();
        root
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lxapp::test_service_env_scope::with;
    use lingxia_app_context::AppEnv;

    fn record(appid: &str, install_path: &str) -> LxAppRecord {
        LxAppRecord::new(
            appid,
            Channel::Release,
            SemanticVersion::from_version(&Version::parse("1.0.0").unwrap()),
            format!("fm-{install_path}"),
            install_path.to_string(),
            0,
        )
    }

    #[test]
    fn install_records_do_not_cross_service_envs() {
        init_for_tests();
        let appid = format!("app.lingxia.metadata-scope.{}", uuid::Uuid::new_v4());
        with(AppEnv::Prod, || {
            upsert(&record(&appid, "prod-install")).unwrap()
        });
        with(AppEnv::Dev, || {
            assert!(get(&appid, Channel::Release).unwrap().is_none());
            upsert(&record(&appid, "dev-install")).unwrap();
            downloaded_upsert(
                &appid,
                Channel::Release,
                "2.0.0",
                std::path::Path::new("dev.zst"),
                None,
            )
            .unwrap();
        });
        with(AppEnv::Prod, || {
            assert_eq!(
                get(&appid, Channel::Release).unwrap().unwrap().install_path,
                "prod-install"
            );
            assert!(downloaded_get(&appid, Channel::Release).unwrap().is_none());
            remove_all(&appid).unwrap();
            assert!(get(&appid, Channel::Release).unwrap().is_none());
        });
        with(AppEnv::Dev, || {
            assert_eq!(
                get(&appid, Channel::Release).unwrap().unwrap().install_path,
                "dev-install"
            );
        });
        assert!(installed_paths().unwrap().contains("dev-install"));
    }

    #[test]
    fn registry_records_do_not_cross_service_envs() {
        init_for_tests();
        let appid = format!("app.lingxia.registry-scope.{}", uuid::Uuid::new_v4());
        let record = |icon: &str| RegistryRecord {
            appid: appid.clone(),
            name: None,
            description: None,
            icon_url: None,
            icon_file: Some(icon.to_string()),
            status: "active".to_string(),
            grants: BTreeMap::new(),
            fetched_at: 0,
        };
        with(AppEnv::Prod, || {
            registry_upsert(&record("prod.png")).unwrap()
        });
        with(AppEnv::Dev, || {
            assert!(registry_get(&appid).unwrap().is_none());
            registry_upsert(&record("dev.png")).unwrap();
            assert_eq!(registry_remove(&appid).unwrap().as_deref(), Some("dev.png"));
        });
        with(AppEnv::Prod, || {
            let kept = registry_get(&appid).unwrap().unwrap();
            assert_eq!(kept.icon_file.as_deref(), Some("prod.png"));
        });
    }
}
