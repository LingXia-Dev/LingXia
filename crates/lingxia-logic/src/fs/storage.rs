use crate::i18n::js_error_from_business_code_with_detail;
pub(crate) use lingxia_service::storage::{
    StorageQuotaError, ensure_app_storage_quota, ensure_app_storage_quota_preserving_many,
    ensure_temp_quota, ensure_usercache_quota, ensure_usercache_quota_preserving,
    ensure_userdata_quota, ensure_userdata_quota_with_removed, path_size,
};
use lingxia_service::storage::{cleanup_cache_to_free_bytes, is_enospc};
use rong::RongJSError;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static ATOMIC_WRITE_SEQ: AtomicU64 = AtomicU64::new(1);

pub(crate) fn quota_error_to_js(err: StorageQuotaError) -> RongJSError {
    js_error_from_business_code_with_detail(1002, err.detail())
}

/// Run a write operation; if it fails with ENOSPC, free up usercache LRU
/// across all LxApps and retry once. `preserve` lists usercache paths the
/// recovery pass must not delete: typically the source (so a read-in-flight
/// op can complete) and the destination (so the prior version of an
/// overwrite isn't wiped before the retry succeeds).
pub(crate) fn with_disk_pressure_recovery<T, F>(
    user_cache_dir: &Path,
    incoming_bytes: u64,
    preserve: &[&Path],
    mut op: F,
) -> io::Result<T>
where
    F: FnMut() -> io::Result<T>,
{
    match op() {
        Err(err) if is_enospc(&err) => {
            let cache_parent = user_cache_dir.parent().unwrap_or(user_cache_dir);
            let target = incoming_bytes
                .saturating_add(incoming_bytes / 4)
                .max(1 << 20);
            cleanup_cache_to_free_bytes(cache_parent, target, preserve);
            op()
        }
        other => other,
    }
}

fn path_exists_no_follow(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok()
}

fn is_dir_no_follow(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_dir())
        .unwrap_or(false)
}

pub(crate) fn copy_file_atomic_with_overwrite(
    source: &Path,
    destination: &Path,
    overwrite: bool,
) -> io::Result<u64> {
    if is_dir_no_follow(destination) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "destination is a directory",
        ));
    }
    if path_exists_no_follow(destination) && !overwrite {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "destination already exists",
        ));
    }
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let source_size = std::fs::symlink_metadata(source)?.len();
    let temp_path = sibling_temp_path(destination);
    let cleanup = TempCleanup::new(temp_path.clone());
    std::fs::copy(source, &temp_path)?;
    replace_with_temp(&temp_path, destination, overwrite)?;
    cleanup.disarm();
    Ok(source_size)
}

pub(crate) fn move_file_atomic(source: &Path, destination: &Path) -> io::Result<()> {
    move_file_atomic_with_overwrite(source, destination, false)
}

pub(crate) fn move_file_atomic_with_overwrite(
    source: &Path,
    destination: &Path,
    overwrite: bool,
) -> io::Result<()> {
    if source == destination {
        return Ok(());
    }
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if is_dir_no_follow(destination) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "destination is a directory",
        ));
    }
    if path_exists_no_follow(destination) && !overwrite {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "destination already exists",
        ));
    }
    if path_exists_no_follow(destination) {
        let temp_path = sibling_temp_path(destination);
        let cleanup = TempCleanup::new(temp_path.clone());
        std::fs::copy(source, &temp_path)?;
        replace_with_temp(&temp_path, destination, true)?;
        cleanup.disarm();
        let _ = std::fs::remove_file(source);
        return Ok(());
    }
    match std::fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(rename_err) => {
            let temp_path = sibling_temp_path(destination);
            let cleanup = TempCleanup::new(temp_path.clone());
            match std::fs::copy(source, &temp_path)
                .and_then(|_| replace_with_temp(&temp_path, destination, overwrite))
            {
                Ok(()) => {
                    cleanup.disarm();
                    let _ = std::fs::remove_file(source);
                    Ok(())
                }
                Err(copy_err) => {
                    if copy_err.kind() == io::ErrorKind::CrossesDevices {
                        Err(rename_err)
                    } else {
                        Err(copy_err)
                    }
                }
            }
        }
    }
}

pub(crate) fn write_file_atomic(
    data: &[u8],
    destination: &Path,
    overwrite: bool,
) -> io::Result<u64> {
    if is_dir_no_follow(destination) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "destination is a directory",
        ));
    }
    if path_exists_no_follow(destination) && !overwrite {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "destination already exists",
        ));
    }
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp_path = sibling_temp_path(destination);
    let cleanup = TempCleanup::new(temp_path.clone());
    std::fs::write(&temp_path, data)?;
    replace_with_temp(&temp_path, destination, overwrite)?;
    cleanup.disarm();
    Ok(data.len() as u64)
}

struct TempCleanup {
    path: PathBuf,
    armed: bool,
}

impl TempCleanup {
    fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }

    fn disarm(mut self) {
        self.armed = false;
    }
}

impl Drop for TempCleanup {
    fn drop(&mut self) {
        if self.armed {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

fn sibling_temp_path(destination: &Path) -> PathBuf {
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    let file_name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("file");
    let seq = ATOMIC_WRITE_SEQ.fetch_add(1, Ordering::Relaxed);
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    parent.join(format!(".{file_name}.lingxia-tmp-{nonce}-{seq}"))
}

fn replace_with_temp(temp_path: &Path, destination: &Path, overwrite: bool) -> io::Result<()> {
    if is_dir_no_follow(destination) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "destination is a directory",
        ));
    }
    if path_exists_no_follow(destination) && !overwrite {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "destination already exists",
        ));
    }
    if !path_exists_no_follow(destination) {
        return std::fs::rename(temp_path, destination);
    }

    let backup_path = sibling_temp_path(destination);
    let backup = TempCleanup::new(backup_path.clone());
    std::fs::rename(destination, &backup_path)?;
    match std::fs::rename(temp_path, destination) {
        Ok(()) => {
            let _ = std::fs::remove_file(&backup_path);
            backup.disarm();
            Ok(())
        }
        Err(err) => {
            let _ = std::fs::rename(&backup_path, destination);
            Err(err)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn test_dir(name: &str) -> PathBuf {
        let seq = ATOMIC_WRITE_SEQ.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "lingxia-storage-{name}-{}-{seq}",
            std::process::id()
        ))
    }

    #[test]
    fn move_same_path_is_noop() {
        let dir = test_dir("same-path");
        fs::create_dir_all(&dir).expect("create test dir");
        let file = dir.join("data.txt");
        fs::write(&file, b"keep").expect("write source");

        move_file_atomic_with_overwrite(&file, &file, false).expect("same path move");

        assert_eq!(fs::read(&file).expect("read source"), b"keep");
        let _ = fs::remove_dir_all(&dir);
    }
}
