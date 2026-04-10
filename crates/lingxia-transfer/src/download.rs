pub mod manager;

use self::manager::{DownloadBehavior, DownloadOwner, DownloadOwnerKind, ResumeMetadata};
use crate::{DownloadsError, Result};
use dashmap::DashMap;
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{broadcast, watch};

const DOWNLOADS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("downloads");
const DOWNLOAD_REQUESTS_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("download_requests");
const BRIDGE_EVENT_STARTED: &str = "BrowserDownloadStarted";
const BRIDGE_EVENT_PROGRESS: &str = "BrowserDownloadProgress";
const BRIDGE_EVENT_COMPLETED: &str = "BrowserDownloadCompleted";
const BRIDGE_EVENT_FAILED: &str = "BrowserDownloadFailed";
const BRIDGE_EVENT_PAUSED: &str = "BrowserDownloadPaused";
const DOWNLOAD_INTERRUPTED_ERROR: &str = "Download interrupted";
const DOWNLOAD_REMOVED_ERROR: &str = "File removed from disk";

static STORES: OnceLock<DashMap<String, Arc<DownloadsStore>>> = OnceLock::new();
static ACTIVE_DOWNLOAD_COMMANDS: OnceLock<DashMap<String, watch::Sender<ActiveDownloadCommand>>> =
    OnceLock::new();
type BrowserTabPathResolver = Arc<dyn Fn(&str) -> String + Send + Sync>;
type BrowserRetryHandler = Arc<dyn Fn(&str) -> Result<()> + Send + Sync>;
static BROWSER_TAB_PATH_RESOLVER: OnceLock<BrowserTabPathResolver> = OnceLock::new();
static BROWSER_RETRY_HANDLER: OnceLock<BrowserRetryHandler> = OnceLock::new();

pub fn register_browser_tab_path_resolver<F>(resolver: F)
where
    F: Fn(&str) -> String + Send + Sync + 'static,
{
    let _ = BROWSER_TAB_PATH_RESOLVER.set(Arc::new(resolver));
}

pub fn register_browser_retry_handler<F>(handler: F)
where
    F: Fn(&str) -> Result<()> + Send + Sync + 'static,
{
    let _ = BROWSER_RETRY_HANDLER.set(Arc::new(handler));
}

