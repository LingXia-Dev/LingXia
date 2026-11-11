//! Compiled View modules keyed by source hash, so opening a page skips the QuickJS compile.
//! Kept in memory for the process and, once a host names a directory, on disk.
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex, OnceLock};

const MAGIC: &[u8; 8] = b"LXVBC\0\0\x01";
const LOWERED_MAGIC: &[u8; 8] = b"LXVLS\0\0\x01";
/// Larger lowered-style sets are not persisted: reading them back would cost what they save.
pub(crate) const LOWERED_BYTES: usize = 256 << 10;
const HEADER: usize = MAGIC.len() + 32 + 8;
const MEMORY_BYTES: usize = 24 << 20;
const DISK_BYTES: u64 = 32 << 20;

pub(crate) type Key = [u8; 32];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    Memory,
    Disk,
    Miss,
}

static DIR: OnceLock<PathBuf> = OnceLock::new();

/// Persist compiled modules under `dir`; the first call wins. Without one they live in memory.
pub fn set_bytecode_dir(dir: PathBuf) {
    let _ = DIR.set(dir);
}

/// Where compiled modules persist, if anywhere; the renderer keeps its pipelines beside them.
#[cfg(feature = "gpu")]
pub(crate) fn cache_dir() -> Option<&'static Path> {
    DIR.get().map(PathBuf::as_path)
}

/// The source plus everything that decides whether bytecode compiled from it can be read back.
pub(crate) fn key(source: &str) -> Key {
    static ENGINE: LazyLock<String> = LazyLock::new(|| {
        // SAFETY: QuickJS returns a static NUL-terminated string.
        let version = unsafe { std::ffi::CStr::from_ptr(rong_quickjs_sys::JS_GetVersion()) };
        format!(
            "{}|{}|{}",
            env!("CARGO_PKG_VERSION"),
            version.to_string_lossy(),
            crate::protocol::UI_ABI
        )
    });
    let mut hash = Sha256::new();
    hash.update(ENGINE.as_bytes());
    hash.update([0]);
    hash.update(source.as_bytes());
    hash.finalize().into()
}

struct Memory {
    entries: HashMap<Key, (Arc<[u8]>, u64)>,
    bytes: usize,
    tick: u64,
}

static MEMORY: LazyLock<Mutex<Memory>> = LazyLock::new(|| {
    Mutex::new(Memory {
        entries: HashMap::new(),
        bytes: 0,
        tick: 0,
    })
});

fn memory() -> std::sync::MutexGuard<'static, Memory> {
    MEMORY
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub(crate) fn remember(key: Key, bytes: Arc<[u8]>) {
    let mut memory = memory();
    memory.tick += 1;
    let tick = memory.tick;
    if let Some((old, _)) = memory.entries.insert(key, (bytes.clone(), tick)) {
        memory.bytes -= old.len();
    }
    memory.bytes += bytes.len();
    while memory.bytes > MEMORY_BYTES && memory.entries.len() > 1 {
        let Some(oldest) = memory
            .entries
            .iter()
            .filter(|(k, _)| **k != key)
            .min_by_key(|(_, (_, used))| *used)
            .map(|(k, _)| *k)
        else {
            break;
        };
        if let Some((evicted, _)) = memory.entries.remove(&oldest) {
            memory.bytes -= evicted.len();
        }
    }
}

/// Bytecode for `key`, from memory or else disk.
pub(crate) fn load(key: &Key) -> Option<(Arc<[u8]>, Outcome)> {
    {
        let mut memory = memory();
        memory.tick += 1;
        let tick = memory.tick;
        if let Some((bytes, used)) = memory.entries.get_mut(key) {
            *used = tick;
            return Some((bytes.clone(), Outcome::Memory));
        }
    }
    let bytes: Arc<[u8]> = read_file(&path(DIR.get()?, key), MAGIC, key, None)?.into();
    remember(*key, bytes.clone());
    Some((bytes, Outcome::Disk))
}

/// Keep freshly compiled bytecode; the disk copy is written off the calling thread.
pub(crate) fn store(key: Key, bytes: Vec<u8>) {
    let bytes: Arc<[u8]> = bytes.into();
    remember(key, bytes.clone());
    let Some(dir) = DIR.get() else {
        return;
    };
    let dir = dir.clone();
    let spawned = std::thread::Builder::new()
        .name("view-bytecode".into())
        .spawn(move || {
            if let Err(error) = write_file(&path(&dir, &key), MAGIC, &key, &bytes) {
                log::warn!(target: "lingxia_view", "BYTECODE_CACHE: cannot write cache: {error}");
            }
            trim(&dir, &path(&dir, &key));
        });
    if let Err(error) = spawned {
        log::warn!(target: "lingxia_view", "BYTECODE_CACHE: cannot write cache: {error}");
    }
}

