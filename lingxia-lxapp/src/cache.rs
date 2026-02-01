use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use filetime::FileTime;
use rong_http::{self as net, BodySink};
use thiserror::Error;

type HashId = String;

struct CacheDownloadSink {
    final_path: PathBuf,
    key_id: String,
    lock_path: PathBuf,
}

impl CacheDownloadSink {
    fn new(final_path: PathBuf, key_id: String, lock_path: PathBuf) -> Self {
        Self {
            final_path,
            key_id,
            lock_path,
        }
    }
}

impl BodySink for CacheDownloadSink {
    fn write(&mut self, _chunk: &[u8]) -> Result<(), String> {
        Ok(())
    }

    fn close(&mut self, result: &Result<(), String>) {
        match result {
            Ok(()) => {
                let _ = fs::write(ok_marker_path(&self.final_path, &self.key_id), b"ok");
            }
            Err(_) => {
                let _ = fs::remove_file(ok_marker_path(&self.final_path, &self.key_id));
            }
        }
        let _ = fs::remove_file(&self.lock_path);
    }
}

struct CacheDownloadSinkWithCallback<F: FnOnce(CacheResult) + Send> {
    final_path: PathBuf,
    key_id: String,
    lock_path: PathBuf,
    callback: Arc<Mutex<Option<F>>>,
}

impl<F: FnOnce(CacheResult) + Send> CacheDownloadSinkWithCallback<F> {
    fn new(
        final_path: PathBuf,
        key_id: String,
        lock_path: PathBuf,
        callback: Arc<Mutex<Option<F>>>,
    ) -> Self {
        Self {
            final_path,
            key_id,
            lock_path,
            callback,
        }
    }
}

impl<F: FnOnce(CacheResult) + Send> BodySink for CacheDownloadSinkWithCallback<F> {
    fn write(&mut self, _chunk: &[u8]) -> Result<(), String> {
        Ok(())
    }

    fn close(&mut self, result: &Result<(), String>) {
        match result {
            Ok(()) => {
                let _ = fs::write(ok_marker_path(&self.final_path, &self.key_id), b"ok");
                if let Some(cb) = self.callback.lock().unwrap().take() {
                    cb(CacheResult::Ready);
                }
            }
            Err(err) => {
                let _ = fs::remove_file(ok_marker_path(&self.final_path, &self.key_id));
                if let Some(cb) = self.callback.lock().unwrap().take() {
                    cb(CacheResult::Failed(err.clone()));
                }
            }
        }
        let _ = fs::remove_file(&self.lock_path);
    }
}

/// Lightweight cache for LxApp resources.
///
/// Only tracks in-flight operations; completed files are discovered via the filesystem.
pub struct LxAppCache {
    cache_dir: PathBuf,
}

impl LxAppCache {
    /// Create a cache rooted at `cache_dir`.
    pub(crate) fn new(cache_dir: PathBuf) -> Result<Self, CacheError> {
        // Assume LxApp already prepared cache_dir; do not create extra directories here.
        Ok(Self { cache_dir })
    }

    /// Request a cached file; start download in background if missing.
    ///
    /// Returns the intended final file path immediately (with an extension derived
    /// from the URL). If the file isn't available yet, a background task begins
    /// downloading it via the existing async runtime. A sidecar marker `<hash>.ok`
    /// is written on success to distinguish complete files from partial ones.
    pub fn get_or_download<K: Hash + ?Sized>(&self, key: &K, url: &str) -> PathBuf {
        let hash_id = hash_key(key);
        let ext = url_path_ext(url).unwrap_or("bin");
        let target_path = self.cache_dir.join(format!("{hash_id}.{}", ext));

        // If already completed and present, return immediately.
        if self.ok_marker_exists(&hash_id) && target_path.exists() {
            return target_path;
        }

        // file-based lock: <hash>.lock indicates a download in progress
        let lock_path = self.cache_dir.join(format!("{}.lock", hash_id));
        let should_spawn = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
            .is_ok();

        if should_spawn {
            let final_path = target_path.clone();
            let key_id = hash_id.clone();
            let lock_path_cloned = lock_path.clone();
            let url_owned = url.to_string();

            let sink = CacheDownloadSink::new(
                final_path.clone(),
                key_id.clone(),
                lock_path_cloned.clone(),
            );
            match net::request_download(url_owned, final_path.clone(), None, Some(Box::new(sink))) {
                Ok(_rx) => {}
                Err(_) => {
                    let _ = fs::remove_file(lock_path);
                }
            }
        }

        target_path
    }

