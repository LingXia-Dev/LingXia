//! Browser bookmarks: persistent store + host routes + chrome-facing helpers.
//!
//! Persistence mirrors proxy settings: one JSON file in the app state
//! directory (`browser-bookmarks.json`). Every mutation goes through the
//! store lock (load → mutate → replace save) and broadcasts the new snapshot
//! so `bookmarks.watch` subscribers (newtab, bookmarks page) re-render live
//! when the native chrome toggles a star.

use crate::bookmarks_html::{ImportedBookmark, export_chrome_html, parse_chrome_html};
use crate::host::{HostCancel, HostResult, StreamContext, await_or_cancel};
use crate::platform_error::map_platform_error;
use lingxia_platform::traits::app_runtime::AppRuntime;
use lingxia_service::file::{ChooseFileRequest, FileDialogFilter};
use lxapp::LxApp;
use lxapp::LxAppError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;

const BOOKMARKS_FILE: &str = "browser-bookmarks.json";
const CURRENT_VERSION: u32 = 1;
const MAX_IMPORT_FILE_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkGroup {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkEntry {
    pub id: String,
    pub url: String,
    #[serde(default)]
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
    /// Pinned bookmarks are the high-frequency subset shown as the sidebar's
    /// top favicon grid. Invariant: pinned ⊆ bookmarked (pin implies the entry
    /// exists here). Order within `entries` is the grid order.
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub created_at_ms: u64,
}

/// The persisted file and the `list`/`watch` payload are the same shape.
/// Entry order within the vec is the display order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BookmarksSnapshot {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    pub groups: Vec<BookmarkGroup>,
    #[serde(default)]
    pub entries: Vec<BookmarkEntry>,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn next_id(prefix: &str) -> String {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    format!(
        "{prefix}{}-{}",
        now_ms(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    )
}

/// Dedup key: trimmed, fragment stripped, trailing `/` stripped, and the
/// scheme+host lowered (paths stay case-sensitive).
fn normalize_url(raw: &str) -> String {
    let mut s = raw.trim();
    if let Some(hash) = s.find('#') {
        s = &s[..hash];
    }
    let s = s.strip_suffix('/').unwrap_or(s);
    match s.split_once("://") {
        Some((scheme, rest)) => {
            let (host, path) = match rest.find(['/', '?']) {
                Some(i) => (&rest[..i], &rest[i..]),
                None => (rest, ""),
            };
            format!(
                "{}://{}{}",
                scheme.to_ascii_lowercase(),
                host.to_ascii_lowercase(),
                path
            )
        }
        None => s.to_string(),
    }
}

fn default_title(url: &str) -> String {
    url.split_once("://")
        .map(|(_, rest)| rest.split('/').next().unwrap_or(rest))
        .unwrap_or(url)
        .to_string()
}

// MARK: - Store

fn bookmarks_path(app_data_dir: &Path) -> PathBuf {
    lingxia_app_context::app_state_file(app_data_dir, BOOKMARKS_FILE)
}

fn store_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn channel() -> &'static broadcast::Sender<BookmarksSnapshot> {
    static CHANNEL: OnceLock<broadcast::Sender<BookmarksSnapshot>> = OnceLock::new();
    CHANNEL.get_or_init(|| broadcast::channel(32).0)
}

fn load(app_data_dir: &Path) -> Result<BookmarksSnapshot, LxAppError> {
    let path = bookmarks_path(app_data_dir);
    if !path.is_file() {
        return Ok(BookmarksSnapshot {
            version: CURRENT_VERSION,
            ..Default::default()
        });
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|e| LxAppError::IoError(format!("read {}: {e}", path.display())))?;
    serde_json::from_str(&content)
        .map_err(|e| LxAppError::InvalidJsonFile(format!("{}: {e}", path.display())))
}

fn save(app_data_dir: &Path, snapshot: &BookmarksSnapshot) -> Result<(), LxAppError> {
    let path = bookmarks_path(app_data_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| LxAppError::IoError(format!("mkdir {}: {e}", parent.display())))?;
    }
    let json = serde_json::to_string_pretty(snapshot)
        .map_err(|e| LxAppError::Bridge(format!("JSON Processing Error: {e}")))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)
        .map_err(|e| LxAppError::IoError(format!("write {}: {e}", tmp.display())))?;
    replace_saved_file(&tmp, &path)?;
    Ok(())
}

#[cfg(not(windows))]
fn replace_saved_file(tmp: &Path, path: &Path) -> Result<(), LxAppError> {
    std::fs::rename(tmp, path)
        .map_err(|e| LxAppError::IoError(format!("rename {}: {e}", path.display())))
}

