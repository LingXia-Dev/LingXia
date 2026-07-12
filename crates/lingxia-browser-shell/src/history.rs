//! Persistent browser history and host routes.
//!
//! History is a high-frequency, indexed data set, so it uses SQLite rather
//! than rewriting a JSON document after each navigation. Bookmarks remain in
//! JSON because they are small and edited infrequently.

use crate::host::{HostResult, StreamContext};
use crate::url_match::normalize_url;
use lingxia_platform::traits::app_runtime::AppRuntime;
use lxapp::{LxApp, LxAppError};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;

const HISTORY_DB: &str = "browser-history.sqlite3";
const COALESCE_WINDOW_MS: u64 = 5_000;
const MAX_ENTRIES: usize = 10_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: String,
    pub url: String,
    pub title: String,
    pub visited_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HistorySnapshot {
    pub entries: Vec<HistoryEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum HistoryWatchEvent {
    Snapshot { entries: Vec<HistoryEntry> },
    Upsert { entry: HistoryEntry },
    Remove { id: String },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdInput {
    id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClearInput {
    #[serde(default)]
    since_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClearResult {
    removed: usize,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn next_id() -> String {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    format!("h{}-{}", now_ms(), SEQ.fetch_add(1, Ordering::Relaxed))
}

fn is_recordable_url(url: &str) -> bool {
    let normalized = normalize_url(url);
    normalized.starts_with("http://") || normalized.starts_with("https://")
}

fn default_title(url: &str) -> String {
    url.split_once("://")
        .map(|(_, rest)| rest.split(['/', '?']).next().unwrap_or(rest).to_string())
        .unwrap_or_else(|| url.to_string())
}

fn history_path(app_data_dir: &Path) -> PathBuf {
    lingxia_app_context::app_state_file(app_data_dir, HISTORY_DB)
}

fn store_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn channel() -> &'static broadcast::Sender<HistoryWatchEvent> {
    static CHANNEL: OnceLock<broadcast::Sender<HistoryWatchEvent>> = OnceLock::new();
    CHANNEL.get_or_init(|| broadcast::channel(32).0)
}

fn database_error(error: rusqlite::Error) -> LxAppError {
    LxAppError::Runtime(format!("browser history database: {error}"))
}

fn open(app_data_dir: &Path) -> Result<Connection, LxAppError> {
    let path = history_path(app_data_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| LxAppError::IoError(format!("mkdir {}: {error}", parent.display())))?;
    }
    let connection = Connection::open(&path).map_err(database_error)?;
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(database_error)?;
    connection
        .execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             CREATE TABLE IF NOT EXISTS history (
                 id TEXT PRIMARY KEY NOT NULL,
                 url TEXT NOT NULL,
                 normalized_url TEXT NOT NULL,
                 title TEXT NOT NULL,
                 visited_at_ms INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS history_visited_at
                 ON history(visited_at_ms DESC);
             CREATE INDEX IF NOT EXISTS history_normalized_url
                 ON history(normalized_url, visited_at_ms DESC);",
        )
        .map_err(database_error)?;
    Ok(connection)
}

fn load_from(connection: &Connection) -> Result<HistorySnapshot, LxAppError> {
    let mut statement = connection
        .prepare(
            "SELECT id, url, title, visited_at_ms
             FROM history
             ORDER BY visited_at_ms DESC, rowid DESC
             LIMIT ?1",
        )
        .map_err(database_error)?;
    let rows = statement
        .query_map([MAX_ENTRIES as i64], |row| {
            Ok(HistoryEntry {
                id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                visited_at_ms: row.get::<_, i64>(3)?.max(0) as u64,
            })
        })
        .map_err(database_error)?;
    let entries = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(database_error)?;
    Ok(HistorySnapshot { entries })
}

fn load(app_data_dir: &Path) -> Result<HistorySnapshot, LxAppError> {
    let connection = open(app_data_dir)?;
    load_from(&connection)
}

fn broadcast_snapshot(connection: &Connection) -> Result<(), LxAppError> {
    // Skip the full snapshot query when nobody is watching.
    if channel().receiver_count() == 0 {
        return Ok(());
    }
    let snapshot = load_from(connection)?;
    let _ = channel().send(HistoryWatchEvent::Snapshot {
        entries: snapshot.entries,
    });
    Ok(())
}

fn load_entry(connection: &Connection, id: &str) -> Result<Option<HistoryEntry>, LxAppError> {
    connection
        .query_row(
            "SELECT id, url, title, visited_at_ms FROM history WHERE id = ?1",
            [id],
            |row| {
                Ok(HistoryEntry {
                    id: row.get(0)?,
                    url: row.get(1)?,
                    title: row.get(2)?,
                    visited_at_ms: row.get::<_, i64>(3)?.max(0) as u64,
                })
            },
        )
        .optional()
        .map_err(database_error)
}

fn broadcast_upsert(connection: &Connection, id: &str) -> Result<(), LxAppError> {
    if channel().receiver_count() == 0 {
        return Ok(());
    }
    if let Some(entry) = load_entry(connection, id)? {
        let _ = channel().send(HistoryWatchEvent::Upsert { entry });
    }
    Ok(())
}

fn record_in(
    app_data_dir: &Path,
    url: &str,
    title: &str,
    visited_at_ms: u64,
) -> Result<bool, LxAppError> {
    let url = url.trim();
    if !is_recordable_url(url) {
        return Ok(false);
    }
    let _guard = store_lock()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let mut connection = open(app_data_dir)?;
    let transaction = connection.transaction().map_err(database_error)?;
    let latest = transaction
        .query_row(
            "SELECT id, normalized_url, visited_at_ms
             FROM history
             ORDER BY visited_at_ms DESC, rowid DESC
             LIMIT 1",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?.max(0) as u64,
                ))
            },
        )
        .optional()
        .map_err(database_error)?;
    let normalized = normalize_url(url);
    let title = title.trim();
    let coalesced = latest.as_ref().is_some_and(|(_, latest_url, latest_at)| {
        latest_url == &normalized && visited_at_ms.saturating_sub(*latest_at) <= COALESCE_WINDOW_MS
    });
    let mut pruned_ids: Vec<String> = Vec::new();
    let affected_id = if coalesced {
        let latest_id = latest
            .as_ref()
            .expect("coalesced history entry exists")
            .0
            .clone();
        if title.is_empty() {
            transaction
                .execute(
                    "UPDATE history SET url = ?1, normalized_url = ?2, visited_at_ms = ?3
                     WHERE id = ?4",
                    params![url, normalized, visited_at_ms as i64, &latest_id],
                )
                .map_err(database_error)?;
        } else {
            transaction
                .execute(
                    "UPDATE history
                     SET url = ?1, normalized_url = ?2, title = ?3, visited_at_ms = ?4
                     WHERE id = ?5",
                    params![url, normalized, title, visited_at_ms as i64, &latest_id],
                )
                .map_err(database_error)?;
        }
        latest_id
    } else {
        let display_title = if title.is_empty() {
            default_title(url)
        } else {
            title.to_string()
        };
        let id = next_id();
        transaction
            .execute(
                "INSERT INTO history (id, url, normalized_url, title, visited_at_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![&id, url, normalized, display_title, visited_at_ms as i64],
            )
            .map_err(database_error)?;
        // Watchers apply deltas, so the cap prune must reach them as explicit
        // removals — a client-side re-sort can tie-break differently and drift
        // from the store.
        let mut prune = transaction
            .prepare(
                "DELETE FROM history WHERE id IN (
                     SELECT id FROM history
                     ORDER BY visited_at_ms DESC, rowid DESC
                     LIMIT -1 OFFSET ?1
                 ) RETURNING id",
            )
            .map_err(database_error)?;
        let ids = prune
            .query_map([MAX_ENTRIES as i64], |row| row.get::<_, String>(0))
            .map_err(database_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(database_error)?;
        drop(prune);
        pruned_ids = ids;
        id
    };
    transaction.commit().map_err(database_error)?;
    broadcast_upsert(&connection, &affected_id)?;
    if channel().receiver_count() > 0 {
        for id in pruned_ids {
            let _ = channel().send(HistoryWatchEvent::Remove { id });
        }
    }
    Ok(!coalesced)
}

