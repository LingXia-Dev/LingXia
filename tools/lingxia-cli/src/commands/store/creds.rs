//! OS-store credential resolution: canonical env group → wallet.
//!
//! Each provider's env group must be complete or absent — a partial group is
//! a hard error and is never mixed with wallet credentials. Wallet slots are
//! identity-keyed (`credentials/stores/<provider>/<identity>/`), written by
//! `lingxia auth login <provider>`.

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use crate::binding::BindingStore;
use crate::resolver::{self, SingleIdentityInput, codes};
use crate::wallet::Wallet;

/// Microsoft Store (Partner Center) — Azure AD client credentials.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MsStoreCreds {
    pub tenant: String,
    pub client_id: String,
    pub client_secret: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seller_id: Option<String>,
}

/// Google Play Developer API — service account. Either point at the JSON key
/// file, or inline its `client_email` + `private_key` (PKCS#8 PEM).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GooglePlayCreds {
    /// Path to the service-account JSON key file (`~` is expanded). When set,
    /// `client_email`/`private_key` are loaded from it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_account_json: Option<String>,
    /// Service-account email (`client_email` field of the JSON key).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_email: Option<String>,
    /// Service-account RSA private key, PKCS#8 PEM (`private_key` field).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_key: Option<String>,
}

/// Xiaomi GetApps open platform — client id/key + secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct XiaomiCreds {
    pub client_id: String,
    pub client_secret: String,
}

/// OPPO open platform — client id/key + secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OppoCreds {
    pub client_id: String,
    pub client_secret: String,
}

/// Honor Developer open platform — client id/key + secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HonorCreds {
    pub client_id: String,
    pub client_secret: String,
}

fn env_nonempty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// A complete-or-absent env group: `Some` values when all are set, `None`
/// when none are, an `CREDENTIAL_ENV_INCOMPLETE` error otherwise.
fn env_group<const N: usize>(keys: [&str; N]) -> Result<Option<[String; N]>> {
    let values: Vec<Option<String>> = keys.iter().map(|k| env_nonempty(k)).collect();
    if values.iter().all(Option::is_none) {
        return Ok(None);
    }
    if values.iter().all(Option::is_some) {
        let values: Vec<String> = values.into_iter().map(Option::unwrap).collect();
        return Ok(Some(values.try_into().expect("length preserved")));
    }
    let missing: Vec<&str> = keys
        .iter()
        .zip(&values)
        .filter(|(_, v)| v.is_none())
        .map(|(k, _)| *k)
        .collect();
    bail!(
        "{}: the credential env group must be complete; missing: {}",
        codes::CREDENTIAL_ENV_INCOMPLETE,
        missing.join(", ")
    );
}

/// Resolve the wallet identity for one store provider (binding cache → sole →
/// one interactive selection), with in-place login when nothing is stored.
fn wallet_identity(
    provider: &'static str,
    label: &'static str,
    inline_login: Option<&dyn Fn() -> Result<String>>,
) -> Result<String> {
    let wallet = Wallet::open()?;
    wallet.notice_legacy_files();
    let identities = wallet.store_identities(provider)?;
    let project = resolver::detect_project()?;
    let bindings = BindingStore::open()?;
    let login_cmd = format!("lingxia auth login {provider}");
    resolver::resolve_single_identity(&SingleIdentityInput {
        provider,
        label,
        login_cmd: &login_cmd,
        identities: &identities,
        bindings: &bindings,
        binding_key: project.as_ref().map(|p| (p.root.as_path(), provider)),
        interactive: resolver::is_interactive(),
        inline_login,
    })
}

fn wallet_creds<T: serde::de::DeserializeOwned>(
    provider: &'static str,
    label: &'static str,
    inline_login: Option<&dyn Fn() -> Result<String>>,
) -> Result<T> {
    let identity = wallet_identity(provider, label, inline_login)?;
    Wallet::open()?
        .load_store_creds(provider, &identity)?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "{}: credentials for {label} {identity} disappeared during resolution",
                codes::CREDENTIALS_MISSING
            )
        })
}