#[cfg(windows)]
fn replace_saved_file(tmp: &Path, path: &Path) -> Result<(), LxAppError> {
    let backup = path.with_extension("json.bak");
    if backup.exists() {
        std::fs::remove_file(&backup)
            .map_err(|e| LxAppError::IoError(format!("remove {}: {e}", backup.display())))?;
    }
    let had_previous = path.exists();
    if had_previous {
        std::fs::rename(path, &backup)
            .map_err(|e| LxAppError::IoError(format!("backup {}: {e}", path.display())))?;
    }
    if let Err(error) = std::fs::rename(tmp, path) {
        if had_previous {
            let _ = std::fs::rename(&backup, path);
        }
        return Err(LxAppError::IoError(format!(
            "replace {}: {error}",
            path.display()
        )));
    }
    if had_previous {
        let _ = std::fs::remove_file(backup);
    }
    Ok(())
}

/// Native-chrome change listener (e.g. macOS sidebar refresh). One per host.
fn change_listener() -> &'static OnceLock<Box<dyn Fn() + Send + Sync>> {
    static LISTENER: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();
    &LISTENER
}

/// Register the chrome-side change callback; fires after every mutation.
pub fn set_change_listener(listener: Box<dyn Fn() + Send + Sync>) {
    let _ = change_listener().set(listener);
}

/// Load → mutate → save → broadcast, under the store lock.
fn mutate<T>(
    app_data_dir: &Path,
    op: impl FnOnce(&mut BookmarksSnapshot) -> Result<T, LxAppError>,
) -> Result<T, LxAppError> {
    let out = {
        let _guard = store_lock().lock().unwrap_or_else(|e| e.into_inner());
        let mut snapshot = load(app_data_dir)?;
        let out = op(&mut snapshot)?;
        snapshot.version = CURRENT_VERSION;
        save(app_data_dir, &snapshot)?;
        let _ = channel().send(snapshot);
        out
    };
    if let Some(listener) = change_listener().get() {
        listener();
    }
    Ok(out)
}

fn find_entry<'a>(
    snapshot: &'a BookmarksSnapshot,
    normalized_url: &str,
) -> Option<&'a BookmarkEntry> {
    snapshot
        .entries
        .iter()
        .find(|e| normalize_url(&e.url) == normalized_url)
}

fn add_entry(
    snapshot: &mut BookmarksSnapshot,
    url: &str,
    title: Option<&str>,
    group_id: Option<&str>,
) -> Result<(BookmarkEntry, bool), LxAppError> {
    let url = url.trim();
    if url.is_empty() {
        return Err(LxAppError::InvalidParameter(
            "bookmark url must not be empty".to_string(),
        ));
    }
    if !crate::address_bar::is_valid_explicit_http_url(url) {
        return Err(LxAppError::InvalidParameter(
            "bookmark url must be an http or https URL with a host".to_string(),
        ));
    }
    let normalized = normalize_url(url);
    if let Some(existing) = find_entry(snapshot, &normalized) {
        return Ok((existing.clone(), false));
    }
    if let Some(gid) = group_id
        && !snapshot.groups.iter().any(|g| g.id == gid)
    {
        return Err(LxAppError::ResourceNotFound(format!(
            "bookmark group not found: {gid}"
        )));
    }
    let title = title
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| default_title(url));
    let entry = BookmarkEntry {
        id: next_id("b"),
        url: url.to_string(),
        title,
        group_id: group_id.map(str::to_string),
        pinned: false,
        created_at_ms: now_ms(),
    };
    snapshot.entries.push(entry.clone());
    Ok((entry, true))
}

// MARK: - Chrome-facing helpers (no LxApp, e.g. from Swift FFI)

fn runtime_data_dir() -> Option<PathBuf> {
    lxapp::get_platform().map(|runtime| runtime.app_data_dir())
}

/// Is `url` bookmarked? `false` when the runtime is not up or the store is
/// unreadable — the chrome star simply shows unstarred.
pub fn is_bookmarked(url: &str) -> bool {
    let Some(dir) = runtime_data_dir() else {
        return false;
    };
    let _guard = store_lock().lock().unwrap_or_else(|e| e.into_inner());
    match load(&dir) {
        Ok(snapshot) => find_entry(&snapshot, &normalize_url(url)).is_some(),
        Err(_) => false,
    }
}

/// Full snapshot as JSON for native chrome (sidebar bookmarks section).
pub fn snapshot_json() -> Option<String> {
    let dir = runtime_data_dir()?;
    let _guard = store_lock().lock().unwrap_or_else(|e| e.into_inner());
    let snapshot = load(&dir).ok()?;
    serde_json::to_string(&snapshot).ok()
}

