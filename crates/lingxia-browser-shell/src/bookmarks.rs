//! Browser bookmarks: persistent store + host routes + chrome-facing helpers.
//!
//! Persistence mirrors proxy settings: one JSON file in the app state
//! directory (`browser-bookmarks.json`). Every mutation goes through the
//! store lock (load → mutate → replace save) and broadcasts the new snapshot
//! so `bookmarks.watch` subscribers (newtab, bookmarks page) re-render live
//! when the native chrome toggles a star.

use crate::bookmarks_html::{ImportedBookmark, export_netscape_html, parse_netscape_html};
use crate::host::{HostCancel, HostResult, StreamContext, await_or_cancel};
use crate::platform_error::map_platform_error;
use crate::url_match::normalize_url;
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

fn default_title(url: &str) -> String {
    url.split_once("://")
        .map(|(_, rest)| rest.split(['/', '?']).next().unwrap_or(rest))
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
    match serde_json::from_str(&content) {
        Ok(snapshot) => Ok(snapshot),
        Err(e) => {
            // A corrupt file would otherwise fail every load forever; set it
            // aside and recover with an empty store.
            log::error!(
                "[BrowserBookmarks] corrupt {}: {e}; starting fresh",
                path.display()
            );
            let _ = std::fs::rename(&path, path.with_extension("json.corrupt"));
            Ok(BookmarksSnapshot {
                version: CURRENT_VERSION,
                ..Default::default()
            })
        }
    }
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

// MARK: - Shared store mutations
// Host routes and native-chrome `command_json` both dispatch through these,
// so the two entry points cannot drift apart.

fn rename_entry_op(
    snapshot: &mut BookmarksSnapshot,
    id: &str,
    title: &str,
) -> Result<BookmarkEntry, LxAppError> {
    let title = validated_name(title, "bookmark title")?;
    let entry = snapshot
        .entries
        .iter_mut()
        .find(|e| e.id == id)
        .ok_or_else(|| LxAppError::ResourceNotFound(format!("bookmark not found: {id}")))?;
    entry.title = title;
    Ok(entry.clone())
}

fn move_entry_op(
    snapshot: &mut BookmarksSnapshot,
    id: &str,
    group_id: Option<String>,
) -> Result<BookmarkEntry, LxAppError> {
    if let Some(gid) = group_id.as_deref()
        && !snapshot.groups.iter().any(|g| g.id == gid)
    {
        return Err(LxAppError::ResourceNotFound(format!(
            "bookmark group not found: {gid}"
        )));
    }
    let entry = snapshot
        .entries
        .iter_mut()
        .find(|e| e.id == id)
        .ok_or_else(|| LxAppError::ResourceNotFound(format!("bookmark not found: {id}")))?;
    entry.group_id = group_id;
    Ok(entry.clone())
}

fn set_pinned_op(
    snapshot: &mut BookmarksSnapshot,
    id: &str,
    pinned: bool,
) -> Result<BookmarkEntry, LxAppError> {
    let entry = snapshot
        .entries
        .iter_mut()
        .find(|e| e.id == id)
        .ok_or_else(|| LxAppError::ResourceNotFound(format!("bookmark not found: {id}")))?;
    entry.pinned = pinned;
    Ok(entry.clone())
}

fn rename_group_op(
    snapshot: &mut BookmarksSnapshot,
    id: &str,
    name: &str,
) -> Result<BookmarkGroup, LxAppError> {
    let name = validated_name(name, "group name")?;
    let group = snapshot
        .groups
        .iter_mut()
        .find(|g| g.id == id)
        .ok_or_else(|| LxAppError::ResourceNotFound(format!("bookmark group not found: {id}")))?;
    group.name = name;
    Ok(group.clone())
}

fn delete_group_op(snapshot: &mut BookmarksSnapshot, id: &str) -> Result<(), LxAppError> {
    let before = snapshot.groups.len();
    snapshot.groups.retain(|g| g.id != id);
    if snapshot.groups.len() == before {
        return Err(LxAppError::ResourceNotFound(format!(
            "bookmark group not found: {id}"
        )));
    }
    // Orphaned entries fall back to ungrouped rather than disappearing.
    for entry in snapshot
        .entries
        .iter_mut()
        .filter(|e| e.group_id.as_deref() == Some(id))
    {
        entry.group_id = None;
    }
    Ok(())
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

/// Absolute path to Rust's cached favicon for `url`. A missing or stale cache
/// entry is refreshed asynchronously; native chrome is notified when ready.
pub fn favicon_path(url: &str) -> Option<String> {
    let runtime = lxapp::get_platform()?;
    let notify = Arc::new(|| {
        if let Some(listener) = change_listener().get() {
            listener();
        }
    });
    lingxia_service::favicon::cached_or_request(&runtime.app_cache_dir(), url, notify)
        .map(|path| path.to_string_lossy().into_owned())
}

/// Pin `url` to the sidebar grid ("Pin to Sidebar" on a tab). Bookmarks the
/// page first when needed, keeping the pinned ⊆ bookmarked invariant without
/// forcing a two-step flow on the user.
pub fn pin_url(url: &str, title: &str) -> bool {
    pin_url_with_favicon(url, title, None)
}

/// Pin a URL and cache its current favicon outside the bookmark JSON.
pub fn pin_url_with_favicon(url: &str, title: &str, favicon_png: Option<&[u8]>) -> bool {
    let Some(runtime) = lxapp::get_platform() else {
        return false;
    };
    let dir = runtime.app_data_dir();
    let title = if title.trim().is_empty() {
        None
    } else {
        Some(title)
    };
    let result = mutate(&dir, |snapshot| {
        let (entry, _) = add_entry(snapshot, url, title, None)?;
        let id = entry.id;
        let entry = snapshot
            .entries
            .iter_mut()
            .find(|e| e.id == id)
            .expect("entry just added or found");
        entry.pinned = true;
        Ok(())
    });
    if let Err(error) = result {
        log::warn!("[BrowserBookmarks] pin failed: {error}");
        return false;
    }
    if let Some(bytes) = favicon_png
        && lingxia_service::favicon::store_for_url(&runtime.app_cache_dir(), url, bytes).is_some()
        && let Some(listener) = change_listener().get()
    {
        listener();
    }
    true
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
        Command::Rename { id, title } => rename_entry_op(snapshot, &id, &title).map(|_| ()),
        Command::Move { id, group_id } => move_entry_op(snapshot, &id, group_id).map(|_| ()),
        Command::CreateGroupAndMove { id, name } => {
            let name = validated_name(&name, "group name")?;
            let group_id = match snapshot.groups.iter().find(|g| g.name == name) {
                Some(existing) => existing.id.clone(),
                None => {
                    let group = BookmarkGroup {
                        id: next_id("g"),
                        name,
                    };
                    let group_id = group.id.clone();
                    snapshot.groups.push(group);
                    group_id
                }
            };
            move_entry_op(snapshot, &id, Some(group_id)).map(|_| ())
        }
        Command::RenameGroup { id, name } => rename_group_op(snapshot, &id, &name).map(|_| ()),
        Command::SetPinned { id, pinned } => set_pinned_op(snapshot, &id, pinned).map(|_| ()),
        Command::DeleteGroup { id } => delete_group_op(snapshot, &id),
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

/// File-picker labels follow the host-owned webui language (the source behind
/// `settings.getLanguage`); `None` = auto, which falls back to the system
/// locale exactly like the webui i18n resolution.
fn webui_locale_is_chinese(app: &LxApp) -> Result<bool, LxAppError> {
    let stored = lingxia_service::settings::webui_language(&app.app_data_dir())
        .map_err(|error| LxAppError::Runtime(error.to_string()))?;
    let language = stored.unwrap_or_else(|| app.runtime.get_system_locale().to_string());
    Ok(is_chinese_locale(&language))
}

fn is_chinese_locale(language: &str) -> bool {
    // Mirrors the webui's /^zh(?:-|$)/ test (plus "_" for POSIX locale ids).
    let bytes = language.trim().as_bytes();
    bytes.len() >= 2
        && bytes[..2].eq_ignore_ascii_case(b"zh")
        && (bytes.len() == 2 || matches!(bytes[2], b'-' | b'_'))
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
    // Compare as multisets: a set comparison lets duplicated ids slip through.
    let mut expected: Vec<&str> = expected.to_vec();
    let mut ordered: Vec<&str> = ordered.iter().map(String::as_str).collect();
    expected.sort_unstable();
    ordered.sort_unstable();
    expected == ordered
}

fn reorder_entries(
    snapshot: &mut BookmarksSnapshot,
    group_id: Option<&str>,
    ordered_ids: &[String],
) -> Result<(), LxAppError> {
    // Reorder only the entries in the given group scope; entries in other
    // scopes keep their relative positions.
    let in_scope = |e: &BookmarkEntry| e.group_id.as_deref() == group_id;
    let scope_ids: Vec<&str> = snapshot
        .entries
        .iter()
        .filter(|e| in_scope(e))
        .map(|e| e.id.as_str())
        .collect();
    if !is_id_permutation(&scope_ids, ordered_ids) {
        return Err(LxAppError::InvalidParameter(
            "orderedIds must be a permutation of the group's bookmarks".to_string(),
        ));
    }
    // Queue per id so duplicated ids in a damaged store still pop one entry each.
    let mut by_id: std::collections::HashMap<String, Vec<BookmarkEntry>> =
        std::collections::HashMap::new();
    for entry in snapshot.entries.iter().filter(|e| in_scope(e)) {
        by_id
            .entry(entry.id.clone())
            .or_default()
            .push(entry.clone());
    }
    let mut ordered = ordered_ids.iter();
    for slot in snapshot.entries.iter_mut().filter(|e| in_scope(e)) {
        // Both iterators walk the same scope, so `ordered` cannot run dry.
        let id = ordered.next().expect("scope sizes verified equal");
        *slot = by_id
            .get_mut(id)
            .and_then(Vec::pop)
            .expect("multiset equality verified");
    }
    Ok(())
}

#[lingxia::native("bookmarks.list")]
fn list_bookmarks(app: Arc<LxApp>) -> HostResult<BookmarksSnapshot> {
    crate::require_builtin_browser(&app)?;
    let _guard = store_lock().lock().unwrap_or_else(|e| e.into_inner());
    load(&app.app_data_dir())
}

#[lingxia::native("bookmarks.add")]
fn add_bookmark(app: Arc<LxApp>, input: UrlInput) -> HostResult<AddResult> {
    crate::require_builtin_browser(&app)?;
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
    crate::require_builtin_browser(&app)?;
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
    crate::require_builtin_browser(&app)?;
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

#[lingxia::native("bookmarks.getStatus")]
fn bookmark_status(app: Arc<LxApp>, input: UrlInput) -> HostResult<StatusResult> {
    crate::require_builtin_browser(&app)?;
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
    crate::require_builtin_browser(&app)?;
    mutate(&app.app_data_dir(), |snapshot| {
        rename_entry_op(snapshot, &input.id, &input.title)
    })
}

#[lingxia::native("bookmarks.move")]
fn move_bookmark(app: Arc<LxApp>, input: MoveInput) -> HostResult<BookmarkEntry> {
    crate::require_builtin_browser(&app)?;
    mutate(&app.app_data_dir(), |snapshot| {
        move_entry_op(snapshot, &input.id, input.group_id.clone())
    })
}

#[lingxia::native("bookmarks.reorder")]
fn reorder_bookmarks(app: Arc<LxApp>, input: ReorderInput) -> HostResult<()> {
    crate::require_builtin_browser(&app)?;
    mutate(&app.app_data_dir(), |snapshot| {
        reorder_entries(snapshot, input.group_id.as_deref(), &input.ordered_ids)
    })
}

#[lingxia::native("bookmarks.setPinned")]
fn set_pinned(app: Arc<LxApp>, input: SetPinnedInput) -> HostResult<BookmarkEntry> {
    crate::require_builtin_browser(&app)?;
    mutate(&app.app_data_dir(), |snapshot| {
        set_pinned_op(snapshot, &input.id, input.pinned)
    })
}

#[lingxia::native("bookmarks.createGroup")]
fn create_group(app: Arc<LxApp>, input: GroupNameInput) -> HostResult<BookmarkGroup> {
    crate::require_builtin_browser(&app)?;
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
    crate::require_builtin_browser(&app)?;
    let name = validated_name(&input.name, "group name")?;
    mutate(&app.app_data_dir(), |snapshot| {
        // Duplicate-name guard is webui-route-only; native-chrome `command_json`
        // has never enforced it.
        if snapshot
            .groups
            .iter()
            .any(|g| g.name == name && g.id != input.id)
        {
            return Err(LxAppError::InvalidParameter(format!(
                "group already exists: {name}"
            )));
        }
        rename_group_op(snapshot, &input.id, &name)
    })
}

#[lingxia::native("bookmarks.deleteGroup")]
fn delete_group(app: Arc<LxApp>, input: IdInput) -> HostResult<()> {
    crate::require_builtin_browser(&app)?;
    mutate(&app.app_data_dir(), |snapshot| {
        delete_group_op(snapshot, &input.id)
    })
}

#[lingxia::native("bookmarks.reorderGroups")]
fn reorder_groups(app: Arc<LxApp>, input: OrderedIdsInput) -> HostResult<()> {
    crate::require_builtin_browser(&app)?;
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

#[lingxia::native("bookmarks.importHtml")]
async fn import_html_bookmarks(
    app: Arc<LxApp>,
    mut cancel: HostCancel,
) -> HostResult<Option<ImportResult>> {
    crate::require_builtin_browser(&app)?;
    let app_for_picker = app.clone();
    let is_chinese = webui_locale_is_chinese(&app)?;
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
        .map_err(|error| map_platform_error("bookmarks.importHtml", error))
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
            "select a Netscape-format bookmark HTML file".to_string(),
        ));
    }
    let bookmarks = parse_netscape_html(&html);
    mutate(&app.app_data_dir(), move |snapshot| {
        Ok(merge_imported(snapshot, bookmarks))
    })
    .map(Some)
}

#[lingxia::native("bookmarks.exportHtml")]
fn export_html_bookmarks(app: Arc<LxApp>) -> HostResult<ExportResult> {
    crate::require_builtin_browser(&app)?;
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
    let (html, count) = export_netscape_html(&snapshot, now_ms());
    std::fs::write(&path, html)
        .map_err(|error| LxAppError::IoError(format!("write {}: {error}", path.display())))?;
    Ok(ExportResult {
        path: path.to_string_lossy().to_string(),
        file_name: path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("bookmarks.html")
            .to_string(),
        count,
    })
}

#[lingxia::native("bookmarks.watch", stream)]
async fn watch_bookmarks(
    app: Arc<LxApp>,
    mut stream: StreamContext<BookmarksSnapshot>,
) -> HostResult<()> {
    crate::require_builtin_browser(&app)?;
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
    lxapp::host::register_host_entry(import_html_bookmarks_host());
    lxapp::host::register_host_entry(export_html_bookmarks_host());
    lxapp::host::register_host_entry(watch_bookmarks_host());
}

#[cfg(test)]
mod tests {
    use super::*;

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
        // Duplicates on both sides fool a set comparison; multisets must not.
        assert!(!is_id_permutation(
            &["a", "a", "b"],
            &["a".to_string(), "b".to_string(), "b".to_string()]
        ));
    }

    #[test]
    fn reorder_with_duplicate_ids_errors_instead_of_panicking() {
        let entry = |id: &str, url: &str| BookmarkEntry {
            id: id.to_string(),
            url: url.to_string(),
            title: String::new(),
            group_id: None,
            pinned: false,
            created_at_ms: 0,
        };
        // Damaged store: two entries share an id.
        let mut snapshot = BookmarksSnapshot {
            entries: vec![
                entry("a", "https://a.test"),
                entry("a", "https://a2.test"),
                entry("b", "https://b.test"),
            ],
            ..Default::default()
        };
        let result = reorder_entries(
            &mut snapshot,
            None,
            &["a".to_string(), "b".to_string(), "b".to_string()],
        );
        assert!(matches!(result, Err(LxAppError::InvalidParameter(_))));
        // A matching multiset still reorders cleanly.
        reorder_entries(
            &mut snapshot,
            None,
            &["b".to_string(), "a".to_string(), "a".to_string()],
        )
        .unwrap();
        assert_eq!(snapshot.entries[0].id, "b");
    }

    #[test]
    fn chinese_locale_matches_webui_resolution() {
        for locale in ["zh", "zh-CN", "zh_CN", "ZH-Hans-CN"] {
            assert!(is_chinese_locale(locale), "expected Chinese: {locale}");
        }
        for locale in ["en-US", "zho", "ja-JP", ""] {
            assert!(!is_chinese_locale(locale), "expected non-Chinese: {locale}");
        }
    }

    #[test]
    fn corrupt_store_recovers_with_default_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let path = bookmarks_path(dir.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{ truncated").unwrap();
        let snapshot = load(dir.path()).unwrap();
        assert!(snapshot.entries.is_empty());
        assert!(!path.exists());
        assert!(path.with_extension("json.corrupt").exists());
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
