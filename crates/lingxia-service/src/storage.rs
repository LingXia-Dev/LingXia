use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const DOWNLOAD_STAGING_DIR: &str = ".download-staging";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageQuotaError {
    Temp,
    UserCache,
    UserData,
    AppStorage,
    DestinationExists,
}

impl StorageQuotaError {
    pub fn detail(self) -> &'static str {
        match self {
            Self::Temp => "TEMP_QUOTA_EXCEEDED",
            Self::UserCache => "USERCACHE_QUOTA_EXCEEDED",
            Self::UserData => "USERDATA_QUOTA_EXCEEDED",
            Self::AppStorage => "APP_STORAGE_QUOTA_EXCEEDED",
            Self::DestinationExists => "DESTINATION_ALREADY_EXISTS",
        }
    }
}

struct TempFileEntry {
    path: PathBuf,
    size: u64,
    modified: SystemTime,
}

struct CacheEntry {
    path: PathBuf,
    size: u64,
    last_access: SystemTime,
}

struct PressureCacheEntry {
    cache_root: PathBuf,
    path: PathBuf,
    size: u64,
    last_access: SystemTime,
}

pub fn dir_size(path: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| {
            let path = entry.path();
            let Ok(metadata) = entry.path().symlink_metadata() else {
                return 0;
            };
            if metadata.is_dir() {
                dir_size(&path)
            } else if metadata.is_file() {
                metadata.len()
            } else {
                0
            }
        })
        .sum()
}

pub fn existing_file_size(path: &Path) -> u64 {
    fs::symlink_metadata(path)
        .ok()
        .filter(|metadata| metadata.is_file())
        .map(|metadata| metadata.len())
        .unwrap_or(0)
}

pub fn path_size(path: &Path) -> u64 {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return 0;
    };
    if metadata.is_dir() {
        dir_size(path)
    } else if metadata.is_file() {
        metadata.len()
    } else {
        0
    }
}

fn projected_size(current: u64, incoming: u64, replaced: u64) -> u64 {
    current.saturating_sub(replaced).saturating_add(incoming)
}

fn projected_size_with_removed(current: u64, incoming: u64, replaced: u64, removed: u64) -> u64 {
    current
        .saturating_sub(replaced)
        .saturating_sub(removed)
        .saturating_add(incoming)
}

fn storage_class_root(path: &Path) -> &Path {
    path.parent().unwrap_or(path)
}

pub fn app_storage_usage_bytes(user_data_dir: &Path, user_cache_dir: &Path) -> u64 {
    dir_size(storage_class_root(user_data_dir))
        .saturating_add(dir_size(storage_class_root(user_cache_dir)))
}

fn app_storage_projected_size(
    user_data_dir: &Path,
    user_cache_dir: &Path,
    destination: &Path,
    incoming_bytes: u64,
) -> u64 {
    projected_size(
        app_storage_usage_bytes(user_data_dir, user_cache_dir),
        incoming_bytes,
        existing_file_size(destination),
    )
}

pub fn ensure_userdata_quota(
    user_data_dir: &Path,
    destination: &Path,
    incoming_bytes: u64,
) -> Result<(), StorageQuotaError> {
    ensure_userdata_quota_with_removed(user_data_dir, destination, incoming_bytes, None)
}

pub fn ensure_userdata_quota_with_removed(
    user_data_dir: &Path,
    destination: &Path,
    incoming_bytes: u64,
    removed_source: Option<&Path>,
) -> Result<(), StorageQuotaError> {
    let max = lingxia_app_context::data_max_size_bytes();
    let removed = removed_source
        .filter(|source| source.starts_with(user_data_dir) && *source != destination)
        .map(path_size)
        .unwrap_or(0);
    if max > 0
        && projected_size_with_removed(
            dir_size(user_data_dir),
            incoming_bytes,
            existing_file_size(destination),
            removed,
        ) > max
    {
        return Err(StorageQuotaError::UserData);
    }
    Ok(())
}

pub fn ensure_usercache_quota(
    user_cache_dir: &Path,
    destination: &Path,
    incoming_bytes: u64,
    removed_source: Option<&Path>,
) -> Result<(), StorageQuotaError> {
    ensure_usercache_quota_preserving(
        user_cache_dir,
        destination,
        incoming_bytes,
        removed_source,
        &[],
    )
}