/// Stable comparison key used by native chrome when matching a stored
/// bookmark to an open tab.
pub fn normalize_url_for_match(raw: &str) -> String {
    normalize_url(raw)
}

/// Typed snapshot for native shell chrome. Platform SDKs use this to render
/// pinned entries without duplicating the persisted JSON schema.
pub fn snapshot() -> Option<BookmarksSnapshot> {
    let dir = runtime_data_dir()?;
    let _guard = store_lock().lock().unwrap_or_else(|e| e.into_inner());
    load(&dir).ok()
}

/// Pin `url` to the sidebar grid ("Pin to Sidebar" on a tab). Bookmarks the
/// page first when needed, keeping the pinned ⊆ bookmarked invariant without
/// forcing a two-step flow on the user.
pub fn pin_url(url: &str, title: &str) -> bool {
    let Some(dir) = runtime_data_dir() else {
        return false;
    };
    let title = if title.trim().is_empty() {
        None
    } else {
        Some(title)
    };
    mutate(&dir, |snapshot| {
        let (entry, _) = add_entry(snapshot, url, title, None)?;
        let id = entry.id;
        let entry = snapshot
            .entries
            .iter_mut()
            .find(|e| e.id == id)
            .expect("entry just added or found");
        entry.pinned = true;
        Ok(())
    })
    .map_err(|e| log::warn!("[BrowserBookmarks] pin failed: {e}"))
    .is_ok()
}

/// Remove `url` from bookmarks (sidebar row action). True when removed.
pub fn remove_by_url(url: &str) -> bool {
    let Some(dir) = runtime_data_dir() else {
        return false;
    };
    mutate(&dir, |snapshot| {
        let normalized = normalize_url(url);
        let before = snapshot.entries.len();
        snapshot
            .entries
            .retain(|e| normalize_url(&e.url) != normalized);
        Ok(snapshot.entries.len() != before)
    })
    .unwrap_or(false)
}

/// One in-place management command from native chrome (sidebar menus), as
/// JSON: `{"op": "rename" | "move" | "createGroupAndMove" | "renameGroup" |
/// "deleteGroup", ...}`. Returns false on any error.
pub fn command_json(json: &str) -> bool {
    #[derive(Deserialize)]
    #[serde(tag = "op", rename_all = "camelCase")]
    enum Command {
        #[serde(rename_all = "camelCase")]
        Rename { id: String, title: String },
        #[serde(rename_all = "camelCase")]
        Move {
            id: String,
            #[serde(default)]
            group_id: Option<String>,
        },
        #[serde(rename_all = "camelCase")]
        CreateGroupAndMove { id: String, name: String },
        #[serde(rename_all = "camelCase")]
        RenameGroup { id: String, name: String },
        #[serde(rename_all = "camelCase")]
        DeleteGroup { id: String },
        #[serde(rename_all = "camelCase")]
        SetPinned { id: String, pinned: bool },
    }

    let Some(dir) = runtime_data_dir() else {
        return false;
    };
    let command: Command = match serde_json::from_str(json) {
        Ok(command) => command,
        Err(err) => {
            log::warn!("[BrowserBookmarks] bad command: {err}");
            return false;
        }
    };
    let result = mutate(&dir, |snapshot| match command {
        Command::Rename { id, title } => {
            let title = title.trim();
            if title.is_empty() {
                return Err(LxAppError::InvalidParameter("empty title".to_string()));
            }
            let entry = snapshot
                .entries
                .iter_mut()
                .find(|e| e.id == id)
                .ok_or(LxAppError::ResourceNotFound(id))?;
            entry.title = title.to_string();
            Ok(())
        }
        Command::Move { id, group_id } => {
            if let Some(gid) = group_id.as_deref()
                && !snapshot.groups.iter().any(|g| g.id == gid)
            {
                return Err(LxAppError::ResourceNotFound(gid.to_string()));
            }
            let entry = snapshot
                .entries
                .iter_mut()
                .find(|e| e.id == id)
                .ok_or(LxAppError::ResourceNotFound(id))?;
            entry.group_id = group_id;
            Ok(())
        }
        Command::CreateGroupAndMove { id, name } => {
            let name = name.trim();
            if name.is_empty() {
                return Err(LxAppError::InvalidParameter("empty group name".to_string()));
            }
            let group_id = match snapshot.groups.iter().find(|g| g.name == name) {
                Some(existing) => existing.id.clone(),
                None => {
                    let group = BookmarkGroup {
                        id: next_id("g"),
                        name: name.to_string(),
                    };
                    let group_id = group.id.clone();
                    snapshot.groups.push(group);
                    group_id
                }
            };
            let entry = snapshot
                .entries
                .iter_mut()
                .find(|e| e.id == id)
                .ok_or(LxAppError::ResourceNotFound(id))?;
            entry.group_id = Some(group_id);
            Ok(())
        }
        Command::RenameGroup { id, name } => {
            let name = name.trim();
            if name.is_empty() {
                return Err(LxAppError::InvalidParameter("empty group name".to_string()));
            }
            let group = snapshot
                .groups
                .iter_mut()
                .find(|g| g.id == id)
                .ok_or(LxAppError::ResourceNotFound(id))?;
            group.name = name.to_string();
            Ok(())
        }
        Command::SetPinned { id, pinned } => {
            let entry = snapshot
                .entries
                .iter_mut()
                .find(|e| e.id == id)
                .ok_or(LxAppError::ResourceNotFound(id))?;
            entry.pinned = pinned;
            Ok(())
        }
        Command::DeleteGroup { id } => {
            let before = snapshot.groups.len();
            snapshot.groups.retain(|g| g.id != id);
            if snapshot.groups.len() == before {
                return Err(LxAppError::ResourceNotFound(id.clone()));
            }
            for entry in snapshot
                .entries
                .iter_mut()
                .filter(|e| e.group_id.as_deref() == Some(id.as_str()))
            {
                entry.group_id = None;
            }
            Ok(())
        }
    });
    result
        .map_err(|e| log::warn!("[BrowserBookmarks] command failed: {e}"))
        .is_ok()
}