    /// Try to resolve an existing path for `key`; returns None if not present.
    fn try_resolve<K: Hash + ?Sized>(&self, key: &K) -> Option<PathBuf> {
        let hash_id = hash_key(key);
        // If we have a complete marker, try to locate a file with any extension
        if self.ok_marker_exists(&hash_id)
            && let Ok(entries) = fs::read_dir(&self.cache_dir)
        {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    // Ignore cache bookkeeping files
                    if !name.starts_with(&hash_id) {
                        continue;
                    }
                    if name.ends_with(".ok") || name.ends_with(".lock") || name.ends_with(".part") {
                        continue;
                    }
                    // Only return regular files
                    if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                        return Some(entry.path());
                    }
                }
            }
        }
        // Fall back to first match without requiring OK marker (e.g., media copies)
        let base = self.cache_dir.join(&hash_id);
        if base.exists() {
            return Some(base);
        }
        if let Ok(entries) = fs::read_dir(&self.cache_dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if !name.starts_with(&hash_id) {
                        continue;
                    }
                    if name.ends_with(".ok") || name.ends_with(".lock") || name.ends_with(".part") {
                        continue;
                    }
                    if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                        return Some(entry.path());
                    }
                }
            }
        }
        None
    }

    /// Compute the canonical target path for a given key and extension.
    fn target_path_for_ext<K: Hash + ?Sized>(&self, key: &K, ext: &str) -> PathBuf {
        let hash_id = hash_key(key);
        if ext.is_empty() {
            self.cache_dir.join(&hash_id)
        } else {
            self.cache_dir.join(format!("{hash_id}.{}", ext))
        }
    }

    /// Resolve a cache path for `key` with desired extension.
    /// - Exists(path): a cached file already exists.
    /// - NonExists(path): caller may write the file to this path.
    pub fn resolve_path_with_ext<K: Hash + ?Sized>(&self, key: &K, ext: &str) -> ResolveResult {
        if let Some(p) = self.try_resolve(key) {
            ResolveResult::Exists(p)
        } else {
            ResolveResult::NonExists(self.target_path_for_ext(key, ext))
        }
    }

    // media: use resolve_path_with_ext and copy when NonExists
    // cleanup removed; add if needed later
}

impl LxAppCache {
    /// Request download with a completion callback.
    /// The callback fires once when the download succeeds or fails.
    pub fn get_or_download_with_callback<K, F>(&self, key: &K, url: &str, on_complete: F) -> PathBuf
    where
        K: Hash + ?Sized,
        F: FnOnce(CacheResult) + Send + 'static,
    {
        use std::time::Duration;

        let hash_id = hash_key(key);
        let ext = url_path_ext(url).unwrap_or("bin");
        let target_path = self.cache_dir.join(format!("{hash_id}.{}", ext));

        // Already complete? Call callback immediately.
        if self.ok_marker_exists(&hash_id) && target_path.exists() {
            on_complete(CacheResult::Ready);
            return target_path;
        }

        let lock_path = self.cache_dir.join(format!("{}.lock", hash_id));
        let acquired = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
            .is_ok();

        if acquired {
            let final_path = target_path.clone();
            let key_id = hash_id.clone();
            let lock_path_cloned = lock_path.clone();
            let url_owned = url.to_string();

            // Share callback so we can invoke it either from the net runtime or on immediate error
            let cb_shared: Arc<Mutex<Option<F>>> = Arc::new(Mutex::new(Some(on_complete)));

            let sink = CacheDownloadSinkWithCallback::new(
                final_path.clone(),
                key_id.clone(),
                lock_path_cloned.clone(),
                cb_shared.clone(),
            );
            match net::request_download(url_owned, final_path.clone(), None, Some(Box::new(sink))) {
                Ok(_rx) => {}
                Err(e) => {
                    let _ = fs::remove_file(lock_path);
                    if let Some(cb) = cb_shared.lock().unwrap().take() {
                        cb(CacheResult::Failed(e));
                    }
                }
            }
        } else {
            // Someone else is downloading; watch for completion and then fire callback
            let final_path = target_path.clone();
            let key_id = hash_id.clone();
            let lock_path_cloned = lock_path.clone();

            let task = async move {
                use tokio::time::sleep;

                loop {
                    let success = ok_marker_path(&final_path, &key_id).exists();
                    let in_progress = lock_path_cloned.exists();
                    if success {
                        on_complete(CacheResult::Ready);
                        break;
                    }
                    if !in_progress {
                        // Not in progress and no success marker -> treat as failure
                        on_complete(CacheResult::Failed("download not completed".to_string()));
                        break;
                    }
                    sleep(Duration::from_millis(200)).await;
                }
            };
            let _ = rong::bg::spawn(task);
        }

        target_path
    }
}

