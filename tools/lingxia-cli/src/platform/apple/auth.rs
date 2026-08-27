//! Apple Developer credential types.
//!
//! Storage lives in the identity-keyed wallet (`crate::wallet`); this module
//! only defines the credential payloads shared by login, provisioning,
//! notarization, and developer-services code.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const APPLE_CREDENTIALS_SUBDIR: &str = "apple";

/// Resolve the `~/.lingxia/apple` directory used for Apple caches
/// (anisette device fingerprint, reusable provisioning profiles).
pub(crate) fn apple_credentials_dir() -> Result<PathBuf> {
    Ok(crate::state_root::lingxia_dir()?.join(APPLE_CREDENTIALS_SUBDIR))
}

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
    /// Apple ID authentication (Xcode private API)
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
}

/// Developer ID Application certificate stored per team for macOS
/// distribution and notarization.
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