/// Toggle `url` (chrome star / ⌘D). Returns the new bookmarked state, or
/// `None` when the store is unavailable.
pub fn toggle_bookmark(url: &str, title: &str) -> Option<bool> {
    let dir = runtime_data_dir()?;
    let title = if title.trim().is_empty() {
        None
    } else {
        Some(title)
    };
    mutate(&dir, |snapshot| {
        let normalized = normalize_url(url);
        if let Some(existing) = find_entry(snapshot, &normalized) {
            let id = existing.id.clone();
            snapshot.entries.retain(|e| e.id != id);
            Ok(false)
        } else {
            add_entry(snapshot, url, title, None)?;
            Ok(true)
        }
    })
    .map_err(|e| log::warn!("[BrowserBookmarks] toggle failed: {e}"))
    .ok()
}

// MARK: - Host routes

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UrlInput {
    url: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    group_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdInput {
    id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenameInput {
    id: String,
    title: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MoveInput {
    id: String,
    #[serde(default)]
    group_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReorderInput {
    #[serde(default)]
    group_id: Option<String>,
    ordered_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GroupNameInput {
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GroupRenameInput {
    id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OrderedIdsInput {
    ordered_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetPinnedInput {
    id: String,
    pinned: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AddResult {
    entry: BookmarkEntry,
    created: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusResult {
    bookmarked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    entry: Option<BookmarkEntry>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ImportResult {
    imported: usize,
    skipped: usize,
    groups_created: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportResult {
    path: String,
    file_name: String,
    count: usize,
}

fn merge_imported(
    snapshot: &mut BookmarksSnapshot,
    bookmarks: Vec<ImportedBookmark>,
) -> ImportResult {
    let mut result = ImportResult {
        imported: 0,
        skipped: 0,
        groups_created: 0,
    };
    for imported in bookmarks {
        let url = imported.url.trim();
        if !crate::address_bar::is_valid_explicit_http_url(url)
            || find_entry(snapshot, &normalize_url(url)).is_some()
        {
            result.skipped += 1;
            continue;
        }

        let group_id = imported.group_name.as_deref().map(|name| {
            if let Some(group) = snapshot.groups.iter().find(|group| group.name == name) {
                return group.id.clone();
            }
            let group = BookmarkGroup {
                id: next_id("g"),
                name: name.to_string(),
            };
            let id = group.id.clone();
            snapshot.groups.push(group);
            result.groups_created += 1;
            id
        });

        match add_entry(
            snapshot,
            url,
            (!imported.title.is_empty()).then_some(imported.title.as_str()),
            group_id.as_deref(),
        ) {
            Ok((entry, true)) => {
                if let Some(created_at_ms) = imported.created_at_ms
                    && let Some(saved) = snapshot
                        .entries
                        .iter_mut()
                        .find(|saved| saved.id == entry.id)
                {
                    saved.created_at_ms = created_at_ms;
                }
                result.imported += 1;
            }
            _ => result.skipped += 1,
        }
    }
    result
}

fn unique_export_path(directory: &Path) -> PathBuf {
    let base = directory.join("bookmarks.html");
    if !base.exists() {
        return base;
    }
    for suffix in 1..10_000 {
        let candidate = directory.join(format!("bookmarks ({suffix}).html"));
        if !candidate.exists() {
            return candidate;
        }
    }
    directory.join(format!("bookmarks-{}.html", now_ms()))
}

fn validated_name(name: &str, what: &str) -> Result<String, LxAppError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(LxAppError::InvalidParameter(format!(
            "{what} must not be empty"
        )));
    }
    Ok(name.to_string())
}

fn is_id_permutation(expected: &[&str], ordered: &[String]) -> bool {
    if expected.len() != ordered.len() {
        return false;
    }
    let expected: std::collections::HashSet<&str> = expected.iter().copied().collect();
    let ordered: std::collections::HashSet<&str> = ordered.iter().map(String::as_str).collect();
    expected.len() == ordered.len() && expected == ordered
}

#[lingxia::native("bookmarks.list")]
fn list_bookmarks(app: Arc<LxApp>) -> HostResult<BookmarksSnapshot> {
    let _guard = store_lock().lock().unwrap_or_else(|e| e.into_inner());
    load(&app.app_data_dir())
}

#[lingxia::native("bookmarks.add")]
fn add_bookmark(app: Arc<LxApp>, input: UrlInput) -> HostResult<AddResult> {
    mutate(&app.app_data_dir(), |snapshot| {
        let (entry, created) = add_entry(
            snapshot,
            &input.url,
            input.title.as_deref(),
            input.group_id.as_deref(),
        )?;
        Ok(AddResult { entry, created })
    })
}

#[lingxia::native("bookmarks.remove")]
fn remove_bookmark(app: Arc<LxApp>, input: IdInput) -> HostResult<()> {
    mutate(&app.app_data_dir(), |snapshot| {
        let before = snapshot.entries.len();
        snapshot.entries.retain(|e| e.id != input.id);
        if snapshot.entries.len() == before {
            return Err(LxAppError::ResourceNotFound(format!(
                "bookmark not found: {}",
                input.id
            )));
        }
        Ok(())
    })
}

#[lingxia::native("bookmarks.toggle")]
fn toggle_bookmark_route(app: Arc<LxApp>, input: UrlInput) -> HostResult<StatusResult> {
    mutate(&app.app_data_dir(), |snapshot| {
        let normalized = normalize_url(&input.url);
        if let Some(existing) = find_entry(snapshot, &normalized) {
            let id = existing.id.clone();
            snapshot.entries.retain(|e| e.id != id);
            Ok(StatusResult {
                bookmarked: false,
                entry: None,
            })
        } else {
            let (entry, _) = add_entry(
                snapshot,
                &input.url,
                input.title.as_deref(),
                input.group_id.as_deref(),
            )?;
            Ok(StatusResult {
                bookmarked: true,
                entry: Some(entry),
            })
        }
    })
}

#[lingxia::native("bookmarks.status")]
fn bookmark_status(app: Arc<LxApp>, input: UrlInput) -> HostResult<StatusResult> {
    let _guard = store_lock().lock().unwrap_or_else(|e| e.into_inner());
    let snapshot = load(&app.app_data_dir())?;
    let entry = find_entry(&snapshot, &normalize_url(&input.url)).cloned();
    Ok(StatusResult {
        bookmarked: entry.is_some(),
        entry,
    })
}

#[lingxia::native("bookmarks.rename")]
fn rename_bookmark(app: Arc<LxApp>, input: RenameInput) -> HostResult<BookmarkEntry> {
    let title = validated_name(&input.title, "bookmark title")?;
    mutate(&app.app_data_dir(), |snapshot| {
        let entry = snapshot
            .entries
            .iter_mut()
            .find(|e| e.id == input.id)
            .ok_or_else(|| {
                LxAppError::ResourceNotFound(format!("bookmark not found: {}", input.id))
            })?;
        entry.title = title;
        Ok(entry.clone())
    })
}

#[lingxia::native("bookmarks.move")]
fn move_bookmark(app: Arc<LxApp>, input: MoveInput) -> HostResult<BookmarkEntry> {
    mutate(&app.app_data_dir(), |snapshot| {
        if let Some(gid) = input.group_id.as_deref()
            && !snapshot.groups.iter().any(|g| g.id == gid)
        {
            return Err(LxAppError::ResourceNotFound(format!(
                "bookmark group not found: {gid}"
            )));
        }
        let entry = snapshot
            .entries
            .iter_mut()
            .find(|e| e.id == input.id)
            .ok_or_else(|| {
                LxAppError::ResourceNotFound(format!("bookmark not found: {}", input.id))
            })?;
        entry.group_id = input.group_id.clone();
        Ok(entry.clone())
    })
}

#[lingxia::native("bookmarks.reorder")]
fn reorder_bookmarks(app: Arc<LxApp>, input: ReorderInput) -> HostResult<()> {
    mutate(&app.app_data_dir(), |snapshot| {
        // Reorder only the entries in the given group scope; entries in other
        // scopes keep their relative positions.
        let in_scope = |e: &BookmarkEntry| e.group_id.as_deref() == input.group_id.as_deref();
        let scope_ids: Vec<&str> = snapshot
            .entries
            .iter()
            .filter(|e| in_scope(e))
            .map(|e| e.id.as_str())
            .collect();
        if !is_id_permutation(&scope_ids, &input.ordered_ids) {
            return Err(LxAppError::InvalidParameter(
                "orderedIds must be a permutation of the group's bookmarks".to_string(),
            ));
        }
        let mut by_id: std::collections::HashMap<String, BookmarkEntry> = snapshot
            .entries
            .iter()
            .filter(|e| in_scope(e))
            .map(|e| (e.id.clone(), e.clone()))
            .collect();
        let mut ordered = input.ordered_ids.iter();
        for slot in snapshot.entries.iter_mut().filter(|e| in_scope(e)) {
            // Both iterators walk the same scope, so `ordered` cannot run dry.
            let id = ordered.next().expect("scope sizes verified equal");
            *slot = by_id.remove(id).expect("permutation verified");
        }
        Ok(())
    })
}

#[lingxia::native("bookmarks.setPinned")]
fn set_pinned(app: Arc<LxApp>, input: SetPinnedInput) -> HostResult<BookmarkEntry> {
    mutate(&app.app_data_dir(), |snapshot| {
        let entry = snapshot
            .entries
            .iter_mut()
            .find(|e| e.id == input.id)
            .ok_or_else(|| {
                LxAppError::ResourceNotFound(format!("bookmark not found: {}", input.id))
            })?;
        entry.pinned = input.pinned;
        Ok(entry.clone())
    })
}

#[lingxia::native("bookmarks.createGroup")]
fn create_group(app: Arc<LxApp>, input: GroupNameInput) -> HostResult<BookmarkGroup> {
    let name = validated_name(&input.name, "group name")?;
    mutate(&app.app_data_dir(), |snapshot| {
        if snapshot.groups.iter().any(|g| g.name == name) {
            return Err(LxAppError::InvalidParameter(format!(
                "group already exists: {name}"
            )));
        }
        let group = BookmarkGroup {
            id: next_id("g"),
            name,
        };
        snapshot.groups.push(group.clone());
        Ok(group)
    })
}

#[lingxia::native("bookmarks.renameGroup")]
fn rename_group(app: Arc<LxApp>, input: GroupRenameInput) -> HostResult<BookmarkGroup> {
    let name = validated_name(&input.name, "group name")?;
    mutate(&app.app_data_dir(), |snapshot| {
        if snapshot
            .groups
            .iter()
            .any(|g| g.name == name && g.id != input.id)
        {
            return Err(LxAppError::InvalidParameter(format!(
                "group already exists: {name}"
            )));
        }
        let group = snapshot
            .groups
            .iter_mut()
            .find(|g| g.id == input.id)
            .ok_or_else(|| {
                LxAppError::ResourceNotFound(format!("bookmark group not found: {}", input.id))
            })?;
        group.name = name;
        Ok(group.clone())
    })
}

#[lingxia::native("bookmarks.deleteGroup")]
fn delete_group(app: Arc<LxApp>, input: IdInput) -> HostResult<()> {
    mutate(&app.app_data_dir(), |snapshot| {
        let before = snapshot.groups.len();
        snapshot.groups.retain(|g| g.id != input.id);
        if snapshot.groups.len() == before {
            return Err(LxAppError::ResourceNotFound(format!(
                "bookmark group not found: {}",
                input.id
            )));
        }
        // Orphaned entries fall back to ungrouped rather than disappearing.
        for entry in snapshot
            .entries
            .iter_mut()
            .filter(|e| e.group_id.as_deref() == Some(input.id.as_str()))
        {
            entry.group_id = None;
        }
        Ok(())
    })
}

#[lingxia::native("bookmarks.reorderGroups")]
fn reorder_groups(app: Arc<LxApp>, input: OrderedIdsInput) -> HostResult<()> {
    mutate(&app.app_data_dir(), |snapshot| {
        let ids: Vec<&str> = snapshot.groups.iter().map(|g| g.id.as_str()).collect();
        if !is_id_permutation(&ids, &input.ordered_ids) {
            return Err(LxAppError::InvalidParameter(
                "orderedIds must be a permutation of all group ids".to_string(),
            ));
        }
        snapshot.groups.sort_by_key(|g| {
            input
                .ordered_ids
                .iter()
                .position(|id| *id == g.id)
                .unwrap_or(usize::MAX)
        });
        Ok(())
    })
}

#[lingxia::native("bookmarks.importChrome")]
async fn import_chrome_bookmarks(
    app: Arc<LxApp>,
    mut cancel: HostCancel,
) -> HostResult<Option<ImportResult>> {
    let app_for_picker = app.clone();
    let is_chinese = lingxia_service::settings::webui_language(&app.app_data_dir())
        .ok()
        .flatten()
        .as_deref()
        == Some("zh-CN");
    let selected = await_or_cancel(&mut cancel, async move {
        lingxia_service::file::choose_file(
            &*app_for_picker.runtime,
            ChooseFileRequest {
                multiple: false,
                filters: vec![FileDialogFilter {
                    name: Some(
                        if is_chinese {
                            "书签 HTML"
                        } else {
                            "Bookmark HTML"
                        }
                        .to_string(),
                    ),
                    extensions: vec!["html".to_string(), "htm".to_string()],
                }],
                title: Some(
                    if is_chinese {
                        "导入书签"
                    } else {
                        "Import Bookmarks"
                    }
                    .to_string(),
                ),
                default_path: None,
            },
        )
        .await
        .map_err(|error| map_platform_error("bookmarks.importChrome", error))
    })
    .await?;
    if selected.canceled {
        return Ok(None);
    }
    let path = selected.paths.first().ok_or_else(|| {
        LxAppError::InvalidParameter("bookmark import did not return a file".to_string())
    })?;
    let metadata = std::fs::metadata(path)
        .map_err(|error| LxAppError::IoError(format!("read {path}: {error}")))?;
    if metadata.len() > MAX_IMPORT_FILE_BYTES {
        return Err(LxAppError::InvalidParameter(format!(
            "bookmark file exceeds the {} MB limit",
            MAX_IMPORT_FILE_BYTES / 1024 / 1024
        )));
    }
    let html = std::fs::read_to_string(path)
        .map_err(|error| LxAppError::IoError(format!("read {path}: {error}")))?;
    if !html
        .get(..html.len().min(1024))
        .unwrap_or(&html)
        .to_ascii_lowercase()
        .contains("netscape-bookmark-file-1")
    {
        return Err(LxAppError::InvalidParameter(
            "select a Chrome-compatible bookmark HTML file".to_string(),
        ));
    }
    let bookmarks = parse_chrome_html(&html);
    mutate(&app.app_data_dir(), move |snapshot| {
        Ok(merge_imported(snapshot, bookmarks))
    })
    .map(Some)
}

#[lingxia::native("bookmarks.exportChrome")]
fn export_chrome_bookmarks(app: Arc<LxApp>) -> HostResult<ExportResult> {
    let snapshot = {
        let _guard = store_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        load(&app.app_data_dir())?
    };
    let directory = lingxia_service::downloads::dir(&app.app_data_dir());
    std::fs::create_dir_all(&directory)
        .map_err(|error| LxAppError::IoError(format!("mkdir {}: {error}", directory.display())))?;
    let path = unique_export_path(&directory);
    let html = export_chrome_html(&snapshot, now_ms());
    std::fs::write(&path, html)
        .map_err(|error| LxAppError::IoError(format!("write {}: {error}", path.display())))?;
    Ok(ExportResult {
        path: path.to_string_lossy().to_string(),
        file_name: path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("bookmarks.html")
            .to_string(),
        count: snapshot.entries.len(),
    })
}

#[lingxia::native("bookmarks.watch", stream)]
async fn watch_bookmarks(
    app: Arc<LxApp>,
    mut stream: StreamContext<BookmarksSnapshot>,
) -> HostResult<()> {
    let mut rx = channel().subscribe();
    // Seed subscribers with the current state so the page renders from the
    // stream alone.
    {
        let _guard = store_lock().lock().unwrap_or_else(|e| e.into_inner());
        let snapshot = load(&app.app_data_dir())?;
        stream.send(snapshot)?;
    }
    loop {
        tokio::select! {
            _ = stream.canceled() => return Ok(()),
            recv = rx.recv() => {
                match recv {
                    Ok(snapshot) => stream.send(snapshot)?,
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        // Snapshots are self-contained; resync from disk.
                        let _guard = store_lock().lock().unwrap_or_else(|e| e.into_inner());
                        let snapshot = load(&app.app_data_dir())?;
                        stream.send(snapshot)?;
                    }
                    Err(broadcast::error::RecvError::Closed) => return stream.end(()),
                }
            }
        }
    }
}

pub(crate) fn register() {
    lxapp::host::register_host_entry(list_bookmarks_host());
    lxapp::host::register_host_entry(add_bookmark_host());
    lxapp::host::register_host_entry(remove_bookmark_host());
    lxapp::host::register_host_entry(toggle_bookmark_route_host());
    lxapp::host::register_host_entry(bookmark_status_host());
    lxapp::host::register_host_entry(rename_bookmark_host());
    lxapp::host::register_host_entry(move_bookmark_host());
    lxapp::host::register_host_entry(reorder_bookmarks_host());
    lxapp::host::register_host_entry(set_pinned_host());
    lxapp::host::register_host_entry(create_group_host());
    lxapp::host::register_host_entry(rename_group_host());
    lxapp::host::register_host_entry(delete_group_host());
    lxapp::host::register_host_entry(reorder_groups_host());
    lxapp::host::register_host_entry(import_chrome_bookmarks_host());
    lxapp::host::register_host_entry(export_chrome_bookmarks_host());
    lxapp::host::register_host_entry(watch_bookmarks_host());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_fragment_slash_and_lowers_host() {
        assert_eq!(
            normalize_url("HTTPS://Example.COM/Path/#frag"),
            "https://example.com/Path"
        );
        assert_eq!(normalize_url("https://example.com/"), "https://example.com");
        assert_eq!(
            normalize_url("https://example.com/A?q=B"),
            "https://example.com/A?q=B"
        );
        assert_eq!(
            normalize_url("https://EXAMPLE.com?q=CaseSensitive"),
            "https://example.com?q=CaseSensitive"
        );
    }

    #[test]
    fn add_dedups_by_normalized_url() {
        let mut snapshot = BookmarksSnapshot::default();
        let (first, created) =
            add_entry(&mut snapshot, "https://example.com/", Some("Example"), None).unwrap();
        assert!(created);
        let (again, created) = add_entry(&mut snapshot, "https://EXAMPLE.com", None, None).unwrap();
        assert!(!created);
        assert_eq!(first.id, again.id);
        assert_eq!(snapshot.entries.len(), 1);
    }

    #[test]
    fn add_defaults_title_to_host() {
        let mut snapshot = BookmarksSnapshot::default();
        let (entry, _) =
            add_entry(&mut snapshot, "https://docs.example.com/guide", None, None).unwrap();
        assert_eq!(entry.title, "docs.example.com");
    }

    #[test]
    fn add_rejects_unknown_group() {
        let mut snapshot = BookmarksSnapshot::default();
        assert!(add_entry(&mut snapshot, "https://a.test", None, Some("nope")).is_err());
    }

    #[test]
    fn add_rejects_non_web_urls_and_missing_hosts() {
        let mut snapshot = BookmarksSnapshot::default();
        for url in [
            "javascript://alert(1)",
            "file:///tmp/private.txt",
            "https://",
            "https:///missing-host",
        ] {
            assert!(
                add_entry(&mut snapshot, url, None, None).is_err(),
                "unexpectedly accepted {url}"
            );
        }
        assert!(snapshot.entries.is_empty());
    }

    #[test]
    fn permutation_rejects_duplicate_ids() {
        let expected = ["a", "b"];
        assert!(is_id_permutation(
            &expected,
            &["b".to_string(), "a".to_string()]
        ));
        assert!(!is_id_permutation(
            &expected,
            &["a".to_string(), "a".to_string()]
        ));
    }

    #[test]
    fn import_merges_valid_bookmarks_and_preserves_existing_entries() {
        let mut snapshot = BookmarksSnapshot::default();
        add_entry(
            &mut snapshot,
            "https://existing.test",
            Some("Existing"),
            None,
        )
        .unwrap();
        let result = merge_imported(
            &mut snapshot,
            vec![
                ImportedBookmark {
                    url: "https://EXISTING.test/".into(),
                    title: "Duplicate".into(),
                    group_name: Some("Imported".into()),
                    created_at_ms: None,
                },
                ImportedBookmark {
                    url: "javascript:alert(1)".into(),
                    title: "Unsafe".into(),
                    group_name: Some("Imported".into()),
                    created_at_ms: None,
                },
                ImportedBookmark {
                    url: "https://new.test/docs".into(),
                    title: "New".into(),
                    group_name: Some("Imported".into()),
                    created_at_ms: Some(42_000),
                },
            ],
        );
        assert_eq!(
            result,
            ImportResult {
                imported: 1,
                skipped: 2,
                groups_created: 1,
            }
        );
        assert_eq!(snapshot.entries.len(), 2);
        assert_eq!(snapshot.groups.len(), 1);
        assert_eq!(snapshot.entries[1].created_at_ms, 42_000);
        assert!(!snapshot.entries[1].pinned);
    }
}
