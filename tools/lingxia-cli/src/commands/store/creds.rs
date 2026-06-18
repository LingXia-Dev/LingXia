//! `lingxia store` credential store: `~/.lingxia/store/credentials.toml`.
//!
//! Resolution precedence is **env var > file**, so `store login` writes the
//! file once for zero-env local dev, while CI injects env vars (no file on
//! disk) and the env transparently overrides the cache. The file is written
//! owner-only (`0600` on Unix) and never belongs in a repo.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const LINGXIA_DIR: &str = ".lingxia";
const STORE_DIR: &str = "store";
const CREDENTIALS_FILE: &str = "credentials.toml";

/// The whole `credentials.toml` (one table per store).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StoreCredentials {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msstore: Option<MsStoreCreds>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub appstore: Option<AppStoreCreds>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub appgallery: Option<AppGalleryCreds>,
}

/// Microsoft Store (Partner Center) — Azure AD client credentials.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MsStoreCreds {
    pub tenant: String,
    pub client_id: String,
    pub client_secret: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seller_id: Option<String>,
}

/// App Store Connect — API key (issuer + key id + `.p8` path).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppStoreCreds {
    pub issuer_id: String,
    pub key_id: String,
    /// Path to the App Store Connect `.p8` private key (`~` is expanded).
    pub key_path: String,
}

/// Huawei AppGallery Connect — client credentials.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppGalleryCreds {
    pub client_id: String,
    pub client_secret: String,
}

fn store_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    Ok(home.join(LINGXIA_DIR).join(STORE_DIR))
}

/// Path to `~/.lingxia/store/credentials.toml`.
pub fn credentials_path() -> Result<PathBuf> {
    Ok(store_dir()?.join(CREDENTIALS_FILE))
}

impl StoreCredentials {
    /// Load the credential file, or an empty set when it does not exist.
    pub fn load() -> Result<Self> {
        let path = credentials_path()?;
        Self::load_from(&path)
    }

    pub fn load_from(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("Failed to parse {}", path.display()))
    }

    /// Persist the credential file owner-only.
    pub fn save(&self) -> Result<()> {
        let dir = store_dir()?;
        fs::create_dir_all(&dir).with_context(|| format!("Failed to create {}", dir.display()))?;
        let path = dir.join(CREDENTIALS_FILE);
        let text = toml::to_string_pretty(self).context("Failed to serialize credentials")?;
        fs::write(&path, text).with_context(|| format!("Failed to write {}", path.display()))?;
        set_owner_only(&path)?;
        Ok(())
    }
}

#[cfg(unix)]
fn set_owner_only(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("Failed to set permissions on {}", path.display()))
}

#[cfg(not(unix))]
fn set_owner_only(_path: &Path) -> Result<()> {
    Ok(())
}

fn env_nonempty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// Resolve Microsoft Store credentials: env (`LINGXIA_MSSTORE_*`) over file.
pub fn resolve_msstore(file: &StoreCredentials) -> Result<MsStoreCreds> {
    let f = file.msstore.as_ref();
    let tenant = env_nonempty("LINGXIA_MSSTORE_TENANT").or_else(|| f.map(|c| c.tenant.clone()));
    let client_id =
        env_nonempty("LINGXIA_MSSTORE_CLIENT_ID").or_else(|| f.map(|c| c.client_id.clone()));
    let client_secret = env_nonempty("LINGXIA_MSSTORE_CLIENT_SECRET")
        .or_else(|| f.map(|c| c.client_secret.clone()));
    let seller_id =
        env_nonempty("LINGXIA_MSSTORE_SELLER_ID").or_else(|| f.and_then(|c| c.seller_id.clone()));
    match (tenant, client_id, client_secret) {
        (Some(tenant), Some(client_id), Some(client_secret)) => Ok(MsStoreCreds {
            tenant,
            client_id,
            client_secret,
            seller_id,
        }),
        _ => bail!(
            "Microsoft Store credentials not found. Run `lingxia store login --platform windows`, \
             or set LINGXIA_MSSTORE_TENANT / _CLIENT_ID / _CLIENT_SECRET."
        ),
    }
}

