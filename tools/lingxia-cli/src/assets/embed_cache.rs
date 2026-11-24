//! The last embed build of each lxapp a host dev session builds, kept per project and
//! profile so a session whose inputs are unchanged seeds its build directory by copying.

use super::bundles::copy_dir_recursive;
use super::hash::{hash_tree, path_key, sha256_hex};
use crate::platform::BuildProfile;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const ENTRY: &str = "entry";
const RECORD: &str = "record.json";
const DIST: &str = "dist";
const LOCK: &str = "lock";
const RETIRED_PREFIX: &str = ".old-";

#[derive(Serialize, Deserialize)]
struct Record {
    inputs: String,
    dist: String,
}

/// `<project>/.lingxia/dev/embed/<profile>/<appId>/`: `entry/` holds `record.json` and `dist/`.
/// An entry is replaced whole by a rename; readers hold the shared lock while they copy.
#[derive(Debug, Clone)]
pub(super) struct EmbedCache {
    dir: PathBuf,
}

impl EmbedCache {
    pub(super) fn new(project_root: &Path, profile: BuildProfile, app_id: &str) -> Self {
        let dir = project_root
            .join(".lingxia")
            .join("dev")
            .join("embed")
            .join(profile.as_str())
            .join(file_safe(app_id));
        Self { dir }
    }

    /// Copy the entry built from `inputs` into `output`. False when there is none, or the
    /// copy does not hash to what was recorded.
    pub(super) fn seed(&self, inputs: &str, output: &Path) -> Result<bool> {
        let lock = self.lock_file()?;
        lock.lock_shared()
            .with_context(|| format!("Failed to lock {}", self.dir.display()))?;
        let entry = self.dir.join(ENTRY);
        let Ok(text) = fs::read(entry.join(RECORD)) else {
            return Ok(false);
        };
        let Ok(record) = serde_json::from_slice::<Record>(&text) else {
            return Ok(false);
        };
        if record.inputs != inputs {
            return Ok(false);
        }
        let _ = fs::remove_dir_all(output);
        copy_dir_recursive(&entry.join(DIST), output)?;
        drop(lock);
        if hash_tree(output, &[])? == record.dist {
            return Ok(true);
        }
        let _ = fs::remove_dir_all(output);
        Ok(false)
    }

    /// Record `output`, built from `inputs`, as this app's entry in place of the last one.
    pub(super) fn store(&self, inputs: &str, output: &Path, dist: &str) -> Result<()> {
        fs::create_dir_all(&self.dir)
            .with_context(|| format!("Failed to create {}", self.dir.display()))?;
        let staging = self.dir.join(format!(".new-{}", unique()));
        let result = self.stage(&staging, inputs, output, dist).and_then(|()| {
            let lock = self.lock_file()?;
            lock.lock()
                .with_context(|| format!("Failed to lock {}", self.dir.display()))?;
            let entry = self.dir.join(ENTRY);
            if entry.exists() {
                fs::rename(
                    &entry,
                    self.dir.join(format!("{RETIRED_PREFIX}{}", unique())),
                )?;
            }
            fs::rename(&staging, &entry)?;
            // No reader can reach a retired entry once the lock is ours.
            for retired in fs::read_dir(&self.dir)?.flatten() {
                if retired
                    .file_name()
                    .to_string_lossy()
                    .starts_with(RETIRED_PREFIX)
                {
                    let _ = fs::remove_dir_all(retired.path());
                }
            }
            Ok(())
        });
        let _ = fs::remove_dir_all(&staging);
        result.with_context(|| format!("Failed to record {}", self.dir.display()))
    }

    fn stage(&self, staging: &Path, inputs: &str, output: &Path, dist: &str) -> Result<()> {
        copy_dir_recursive(output, &staging.join(DIST))?;
        let record = Record {
            inputs: inputs.to_string(),
            dist: dist.to_string(),
        };
        fs::write(staging.join(RECORD), serde_json::to_vec(&record)?)?;
        Ok(())
    }

    fn lock_file(&self) -> Result<fs::File> {
        fs::create_dir_all(&self.dir)
            .with_context(|| format!("Failed to create {}", self.dir.display()))?;
        let path = self.dir.join(LOCK);
        fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .with_context(|| format!("Failed to open {}", path.display()))
    }
}

/// `inputs` (the bundle tree and its linked packages) plus the files outside the bundle
/// that its last build's GPU pages compiled from, the list the dev watcher follows.
pub(super) fn embed_inputs(bundle_dir: &Path, inputs: &str) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(inputs.as_bytes());
    for file in external_inputs(bundle_dir) {
        hasher.update(path_key(&file).as_bytes());
        hasher.update([0]);
        match fs::read(&file) {
            Ok(data) => {
                hasher.update((data.len() as u64).to_le_bytes());
                hasher.update(&data);
            }
            Err(_) => hasher.update(b"missing"),
        }
        hasher.update([0]);
    }
    sha256_hex(&hasher.finalize())
}

fn external_inputs(bundle_dir: &Path) -> BTreeSet<PathBuf> {
    let root = bundle_dir
        .canonicalize()
        .unwrap_or_else(|_| bundle_dir.to_path_buf());
    let Ok(text) = fs::read(bundle_dir.join(".lingxia").join("gpu-inputs.json")) else {
        return BTreeSet::new();
    };
    let Ok(pages) = serde_json::from_slice::<BTreeMap<String, Vec<String>>>(&text) else {
        return BTreeSet::new();
    };
    pages
        .into_values()
        .flatten()
        .map(|file| {
            let path = bundle_dir.join(file);
            path.canonicalize().unwrap_or(path)
        })
        .filter(|path| !path.starts_with(&root))
        .collect()
}

fn unique() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    format!(
        "{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

fn file_safe(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '_' | '-' => ch,
            _ => '_',
        })
        .collect()
}