fn url_path_ext(url: &str) -> Option<&str> {
    // crude parse: strip query/fragment, take suffix after last '.' if short and sane
    let path = url.split(&['?', '#'][..]).next().unwrap_or(url);
    let seg = path.rsplit('/').next().unwrap_or(path);
    let dot = seg.rfind('.')?;
    let ext = &seg[dot + 1..];
    if !ext.is_empty() && ext.len() <= 8 {
        Some(ext)
    } else {
        None
    }
}

fn ok_marker_path(final_path: &Path, hash_id: &str) -> PathBuf {
    // place marker alongside cache root: <cache_dir>/<hash>.ok
    let dir = final_path.parent().unwrap_or_else(|| Path::new("."));
    dir.join(format!("{}.ok", hash_id))
}

impl LxAppCache {
    fn ok_marker_exists(&self, hash_id: &str) -> bool {
        self.cache_dir.join(format!("{}.ok", hash_id)).exists()
    }
}

#[derive(Debug, Clone)]
pub enum CacheResult {
    Ready,
    Failed(String),
}

#[derive(Debug, Clone)]
pub enum ResolveResult {
    Exists(PathBuf),
    NonExists(PathBuf),
}

#[derive(Debug, Error)]
pub enum CacheError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

fn hash_key<K: Hash + ?Sized>(key: &K) -> HashId {
    // Stable 64-bit FNV-1a hasher for deterministic IDs across runs.
    let mut hasher = Fnv64Hasher::new();
    key.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[derive(Default)]
struct Fnv64Hasher(u64);

impl Fnv64Hasher {
    fn new() -> Self {
        // 64-bit FNV-1a offset basis
        Self(0xcbf29ce484222325)
    }
}

impl Hasher for Fnv64Hasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        // 64-bit FNV-1a prime
        const FNV_PRIME: u64 = 0x00000100000001B3;
        let mut hash = self.0;
        for &b in bytes {
            hash ^= u64::from(b);
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        self.0 = hash;
    }
}

pub fn touch_access_time(path: &Path) {
    let now = FileTime::now();
    let _ = filetime::set_file_atime(path, now);
}

// Falls back to mtime if atime unavailable
fn get_file_age_and_size(path: &Path) -> Option<(Duration, u64)> {
    let metadata = path.metadata().ok()?;
    let now = SystemTime::now();
    let last_access = metadata.accessed().or_else(|_| metadata.modified()).ok()?;

    let age = now.duration_since(last_access).ok()?;
    Some((age, metadata.len()))
}

fn should_skip_cleanup(filename: &str) -> bool {
    filename.ends_with(".lock") || filename.ends_with(".part")
}

/// Clean up stale files in a cache directory.
/// Removes files that haven't been accessed for longer than `max_age_days`.
///
/// This function:
/// - Skips `.lock` and `.part` files (in-progress downloads)
/// - Removes both data files and their corresponding `.ok` marker files
/// - Silently ignores errors for individual files
pub fn cleanup_stale_files(cache_dir: &Path, max_age_days: u64) {
    if max_age_days == 0 {
        return;
    }

    let max_age = Duration::from_secs(max_age_days * 24 * 60 * 60);

    let Ok(entries) = fs::read_dir(cache_dir) else {
        return;
    };

    let mut files_removed = 0u32;
    let mut bytes_freed = 0u64;

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            continue;
        }

        let Some(filename) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        // Skip in-progress files and marker files (we'll clean markers with their data files)
        if should_skip_cleanup(filename) || filename.ends_with(".ok") {
            continue;
        }

        if let Some((age, file_size)) = get_file_age_and_size(&path) {
            if age > max_age && fs::remove_file(&path).is_ok() {
                files_removed += 1;
                bytes_freed += file_size;

                // Also remove the corresponding .ok marker file
                // Hash is the filename stem (e.g., "a1b2c3" from "a1b2c3.png")
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    let ok_file = cache_dir.join(format!("{}.ok", stem));
                    let _ = fs::remove_file(&ok_file);
                }
            }
        }
    }

    if files_removed > 0 {
        crate::info!(
            "Cache cleanup: removed {} files, freed {} bytes",
            files_removed,
            bytes_freed
        );
    }
}
