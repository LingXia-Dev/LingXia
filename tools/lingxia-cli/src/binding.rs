//! Per-checkout credential binding cache under `<state root>/project-bindings/`.
//!
//! A binding is a pure cache keyed by `(canonical project root, channel)`: it
//! remembers which provider identity a checkout resolved to, plus the project
//! constraint snapshot from that resolution. Correctness lives in use-time
//! validation, never in the key — a stale or conflicting binding is dropped
//! and resolution reruns; it is never an error source. No secrets are stored.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

const SCHEMA: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Binding {
    pub schema: u32,
    /// Canonical project root the binding was made for (display/debugging).
    pub project_root: String,
    pub channel: String,
    pub provider: String,
    /// Stable provider identity (Apple: Team ID).
    pub identity: String,
    /// Project constraint snapshot at resolution time (e.g. `ios.teamId`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constraint: Option<String>,
}

pub struct BindingStore {
    root: PathBuf,
}

impl BindingStore {
    pub fn open() -> Result<Self> {
        Ok(Self::at(crate::state_root::lingxia_dir()?))
    }

    pub fn at(state_root: impl AsRef<Path>) -> Self {
        Self {
            root: state_root.as_ref().join("project-bindings"),
        }
    }

    fn path_for(&self, project_root: &Path, channel: &str) -> PathBuf {
        let canonical = canonical_root(project_root);
        let mut hasher = Sha256::new();
        hasher.update(canonical.as_bytes());
        let digest = hasher.finalize();
        let hash: String = digest[..8].iter().map(|b| format!("{b:02x}")).collect();
        self.root.join(hash).join(format!("{channel}.toml"))
    }

    pub fn load(&self, project_root: &Path, channel: &str) -> Option<Binding> {
        let path = self.path_for(project_root, channel);
        let content = fs::read_to_string(path).ok()?;
        let binding: Binding = toml::from_str(&content).ok()?;
        if binding.schema != SCHEMA || binding.channel != channel {
            return None;
        }
        Some(binding)
    }

    pub fn save(
        &self,
        project_root: &Path,
        channel: &str,
        provider: &str,
        identity: &str,
        constraint: Option<&str>,
    ) -> Result<()> {
        let binding = Binding {
            schema: SCHEMA,
            project_root: canonical_root(project_root),
            channel: channel.to_string(),
            provider: provider.to_string(),
            identity: identity.to_string(),
            constraint: constraint.map(str::to_string),
        };
        let path = self.path_for(project_root, channel);
        let parent = path.parent().expect("binding path has a parent");
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
        let content = toml::to_string_pretty(&binding)?;
        let tmp = tempfile::NamedTempFile::new_in(parent)?;
        fs::write(tmp.path(), content)?;
        tmp.persist(&path)
            .with_context(|| format!("write {}", path.display()))?;
        Ok(())
    }

    /// Drop the binding for this checkout/channel. Returns whether one existed.
    pub fn forget(&self, project_root: &Path, channel: &str) -> Result<bool> {
        let path = self.path_for(project_root, channel);
        if !path.exists() {
            return Ok(false);
        }
        fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
        Ok(true)
    }
}

fn canonical_root(project_root: &Path) -> String {
    let root = project_root
        .canonicalize()
        .unwrap_or_else(|_| project_root.to_path_buf());
    root.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_forget() {
        let state = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let store = BindingStore::at(state.path());

        assert!(store.load(project.path(), "ios").is_none());
        store
            .save(
                project.path(),
                "ios",
                "apple",
                "TEAMAAAAAA",
                Some("TEAMAAAAAA"),
            )
            .unwrap();

        let binding = store.load(project.path(), "ios").unwrap();
        assert_eq!(binding.identity, "TEAMAAAAAA");
        assert_eq!(binding.constraint.as_deref(), Some("TEAMAAAAAA"));

        // Channels do not share bindings.
        assert!(store.load(project.path(), "macos").is_none());

        // Different roots do not share bindings.
        let other = tempfile::tempdir().unwrap();
        assert!(store.load(other.path(), "ios").is_none());

        assert!(store.forget(project.path(), "ios").unwrap());
        assert!(!store.forget(project.path(), "ios").unwrap());
        assert!(store.load(project.path(), "ios").is_none());
    }
}