/// Resolve Microsoft Store credentials: env group (`LINGXIA_MSSTORE_*`) → wallet.
pub fn resolve_msstore() -> Result<MsStoreCreds> {
    if let Some([tenant, client_id, client_secret]) = env_group([
        "LINGXIA_MSSTORE_TENANT",
        "LINGXIA_MSSTORE_CLIENT_ID",
        "LINGXIA_MSSTORE_CLIENT_SECRET",
    ])? {
        return Ok(MsStoreCreds {
            tenant,
            client_id,
            client_secret,
            seller_id: env_nonempty("LINGXIA_MSSTORE_SELLER_ID"),
        });
    }
    wallet_creds(
        "msstore",
        "Microsoft Store identity",
        Some(&|| crate::commands::auth::store_inline_login("msstore")),
    )
}

/// Resolve Google Play credentials: env (`LINGXIA_GPLAY_*`, JSON path or the
/// inline email + key pair) → wallet.
pub fn resolve_googleplay() -> Result<GooglePlayCreds> {
    if let Some([path]) = env_group(["LINGXIA_GPLAY_SERVICE_ACCOUNT_JSON"])? {
        return Ok(GooglePlayCreds {
            service_account_json: Some(path),
            client_email: None,
            private_key: None,
        });
    }
    if let Some([email, key]) =
        env_group(["LINGXIA_GPLAY_CLIENT_EMAIL", "LINGXIA_GPLAY_PRIVATE_KEY"])?
    {
        return Ok(GooglePlayCreds {
            service_account_json: None,
            client_email: Some(email),
            private_key: Some(key),
        });
    }
    wallet_creds(
        "googleplay",
        "Google Play service account",
        Some(&|| crate::commands::auth::store_inline_login("googleplay")),
    )
}

/// Resolve Xiaomi credentials: env group (`LINGXIA_XIAOMI_*`) → wallet.
pub fn resolve_xiaomi() -> Result<XiaomiCreds> {
    if let Some([client_id, client_secret]) =
        env_group(["LINGXIA_XIAOMI_CLIENT_ID", "LINGXIA_XIAOMI_CLIENT_SECRET"])?
    {
        return Ok(XiaomiCreds {
            client_id,
            client_secret,
        });
    }
    wallet_creds(
        "xiaomi",
        "Xiaomi GetApps identity",
        Some(&|| crate::commands::auth::store_inline_login("xiaomi")),
    )
}

/// Resolve OPPO credentials: env group (`LINGXIA_OPPO_*`) → wallet.
pub fn resolve_oppo() -> Result<OppoCreds> {
    if let Some([client_id, client_secret]) =
        env_group(["LINGXIA_OPPO_CLIENT_ID", "LINGXIA_OPPO_CLIENT_SECRET"])?
    {
        return Ok(OppoCreds {
            client_id,
            client_secret,
        });
    }
    wallet_creds(
        "oppo",
        "OPPO store identity",
        Some(&|| crate::commands::auth::store_inline_login("oppo")),
    )
}

/// Resolve Honor credentials: env group (`LINGXIA_HONOR_*`) → wallet.
pub fn resolve_honor() -> Result<HonorCreds> {
    if let Some([client_id, client_secret]) =
        env_group(["LINGXIA_HONOR_CLIENT_ID", "LINGXIA_HONOR_CLIENT_SECRET"])?
    {
        return Ok(HonorCreds {
            client_id,
            client_secret,
        });
    }
    wallet_creds(
        "honor",
        "Honor store identity",
        Some(&|| crate::commands::auth::store_inline_login("honor")),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_group_complete_or_absent() {
        // Absent group resolves to None.
        unsafe {
            std::env::remove_var("LX_TEST_A");
            std::env::remove_var("LX_TEST_B");
        }
        assert!(env_group(["LX_TEST_A", "LX_TEST_B"]).unwrap().is_none());

        // Partial group is a hard error naming the missing keys.
        unsafe { std::env::set_var("LX_TEST_A", "x") };
        let err = env_group(["LX_TEST_A", "LX_TEST_B"])
            .unwrap_err()
            .to_string();
        assert!(err.starts_with(codes::CREDENTIAL_ENV_INCOMPLETE));
        assert!(err.contains("LX_TEST_B") && !err.contains("LX_TEST_A,"));

        // Complete group returns the values in order.
        unsafe { std::env::set_var("LX_TEST_B", "y") };
        let [a, b] = env_group(["LX_TEST_A", "LX_TEST_B"]).unwrap().unwrap();
        assert_eq!((a.as_str(), b.as_str()), ("x", "y"));
        unsafe {
            std::env::remove_var("LX_TEST_A");
            std::env::remove_var("LX_TEST_B");
        }
    }
}