/// Drop an entry the engine refused to read.
pub(crate) fn forget(key: &Key) {
    let mut memory = memory();
    if let Some((bytes, _)) = memory.entries.remove(key) {
        memory.bytes -= bytes.len();
    }
    drop(memory);
    if let Some(dir) = DIR.get() {
        let _ = std::fs::remove_file(path(dir, key));
    }
}

/// Style lowerings an earlier isolate of module `key` persisted, as `__lingxiaSeedLowered` takes them.
pub(crate) fn load_lowered(key: &Key) -> Option<String> {
    let path = lowered_path(DIR.get()?, key);
    let body = read_file(&path, LOWERED_MAGIC, key, Some(LOWERED_BYTES))?;
    String::from_utf8(body)
        .inspect_err(|_| {
            let _ = std::fs::remove_file(&path);
        })
        .ok()
}

/// Persist the lowerings module `key` took, off the calling thread; oversized sets are dropped.
pub(crate) fn store_lowered(key: Key, text: String) {
    if text.len() > LOWERED_BYTES {
        log::debug!(target: "lingxia_view", "BYTECODE_CACHE: lowered styles not kept, {} KB", text.len() / 1024);
        return;
    }
    let Some(dir) = DIR.get() else {
        return;
    };
    let dir = dir.clone();
    let spawned = std::thread::Builder::new()
        .name("view-lowered".into())
        .spawn(move || {
            let written = std::fs::create_dir_all(&dir).and_then(|()| {
                write_file(&lowered_path(&dir, &key), LOWERED_MAGIC, &key, text.as_bytes())
            });
            if let Err(error) = written {
                log::warn!(target: "lingxia_view", "BYTECODE_CACHE: cannot write lowered styles: {error}");
            }
        });
    if let Err(error) = spawned {
        log::warn!(target: "lingxia_view", "BYTECODE_CACHE: cannot write lowered styles: {error}");
    }
}

/// Drop lowerings the runtime refused to seed from.
pub(crate) fn forget_lowered(key: &Key) {
    if let Some(dir) = DIR.get() {
        let _ = std::fs::remove_file(lowered_path(dir, key));
    }
}

fn hex(key: &Key) -> String {
    key.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn path(dir: &Path, key: &Key) -> PathBuf {
    dir.join(format!("{}.qbc", hex(key)))
}

fn lowered_path(dir: &Path, key: &Key) -> PathBuf {
    dir.join(format!("{}.lowered", hex(key)))
}

/// The header repeats the key and the length, so a renamed or truncated file reads as a miss.
fn read_file(path: &Path, magic: &[u8; 8], key: &Key, max: Option<usize>) -> Option<Vec<u8>> {
    if let Some(max) = max
        && std::fs::metadata(path).ok()?.len() > (HEADER + max) as u64
    {
        let _ = std::fs::remove_file(path);
        return None;
    }
    let data = std::fs::read(path).ok()?;
    let (header, body) = data.split_at_checked(HEADER)?;
    let length = u64::from_le_bytes(header[MAGIC.len() + 32..].try_into().ok()?);
    if &header[..MAGIC.len()] != magic
        || header[MAGIC.len()..MAGIC.len() + 32] != key[..]
        || length != body.len() as u64
    {
        let _ = std::fs::remove_file(path);
        return None;
    }
    // Recently read entries survive trimming.
    if let Ok(file) = std::fs::File::options().append(true).open(path) {
        let _ = file.set_modified(std::time::SystemTime::now());
    }
    Some(body.to_vec())
}

fn write_file(target: &Path, magic: &[u8; 8], key: &Key, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = target.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // Per extension, so a module's bytecode and lowered styles never share a staging file.
    let extension = target.extension().unwrap_or_default().to_string_lossy();
    let staging = target.with_extension(format!("{extension}.{}.tmp", std::process::id()));
    let mut data = Vec::with_capacity(HEADER + bytes.len());
    data.extend_from_slice(magic);
    data.extend_from_slice(key);
    data.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    data.extend_from_slice(bytes);
    std::fs::write(&staging, data)?;
    std::fs::rename(&staging, target).inspect_err(|_| {
        let _ = std::fs::remove_file(&staging);
    })
}

/// Evict least recently used entries past the disk budget, never the one just written.
fn trim(dir: &Path, keep: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = entries
        .flatten()
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "qbc"))
        .filter_map(|entry| {
            let meta = entry.metadata().ok()?;
            Some((meta.modified().ok()?, meta.len(), entry.path()))
        })
        .collect();
    let mut total: u64 = files.iter().map(|(_, len, _)| len).sum();
    files.sort();
    for (_, len, path) in files {
        if total <= DISK_BYTES {
            break;
        }
        if path != keep && std::fs::remove_file(&path).is_ok() {
            total -= len;
        }
    }
    // Lowered styles belong to a module; they go when its bytecode does.
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let lowered = entry.path();
        if lowered.extension().is_some_and(|ext| ext == "lowered")
            && !lowered.with_extension("qbc").exists()
        {
            let _ = std::fs::remove_file(&lowered);
        }
    }
}