fn update_title_in(app_data_dir: &Path, url: &str, title: &str) -> Result<bool, LxAppError> {
    let _guard = store_lock()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let connection = open(app_data_dir)?;
    // Retitle only the newest visit of the URL; the inequality lives on the
    // outer UPDATE so an already-current title never retargets an older row.
    let changed = connection
        .execute(
            "UPDATE history SET title = ?1
             WHERE title <> ?1 AND id = (
                 SELECT id FROM history
                 WHERE normalized_url = ?2
                 ORDER BY visited_at_ms DESC, rowid DESC
                 LIMIT 1
             )",
            params![title, normalize_url(url)],
        )
        .map_err(database_error)?
        > 0;
    if changed && channel().receiver_count() > 0 {
        let newest = connection
            .query_row(
                "SELECT id, url, title, visited_at_ms FROM history
                 WHERE normalized_url = ?1
                 ORDER BY visited_at_ms DESC, rowid DESC LIMIT 1",
                [normalize_url(url)],
                |row| {
                    Ok(HistoryEntry {
                        id: row.get(0)?,
                        url: row.get(1)?,
                        title: row.get(2)?,
                        visited_at_ms: row.get::<_, i64>(3)?.max(0) as u64,
                    })
                },
            )
            .optional()
            .map_err(database_error)?;
        if let Some(entry) = newest {
            let _ = channel().send(HistoryWatchEvent::Upsert { entry });
        }
    }
    Ok(changed)
}