pub fn ensure_usercache_quota_preserving(
    user_cache_dir: &Path,
    destination: &Path,
    incoming_bytes: u64,
    removed_source: Option<&Path>,
    preserve: &[&Path],
) -> Result<(), StorageQuotaError> {
    let max = lingxia_app_context::cache_max_size_bytes();
    if max == 0 {
        return Ok(());
    }

    if incoming_bytes > max {
        return Err(StorageQuotaError::UserCache);
    }

    let replaced = existing_file_size(destination);
    let removed = removed_source
        .filter(|source| source.starts_with(user_cache_dir) && *source != destination)
        .map(path_size)
        .unwrap_or(0);
    let net_incoming = incoming_bytes.saturating_sub(replaced.saturating_add(removed));
    let mut preserve_for_cleanup = Vec::with_capacity(preserve.len() + 2);
    preserve_for_cleanup.push(destination);
    preserve_for_cleanup.extend_from_slice(preserve);
    if let Some(source) = removed_source {
        preserve_for_cleanup.push(source);
    }
    cleanup_cache_dir_for_write(user_cache_dir, max, net_incoming, &preserve_for_cleanup);

    if projected_size_with_removed(dir_size(user_cache_dir), incoming_bytes, replaced, removed)
        > max
    {
        return Err(StorageQuotaError::UserCache);
    }
    Ok(())
}

pub fn ensure_app_storage_quota(
    user_data_dir: &Path,
    user_cache_dir: &Path,
    destination: &Path,
    incoming_bytes: u64,
) -> Result<(), StorageQuotaError> {
    ensure_app_storage_quota_preserving(
        user_data_dir,
        user_cache_dir,
        destination,
        incoming_bytes,
        None,
    )
}

pub fn ensure_app_storage_quota_preserving(
    user_data_dir: &Path,
    user_cache_dir: &Path,
    destination: &Path,
    incoming_bytes: u64,
    keep_cache_path: Option<&Path>,
) -> Result<(), StorageQuotaError> {
    match keep_cache_path {
        Some(path) => ensure_app_storage_quota_preserving_many(
            user_data_dir,
            user_cache_dir,
            destination,
            incoming_bytes,
            &[path],
        ),
        None => ensure_app_storage_quota_preserving_many(
            user_data_dir,
            user_cache_dir,
            destination,
            incoming_bytes,
            &[],
        ),
    }
}

pub fn ensure_app_storage_quota_preserving_many(
    user_data_dir: &Path,
    user_cache_dir: &Path,
    destination: &Path,
    incoming_bytes: u64,
    preserve_cache_paths: &[&Path],
) -> Result<(), StorageQuotaError> {
    let max = lingxia_app_context::app_storage_max_size_bytes();
    if max == 0 {
        return Ok(());
    }

    if app_storage_projected_size(user_data_dir, user_cache_dir, destination, incoming_bytes) <= max
    {
        return Ok(());
    }

    cleanup_all_cache_dirs_preserving_many(
        user_cache_dir,
        lingxia_app_context::cache_max_size_bytes(),
        preserve_cache_paths,
    );
    if app_storage_projected_size(user_data_dir, user_cache_dir, destination, incoming_bytes) <= max
    {
        return Ok(());
    }

    if cleanup_cache_for_storage_pressure_preserving_many(
        user_cache_dir,
        storage_class_root(user_data_dir),
        storage_class_root(user_cache_dir),
        destination,
        incoming_bytes,
        max,
        preserve_cache_paths,
    ) {
        Ok(())
    } else {
        Err(StorageQuotaError::AppStorage)
    }
}

pub fn ensure_temp_quota(
    temp_root: &Path,
    keep: &Path,
    incoming_bytes: u64,
) -> Result<(), StorageQuotaError> {
    let max_bytes = lingxia_app_context::temp_max_size_bytes();
    if max_bytes == 0 {
        return Ok(());
    }
    let mut files = Vec::new();
    collect_temp_files(temp_root, &mut files);
    let mut total = files.iter().map(|entry| entry.size).sum::<u64>();
    let replaced = existing_file_size(keep);
    let mut projected = projected_size(total, incoming_bytes, replaced);
    if projected <= max_bytes {
        return Ok(());
    }

    files.sort_by_key(|entry| entry.modified);
    let low_water = max_bytes.saturating_mul(8) / 10;
    let desired_projected =
        if incoming_bytes.saturating_sub(replaced) > max_bytes.saturating_sub(low_water) {
            max_bytes
        } else {
            low_water
        };
    for entry in files {
        if projected <= desired_projected {
            break;
        }
        if entry.path == keep {
            continue;
        }
        if fs::remove_file(&entry.path).is_ok() {
            total = total.saturating_sub(entry.size);
            projected = projected_size(total, incoming_bytes, replaced);
        }
    }

    if projected > max_bytes {
        let _ = fs::remove_file(keep);
        Err(StorageQuotaError::Temp)
    } else {
        Ok(())
    }
}

