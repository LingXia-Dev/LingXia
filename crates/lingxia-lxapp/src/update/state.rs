use super::*;
use dashmap::DashMap;
use redb::{ReadableDatabase, TableDefinition};
use std::sync::OnceLock;
use tokio::sync::watch;

pub(crate) const UPDATE_STATE_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("runtime_state");

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ForceUpdateDownloadState {
    Downloading { version: String },
    Completed,
    Failed(String),
}

pub(super) struct ForceUpdateDownloadTracker {
    downloads: DashMap<String, watch::Sender<ForceUpdateDownloadState>>,
}

impl ForceUpdateDownloadTracker {
    fn new() -> Self {
        Self {
            downloads: DashMap::new(),
        }
    }

    pub(super) fn try_start_download(
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

    pub(super) fn mark_completed(&self, key: &str) {
        if let Some(entry) = self.downloads.get(key) {
            let _ = entry.send(ForceUpdateDownloadState::Completed);
        }
        self.downloads.remove(key);
    }

    pub(super) fn mark_failed(&self, key: &str, error: String) {
        if let Some(entry) = self.downloads.get(key) {
            let _ = entry.send(ForceUpdateDownloadState::Failed(error));
        }
        self.downloads.remove(key);
    }

    pub(super) fn wait_for_download(
        &self,
        key: &str,
    ) -> Option<watch::Receiver<ForceUpdateDownloadState>> {
        self.downloads.get(key).map(|entry| entry.subscribe())
    }

    fn state(&self, key: &str) -> Option<ForceUpdateDownloadState> {
        self.downloads.get(key).map(|entry| entry.borrow().clone())
    }
}

static FORCE_UPDATE_DOWNLOAD_TRACKER: OnceLock<ForceUpdateDownloadTracker> = OnceLock::new();
const UPDATE_CHECK_NEXT_AT_PREFIX: &str = "update_check_next_at:";
const UPDATE_CHECK_COOLDOWN_SECS: i64 = 6 * 60 * 60;

pub(super) fn force_update_tracker() -> &'static ForceUpdateDownloadTracker {
    FORCE_UPDATE_DOWNLOAD_TRACKER.get_or_init(ForceUpdateDownloadTracker::new)
}

pub(super) fn force_update_download_key(lxappid: &str, release_type: ReleaseType) -> String {
    UpdateTarget::lxapp(
        lxappid,
        release_type,
        LxAppUpdateQuery::latest(None::<String>),
    )
    .scope_key()
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn get(key: &str) -> Result<Option<String>, LxAppError> {
    let db = metadata::database()?;
    let txn = db
        .begin_read()
        .map_err(|e| metadata::metadata_error("begin read transaction", e))?;
    let table = txn
        .open_table(UPDATE_STATE_TABLE)
        .map_err(|e| metadata::metadata_error("open update state table", e))?;
    if let Some(value) = table
        .get(key)
        .map_err(|e| metadata::metadata_error("read update state value", e))?
    {
        let value = String::from_utf8(value.value().to_vec())
            .map_err(|e| LxAppError::Runtime(format!("update state value decode failed: {}", e)))?;
        Ok(Some(value))
    } else {
        Ok(None)
    }
}

fn set(key: &str, value: &str) -> Result<(), LxAppError> {
    let db = metadata::database()?;
    let txn = db
        .begin_write()
        .map_err(|e| metadata::metadata_error("begin write transaction", e))?;
    {
        let mut table = txn
            .open_table(UPDATE_STATE_TABLE)
            .map_err(|e| metadata::metadata_error("open update state table", e))?;
        table
            .insert(key, value.as_bytes())
            .map_err(|e| metadata::metadata_error("write update state value", e))?;
    }
    txn.commit()
        .map_err(|e| metadata::metadata_error("commit update state write", e))?;
    Ok(())
}

fn update_check_next_at(target: &str) -> Option<i64> {
    get(&format!("{}{}", UPDATE_CHECK_NEXT_AT_PREFIX, target))
        .ok()
        .flatten()
        .and_then(|s| s.parse::<i64>().ok())
}

fn set_update_check_next_at(target: &str, ts: i64) -> Result<(), LxAppError> {
    set(
        &format!("{}{}", UPDATE_CHECK_NEXT_AT_PREFIX, target),
        &ts.to_string(),
    )
}

pub(super) fn try_acquire_update_check_window(target: &str) -> bool {
    let now = unix_now();
    if let Some(next_check_at) = update_check_next_at(target)
        && now < next_check_at
    {
        crate::info!(
            "Skip update check due to cooldown: target={} next_check_at={} now={}",
            target,
            next_check_at,
            now
        );
        return false;
    }

    if let Err(err) = set_update_check_next_at(target, now + UPDATE_CHECK_COOLDOWN_SECS) {
        crate::warn!(
            "Failed to persist update-check cooldown for target {}: {}",
            target,
            err
        );
    }

    true
}

pub fn is_force_update_downloading(lxappid: &str, release_type: ReleaseType) -> bool {
    matches!(
        force_update_tracker().state(&force_update_download_key(lxappid, release_type)),
        Some(ForceUpdateDownloadState::Downloading { .. })
    )
}
