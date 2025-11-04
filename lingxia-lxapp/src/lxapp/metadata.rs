use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use super::version::Version;
use crate::LxAppError;

const LXAPPS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("lxapps");

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

    pub fn to_version(&self) -> Version {
        Version {
            major: self.major,
            minor: self.minor,
            patch: self.patch,
        }
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
    pub install_path: String,
    pub installed_at: i64,
}

impl LxAppRecord {
    pub fn new(
        lxappid: &str,
        release_type: ReleaseType,
        version: SemanticVersion,
        install_path: String,
        installed_at: i64,
    ) -> Self {
        Self {
            lxappid: lxappid.to_string(),
            release_type,
            version,
            install_path,
            installed_at,
        }
    }

    pub fn version_string(&self) -> String {
        self.version.to_version_string()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ReleaseType {
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
        let _table = write_txn
            .open_table(LXAPPS_TABLE)
            .map_err(|e| metadata_error("open table", e))?;
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
        .open_table(LXAPPS_TABLE)
        .map_err(|e| metadata_error("open table", e))?;
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
            .open_table(LXAPPS_TABLE)
            .map_err(|e| metadata_error("open table", e))?;
        let serialized = serde_json::to_vec(record)?;
        table
            .insert(key.as_str(), serialized.as_slice())
            .map_err(|e| metadata_error("write record", e))?;
    }
    txn.commit()
        .map_err(|e| metadata_error("commit write transaction", e))?;
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
            .open_table(LXAPPS_TABLE)
            .map_err(|e| metadata_error("open table", e))?;
        let mut keys_to_remove = Vec::new();
        let iter = table
            .iter()
            .map_err(|e| metadata_error("iterate records", e))?;

        for entry in iter {
            let (key, _) = entry.map_err(|e| metadata_error("read record", e))?;
            let key_value = key.value();
            if key_value.starts_with(&prefix) {
                keys_to_remove.push(key_value.to_string());
            }
        }

        for key in keys_to_remove {
            table
                .remove(key.as_str())
                .map_err(|e| metadata_error("delete record", e))?;
        }
    }

    txn.commit()
        .map_err(|e| metadata_error("commit delete transaction", e))?;
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