pub fn cleanup_cache_dir(cache_dir: &Path, max_bytes: u64) {
    cleanup_cache_dir_preserving(cache_dir, max_bytes, None)
}

pub fn cleanup_usercache_preserving(user_cache_dir: &Path, preserve: Option<&Path>) {
    cleanup_cache_dir_preserving(
        user_cache_dir,
        lingxia_app_context::cache_max_size_bytes(),
        preserve,
    )
}

pub fn cleanup_cache_dir_preserving(cache_dir: &Path, max_bytes: u64, preserve: Option<&Path>) {
    if max_bytes == 0 {
        return;
    }
    match preserve {
        Some(preserve) => {
            let _ = enforce_cache_limits_preserving(cache_dir, max_bytes, 0, &[preserve]);
        }
        None => {
            let _ = enforce_cache_limits_preserving(cache_dir, max_bytes, 0, &[]);
        }
    }
}

/// Run cleanup before a write. `incoming_bytes` is the operation's net cache
/// growth after accounting for overwritten destination or removed source bytes.
/// Trigger and target use `current + incoming` so a write that would push usage
/// over the high water mark fires cleanup even if current usage alone is still
/// below it.
pub fn cleanup_cache_dir_for_write(
    cache_dir: &Path,
    max_bytes: u64,
    incoming_bytes: u64,
    preserve: &[&Path],
) {
    if max_bytes == 0 {
        return;
    }
    let _ = enforce_cache_limits_preserving(cache_dir, max_bytes, incoming_bytes, preserve);
}

pub fn cleanup_all_cache_dirs(cache_dir: &Path, max_bytes: u64) {
    cleanup_all_cache_dirs_preserving(cache_dir, max_bytes, None)
}

pub fn cleanup_all_cache_dirs_preserving(
    cache_dir: &Path,
    max_bytes: u64,
    preserve: Option<&Path>,
) {
    match preserve {
        Some(preserve) => cleanup_all_cache_dirs_preserving_many(cache_dir, max_bytes, &[preserve]),
        None => cleanup_all_cache_dirs_preserving_many(cache_dir, max_bytes, &[]),
    }
}

fn cleanup_all_cache_dirs_preserving_many(cache_dir: &Path, max_bytes: u64, preserve: &[&Path]) {
    let Some(cache_parent) = cache_dir.parent() else {
        cleanup_cache_dir_for_write(cache_dir, max_bytes, 0, preserve);
        return;
    };
    let Ok(entries) = fs::read_dir(cache_parent) else {
        cleanup_cache_dir_for_write(cache_dir, max_bytes, 0, preserve);
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
            let preserve_for_dir: Vec<&Path> = preserve
                .iter()
                .copied()
                .filter(|path_to_keep| path_to_keep.starts_with(&path))
                .collect();
            cleanup_cache_dir_for_write(&path, max_bytes, 0, &preserve_for_dir);
        }
    }
}

pub fn is_enospc(err: &std::io::Error) -> bool {
    if matches!(err.kind(), std::io::ErrorKind::StorageFull) {
        return true;
    }
    err.raw_os_error() == Some(28)
}

pub fn cleanup_cache_to_free_bytes(
    cache_parent: &Path,
    target_bytes: u64,
    preserve: &[&Path],
) -> u64 {
    if target_bytes == 0 {
        return 0;
    }
    let mut entries = Vec::new();
    collect_all_cache_entries(cache_parent, &mut entries);
    entries.sort_by_key(|entry| entry.last_access);
    let preserve = canonicalize_preserve_paths(preserve);
    let mut freed = 0u64;
    for entry in entries {
        if freed >= target_bytes {
            break;
        }
        if matches_preserve(&entry.path, &preserve) {
            continue;
        }
        let cache_root = entry
            .cache_root
            .canonicalize()
            .unwrap_or_else(|_| entry.cache_root.clone());
        if try_remove_cache_entry(&entry.cache_root, &cache_root, &entry.path) {
            freed = freed.saturating_add(entry.size);
        }
    }
    freed
}

fn canonicalize_preserve_paths(paths: &[&Path]) -> Vec<PathBuf> {
    paths
        .iter()
        .filter_map(|path| path.canonicalize().ok())
        .collect()
}