pub fn retry_browser_owned_download(task_id: &str) -> Result<()> {
    let handler = BROWSER_RETRY_HANDLER.get().ok_or_else(|| {
        DownloadsError::UnsupportedOperation(
            "browser download retry handler is unavailable".to_string(),
        )
    })?;
    handler(task_id)
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DownloadStatus {
    Downloading,
    Paused,
    Completed,
    Failed,
    Removed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadRecord {
    pub task_id: String,
    #[serde(default)]
    pub owner: DownloadOwner,
    pub tab_id: String,
    pub url: String,
    pub file_name: String,
    pub target_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    pub status: DownloadStatus,
    pub downloaded_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_bytes: Option<u64>,
    pub resumed_bytes: u64,
    #[serde(default)]
    pub retry: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<i64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DownloadEventKind {
    Started,
    Progress,
    Paused,
    Completed,
    Failed,
    Removed,
    Cleared,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadEvent {
    pub kind: DownloadEventKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download: Option<DownloadRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub removed_count: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadsSnapshot {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download_dir: Option<String>,
    pub has_active_downloads: bool,
    pub downloads: Vec<DownloadRecord>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartedPayload {
    task_id: String,
    tab_id: String,
    url: String,
    file_name: String,
    target_path: String,
    mime_type: Option<String>,
    total_bytes: Option<u64>,
    resumed_bytes: u64,
    user_agent: Option<String>,
    suggested_filename: Option<String>,
    source_page_url: Option<String>,
    cookie: Option<String>,
    #[serde(default)]
    behavior: DownloadBehavior,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProgressPayload {
    task_id: String,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompletedPayload {
    task_id: String,
    tab_id: String,
    url: String,
    file_name: String,
    path: String,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FailedPayload {
    task_id: String,
    tab_id: String,
    url: String,
    error: String,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PausedPayload {
    task_id: String,
    tab_id: String,
    url: String,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct DownloadRequestContext {
    #[serde(default)]
    pub owner: DownloadOwner,
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_filename: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_page_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cookie: Option<String>,
    #[serde(default)]
    pub behavior: DownloadBehavior,
    #[serde(default)]
    pub resume: ResumeMetadata,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ActiveDownloadCommand {
    #[default]
    None,
    Pause,
    Cancel,
}

struct DownloadsStore {
    db: Arc<Database>,
    records: DashMap<String, DownloadRecord>,
    requests: DashMap<String, DownloadRequestContext>,
    events: broadcast::Sender<DownloadEvent>,
}

impl DownloadsStore {
    fn open(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let db = if db_path.exists() {
            Database::open(db_path).map_err(|e| db_error("open database", e))?
        } else {
            Database::create(db_path).map_err(|e| db_error("create database", e))?
        };

        let write_txn = db
            .begin_write()
            .map_err(|e| db_error("begin write transaction", e))?;
        {
            let _table = write_txn
                .open_table(DOWNLOADS_TABLE)
                .map_err(|e| db_error("open downloads table", e))?;
            let _request_table = write_txn
                .open_table(DOWNLOAD_REQUESTS_TABLE)
                .map_err(|e| db_error("open download requests table", e))?;
        }
        write_txn
            .commit()
            .map_err(|e| db_error("commit downloads table creation", e))?;

        let records = DashMap::new();
        let requests = DashMap::new();
        let read_txn = db
            .begin_read()
            .map_err(|e| db_error("begin read transaction", e))?;
        let table = read_txn
            .open_table(DOWNLOADS_TABLE)
            .map_err(|e| db_error("open downloads table", e))?;
        let iter = table.iter().map_err(|e| db_error("iterate downloads", e))?;
        for entry in iter {
            let (key, value) = entry.map_err(|e| db_error("read downloads entry", e))?;
            let record: DownloadRecord = serde_json::from_slice(value.value())?;
            records.insert(key.value().to_string(), record);
        }
        let request_table = read_txn
            .open_table(DOWNLOAD_REQUESTS_TABLE)
            .map_err(|e| db_error("open download requests table", e))?;
        let request_iter = request_table
            .iter()
            .map_err(|e| db_error("iterate download requests", e))?;
        for entry in request_iter {
            let (key, value) = entry.map_err(|e| db_error("read download request entry", e))?;
            let request: DownloadRequestContext = serde_json::from_slice(value.value())?;
            requests.insert(key.value().to_string(), request);
        }

        let (events, _) = broadcast::channel(128);
        Ok(Self {
            db: Arc::new(db),
            records,
            requests,
            events,
        })
    }

    fn snapshot(&self, app_data_dir: &Path) -> Result<DownloadsSnapshot> {
        let mut downloads = self.reconciled_records()?;
        downloads.sort_by(|a, b| {
            b.updated_at
                .cmp(&a.updated_at)
                .then_with(|| b.created_at.cmp(&a.created_at))
        });

        Ok(DownloadsSnapshot {
            download_dir: Some(
                self::manager::download_root(app_data_dir)
                    .to_string_lossy()
                    .to_string(),
            ),
            has_active_downloads: downloads
                .iter()
                .any(|record| record.status == DownloadStatus::Downloading),
            downloads,
        })
    }

    fn subscribe(&self) -> broadcast::Receiver<DownloadEvent> {
        self.events.subscribe()
    }

    fn get_record(&self, task_id: &str) -> Option<DownloadRecord> {
        self.records.get(task_id).map(|entry| entry.value().clone())
    }

    fn get_request_context(&self, task_id: &str) -> Option<DownloadRequestContext> {
        self.requests
            .get(task_id)
            .map(|entry| entry.value().clone())
    }

    fn upsert_request_context(&self, task_id: &str, context: DownloadRequestContext) -> Result<()> {
        self.persist_request_context(task_id, &context)?;
        self.requests.insert(task_id.to_string(), context);
        Ok(())
    }

    fn get_record_reconciled(&self, task_id: &str) -> Result<Option<DownloadRecord>> {
        let Some(record) = self.get_record(task_id) else {
            return Ok(None);
        };
        self.reconcile_record(record).map(Some)
    }

    fn upsert_record(&self, record: DownloadRecord, kind: DownloadEventKind) -> Result<()> {
        self.persist_record(&record)?;
        self.records.insert(record.task_id.clone(), record.clone());
        let _ = self.events.send(DownloadEvent {
            kind,
            download: Some(record),
            task_id: None,
            removed_count: None,
        });
        Ok(())
    }

    fn remove_record(&self, task_id: &str) -> Result<()> {
        let txn = self
            .db
            .begin_write()
            .map_err(|e| db_error("begin write transaction", e))?;
        {
            let mut table = txn
                .open_table(DOWNLOADS_TABLE)
                .map_err(|e| db_error("open downloads table", e))?;
            table
                .remove(task_id)
                .map_err(|e| db_error("remove download", e))?;
            let mut request_table = txn
                .open_table(DOWNLOAD_REQUESTS_TABLE)
                .map_err(|e| db_error("open download requests table", e))?;
            request_table
                .remove(task_id)
                .map_err(|e| db_error("remove download request", e))?;
        }
        txn.commit()
            .map_err(|e| db_error("commit download remove", e))?;

        self.records.remove(task_id);
        self.requests.remove(task_id);
        let _ = self.events.send(DownloadEvent {
            kind: DownloadEventKind::Removed,
            download: None,
            task_id: Some(task_id.to_string()),
            removed_count: None,
        });
        Ok(())
    }

    fn clear_completed(&self) -> Result<u64> {
        let completed_ids = self
            .records
            .iter()
            .filter(|entry| entry.value().status == DownloadStatus::Completed)
            .map(|entry| entry.key().clone())
            .collect::<Vec<_>>();
        if completed_ids.is_empty() {
            return Ok(0);
        }

        let txn = self
            .db
            .begin_write()
            .map_err(|e| db_error("begin write transaction", e))?;
        {
            let mut table = txn
                .open_table(DOWNLOADS_TABLE)
                .map_err(|e| db_error("open downloads table", e))?;
            let mut request_table = txn
                .open_table(DOWNLOAD_REQUESTS_TABLE)
                .map_err(|e| db_error("open download requests table", e))?;
            for task_id in &completed_ids {
                table
                    .remove(task_id.as_str())
                    .map_err(|e| db_error("remove completed download", e))?;
                request_table
                    .remove(task_id.as_str())
                    .map_err(|e| db_error("remove completed download request", e))?;
            }
        }
        txn.commit()
            .map_err(|e| db_error("commit downloads clear", e))?;

        for task_id in &completed_ids {
            self.records.remove(task_id);
            self.requests.remove(task_id);
        }

        let _ = self.events.send(DownloadEvent {
            kind: DownloadEventKind::Cleared,
            download: None,
            task_id: None,
            removed_count: Some(completed_ids.len() as u64),
        });
        Ok(completed_ids.len() as u64)
    }

    fn persist_record(&self, record: &DownloadRecord) -> Result<()> {
        let serialized = serde_json::to_vec(record)?;
        let txn = self
            .db
            .begin_write()
            .map_err(|e| db_error("begin write transaction", e))?;
        {
            let mut table = txn
                .open_table(DOWNLOADS_TABLE)
                .map_err(|e| db_error("open downloads table", e))?;
            table
                .insert(record.task_id.as_str(), serialized.as_slice())
                .map_err(|e| db_error("write download", e))?;
        }
        txn.commit()
            .map_err(|e| db_error("commit download write", e))?;
        Ok(())
    }

    fn persist_request_context(
        &self,
        task_id: &str,
        context: &DownloadRequestContext,
    ) -> Result<()> {
        let serialized = serde_json::to_vec(context)?;
        let txn = self
            .db
            .begin_write()
            .map_err(|e| db_error("begin write transaction", e))?;
        {
            let mut table = txn
                .open_table(DOWNLOAD_REQUESTS_TABLE)
                .map_err(|e| db_error("open download requests table", e))?;
            table
                .insert(task_id, serialized.as_slice())
                .map_err(|e| db_error("write download request", e))?;
        }
        txn.commit()
            .map_err(|e| db_error("commit download request write", e))?;
        Ok(())
    }

    fn reconciled_records(&self) -> Result<Vec<DownloadRecord>> {
        self.records
            .iter()
            .map(|entry| entry.value().clone())
            .collect::<Vec<_>>()
            .into_iter()
            .map(|record| self.reconcile_record(record))
            .collect()
    }

    fn reconcile_record(&self, mut record: DownloadRecord) -> Result<DownloadRecord> {
        let mut changed = reconcile_download_record(&mut record);
        let retry = self.get_request_context(&record.task_id).is_some()
            && download_should_allow_retry(&record);
        if record.retry != retry {
            record.retry = retry;
            changed = true;
        }
        if !changed {
            return Ok(record);
        }

        self.persist_record(&record)?;
        self.records.insert(record.task_id.clone(), record.clone());
        Ok(record)
    }
}

pub fn record_bridge_event(app_data_dir: &Path, event_name: &str, payload: &Value) -> Result<()> {
    let store = downloads_store(app_data_dir)?;
    match event_name {
        BRIDGE_EVENT_STARTED => {
            let started: StartedPayload = serde_json::from_value(payload.clone())?;
            let owner = tab_download_owner(&started.tab_id);
            let now = unix_ms_now();
            let existing = store.get_record(&started.task_id);
            let resume = store
                .get_request_context(&started.task_id)
                .map(|context| context.resume)
                .unwrap_or_default();
            let request_context = DownloadRequestContext {
                owner: owner.clone(),
                headers: Vec::new(),
                user_agent: started.user_agent,
                suggested_filename: started.suggested_filename,
                source_page_url: started.source_page_url,
                cookie: started.cookie,
                behavior: started.behavior,
                resume,
            };
            store.upsert_request_context(&started.task_id, request_context)?;
            store.upsert_record(
                DownloadRecord {
                    task_id: started.task_id,
                    owner,
                    tab_id: started.tab_id,
                    url: started.url,
                    file_name: started.file_name,
                    target_path: started.target_path,
                    mime_type: started.mime_type,
                    status: DownloadStatus::Downloading,
                    downloaded_bytes: started.resumed_bytes,
                    total_bytes: started.total_bytes,
                    resumed_bytes: started.resumed_bytes,
                    retry: false,
                    error: None,
                    created_at: existing
                        .as_ref()
                        .map(|record| record.created_at)
                        .unwrap_or(now),
                    updated_at: now,
                    completed_at: None,
                },
                DownloadEventKind::Started,
            )?;
        }
        BRIDGE_EVENT_PROGRESS => {
            let progress: ProgressPayload = serde_json::from_value(payload.clone())?;
            if let Some(mut record) = store.get_record(&progress.task_id) {
                record.status = DownloadStatus::Downloading;
                record.downloaded_bytes = progress.downloaded_bytes;
                record.total_bytes = progress.total_bytes;
                record.retry = false;
                record.updated_at = unix_ms_now();
                store.upsert_record(record, DownloadEventKind::Progress)?;
            }
        }
        BRIDGE_EVENT_COMPLETED => {
            let completed: CompletedPayload = serde_json::from_value(payload.clone())?;
            let mut record =
                store
                    .get_record(&completed.task_id)
                    .unwrap_or_else(|| DownloadRecord {
                        task_id: completed.task_id.clone(),
                        owner: tab_download_owner(&completed.tab_id),
                        tab_id: completed.tab_id.clone(),
                        url: completed.url.clone(),
                        file_name: completed.file_name.clone(),
                        target_path: completed.path.clone(),
                        mime_type: None,
                        status: DownloadStatus::Completed,
                        downloaded_bytes: 0,
                        total_bytes: completed.total_bytes,
                        resumed_bytes: 0,
                        retry: false,
                        error: None,
                        created_at: unix_ms_now(),
                        updated_at: unix_ms_now(),
                        completed_at: None,
                    });
            let now = unix_ms_now();
            record.owner = tab_download_owner(&completed.tab_id);
            record.tab_id = completed.tab_id;
            record.url = completed.url;
            record.file_name = completed.file_name;
            record.target_path = completed.path;
            record.status = DownloadStatus::Completed;
            record.downloaded_bytes = completed.downloaded_bytes;
            record.total_bytes = completed.total_bytes;
            record.retry = false;
            record.error = None;
            record.updated_at = now;
            record.completed_at = Some(now);
            store.upsert_record(record, DownloadEventKind::Completed)?;
        }
        BRIDGE_EVENT_PAUSED => {
            let paused: PausedPayload = serde_json::from_value(payload.clone())?;
            if let Some(mut record) = store.get_record(&paused.task_id) {
                record.owner = tab_download_owner(&paused.tab_id);
                record.status = DownloadStatus::Paused;
                record.tab_id = paused.tab_id;
                record.url = paused.url;
                record.error = None;
                record.downloaded_bytes = paused.downloaded_bytes;
                record.total_bytes = paused.total_bytes;
                record.retry = true;
                record.completed_at = None;
                record.updated_at = unix_ms_now();
                store.upsert_record(record, DownloadEventKind::Paused)?;
            }
        }
        BRIDGE_EVENT_FAILED => {
            let failed: FailedPayload = serde_json::from_value(payload.clone())?;
            if let Some(mut record) = store.get_record(&failed.task_id) {
                record.owner = tab_download_owner(&failed.tab_id);
                record.status = DownloadStatus::Failed;
                record.tab_id = failed.tab_id;
                record.url = failed.url;
                record.error = Some(failed.error);
                record.downloaded_bytes = failed.downloaded_bytes;
                record.total_bytes = failed.total_bytes;
                record.updated_at = unix_ms_now();
                store.upsert_record(record, DownloadEventKind::Failed)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub(crate) fn snapshot(app_data_dir: &Path) -> Result<DownloadsSnapshot> {
    let store = downloads_store(app_data_dir)?;
    store.snapshot(app_data_dir)
}

pub(crate) fn subscribe(app_data_dir: &Path) -> Result<broadcast::Receiver<DownloadEvent>> {
    let store = downloads_store(app_data_dir)?;
    Ok(store.subscribe())
}

pub fn get_record(app_data_dir: &Path, task_id: &str) -> Result<Option<DownloadRecord>> {
    let store = downloads_store(app_data_dir)?;
    store.get_record_reconciled(task_id)
}

pub fn get_request_context(
    app_data_dir: &Path,
    task_id: &str,
) -> Result<Option<DownloadRequestContext>> {
    let store = downloads_store(app_data_dir)?;
    Ok(store.get_request_context(task_id))
}

pub(crate) fn load_resume_metadata(
    app_data_dir: &Path,
    task_id: &str,
) -> Result<Option<ResumeMetadata>> {
    let store = downloads_store(app_data_dir)?;
    Ok(store
        .get_request_context(task_id)
        .map(|context| context.resume))
}

pub(crate) fn save_resume_metadata(
    app_data_dir: &Path,
    task_id: &str,
    resume: ResumeMetadata,
) -> Result<()> {
    let store = downloads_store(app_data_dir)?;
    let mut context = store.get_request_context(task_id).unwrap_or_default();
    context.resume = resume;
    store.upsert_request_context(task_id, context)
}

pub(crate) fn clear_resume_metadata(app_data_dir: &Path, task_id: &str) -> Result<()> {
    let store = downloads_store(app_data_dir)?;
    let Some(mut context) = store.get_request_context(task_id) else {
        return Ok(());
    };
    if context.resume == ResumeMetadata::default() {
        return Ok(());
    }
    context.resume = ResumeMetadata::default();
    store.upsert_request_context(task_id, context)
}

pub(crate) fn clear_completed(app_data_dir: &Path) -> Result<u64> {
    let store = downloads_store(app_data_dir)?;
    store.clear_completed()
}

pub(crate) fn record_managed_download_started(
    app_data_dir: &Path,
    task_id: &str,
    owner: DownloadOwner,
    url: &str,
    file_name: &str,
    target_path: &Path,
    mime_type: Option<&str>,
    total_bytes: Option<u64>,
    resumed_bytes: u64,
    headers: Vec<(String, String)>,
    user_agent: Option<String>,
    behavior: DownloadBehavior,
) -> Result<()> {
    let store = downloads_store(app_data_dir)?;
    let now = unix_ms_now();
    let existing = store.get_record(task_id);
    let resume = store
        .get_request_context(task_id)
        .map(|context| context.resume)
        .unwrap_or_default();
    store.upsert_request_context(
        task_id,
        DownloadRequestContext {
            owner: owner.clone(),
            headers,
            user_agent,
            suggested_filename: Some(file_name.to_string()),
            source_page_url: None,
            cookie: None,
            behavior,
            resume,
        },
    )?;
    let record = DownloadRecord {
        task_id: task_id.to_string(),
        owner,
        tab_id: String::new(),
        url: url.to_string(),
        file_name: file_name.to_string(),
        target_path: target_path.to_string_lossy().to_string(),
        mime_type: mime_type.map(ToOwned::to_owned),
        status: DownloadStatus::Downloading,
        downloaded_bytes: resumed_bytes,
        total_bytes,
        resumed_bytes,
        retry: false,
        error: None,
        created_at: existing
            .as_ref()
            .map(|record| record.created_at)
            .unwrap_or(now),
        updated_at: now,
        completed_at: None,
    };
    store.upsert_record(record, DownloadEventKind::Started)?;
    Ok(())
}

pub(crate) fn record_managed_download_paused(
    app_data_dir: &Path,
    task_id: &str,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
) -> Result<()> {
    let store = downloads_store(app_data_dir)?;
    if let Some(mut record) = store.get_record(task_id) {
        record.status = DownloadStatus::Paused;
        record.downloaded_bytes = downloaded_bytes;
        record.total_bytes = total_bytes;
        record.retry = true;
        record.error = None;
        record.completed_at = None;
        record.updated_at = unix_ms_now();
        store.upsert_record(record, DownloadEventKind::Paused)?;
    }
    Ok(())
}

pub(crate) fn record_managed_download_progress(
    app_data_dir: &Path,
    task_id: &str,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
) -> Result<()> {
    let store = downloads_store(app_data_dir)?;
    if let Some(mut record) = store.get_record(task_id) {
        record.status = DownloadStatus::Downloading;
        record.downloaded_bytes = downloaded_bytes;
        record.total_bytes = total_bytes;
        record.retry = false;
        record.updated_at = unix_ms_now();
        store.upsert_record(record, DownloadEventKind::Progress)?;
    }
    Ok(())
}

pub(crate) fn record_managed_download_completed(
    app_data_dir: &Path,
    task_id: &str,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
) -> Result<()> {
    let store = downloads_store(app_data_dir)?;
    if let Some(mut record) = store.get_record(task_id) {
        let now = unix_ms_now();
        record.status = DownloadStatus::Completed;
        record.downloaded_bytes = downloaded_bytes;
        record.total_bytes = total_bytes;
        record.retry = false;
        record.error = None;
        record.updated_at = now;
        record.completed_at = Some(now);
        store.upsert_record(record, DownloadEventKind::Completed)?;
    }
    Ok(())
}

pub(crate) fn record_managed_download_failed(
    app_data_dir: &Path,
    task_id: &str,
    error: &str,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
) -> Result<()> {
    let store = downloads_store(app_data_dir)?;
    if let Some(mut record) = store.get_record(task_id) {
        record.status = DownloadStatus::Failed;
        record.error = Some(error.to_string());
        record.downloaded_bytes = downloaded_bytes;
        record.total_bytes = total_bytes;
        record.updated_at = unix_ms_now();
        store.upsert_record(record, DownloadEventKind::Failed)?;
    }
    Ok(())
}

pub fn remove(app_data_dir: &Path, task_id: &str) -> Result<Option<DownloadRecord>> {
    let store = downloads_store(app_data_dir)?;
    let Some(record) = store.get_record(task_id) else {
        return Ok(None);
    };
    if record.status == DownloadStatus::Downloading && is_active_download(task_id) {
        return Err(DownloadsError::UnsupportedOperation(
            "active downloads cannot be removed from history".to_string(),
        ));
    }
    store.remove_record(task_id)?;
    Ok(Some(record))
}

fn downloads_store(app_data_dir: &Path) -> Result<Arc<DownloadsStore>> {
    let db_path = downloads_db_path(app_data_dir);
    let key = db_path.to_string_lossy().to_string();
    let stores = STORES.get_or_init(DashMap::new);
    match stores.entry(key) {
        dashmap::mapref::entry::Entry::Occupied(existing) => Ok(existing.get().clone()),
        dashmap::mapref::entry::Entry::Vacant(entry) => {
            let store = Arc::new(DownloadsStore::open(&db_path)?);
            entry.insert(store.clone());
            Ok(store)
        }
    }
}

fn active_download_commands() -> &'static DashMap<String, watch::Sender<ActiveDownloadCommand>> {
    ACTIVE_DOWNLOAD_COMMANDS.get_or_init(DashMap::new)
}

pub fn register_active_download(task_id: &str) -> watch::Receiver<ActiveDownloadCommand> {
    let (tx, rx) = watch::channel(ActiveDownloadCommand::None);
    active_download_commands().insert(task_id.to_string(), tx);
    rx
}

pub fn unregister_active_download(task_id: &str) {
    active_download_commands().remove(task_id);
}

pub fn cancel_active_download(task_id: &str) -> bool {
    let Some(sender) = active_download_commands().get(task_id) else {
        return false;
    };
    sender.send(ActiveDownloadCommand::Cancel).is_ok()
}

pub fn pause_active_download(task_id: &str) -> bool {
    let Some(sender) = active_download_commands().get(task_id) else {
        return false;
    };
    sender.send(ActiveDownloadCommand::Pause).is_ok()
}

fn is_active_download(task_id: &str) -> bool {
    active_download_commands().contains_key(task_id)
}

pub fn has_active_download(task_id: &str) -> bool {
    is_active_download(task_id)
}

fn downloads_db_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("app_state").join("downloads.redb")
}

fn download_part_path(target_path: &Path) -> PathBuf {
    target_path.with_extension("part")
}

fn file_len(path: &Path) -> Option<u64> {
    std::fs::metadata(path).ok().map(|metadata| metadata.len())
}

fn download_should_allow_retry(record: &DownloadRecord) -> bool {
    matches!(
        record.status,
        DownloadStatus::Failed | DownloadStatus::Paused
    )
}

fn reconcile_download_record(record: &mut DownloadRecord) -> bool {
    let original = record.clone();
    let now = unix_ms_now();
    let target_path = Path::new(&record.target_path);
    let part_path = download_part_path(target_path);
    let target_exists = target_path.exists();
    let part_exists = part_path.exists();

    match record.status {
        DownloadStatus::Downloading => {
            if is_active_download(&record.task_id) {
                if let Some(part_len) = file_len(&part_path) {
                    record.downloaded_bytes = record.downloaded_bytes.max(part_len);
                }
            } else if target_exists {
                if let Some(target_len) = file_len(target_path) {
                    record.downloaded_bytes = target_len;
                    record.total_bytes =
                        Some(record.total_bytes.unwrap_or(target_len).max(target_len));
                }
                record.status = DownloadStatus::Completed;
                record.error = None;
                record.completed_at.get_or_insert(now);
                record.updated_at = now;
            } else if part_exists {
                if let Some(part_len) = file_len(&part_path) {
                    record.downloaded_bytes = record.downloaded_bytes.max(part_len);
                }
                record.status = DownloadStatus::Failed;
                record.error = Some(DOWNLOAD_INTERRUPTED_ERROR.to_string());
                record.completed_at = None;
                record.updated_at = now;
            } else {
                record.status = DownloadStatus::Removed;
                record.error = Some(DOWNLOAD_REMOVED_ERROR.to_string());
                record.completed_at = None;
                record.updated_at = now;
            }
        }
        DownloadStatus::Paused => {
            if target_exists && !part_exists {
                if let Some(target_len) = file_len(target_path) {
                    record.downloaded_bytes = target_len;
                    record.total_bytes =
                        Some(record.total_bytes.unwrap_or(target_len).max(target_len));
                }
                record.status = DownloadStatus::Completed;
                record.error = None;
                record.retry = false;
                record.completed_at.get_or_insert(now);
                record.updated_at = now;
            } else if part_exists {
                if let Some(part_len) = file_len(&part_path) {
                    record.downloaded_bytes = record.downloaded_bytes.max(part_len);
                }
                record.error = None;
                record.retry = true;
                record.completed_at = None;
            } else {
                record.status = DownloadStatus::Removed;
                record.error = Some(DOWNLOAD_REMOVED_ERROR.to_string());
                record.retry = false;
                record.completed_at = None;
                record.updated_at = now;
            }
        }
        DownloadStatus::Completed => {
            if !target_exists {
                record.status = DownloadStatus::Removed;
                record.error = Some(DOWNLOAD_REMOVED_ERROR.to_string());
                record.updated_at = now;
            }
        }
        DownloadStatus::Failed => {
            if !target_exists && !part_exists {
                record.status = DownloadStatus::Removed;
                if record
                    .error
                    .as_deref()
                    .unwrap_or_default()
                    .trim()
                    .is_empty()
                {
                    record.error = Some(DOWNLOAD_REMOVED_ERROR.to_string());
                }
                record.updated_at = now;
            }
        }
        DownloadStatus::Removed => {}
    }

    *record != original
}

fn unix_ms_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn db_error(action: &str, err: impl std::fmt::Display) -> DownloadsError {
    DownloadsError::Runtime(format!("downloads store {action} failed: {err}"))
}

fn download_tab_path(tab_id: &str) -> String {
    BROWSER_TAB_PATH_RESOLVER
        .get()
        .map(|resolver| resolver(tab_id))
        .unwrap_or_else(|| format!("/tabs/{tab_id}"))
}

fn tab_download_owner(tab_id: &str) -> DownloadOwner {
    DownloadOwner {
        kind: DownloadOwnerKind::Browser,
        appid: "app.lingxia.browser".to_string(),
        page_path: Some(download_tab_path(tab_id)),
        tab_id: Some(tab_id.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "lingxia-browser-downloads-test-{}-{}",
            name,
            unix_ms_now()
        ));
        std::fs::create_dir_all(&root).expect("create temp root");
        root
    }

    #[test]
    fn snapshot_sorts_newest_first() {
        let root = temp_root("snapshot");
        record_bridge_event(
            &root,
            BRIDGE_EVENT_STARTED,
            &json!({
                "taskId": "a",
                "tabId": "tab-a",
                "url": "https://example.com/a",
                "fileName": "a.txt",
                "targetPath": "/tmp/a.txt",
                "resumedBytes": 0,
                "totalBytes": 10
            }),
        )
        .expect("record start a");
        record_bridge_event(
            &root,
            BRIDGE_EVENT_STARTED,
            &json!({
                "taskId": "b",
                "tabId": "tab-b",
                "url": "https://example.com/b",
                "fileName": "b.txt",
                "targetPath": "/tmp/b.txt",
                "resumedBytes": 0,
                "totalBytes": 20
            }),
        )
        .expect("record start b");

        let snapshot = snapshot(&root).expect("snapshot");
        assert_eq!(snapshot.downloads.len(), 2);
        assert_eq!(snapshot.downloads[0].task_id, "b");
        assert_eq!(snapshot.downloads[1].task_id, "a");
    }

    #[test]
    fn completed_records_can_be_cleared() {
        let root = temp_root("clear");
        record_bridge_event(
            &root,
            BRIDGE_EVENT_STARTED,
            &json!({
                "taskId": "done",
                "tabId": "tab-a",
                "url": "https://example.com/file",
                "fileName": "file.txt",
                "targetPath": "/tmp/file.txt",
                "resumedBytes": 0,
                "totalBytes": 10
            }),
        )
        .expect("record start");
        record_bridge_event(
            &root,
            BRIDGE_EVENT_COMPLETED,
            &json!({
                "taskId": "done",
                "tabId": "tab-a",
                "url": "https://example.com/file",
                "fileName": "file.txt",
                "path": "/tmp/file.txt",
                "downloadedBytes": 10,
                "totalBytes": 10
            }),
        )
        .expect("record complete");

        let removed = clear_completed(&root).expect("clear completed");
        assert_eq!(removed, 1);
        let snapshot = snapshot(&root).expect("snapshot");
        assert!(snapshot.downloads.is_empty());
    }

    #[test]
    fn snapshot_marks_missing_completed_files_as_removed() {
        let root = temp_root("removed");
        let target_path = root.join("missing.txt");
        record_bridge_event(
            &root,
            BRIDGE_EVENT_STARTED,
            &json!({
                "taskId": "gone",
                "tabId": "tab-a",
                "url": "https://example.com/file",
                "fileName": "missing.txt",
                "targetPath": target_path,
                "resumedBytes": 0,
                "totalBytes": 10
            }),
        )
        .expect("record start");
        record_bridge_event(
            &root,
            BRIDGE_EVENT_COMPLETED,
            &json!({
                "taskId": "gone",
                "tabId": "tab-a",
                "url": "https://example.com/file",
                "fileName": "missing.txt",
                "path": target_path,
                "downloadedBytes": 10,
                "totalBytes": 10
            }),
        )
        .expect("record complete");

        let snapshot = snapshot(&root).expect("snapshot");
        assert_eq!(snapshot.downloads[0].status, DownloadStatus::Removed);
        assert_eq!(
            snapshot.downloads[0].error.as_deref(),
            Some(DOWNLOAD_REMOVED_ERROR)
        );
    }

    #[test]
    fn snapshot_marks_stale_downloading_records_as_interrupted() {
        let root = temp_root("stale-downloading");
        let target_path = root.join("file.bin");
        let part_path = download_part_path(&target_path);
        std::fs::write(&part_path, vec![0_u8; 7]).expect("write part file");

        record_bridge_event(
            &root,
            BRIDGE_EVENT_STARTED,
            &json!({
                "taskId": "stale",
                "tabId": "tab-a",
                "url": "https://example.com/file",
                "fileName": "file.bin",
                "targetPath": target_path,
                "resumedBytes": 0,
                "totalBytes": 10
            }),
        )
        .expect("record start");

        let snapshot = snapshot(&root).expect("snapshot");
        assert_eq!(snapshot.downloads[0].status, DownloadStatus::Failed);
        assert_eq!(
            snapshot.downloads[0].error.as_deref(),
            Some(DOWNLOAD_INTERRUPTED_ERROR)
        );
        assert_eq!(snapshot.downloads[0].downloaded_bytes, 7);
    }

    #[test]
    fn snapshot_promotes_finished_files_to_completed_after_restart() {
        let root = temp_root("promote-completed");
        let target_path = root.join("done.bin");
        std::fs::write(&target_path, vec![0_u8; 11]).expect("write target file");

        record_bridge_event(
            &root,
            BRIDGE_EVENT_STARTED,
            &json!({
                "taskId": "done-after-restart",
                "tabId": "tab-a",
                "url": "https://example.com/file",
                "fileName": "done.bin",
                "targetPath": target_path,
                "resumedBytes": 0,
                "totalBytes": 11
            }),
        )
        .expect("record start");

        let snapshot = snapshot(&root).expect("snapshot");
        assert_eq!(snapshot.downloads[0].status, DownloadStatus::Completed);
        assert_eq!(snapshot.downloads[0].downloaded_bytes, 11);
        assert!(snapshot.downloads[0].completed_at.is_some());
    }

    #[test]
    fn managed_start_updates_resumed_bytes_without_resetting_created_at() {
        let root = temp_root("managed-restart");
        let target_path = root.join("download.bin");
        let owner = DownloadOwner {
            kind: DownloadOwnerKind::LxApp,
            appid: "app.test".to_string(),
            page_path: None,
            tab_id: None,
        };

        record_managed_download_started(
            &root,
            "managed-task",
            owner.clone(),
            "https://example.com/file",
            "download.bin",
            &target_path,
            Some("application/octet-stream"),
            Some(100),
            64,
            Vec::new(),
            None,
            DownloadBehavior::default(),
        )
        .expect("record managed start");
        let first = get_record(&root, "managed-task")
            .expect("get first record")
            .expect("first record");

        std::thread::sleep(std::time::Duration::from_millis(2));

        record_managed_download_started(
            &root,
            "managed-task",
            owner,
            "https://example.com/file",
            "download.bin",
            &target_path,
            Some("application/octet-stream"),
            Some(100),
            0,
            Vec::new(),
            None,
            DownloadBehavior::default(),
        )
        .expect("record managed restart");
        let second = get_record(&root, "managed-task")
            .expect("get second record")
            .expect("second record");

        assert_eq!(second.resumed_bytes, 0);
        assert_eq!(second.downloaded_bytes, 0);
        assert_eq!(second.created_at, first.created_at);
        assert!(second.updated_at >= first.updated_at);
    }

    #[test]
    fn downloads_store_initialization_is_shared_across_concurrent_callers() {
        let root = temp_root("store-concurrent");
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));

        let worker = |root: PathBuf, barrier: std::sync::Arc<std::sync::Barrier>| {
            std::thread::spawn(move || {
                barrier.wait();
                downloads_store(&root).expect("downloads_store")
            })
        };

        let first = worker(root.clone(), barrier.clone());
        let second = worker(root.clone(), barrier.clone());

        barrier.wait();
        let first = first.join().expect("join first");
        let second = second.join().expect("join second");

        assert!(Arc::ptr_eq(&first, &second));
    }
}
