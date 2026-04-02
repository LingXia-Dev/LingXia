use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use filetime::FileTime;
use rong_rt::download::{self as net, BodySink};
use thiserror::Error;
use tokio::sync::mpsc;

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
        // Fast path: if the caller knows the expected extension (common for https resources),
        // avoid scanning the whole cache directory.
        let hash_id = hash_key(key);
        let candidate = self.target_path_for_ext(key, ext);

        if self.ok_marker_exists(&hash_id) && candidate.exists() {
            return ResolveResult::Exists(candidate);
        }

        // Fallback to legacy resolution (supports extension mismatch / media copies).
        if let Some(p) = self.try_resolve(key) {
            ResolveResult::Exists(p)
        } else {
            ResolveResult::NonExists(candidate)
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
            let _ = crate::executor::spawn(task);
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

struct CacheEntry {
    path: PathBuf,
    size: u64,
    last_access: SystemTime,
}

pub struct CacheCapacityManager {
    cache_dir: PathBuf,
    max_bytes: u64,
    max_age: Duration,
    min_check_interval: Duration,
    worker: Mutex<Option<CacheCapacityWorker>>,
}

struct CacheCapacityWorker {
    tx: mpsc::Sender<CacheCapacityEvent>,
}

enum CacheCapacityEvent {
    Access,
    Shutdown,
}

impl CacheCapacityManager {
    pub fn new(
        cache_dir: PathBuf,
        max_bytes: u64,
        max_age: Duration,
        min_check_interval: Duration,
    ) -> Self {
        Self {
            cache_dir,
            max_bytes,
            max_age,
            min_check_interval,
            worker: Mutex::new(None),
        }
    }

    pub fn on_cache_access(&self, path: &Path) {
        if path.exists() {
            touch_access_time(path);
        }
        self.enqueue_access_check();
    }

    pub fn shutdown(&self) {
        let worker = self.worker.lock().ok().and_then(|mut worker| worker.take());
        if let Some(worker) = worker {
            // If the queue is full, dropping the sender still closes the channel
            // and lets the worker task exit once pending events are drained.
            let _ = worker.tx.try_send(CacheCapacityEvent::Shutdown);
        }
    }

    fn enqueue_access_check(&self) {
        if self.max_bytes == 0 && self.max_age.is_zero() {
            return;
        }

        let Some(tx) = self.ensure_worker_sender() else {
            return;
        };

        match tx.try_send(CacheCapacityEvent::Access) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {}
            Err(mpsc::error::TrySendError::Closed(_)) => {
                self.worker.lock().unwrap().take();
                if let Some(retry_tx) = self.ensure_worker_sender() {
                    let _ = retry_tx.try_send(CacheCapacityEvent::Access);
                }
            }
        }
    }

    fn ensure_worker_sender(&self) -> Option<mpsc::Sender<CacheCapacityEvent>> {
        let mut worker = self.worker.lock().unwrap();
        if let Some(worker) = worker.as_ref() {
            return Some(worker.tx.clone());
        }

        let (tx, rx) = mpsc::channel(32);
        let cache_dir = self.cache_dir.clone();
        let max_bytes = self.max_bytes;
        let max_age = self.max_age;
        let min_check_interval = self.min_check_interval;

        let _ = crate::executor::spawn(async move {
            run_cache_capacity_worker(cache_dir, max_bytes, max_age, min_check_interval, rx).await;
        });
        // Send initial access event so cleanup runs once at startup
        let _ = tx.try_send(CacheCapacityEvent::Access);
        *worker = Some(CacheCapacityWorker { tx: tx.clone() });
        Some(tx)
    }
}

impl Drop for CacheCapacityManager {
    fn drop(&mut self) {
        self.shutdown();
    }
}