fn matches_preserve(path: &Path, preserve: &[PathBuf]) -> bool {
    if preserve.is_empty() {
        return false;
    }
    let Ok(canon) = path.canonicalize() else {
        return false;
    };
    preserve.iter().any(|p| p == &canon)
}

pub fn cleanup_cache_for_storage_pressure(
    cache_dir: &Path,
    user_data_root: &Path,
    user_cache_root: &Path,
    destination: &Path,
    incoming_bytes: u64,
    max_bytes: u64,
) -> bool {
    cleanup_cache_for_storage_pressure_preserving(
        cache_dir,
        user_data_root,
        user_cache_root,
        destination,
        incoming_bytes,
        max_bytes,
        None,
    )
}

pub fn cleanup_cache_for_storage_pressure_preserving(
    cache_dir: &Path,
    user_data_root: &Path,
    user_cache_root: &Path,
    destination: &Path,
    incoming_bytes: u64,
    max_bytes: u64,
    preserve: Option<&Path>,
) -> bool {
    match preserve {
        Some(preserve) => cleanup_cache_for_storage_pressure_preserving_many(
            cache_dir,
            user_data_root,
            user_cache_root,
            destination,
            incoming_bytes,
            max_bytes,
            &[preserve],
        ),
        None => cleanup_cache_for_storage_pressure_preserving_many(
            cache_dir,
            user_data_root,
            user_cache_root,
            destination,
            incoming_bytes,
            max_bytes,
            &[],
        ),
    }
}

fn cleanup_cache_for_storage_pressure_preserving_many(
    cache_dir: &Path,
    user_data_root: &Path,
    user_cache_root: &Path,
    destination: &Path,
    incoming_bytes: u64,
    max_bytes: u64,
    preserve: &[&Path],
) -> bool {
    let Some(cache_parent) = cache_dir.parent() else {
        return app_storage_fits(
            user_data_root,
            user_cache_root,
            destination,
            incoming_bytes,
            max_bytes,
        );
    };
    let mut files = Vec::new();
    collect_all_cache_entries(cache_parent, &mut files);
    files.sort_by_key(|entry| entry.last_access);
    let preserve = canonicalize_preserve_paths(preserve);

    for entry in files {
        if app_storage_fits(
            user_data_root,
            user_cache_root,
            destination,
            incoming_bytes,
            max_bytes,
        ) {
            return true;
        }
        if matches_preserve(&entry.path, &preserve) {
            continue;
        }
        let cache_root = entry
            .cache_root
            .canonicalize()
            .unwrap_or_else(|_| entry.cache_root.clone());
        let _ = try_remove_cache_entry(&entry.cache_root, &cache_root, &entry.path);
    }

    app_storage_fits(
        user_data_root,
        user_cache_root,
        destination,
        incoming_bytes,
        max_bytes,
    )
}

fn enforce_cache_limits_preserving(
    cache_dir: &Path,
    max_bytes: u64,
    incoming_bytes: u64,
    preserve: &[&Path],
) -> (u32, u64) {
    if max_bytes == 0 {
        return (0, 0);
    }
    let cache_root = cache_dir
        .canonicalize()
        .unwrap_or_else(|_| cache_dir.to_path_buf());
    let mut total_bytes = 0u64;
    let mut entries = collect_cache_entries(cache_dir, &mut total_bytes);

    // High-water trigger / low-water target — both computed against projected
    // size (current + net incoming), not just current. This way a write that
    // would push usage over the high water mark fires cleanup even if current
    // usage alone is still below it.
    //
    // Trigger: projected (current + net incoming) >= 80% of `max_bytes`.
    // Below that, do nothing — keep assets intact even if their atime has
    // gone stale (e.g. a WebView served the file from its own resource cache
    // for days without re-hitting the scheme handler).
    //
    // Target: evict LRU until current <= (50% of `max_bytes` - net incoming)
    // so that after the pending write the cache settles around 50%. Going
    // deeper than just-under-cap prevents thrash: the next round of writes
    // can fill back to 80% before another cleanup fires.
    let high_water = max_bytes.saturating_mul(8) / 10;
    if total_bytes.saturating_add(incoming_bytes) < high_water {
        return (0, 0);
    }
    let low_water = max_bytes / 2;
    let target_total = low_water.saturating_sub(incoming_bytes);

    let preserve = canonicalize_preserve_paths(preserve);
    let mut files_removed = 0u32;
    let mut bytes_freed = 0u64;

    {
        entries.sort_by_key(|entry| entry.last_access);
        for entry in entries {
            if total_bytes <= target_total {
                break;
            }
            if matches_preserve(&entry.path, &preserve) {
                continue;
            }
            if try_remove_cache_entry(cache_dir, &cache_root, &entry.path) {
                total_bytes = total_bytes.saturating_sub(entry.size);
                files_removed += 1;
                bytes_freed = bytes_freed.saturating_add(entry.size);
            }
        }
    }

    (files_removed, bytes_freed)
}

