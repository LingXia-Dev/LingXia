use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use super::version::Version;
use crate::LxAppError;

const INSTALLED_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("installed");
const DOWNLOADED_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("downloaded");

static DATABASE: OnceLock<Arc<Database>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SemanticVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl SemanticVersion {
    pub fn from_version(version: &Version) -> Self {
        Self {
            major: version.major,
            minor: version.minor,
            patch: version.patch,
        }
    }

    pub fn to_version_string(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl fmt::Display for SemanticVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct LxAppRecord {
    pub lxappid: String,
    pub release_type: ReleaseType,
    pub version: SemanticVersion,
    pub fingermark: String,
    pub install_path: String,
    pub last_open_at: i64,
}

impl LxAppRecord {
    pub fn new(
        lxappid: &str,
        release_type: ReleaseType,
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
        }
    }

    pub fn version_string(&self) -> String {
        self.version.to_version_string()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ReleaseType {
    Release,
    Preview,
    Developer,
}

impl ReleaseType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ReleaseType::Release => "release",
            ReleaseType::Preview => "preview",
            ReleaseType::Developer => "developer",
        }
    }
}

impl Default for ReleaseType {
    fn default() -> Self {
        ReleaseType::Release
    }
}

impl fmt::Display for ReleaseType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
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
    }
    write_txn
        .commit()
        .map_err(|e| metadata_error("commit table creation", e))?;

    let _ = DATABASE.set(Arc::new(db));
    Ok(())
}

pub(crate) fn get(
    lxappid: &str,
    release_type: ReleaseType,
) -> Result<Option<LxAppRecord>, LxAppError> {
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

pub(crate) fn remove_all(lxappid: &str) -> Result<(), LxAppError> {
    let prefix = format!("{}::", lxappid);
    let db = database()?;
    let txn = db
        .begin_write()
        .map_err(|e| metadata_error("begin write transaction", e))?;

    {
        let mut table = txn
            .open_table(INSTALLED_TABLE)
            .map_err(|e| metadata_error("open installed table", e))?;
        let mut keys_to_remove = Vec::new();
        let iter = table
            .iter()
            .map_err(|e| metadata_error("iterate installed records", e))?;

        for entry in iter {
            let (key, _) = entry.map_err(|e| metadata_error("read installed record", e))?;
            let key_value = key.value();
            if key_value.starts_with(&prefix) {
                keys_to_remove.push(key_value.to_string());
            }
        }

        for key in keys_to_remove {
            table
                .remove(key.as_str())
                .map_err(|e| metadata_error("delete installed record", e))?;
        }
    }

    txn.commit()
        .map_err(|e| metadata_error("commit installed delete", e))?;
    Ok(())
}

pub(crate) fn exists(lxappid: &str, release_type: ReleaseType) -> Result<bool, LxAppError> {
    Ok(get(lxappid, release_type)?.is_some())
}

fn key_for(lxappid: &str, release_type: ReleaseType) -> String {
    format!("{}::{}", lxappid, release_type.as_str())
}

fn database() -> Result<Arc<Database>, LxAppError> {
    DATABASE
        .get()
        .cloned()
        .ok_or_else(|| LxAppError::Runtime("metadata database not initialized".to_string()))
}

fn metadata_error(action: &str, err: impl fmt::Display) -> LxAppError {
    LxAppError::Runtime(format!("metadata database {} failed: {}", action, err))
}

// Update last open time for an installed app
pub(crate) fn touch_last_open(
    lxappid: &str,
    release_type: ReleaseType,
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
    pub release_type: ReleaseType,
    pub version: SemanticVersion,
    pub zip_path: String,
}

pub(crate) fn downloaded_get(
    lxappid: &str,
    release_type: ReleaseType,
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

pub(crate) fn downloaded_remove(
    lxappid: &str,
    release_type: ReleaseType,
) -> Result<(), LxAppError> {
    // Fetch record for archive path
    let record = downloaded_get(lxappid, release_type)?;

    // Best-effort delete archive file
    if let Some(rec) = record {
        let archive_path = std::path::PathBuf::from(&rec.zip_path);
        if archive_path.exists() {
            if let Err(e) = std::fs::remove_file(&archive_path) {
                crate::warn!(
                    "Failed to remove archive file at {}: {}. Disk space may be wasted.",
                    archive_path.display(),
                    e
                );
            }
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
    release_type: ReleaseType,
    version: &str,
    zip_path: &std::path::Path,
) -> Result<(), LxAppError> {
    let parsed_version = Version::parse(version).map_err(|_| {
        LxAppError::InvalidParameter(format!("Invalid semantic version: {}", version))
    })?;
    let record = PendingUpdateRecord {
        lxappid: lxappid.to_string(),
        release_type,
        version: SemanticVersion::from_version(&parsed_version),
        zip_path: zip_path.to_string_lossy().to_string(),
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