enum Job {
    Record {
        url: String,
        title: String,
        visited_at_ms: u64,
    },
    UpdateTitle {
        url: String,
        title: String,
    },
}

/// SQLite writes run on a dedicated thread: `record_visit`/`update_title`
/// fire from platform navigation callbacks (UI thread) and must not block.
/// A single queue preserves navigation event order.
fn worker() -> &'static std::sync::mpsc::Sender<Job> {
    static WORKER: OnceLock<std::sync::mpsc::Sender<Job>> = OnceLock::new();
    WORKER.get_or_init(|| {
        let (sender, receiver) = std::sync::mpsc::channel::<Job>();
        std::thread::Builder::new()
            .name("browser-history".to_string())
            .spawn(move || {
                while let Ok(job) = receiver.recv() {
                    run_job(job);
                }
            })
            .expect("spawn browser-history worker");
        sender
    })
}

fn run_job(job: Job) {
    let Some(runtime) = lxapp::get_platform() else {
        return;
    };
    let app_data_dir = runtime.app_data_dir();
    match job {
        Job::Record {
            url,
            title,
            visited_at_ms,
        } => {
            if let Err(error) = record_in(&app_data_dir, &url, &title, visited_at_ms) {
                log::warn!("[BrowserHistory] record failed: {error}");
            }
        }
        Job::UpdateTitle { url, title } => {
            if let Err(error) = update_title_in(&app_data_dir, &url, &title) {
                log::warn!("[BrowserHistory] title update failed: {error}");
            }
        }
    }
}

pub fn record_visit(url: &str, title: &str) {
    let url = url.trim();
    if !is_recordable_url(url) {
        return;
    }
    let _ = worker().send(Job::Record {
        url: url.to_string(),
        title: title.to_string(),
        visited_at_ms: now_ms(),
    });
}

pub fn update_title(url: &str, title: &str) {
    let title = title.trim();
    if title.is_empty() || !is_recordable_url(url) {
        return;
    }
    let _ = worker().send(Job::UpdateTitle {
        url: url.to_string(),
        title: title.to_string(),
    });
}

pub(crate) fn clear_since_in(
    app_data_dir: &Path,
    since_ms: Option<u64>,
) -> Result<usize, LxAppError> {
    let _guard = store_lock()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let connection = open(app_data_dir)?;
    let removed = match since_ms {
        Some(cutoff) => connection
            .execute(
                "DELETE FROM history WHERE visited_at_ms >= ?1",
                // Saturate: an `as` cast wraps huge cutoffs negative and would delete everything.
                [i64::try_from(cutoff).unwrap_or(i64::MAX)],
            )
            .map_err(database_error)?,
        None => {
            let removed = connection
                .execute("DELETE FROM history", [])
                .map_err(database_error)?;
            // Best-effort: keep cleared URLs from surviving in the WAL.
            let _ = connection.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()));
            removed
        }
    };
    broadcast_snapshot(&connection)?;
    Ok(removed)
}

pub(crate) fn count_in(app_data_dir: &Path) -> Result<usize, LxAppError> {
    let _guard = store_lock()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let connection = open(app_data_dir)?;
    connection
        .query_row("SELECT COUNT(*) FROM history", [], |row| {
            row.get::<_, i64>(0)
        })
        .map(|count| count.max(0) as usize)
        .map_err(database_error)
}

/// Returns at most `MAX_ENTRIES` (10k) newest visits — the store itself is
/// pruned to that cap. No query input: this is an internal webui route and the
/// history page filters/searches client-side by design.
#[lingxia::native("history.list")]
fn list_history(app: Arc<LxApp>) -> HostResult<HistorySnapshot> {
    crate::require_builtin_browser(&app)?;
    let _guard = store_lock()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    load(&app.app_data_dir())
}

