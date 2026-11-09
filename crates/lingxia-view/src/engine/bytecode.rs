//! Compiled View modules keyed by source hash, so opening a page skips the QuickJS compile.
//! Kept in memory for the process and, once a host names a directory, on disk.
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex, OnceLock};

const MAGIC: &[u8; 8] = b"LXVBC\0\0\x01";
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
    let bytes: Arc<[u8]> = read_file(&path(DIR.get()?, key), key)?.into();
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
            if let Err(error) = write_file(&dir, &key, &bytes) {
                log::warn!(target: "lingxia_view", "UI_BYTECODE: cannot write cache: {error}");
            }
            trim(&dir, &path(&dir, &key));
        });
    if let Err(error) = spawned {
        log::warn!(target: "lingxia_view", "UI_BYTECODE: cannot write cache: {error}");
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

fn path(dir: &Path, key: &Key) -> PathBuf {
    let name: String = key.iter().map(|byte| format!("{byte:02x}")).collect();
    dir.join(format!("{name}.qbc"))
}

/// The header repeats the key and the length, so a renamed or truncated file reads as a miss.
fn read_file(path: &Path, key: &Key) -> Option<Vec<u8>> {
    let data = std::fs::read(path).ok()?;
    let (header, body) = data.split_at_checked(HEADER)?;
    let length = u64::from_le_bytes(header[MAGIC.len() + 32..].try_into().ok()?);
    if &header[..MAGIC.len()] != MAGIC
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

fn write_file(dir: &Path, key: &Key, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let target = path(dir, key);
    let staging = target.with_extension(format!("{}.tmp", std::process::id()));
    let mut data = Vec::with_capacity(HEADER + bytes.len());
    data.extend_from_slice(MAGIC);
    data.extend_from_slice(key);
    data.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    data.extend_from_slice(bytes);
    std::fs::write(&staging, data)?;
    std::fs::rename(&staging, &target).inspect_err(|_| {
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
        "UI_BYTECODE: {source}, {} KB in {:.1} ms",
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
        write_file(&dir, &key, b"bytecode").unwrap();
        assert_eq!(read_file(&path(&dir, &key), &key).unwrap(), b"bytecode");
        std::fs::copy(path(&dir, &key), path(&dir, &other)).unwrap();
        assert!(read_file(&path(&dir, &other), &other).is_none());
        assert!(
            !path(&dir, &other).exists(),
            "a mismatched entry is removed"
        );
        let file = path(&dir, &key);
        let data = std::fs::read(&file).unwrap();
        std::fs::write(&file, &data[..data.len() - 1]).unwrap();
        assert!(
            read_file(&file, &key).is_none(),
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
            write_file(&dir, key, &big).unwrap();
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
