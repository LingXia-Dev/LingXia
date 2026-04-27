use std::path::Path;
use std::time::Duration;

use filetime::FileTime;

pub fn touch_access_time(path: &Path) {
    let now = FileTime::now();
    let _ = filetime::set_file_atime(path, now);
}

pub fn cleanup_cache_dir(cache_dir: &Path, max_bytes: u64, max_age: Duration) {
    lingxia_service::storage::cleanup_cache_dir(cache_dir, max_bytes, max_age);
}

pub fn cleanup_cache_dir_keep(
    cache_dir: &Path,
    max_bytes: u64,
    max_age: Duration,
    keep: Option<&Path>,
) {
    lingxia_service::storage::cleanup_cache_dir_preserving(cache_dir, max_bytes, max_age, keep);
}

pub fn cleanup_all_cache_dirs(cache_dir: &Path, max_bytes: u64, max_age: Duration) {
    lingxia_service::storage::cleanup_all_cache_dirs(cache_dir, max_bytes, max_age);
}

pub fn cleanup_all_cache_dirs_keep(
    cache_dir: &Path,
    max_bytes: u64,
    max_age: Duration,
    keep: Option<&Path>,
) {
    lingxia_service::storage::cleanup_all_cache_dirs_preserving(
        cache_dir, max_bytes, max_age, keep,
    );
}

pub fn cleanup_cache_for_storage_pressure(
    cache_dir: &Path,
    user_data_root: &Path,
    user_cache_root: &Path,
    destination: &Path,
    incoming_bytes: u64,
    max_bytes: u64,
) -> bool {
    lingxia_service::storage::cleanup_cache_for_storage_pressure(
        cache_dir,
        user_data_root,
        user_cache_root,
        destination,
        incoming_bytes,
        max_bytes,
    )
}

pub fn cleanup_cache_for_storage_pressure_keep(
    cache_dir: &Path,
    user_data_root: &Path,
    user_cache_root: &Path,
    destination: &Path,
    incoming_bytes: u64,
    max_bytes: u64,
    keep: Option<&Path>,
) -> bool {
    lingxia_service::storage::cleanup_cache_for_storage_pressure_preserving(
        cache_dir,
        user_data_root,
        user_cache_root,
        destination,
        incoming_bytes,
        max_bytes,
        keep,
    )
}