/// Dev sessions log where each module came from.
pub(crate) fn report(outcome: Outcome, bytes: usize, elapsed: std::time::Duration) {
    if !super::dev_globals() {
        return;
    }
    let source = match outcome {
        Outcome::Memory => "hit (memory)",
        Outcome::Disk => "hit (disk)",
        Outcome::Miss => "miss (compiled)",
    };
    log::info!(
        target: "lingxia_view",
        "BYTECODE_CACHE: {source}, {} KB in {:.1} ms",
        bytes / 1024,
        elapsed.as_secs_f64() * 1000.0
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_disk_entry_reads_back_only_under_its_own_key_and_length() {
        let dir = tempfile_dir();
        let key = key("export const a = 1;");
        let other = super::key("export const a = 2;");
        assert_ne!(key, other);
        write_file(&path(&dir, &key), MAGIC, &key, b"bytecode").unwrap();
        assert_eq!(
            read_file(&path(&dir, &key), MAGIC, &key, None).unwrap(),
            b"bytecode"
        );
        std::fs::copy(path(&dir, &key), path(&dir, &other)).unwrap();
        assert!(read_file(&path(&dir, &other), MAGIC, &other, None).is_none());
        assert!(
            !path(&dir, &other).exists(),
            "a mismatched entry is removed"
        );
        let file = path(&dir, &key);
        let data = std::fs::read(&file).unwrap();
        std::fs::write(&file, &data[..data.len() - 1]).unwrap();
        assert!(
            read_file(&file, MAGIC, &key, None).is_none(),
            "a truncated entry is a miss"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn trimming_keeps_the_entry_just_written() {
        let dir = tempfile_dir();
        let big = vec![0u8; (DISK_BYTES / 2) as usize + 1];
        let keys = [key("one"), key("two"), key("three")];
        for key in &keys {
            write_file(&path(&dir, key), MAGIC, key, &big).unwrap();
        }
        trim(&dir, &path(&dir, &keys[2]));
        assert!(path(&dir, &keys[2]).exists());
        let left = std::fs::read_dir(&dir).unwrap().count();
        assert_eq!(
            left, 1,
            "two half-budget entries do not fit beside the newest"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn lowered_styles_read_back_only_as_their_own_kind_and_size() {
        let dir = tempfile_dir();
        let key = key("export const a = 1;");
        let lowered = lowered_path(&dir, &key);
        write_file(&lowered, LOWERED_MAGIC, &key, b"{}").unwrap();
        let read = |path: &Path, magic| read_file(path, magic, &key, Some(LOWERED_BYTES));
        assert_eq!(read(&lowered, LOWERED_MAGIC).unwrap(), b"{}");
        assert!(
            read(&lowered, MAGIC).is_none(),
            "bytecode never reads as lowered styles"
        );
        let big = vec![b' '; LOWERED_BYTES + 1];
        write_file(&lowered, LOWERED_MAGIC, &key, &big).unwrap();
        assert!(read(&lowered, LOWERED_MAGIC).is_none());
        assert!(!lowered.exists(), "an oversized entry is removed");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn trimming_drops_lowered_styles_whose_bytecode_is_gone() {
        let dir = tempfile_dir();
        let (kept, gone) = (key("kept"), key("gone"));
        write_file(&path(&dir, &kept), MAGIC, &kept, b"bytecode").unwrap();
        for key in [&kept, &gone] {
            write_file(&lowered_path(&dir, key), LOWERED_MAGIC, key, b"{}").unwrap();
        }
        trim(&dir, &path(&dir, &kept));
        assert!(lowered_path(&dir, &kept).exists());
        assert!(!lowered_path(&dir, &gone).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn tempfile_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "lingxia-view-bytecode-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }
}
