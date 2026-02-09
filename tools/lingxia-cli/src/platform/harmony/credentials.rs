//! Credential storage for Harmony AGC Connect API authentication.
//!
//! Stores and manages AGC Connect API credentials in a local JSON file.

use super::agc::AgcApiCredentials;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Storage for AGC Connect API credentials (client_id/client_secret).
///
/// Stored at: `~/.lingxia/harmony/agc_credentials.json`
pub struct AgcCredentialStorage {
    storage_path: PathBuf,
}

impl AgcCredentialStorage {
    /// Create a new AGC credential storage instance.
    pub fn new() -> Result<Self> {
        let home = dirs::home_dir().context("Could not determine home directory")?;
        let storage_path = home
            .join(".lingxia")
            .join("harmony")
            .join("agc_credentials.json");
        Ok(Self { storage_path })
    }

    /// Get the storage file path.
    pub fn path(&self) -> &Path {
        &self.storage_path
    }

    /// Save AGC API credentials.
    pub fn save(&self, credentials: &AgcApiCredentials) -> Result<()> {
        if let Some(parent) = self.storage_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }

        let json = serde_json::to_string_pretty(credentials)
            .context("Failed to serialize AGC credentials")?;

        fs::write(&self.storage_path, &json).with_context(|| {
            format!(
                "Failed to write AGC credentials to {}",
                self.storage_path.display()
            )
        })?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = fs::Permissions::from_mode(0o600);
            fs::set_permissions(&self.storage_path, permissions).ok();
        }

        Ok(())
    }

    /// Load AGC API credentials.
    pub fn load(&self) -> Result<Option<AgcApiCredentials>> {
        if !self.storage_path.exists() {
            return Ok(None);
        }

        let json = fs::read_to_string(&self.storage_path).with_context(|| {
            format!(
                "Failed to read AGC credentials from {}",
                self.storage_path.display()
            )
        })?;

        let credentials: AgcApiCredentials =
            serde_json::from_str(&json).context("Failed to parse AGC credentials")?;

        Ok(Some(credentials))
    }

    /// Clear stored AGC credentials.
    pub fn clear(&self) -> Result<()> {
        if self.storage_path.exists() {
            fs::remove_file(&self.storage_path)
                .with_context(|| format!("Failed to remove {}", self.storage_path.display()))?;
        }
        Ok(())
    }
}

impl Default for AgcCredentialStorage {
    fn default() -> Self {
        Self::new().expect("Failed to create default AGC credential storage")
    }
}