async fn run_cache_capacity_worker(
    cache_dir: PathBuf,
    max_bytes: u64,
    max_age: Duration,
    min_check_interval: Duration,
    mut rx: mpsc::Receiver<CacheCapacityEvent>,
) {
    let mut last_check: Option<Instant> = None;

    while let Some(event) = rx.recv().await {
        match event {
            CacheCapacityEvent::Shutdown => break,
            CacheCapacityEvent::Access => {
                let now = Instant::now();
                if let Some(prev) = last_check
                    && now.duration_since(prev) < min_check_interval
                {
                    continue;
                }
                last_check = Some(now);

                let cache_dir_clone = cache_dir.clone();
                let blocking = crate::executor::spawn_blocking(move || {
                    enforce_cache_limits(&cache_dir_clone, max_bytes, max_age)
                });

                match blocking.await {
                    Ok(outcome) => {
                        if outcome.files_removed > 0 {
                            crate::info!(
                                "Cache cleanup: removed {} files, freed {} bytes (limit={} bytes, max_age={}s)",
                                outcome.files_removed,
                                outcome.bytes_freed,
                                max_bytes,
                                max_age.as_secs()
                            );
                        }
                    }
                    Err(e) => {
                        crate::error!("Cache cleanup task failed: {}", e);
                    }
                }
            }
        }
    }
}

struct CapacityCleanupOutcome {
    files_removed: u32,
    bytes_freed: u64,
}

fn enforce_cache_limits(
    cache_dir: &Path,
    max_bytes: u64,
    max_age: Duration,
) -> CapacityCleanupOutcome {
    let mut outcome = CapacityCleanupOutcome {
        files_removed: 0,
        bytes_freed: 0,
    };

    let cache_root = cache_dir
        .canonicalize()
        .unwrap_or_else(|_| cache_dir.to_path_buf());

    let mut total_bytes = 0u64;
    let mut entries = collect_cache_entries(cache_dir, &mut total_bytes);

    // First pass: remove files older than max_age
    if !max_age.is_zero() {
        let now = SystemTime::now();
        entries.retain(|entry| {
            let age = now
                .duration_since(entry.last_access)
                .unwrap_or(Duration::ZERO);
            if age > max_age {
                if try_remove_cache_entry(cache_dir, &cache_root, &entry.path) {
                    total_bytes = total_bytes.saturating_sub(entry.size);
                    outcome.files_removed += 1;
                    outcome.bytes_freed = outcome.bytes_freed.saturating_add(entry.size);
                }
                false // remove from entries list
            } else {
                true // keep for potential LRU pass
            }
        });
    }

    // Second pass: LRU eviction if still over capacity
    if max_bytes > 0 && total_bytes > max_bytes {
        entries.sort_by_key(|entry| entry.last_access);

        for entry in entries {
            if total_bytes <= max_bytes {
                break;
            }

            if try_remove_cache_entry(cache_dir, &cache_root, &entry.path) {
                total_bytes = total_bytes.saturating_sub(entry.size);
                outcome.files_removed += 1;
                outcome.bytes_freed = outcome.bytes_freed.saturating_add(entry.size);
            }
        }
    }

    outcome
}

pub fn cleanup_cache_dir(cache_dir: &Path, max_bytes: u64, max_age: Duration) {
    if max_bytes == 0 && max_age.is_zero() {
        return;
    }
    let outcome = enforce_cache_limits(cache_dir, max_bytes, max_age);
    if outcome.files_removed > 0 {
        crate::info!(
            "Startup cache cleanup: removed {} files, freed {} bytes",
            outcome.files_removed,
            outcome.bytes_freed
        );
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

            // Never recurse into symlink directories.
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

            // Keep marker files and lock-associated files in total usage accounting,
            // but never evict them directly while protected by an active lock.
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

fn remove_ok_marker_for(_cache_dir: &Path, data_path: &Path) {
    let Some(parent) = data_path.parent() else {
        return;
    };
    if let Some(stem) = data_path.file_stem().and_then(|s| s.to_str()) {
        let _ = fs::remove_file(parent.join(format!("{}.ok", stem)));
    }
}

fn try_remove_cache_entry(cache_dir: &Path, cache_root: &Path, data_path: &Path) -> bool {
    if !is_path_within_root(cache_root, data_path) {
        crate::warn!(
            "Skip cache cleanup outside root: root={}, path={}",
            cache_root.display(),
            data_path.display()
        );
        return false;
    }
    if fs::remove_file(data_path).is_err() {
        return false;
    }
    remove_ok_marker_for(cache_dir, data_path);
    remove_empty_parent_dirs(cache_dir, data_path);
    true
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