/// Resolve App Store Connect credentials: env (`LINGXIA_ASC_*`) over file.
pub fn resolve_appstore(file: &StoreCredentials) -> Result<AppStoreCreds> {
    let f = file.appstore.as_ref();
    let issuer_id =
        env_nonempty("LINGXIA_ASC_ISSUER_ID").or_else(|| f.map(|c| c.issuer_id.clone()));
    let key_id = env_nonempty("LINGXIA_ASC_KEY_ID").or_else(|| f.map(|c| c.key_id.clone()));
    let key_path = env_nonempty("LINGXIA_ASC_KEY_PATH").or_else(|| f.map(|c| c.key_path.clone()));
    match (issuer_id, key_id, key_path) {
        (Some(issuer_id), Some(key_id), Some(key_path)) => Ok(AppStoreCreds {
            issuer_id,
            key_id,
            key_path,
        }),
        _ => bail!(
            "App Store Connect credentials not found. Run `lingxia store login --platform ios`, \
             or set LINGXIA_ASC_ISSUER_ID / _KEY_ID / _KEY_PATH."
        ),
    }
}

/// Resolve AppGallery credentials: env (`LINGXIA_AGC_*`) over file.
pub fn resolve_appgallery(file: &StoreCredentials) -> Result<AppGalleryCreds> {
    let f = file.appgallery.as_ref();
    let client_id =
        env_nonempty("LINGXIA_AGC_CLIENT_ID").or_else(|| f.map(|c| c.client_id.clone()));
    let client_secret =
        env_nonempty("LINGXIA_AGC_CLIENT_SECRET").or_else(|| f.map(|c| c.client_secret.clone()));
    match (client_id, client_secret) {
        (Some(client_id), Some(client_secret)) => Ok(AppGalleryCreds {
            client_id,
            client_secret,
        }),
        _ => bail!(
            "AppGallery credentials not found. Run `lingxia store login --platform harmony`, \
             or set LINGXIA_AGC_CLIENT_ID / _CLIENT_SECRET."
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> StoreCredentials {
        StoreCredentials {
            msstore: Some(MsStoreCreds {
                tenant: "t".into(),
                client_id: "c".into(),
                client_secret: "s".into(),
                seller_id: None,
            }),
            appstore: None,
            appgallery: None,
        }
    }

    #[test]
    fn toml_roundtrips() {
        let text = toml::to_string_pretty(&sample()).unwrap();
        let back: StoreCredentials = toml::from_str(&text).unwrap();
        assert_eq!(back.msstore, sample().msstore);
    }

    #[test]
    fn resolve_uses_file_when_no_env() {
        // Guard against a polluted env in the test runner.
        for k in [
            "LINGXIA_MSSTORE_TENANT",
            "LINGXIA_MSSTORE_CLIENT_ID",
            "LINGXIA_MSSTORE_CLIENT_SECRET",
        ] {
            unsafe { std::env::remove_var(k) };
        }
        let creds = resolve_msstore(&sample()).unwrap();
        assert_eq!(creds.tenant, "t");
    }

    #[test]
    fn resolve_missing_errors() {
        for k in [
            "LINGXIA_ASC_ISSUER_ID",
            "LINGXIA_ASC_KEY_ID",
            "LINGXIA_ASC_KEY_PATH",
        ] {
            unsafe { std::env::remove_var(k) };
        }
        assert!(resolve_appstore(&StoreCredentials::default()).is_err());
    }

    #[test]
    fn load_missing_file_is_empty() {
        let dir = std::env::temp_dir().join(format!("lx-store-test-{}", std::process::id()));
        let path = dir.join("nope.toml");
        let creds = StoreCredentials::load_from(&path).unwrap();
        assert!(creds.msstore.is_none() && creds.appstore.is_none());
    }
}