fn collect_temp_files(root: &Path, out: &mut Vec<TempFileEntry>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(metadata) = entry.path().symlink_metadata() else {
            continue;
        };
        if metadata.is_dir() {
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == DOWNLOAD_STAGING_DIR)
            {
                continue;
            }
            collect_temp_files(&path, out);
        } else if metadata.is_file() {
            out.push(TempFileEntry {
                path,
                size: metadata.len(),
                modified: metadata.modified().unwrap_or(UNIX_EPOCH),
            });
        }
    }
}

fn collect_all_cache_entries(cache_parent: &Path, out: &mut Vec<PressureCacheEntry>) {
    let Ok(entries) = fs::read_dir(cache_parent) else {
        return;
    };
    for entry in entries.flatten() {
        let cache_dir = entry.path();
        if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
            continue;
        }
        let mut total_bytes = 0;
        for entry in collect_cache_entries(&cache_dir, &mut total_bytes) {
            out.push(PressureCacheEntry {
                cache_root: cache_dir.clone(),
                path: entry.path,
                size: entry.size,
                last_access: entry.last_access,
            });
        }
    }
}

fn collect_cache_entries(cache_dir: &Path, total_bytes: &mut u64) -> Vec<CacheEntry> {
    let mut out = Vec::new();
    let mut pending_dirs = vec![cache_dir.to_path_buf()];

    while let Some(dir) = pending_dirs.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                pending_dirs.push(path);
                continue;
            }
            if !file_type.is_file() {
                continue;
            }

            let Some(filename) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let protected_name = should_skip_cleanup(filename);
            let Ok(metadata) = path.metadata() else {
                continue;
            };
            let size = metadata.len();
            *total_bytes = total_bytes.saturating_add(size);

            if protected_name || filename.ends_with(".ok") || has_active_lock_for(&path) {
                continue;
            }

            let last_access = metadata
                .accessed()
                .or_else(|_| metadata.modified())
                .unwrap_or(UNIX_EPOCH);
            out.push(CacheEntry {
                path,
                size,
                last_access,
            });
        }
    }

    out
}

fn app_storage_fits(
    user_data_root: &Path,
    user_cache_root: &Path,
    destination: &Path,
    incoming_bytes: u64,
    max_bytes: u64,
) -> bool {
    projected_size(
        dir_size(user_data_root).saturating_add(dir_size(user_cache_root)),
        incoming_bytes,
        existing_file_size(destination),
    ) <= max_bytes
}

fn try_remove_cache_entry(cache_dir: &Path, cache_root: &Path, data_path: &Path) -> bool {
    if !is_path_within_root(cache_root, data_path) {
        return false;
    }
    if fs::remove_file(data_path).is_err() {
        return false;
    }
    remove_ok_marker_for(data_path);
    remove_empty_parent_dirs(cache_dir, data_path);
    true
}

fn remove_ok_marker_for(data_path: &Path) {
    let Some(parent) = data_path.parent() else {
        return;
    };
    if let Some(stem) = data_path.file_stem().and_then(|s| s.to_str()) {
        let _ = fs::remove_file(parent.join(format!("{}.ok", stem)));
    }
}

fn is_path_within_root(cache_root: &Path, data_path: &Path) -> bool {
    data_path
        .canonicalize()
        .map(|p| p.starts_with(cache_root))
        .unwrap_or(false)
}

fn has_active_lock_for(data_path: &Path) -> bool {
    let Some(stem) = data_path.file_stem().and_then(|s| s.to_str()) else {
        return false;
    };
    let dir = data_path.parent().unwrap_or_else(|| Path::new("."));
    dir.join(format!("{}.lock", stem)).exists()
}

fn remove_empty_parent_dirs(cache_root: &Path, data_path: &Path) {
    let mut current = data_path.parent();
    while let Some(dir) = current {
        if dir == cache_root {
            break;
        }
        if !dir.starts_with(cache_root) {
            break;
        }
        if fs::remove_dir(dir).is_ok() {
            current = dir.parent();
        } else {
            break;
        }
    }
}

fn should_skip_cleanup(filename: &str) -> bool {
    filename.ends_with(".lock") || filename.ends_with(".part")
}
