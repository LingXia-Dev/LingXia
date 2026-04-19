use crate::i18n::js_error_from_business_code_with_detail;
use rong::RongJSError;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DOWNLOAD_STAGING_DIR: &str = ".download-staging";
static ATOMIC_WRITE_SEQ: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StorageQuotaError {
    Temp,
    UserData,
    AppStorage,
}

impl StorageQuotaError {
    pub(crate) fn detail(self) -> &'static str {
        match self {
            Self::Temp => "TEMP_QUOTA_EXCEEDED",
            Self::UserData => "USERDATA_QUOTA_EXCEEDED",
            Self::AppStorage => "APP_STORAGE_QUOTA_EXCEEDED",
        }
    }

    pub(crate) fn into_js_error(self) -> RongJSError {
        js_error_from_business_code_with_detail(1002, self.detail())
    }
}

struct TempFileEntry {
    path: PathBuf,
    size: u64,
    modified: SystemTime,
}

pub(crate) fn dir_size(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| {
            let path = entry.path();
            let Ok(metadata) = entry.metadata() else {
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

pub(crate) fn existing_file_size(path: &Path) -> u64 {
    std::fs::metadata(path)
        .ok()
        .filter(|metadata| metadata.is_file())
        .map(|metadata| metadata.len())
        .unwrap_or(0)
}

pub(crate) fn projected_size(current: u64, incoming: u64, replaced: u64) -> u64 {
    current.saturating_sub(replaced).saturating_add(incoming)
}

pub(crate) fn copy_file_atomic(
    source: &Path,
    destination: &Path,
    overwrite: bool,
) -> io::Result<u64> {
    if destination.exists() && !overwrite {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "destination already exists",
        ));
    }
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let source_size = std::fs::metadata(source)?.len();
    let temp_path = sibling_temp_path(destination);
    let cleanup = TempCleanup::new(temp_path.clone());
    std::fs::copy(source, &temp_path)?;
    replace_with_temp(&temp_path, destination, overwrite)?;
    cleanup.disarm();
    Ok(source_size)
}

pub(crate) fn move_file_atomic(source: &Path, destination: &Path) -> io::Result<()> {
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    match std::fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(rename_err) => {
            let temp_path = sibling_temp_path(destination);
            let cleanup = TempCleanup::new(temp_path.clone());
            match std::fs::copy(source, &temp_path)
                .and_then(|_| replace_with_temp(&temp_path, destination, true))
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

fn storage_class_root(path: &Path) -> &Path {
    path.parent().unwrap_or(path)
}

pub(crate) fn app_storage_usage_bytes(user_data_dir: &Path, user_cache_dir: &Path) -> u64 {
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

pub(crate) fn ensure_userdata_quota(
    user_data_dir: &Path,
    destination: &Path,
    incoming_bytes: u64,
) -> Result<(), StorageQuotaError> {
    let max = lingxia_app_context::data_max_size_bytes();
    if max > 0
        && projected_size(
            dir_size(user_data_dir),
            incoming_bytes,
            existing_file_size(destination),
        ) > max
    {
        return Err(StorageQuotaError::UserData);
    }
    Ok(())
}

pub(crate) fn ensure_app_storage_quota(
    user_data_dir: &Path,
    user_cache_dir: &Path,
    destination: &Path,
    incoming_bytes: u64,
    incoming_already_in_app_storage: bool,
) -> Result<(), StorageQuotaError> {
    let max = lingxia_app_context::app_storage_max_size_bytes();
    if max == 0 {
        return Ok(());
    }

    let incoming_for_app = if incoming_already_in_app_storage {
        0
    } else {
        incoming_bytes
    };
    if app_storage_projected_size(user_data_dir, user_cache_dir, destination, incoming_for_app)
        <= max
    {
        return Ok(());
    }

    lxapp::cleanup_all_cache_dirs(
        user_cache_dir,
        lingxia_app_context::cache_max_size_bytes(),
        cache_max_age_duration(),
    );
    if app_storage_projected_size(user_data_dir, user_cache_dir, destination, incoming_for_app)
        <= max
    {
        return Ok(());
    }

    if lxapp::cleanup_cache_for_storage_pressure(
        user_cache_dir,
        storage_class_root(user_data_dir),
        storage_class_root(user_cache_dir),
        destination,
        incoming_for_app,
        max,
    ) {
        Ok(())
    } else {
        Err(StorageQuotaError::AppStorage)
    }
}

pub(crate) fn ensure_temp_quota(temp_root: &Path, keep: &Path) -> Result<(), StorageQuotaError> {
    let max = lingxia_app_context::temp_max_size_bytes();
    if max == 0 {
        return Ok(());
    }
    let mut files = Vec::new();
    collect_temp_files(temp_root, &mut files);
    let mut total = files.iter().map(|entry| entry.size).sum::<u64>();
    if total <= max {
        return Ok(());
    }

    files.sort_by_key(|entry| entry.modified);
    let low_water = max.saturating_mul(8) / 10;
    for entry in files {
        if total <= low_water {
            break;
        }
        if entry.path == keep {
            continue;
        }
        if std::fs::remove_file(&entry.path).is_ok() {
            total = total.saturating_sub(entry.size);
        }
    }

    if total > max {
        let _ = std::fs::remove_file(keep);
        Err(StorageQuotaError::Temp)
    } else {
        Ok(())
    }
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
    if !overwrite && destination.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "destination already exists",
        ));
    }
    match std::fs::rename(temp_path, destination) {
        Ok(()) => Ok(()),
        Err(err) if overwrite && destination.exists() => {
            std::fs::remove_file(destination)?;
            std::fs::rename(temp_path, destination).map_err(|_| err)
        }
        Err(err) => Err(err),
    }
}

fn collect_temp_files(root: &Path, out: &mut Vec<TempFileEntry>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(metadata) = entry.metadata() else {
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

fn cache_max_age_duration() -> Duration {
    Duration::from_secs(lingxia_app_context::cache_max_age_days().saturating_mul(86400))
}