#[lingxia::native("history.remove")]
fn remove_history_entry(app: Arc<LxApp>, input: IdInput) -> HostResult<()> {
    crate::require_builtin_browser(&app)?;
    let _guard = store_lock()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let connection = open(&app.app_data_dir())?;
    let removed = connection
        .execute("DELETE FROM history WHERE id = ?1", [&input.id])
        .map_err(database_error)?;
    if removed == 0 {
        return Err(LxAppError::ResourceNotFound(format!(
            "history entry not found: {}",
            input.id
        )));
    }
    let _ = channel().send(HistoryWatchEvent::Remove {
        id: input.id.clone(),
    });
    Ok(())
}

#[lingxia::native("history.clear")]
fn clear_history(app: Arc<LxApp>, input: ClearInput) -> HostResult<ClearResult> {
    crate::require_builtin_browser(&app)?;
    clear_since_in(&app.app_data_dir(), input.since_ms).map(|removed| ClearResult { removed })
}

#[lingxia::native("history.watch", stream)]
async fn watch_history(
    app: Arc<LxApp>,
    mut stream: StreamContext<HistoryWatchEvent>,
) -> HostResult<()> {
    crate::require_builtin_browser(&app)?;
    let mut receiver = channel().subscribe();
    {
        let _guard = store_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let snapshot = load(&app.app_data_dir())?;
        stream.send(HistoryWatchEvent::Snapshot {
            entries: snapshot.entries,
        })?;
    }
    loop {
        tokio::select! {
            _ = stream.canceled() => return Ok(()),
            received = receiver.recv() => {
                match received {
                    Ok(event) => stream.send(event)?,
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        let _guard = store_lock().lock().unwrap_or_else(|error| error.into_inner());
                        let snapshot = load(&app.app_data_dir())?;
                        stream.send(HistoryWatchEvent::Snapshot {
                            entries: snapshot.entries,
                        })?;
                    }
                    Err(broadcast::error::RecvError::Closed) => return stream.end(()),
                }
            }
        }
    }
}

pub(crate) fn register() {
    lxapp::host::register_host_entry(list_history_host());
    lxapp::host::register_host_entry(remove_history_entry_host());
    lxapp::host::register_host_entry(clear_history_host());
    lxapp::host::register_host_entry(watch_history_host());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_coalesces_immediate_duplicate() {
        let dir = tempfile::tempdir().unwrap();
        assert!(record_in(dir.path(), "https://example.com/", "First", 1_000).unwrap());
        assert!(!record_in(dir.path(), "https://EXAMPLE.com", "Second", 2_000).unwrap());
        let snapshot = load(dir.path()).unwrap();
        assert_eq!(snapshot.entries.len(), 1);
        assert_eq!(snapshot.entries[0].title, "Second");
    }

    #[test]
    fn update_title_never_retargets_an_older_visit() {
        let dir = tempfile::tempdir().unwrap();
        record_in(dir.path(), "https://example.com", "Old", 1_000).unwrap();
        record_in(dir.path(), "https://example.com", "Newest", 10_000).unwrap();
        // Newest row already carries the title: nothing may change.
        assert!(!update_title_in(dir.path(), "https://example.com", "Newest").unwrap());
        let snapshot = load(dir.path()).unwrap();
        assert_eq!(snapshot.entries[0].title, "Newest");
        assert_eq!(snapshot.entries[1].title, "Old");
        // A new title still lands on the newest row only.
        assert!(update_title_in(dir.path(), "https://example.com", "Renamed").unwrap());
        let snapshot = load(dir.path()).unwrap();
        assert_eq!(snapshot.entries[0].title, "Renamed");
        assert_eq!(snapshot.entries[1].title, "Old");
    }

    #[test]
    fn clear_since_preserves_older_entries() {
        let dir = tempfile::tempdir().unwrap();
        record_in(dir.path(), "https://one.test", "One", 1_000).unwrap();
        record_in(dir.path(), "https://two.test", "Two", 10_000).unwrap();
        assert_eq!(clear_since_in(dir.path(), Some(5_000)).unwrap(), 1);
        let snapshot = load(dir.path()).unwrap();
        assert_eq!(snapshot.entries[0].title, "One");
    }

    #[test]
    fn history_is_stored_in_sqlite() {
        let dir = tempfile::tempdir().unwrap();
        record_in(dir.path(), "https://example.com", "Example", 1_000).unwrap();
        let header = std::fs::read(history_path(dir.path())).unwrap();
        assert!(header.starts_with(b"SQLite format 3\0"));
    }
}
