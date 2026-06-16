//! Apple Developer authentication and credential storage.
//!
//! Provides authentication support for Apple Developer accounts:
//! - App Store Connect API (paid accounts) - JWT-based authentication
//! - Xcode Private API (free accounts) - Apple ID + 2FA (future)
//!
//! Credentials are stored in ~/.lingxia/apple/credentials.json

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const CREDENTIALS_DIR: &str = ".lingxia";
const APPLE_CREDENTIALS_SUBDIR: &str = "apple";
const CREDENTIALS_FILE: &str = "credentials.json";
const DEVELOPER_ID_FILE: &str = "developer-id.json";

/// Resolve the `~/.lingxia/apple` directory used for all Apple credentials.
fn apple_credentials_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    Ok(home.join(CREDENTIALS_DIR).join(APPLE_CREDENTIALS_SUBDIR))
}

// =============================================================================
// Credentials
// =============================================================================

/// Stored authentication credentials
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AuthCredentials {
    /// App Store Connect API Key (paid developer accounts)
    #[serde(rename = "appStoreConnect")]
    AppStoreConnect {
        /// API Key ID (e.g., "ABC123DEF4")
        key_id: String,
        /// Issuer ID (e.g., "12345678-1234-1234-1234-123456789012")
        issuer_id: String,
        /// Private key content in PKCS#8 PEM format
        private_key_pem: String,
        /// Selected team ID
        team_id: String,
        /// Cached signing identity created via App Store Connect API
        #[serde(default)]
        cached_signing_identity: Option<CachedSigningIdentity>,
    },
    /// Apple ID authentication (for Xcode Private API - future)
    #[serde(rename = "appleId")]
    AppleId {
        /// Apple Directory Services ID
        adsid: String,
        /// IDMS token (used for re-authentication)
        token: String,
        /// App token for Developer Services API (com.apple.gs.xcode.auth)
        app_token: String,
        /// Selected team ID
        team_id: String,
        /// Token expiration time
        expiry: DateTime<Utc>,
    },
}

/// Cached signing material for App Store Connect API mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedSigningIdentity {
    pub cert_id: String,
    pub signing_identity: String,
    pub cert_data_b64: String,
    pub private_key: String,
}

impl AuthCredentials {
    /// Get the team ID from credentials
    pub fn team_id(&self) -> &str {
        match self {
            AuthCredentials::AppStoreConnect { team_id, .. } => team_id,
            AuthCredentials::AppleId { team_id, .. } => team_id,
        }
    }

    /// Check if credentials are expired
    pub fn is_expired(&self) -> bool {
        match self {
            AuthCredentials::AppStoreConnect { .. } => false, // API keys don't expire
            AuthCredentials::AppleId { expiry, .. } => *expiry < Utc::now(),
        }
    }

    /// Get a display name for the credential type
    pub fn credential_type(&self) -> &str {
        match self {
            AuthCredentials::AppStoreConnect { .. } => "App Store Connect API Key",
            AuthCredentials::AppleId { .. } => "Apple ID",
        }
    }
}

// =============================================================================
// Credential Storage
// =============================================================================

/// Manages credential storage
pub struct CredentialStorage {
    credentials_path: PathBuf,
}

impl CredentialStorage {
    /// Create a new credential storage instance
    pub fn new() -> Result<Self> {
        let credentials_path = apple_credentials_dir()?.join(CREDENTIALS_FILE);
        Ok(Self { credentials_path })
    }

    /// Get the path to the credentials file
    pub fn path(&self) -> &PathBuf {
        &self.credentials_path
    }

    /// Load stored credentials
    pub fn load(&self) -> Result<Option<AuthCredentials>> {
        if !self.credentials_path.exists() {
            return Ok(None);
        }

        self.read_credentials_file(&self.credentials_path).map(Some)
    }

    /// Save credentials
    pub fn save(&self, credentials: &AuthCredentials) -> Result<()> {
        // Ensure directory exists
        if let Some(parent) = self.credentials_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory {}", parent.display()))?;
        }

        let content =
            serde_json::to_string_pretty(credentials).context("Failed to serialize credentials")?;

        fs::write(&self.credentials_path, content)
            .with_context(|| format!("Failed to write {}", self.credentials_path.display()))?;

        // Set restrictive permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = fs::Permissions::from_mode(0o600);
            fs::set_permissions(&self.credentials_path, permissions)?;
        }

        Ok(())
    }

    /// Delete stored credentials
    pub fn delete(&self) -> Result<bool> {
        if self.credentials_path.exists() {
            fs::remove_file(&self.credentials_path)
                .with_context(|| format!("Failed to delete {}", self.credentials_path.display()))?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn read_credentials_file(&self, path: &PathBuf) -> Result<AuthCredentials> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        serde_json::from_str(&content).with_context(|| {
            format!(
                "Failed to parse {}. Re-run 'lingxia auth apple login' to refresh credentials.",
                path.display()
            )
        })
    }
}

// =============================================================================
// Developer ID Credentials
// =============================================================================

/// Developer ID Application certificate stored for macOS distribution and
/// notarization. Persisted to `~/.lingxia/apple/developer-id.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeveloperIdCredentials {
    /// Base64-encoded `.p12` (PKCS#12) certificate bundle.
    pub p12_base64: String,
    /// Password protecting the `.p12` bundle.
    pub password: String,
    /// Optional codesign identity name (the "Developer ID Application: ..."
    /// common name). When absent it is auto-detected at signing time.
    #[serde(default)]
    pub identity: Option<String>,
}

impl DeveloperIdCredentials {
    /// Resolve the on-disk path (`~/.lingxia/apple/developer-id.json`).
    pub fn path() -> Result<PathBuf> {
        Ok(apple_credentials_dir()?.join(DEVELOPER_ID_FILE))
    }

    /// Load stored Developer ID credentials, if any.
    pub fn load() -> Result<Option<Self>> {
        let path = Self::path()?;
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let creds = serde_json::from_str(&content).with_context(|| {
            format!(
                "Failed to parse {}. Re-run 'lingxia auth apple import-developer-id' to refresh it.",
                path.display()
            )
        })?;
        Ok(Some(creds))
    }

    /// Persist these credentials with restrictive (0600) permissions.
    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory {}", parent.display()))?;
        }

        let content = serde_json::to_string_pretty(self)
            .context("Failed to serialize Developer ID credentials")?;
        fs::write(&path, content).with_context(|| format!("Failed to write {}", path.display()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = fs::Permissions::from_mode(0o600);
            fs::set_permissions(&path, permissions)?;
        }

        Ok(())
    }
}
