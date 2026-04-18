use super::agc::{
    AgcApiCredentials, AgcConnectClient, AgcToken, AppIdInfo, CertInfo, CreateProfileParams,
    ProvisionInfo,
};
use super::credentials::AgcCredentialStorage;
use super::keygen::{self, CsrSubject};
use super::signer::{SignAlgorithm, SigningConfig};
use crate::permission_cache::{PermissionCache, PermissionPlatform};
use anyhow::{Context, Result, anyhow};
use base64::Engine as _;
use openssl::pkcs12::Pkcs12;
use openssl::pkey::PKey;
use openssl::x509::X509;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SigningMode {
    Debug,
    Release,
}

impl SigningMode {
    pub fn cert_type(self) -> i32 {
        match self {
            Self::Debug => 1,
            Self::Release => 2,
        }
    }

    pub fn provision_type(self) -> i32 {
        match self {
            Self::Debug => 1,
            Self::Release => 2,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Release => "release",
        }
    }
}

const PKCS12_ALIAS: &str = "lingxiaHarmonyKey";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SigningState {
    cert_id: Option<String>,
    profile_id: Option<String>,
    keystore_password: Option<String>,
}

#[derive(Debug, Clone)]
struct SigningPaths {
    key_path: PathBuf,
    csr_path: PathBuf,
    cert_path: PathBuf,
    profile_path: PathBuf,
    keystore_path: PathBuf,
    state_path: PathBuf,
}

pub struct ProvisioningManager {
    client: AgcConnectClient,
    credentials: AgcApiCredentials,
    storage: AgcCredentialStorage,
}

impl ProvisioningManager {
    pub fn from_storage() -> Result<Self> {
        let api_storage = AgcCredentialStorage::new()?;
        let credentials = api_storage.load()?.ok_or_else(|| {
            anyhow!(
                "Harmony AGC API credentials are missing. Run `lingxia auth harmony login --mode api` first.\n\
                 Use AGC Connect API client credentials and set Project to `N/A`."
            )
        })?;

        Ok(Self {
            client: AgcConnectClient::new(),
            credentials,
            storage: api_storage,
        })
    }

    pub fn prepare_signing_config(
        &mut self,
        bundle_name: &str,
        mode: SigningMode,
        target_udids: &[String],
        acl_permissions: &[String],
    ) -> Result<SigningConfig> {
        let app = self
            .ensure_app_id(bundle_name)
            .with_context(|| format!("Failed to resolve appId for bundle `{bundle_name}`"))?;

        let paths = signing_paths(bundle_name, mode)?;
        if let Some(parent) = paths.state_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }

        let mut state = load_signing_state(&paths.state_path)?;
        let (private_key_pem, csr_pem) = ensure_local_key_material(&paths, bundle_name, mode)?;
        let device_ids = self.ensure_devices(mode, target_udids)?;
        let mut cert = self.ensure_certificate(mode, &private_key_pem, &csr_pem, &mut state)?;
        let mut cert_bytes = self
            .download_signed_asset(&cert.cert_download_url)
            .context("Failed to download certificate file")?;

        if !private_key_matches_certificate(&private_key_pem, &cert_bytes)? {
            // Cached certificate may belong to an old private key.
            // Clear cert/profile ids and re-create a matching certificate once.
            state.cert_id = None;
            state.profile_id = None;
            cert = self.ensure_certificate(mode, &private_key_pem, &csr_pem, &mut state)?;
            cert_bytes = self
                .download_signed_asset(&cert.cert_download_url)
                .context("Failed to download certificate file")?;
        }

        if !private_key_matches_certificate(&private_key_pem, &cert_bytes)? {
            return Err(anyhow!(
                "AGC certificate `{}` does not match local private key even after cert reset. Please clear `~/.lingxia/harmony/signing/{}` and retry.",
                cert.id,
                sanitize_for_path(bundle_name)
            ));
        }
        std::fs::write(&paths.cert_path, &cert_bytes)
            .with_context(|| format!("Failed to write {}", paths.cert_path.display()))?;

        let profile =
            self.ensure_profile(mode, &app, &cert, &device_ids, acl_permissions, &mut state)?;
        update_permission_cache(
            PermissionPlatform::Harmony,
            bundle_name,
            &profile.acl_permissions,
        );
        let profile_bytes = self
            .download_signed_asset(&profile.provision_download_url)
            .context("Failed to download provisioning profile")?;
        std::fs::write(&paths.profile_path, &profile_bytes)
            .with_context(|| format!("Failed to write {}", paths.profile_path.display()))?;

        let keystore_password = state
            .keystore_password
            .clone()
            .unwrap_or_else(generate_keystore_password);
        state.keystore_password = Some(keystore_password.clone());

        write_pkcs12(
            &private_key_pem,
            &cert_bytes,
            &keystore_password,
            &paths.keystore_path,
        )
        .context("Failed to build local PKCS#12 keystore")?;

        save_signing_state(&paths.state_path, &state)?;

        Ok(SigningConfig {
            keystore_path: paths.keystore_path,
            keystore_password: keystore_password.clone(),
            key_password: Some(keystore_password),
            cert_path: paths.cert_path,
            profile_path: paths.profile_path,
            sign_algorithm: SignAlgorithm::SHA256withECDSA,
        })
    }

    fn ensure_valid_token(
        client: &AgcConnectClient,
        credentials: &mut AgcApiCredentials,
        storage: &AgcCredentialStorage,
    ) -> Result<AgcToken> {
        let token = client.ensure_valid_token(credentials)?;
        let changed = credentials.token.as_ref().is_none_or(|old| {
            old.access_token != token.access_token || old.expires_at != token.expires_at
        });
        if changed {
            credentials.token = Some(token.clone());
            storage
                .save(credentials)
                .context("Failed to persist refreshed AGC token")?;
        }
        Ok(token)
    }

    fn ensure_app_id(&mut self, bundle_name: &str) -> Result<AppIdInfo> {
        let token = Self::ensure_valid_token(&self.client, &mut self.credentials, &self.storage)?;
        self.client
            .find_app_id_by_package_name(&token, bundle_name)?
            .ok_or_else(|| {
            anyhow!(
                "AppId not found for bundle `{}` in AppGallery Connect.\nCreate it in AGC first, then rerun.",
                bundle_name
            )
        })
    }

    fn ensure_devices(
        &mut self,
        mode: SigningMode,
        target_udids: &[String],
    ) -> Result<Vec<String>> {
        if mode == SigningMode::Release {
            return Ok(Vec::new());
        }

        let unique_udids = unique_values(target_udids);
        if unique_udids.is_empty() {
            return Ok(Vec::new());
        }

        let token = Self::ensure_valid_token(&self.client, &mut self.credentials, &self.storage)?;
        let mut current = self.client.query_devices(&token, None)?;
        let mut device_ids = Vec::with_capacity(unique_udids.len());
        for udid in &unique_udids {
            if let Some(existing) = current.iter().find(|d| d.udid == *udid) {
                device_ids.push(existing.id.clone());
                continue;
            }

            let name = build_device_name(udid);
            self.client
                .add_device(&token, &name, udid)
                .with_context(|| format!("Failed to register Harmony device {udid}"))?;
            current = self.client.query_devices(&token, None)?;
            let created = current
                .iter()
                .find(|d| d.udid == *udid)
                .ok_or_else(|| anyhow!("Failed to load device id for {udid} after registration"))?;
            device_ids.push(created.id.clone());
        }
        Ok(device_ids)
    }

    fn ensure_certificate(
        &mut self,
        mode: SigningMode,
        private_key_pem: &str,
        csr_pem: &str,
        state: &mut SigningState,
    ) -> Result<CertInfo> {
        let cert_type = mode.cert_type();
        let token = Self::ensure_valid_token(&self.client, &mut self.credentials, &self.storage)?;
        let certs = self.client.query_certificates(&token, cert_type)?;

        if let Some(cert_id) = state.cert_id.as_ref()
            && let Some(cert) = certs.iter().find(|c| c.id == *cert_id)
        {
            return Ok(cert.clone());
        }

        for cert in &certs {
            let Ok(cert_bytes) = self
                .client
                .download_signed_asset(&token, &cert.cert_download_url)
            else {
                continue;
            };
            if private_key_matches_certificate(private_key_pem, &cert_bytes).unwrap_or(false) {
                state.cert_id = Some(cert.id.clone());
                return Ok(cert.clone());
            }
        }

        let create_result =
            self.client
                .create_certificate(&token, csr_pem, mode == SigningMode::Debug);
        let cert = match create_result {
            Ok(cert) => cert,
            Err(err) if is_certificate_limit_error(&err) => {
                let reclaimed = reclaim_api_certificate_slot(&self.client, &token, &certs, state)?;
                if !reclaimed {
                    return Err(err).with_context(|| {
                        format!("Failed to create {} certificate in AGC", mode.as_str())
                    });
                }
                self.client
                    .create_certificate(&token, csr_pem, mode == SigningMode::Debug)
                    .with_context(|| {
                        format!("Failed to create {} certificate in AGC", mode.as_str())
                    })?
            }
            Err(err) => {
                return Err(err).with_context(|| {
                    format!("Failed to create {} certificate in AGC", mode.as_str())
                });
            }
        };
        state.cert_id = Some(cert.id.clone());
        Ok(cert)
    }

    fn ensure_profile(
        &mut self,
        mode: SigningMode,
        app: &AppIdInfo,
        cert: &CertInfo,
        required_device_ids: &[String],
        required_acl_permissions: &[String],
        state: &mut SigningState,
    ) -> Result<ProvisionInfo> {
        let mut load_or_create = |profiles: Vec<ProvisionInfo>,
                                  create_profile: &mut dyn FnMut() -> Result<ProvisionInfo>|
         -> Result<ProvisionInfo> {
            if let Some(profile_id) = state.profile_id.as_ref()
                && let Some(existing) = profiles.iter().find(|p| p.id == *profile_id)
                && profile_matches(
                    existing,
                    app,
                    cert,
                    required_device_ids,
                    required_acl_permissions,
                )
            {
                return Ok(existing.clone());
            }

            if let Some(existing) = profiles.iter().find(|profile| {
                profile_matches(
                    profile,
                    app,
                    cert,
                    required_device_ids,
                    required_acl_permissions,
                )
            }) {
                state.profile_id = Some(existing.id.clone());
                return Ok(existing.clone());
            }

            if mode == SigningMode::Debug && required_device_ids.is_empty() {
                return Err(anyhow!(
                    "No debug profile found for `{}` and no connected device was provided.\nConnect a device (or pass --device) so LingXia can create a debug profile.",
                    app.package_name
                ));
            }

            let created = create_profile();
            let profile = match created {
                Ok(profile) => profile,
                Err(err) => {
                    if let Some(existing) = profiles.iter().find(|profile| {
                        profile_matches(
                            profile,
                            app,
                            cert,
                            required_device_ids,
                            required_acl_permissions,
                        )
                    }) {
                        existing.clone()
                    } else {
                        return Err(err);
                    }
                }
            };
            state.profile_id = Some(profile.id.clone());
            Ok(profile)
        };

        let profile_name = profile_name_for(&app.package_name, mode);
        let token = Self::ensure_valid_token(&self.client, &mut self.credentials, &self.storage)?;
        let profiles =
            self.client
                .query_profiles(&token, mode.provision_type(), Some(&app.app_id))?;
        load_or_create(profiles, &mut || {
            self.client
                .create_profile(
                    &token,
                    CreateProfileParams {
                        name: profile_name.clone(),
                        app_id: app.app_id.clone(),
                        cert_id: cert.id.clone(),
                        device_ids: required_device_ids.to_vec(),
                        acl_permissions: required_acl_permissions.to_vec(),
                        is_debug: mode == SigningMode::Debug,
                    },
                )
                .with_context(|| {
                    format!(
                        "Failed to create {} profile `{}`",
                        mode.as_str(),
                        profile_name
                    )
                })
        })
    }

    fn download_signed_asset(&mut self, target: &str) -> Result<Vec<u8>> {
        let token = Self::ensure_valid_token(&self.client, &mut self.credentials, &self.storage)?;
        self.client.download_signed_asset(&token, target)
    }
}

fn update_permission_cache(platform: PermissionPlatform, app_id: &str, permissions: &[String]) {
    let Ok(mut cache) = PermissionCache::load() else {
        return;
    };
    cache.set(platform, app_id, permissions);
    let _ = cache.save();
}

fn signing_paths(bundle_name: &str, mode: SigningMode) -> Result<SigningPaths> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not determine home directory"))?;
    let root = home
        .join(".lingxia")
        .join("harmony")
        .join("signing")
        .join(sanitize_for_path(bundle_name))
        .join(mode.as_str());

    Ok(SigningPaths {
        key_path: root.join("signing.key.pem"),
        csr_path: root.join("signing.csr.pem"),
        cert_path: root.join("signing.cer"),
        profile_path: root.join("signing.p7b"),
        keystore_path: root.join("signing.p12"),
        state_path: root.join("state.json"),
    })
}

fn ensure_local_key_material(
    paths: &SigningPaths,
    bundle_name: &str,
    mode: SigningMode,
) -> Result<(String, String)> {
    if paths.key_path.exists() && paths.csr_path.exists() {
        let key_pem = std::fs::read_to_string(&paths.key_path)
            .with_context(|| format!("Failed to read {}", paths.key_path.display()))?;
        let csr_pem = std::fs::read_to_string(&paths.csr_path)
            .with_context(|| format!("Failed to read {}", paths.csr_path.display()))?;
        return Ok((key_pem, csr_pem));
    }

    let parent = paths
        .key_path
        .parent()
        .ok_or_else(|| anyhow!("Invalid signing key path"))?;
    std::fs::create_dir_all(parent)
        .with_context(|| format!("Failed to create {}", parent.display()))?;

    let subject = CsrSubject {
        common_name: format!("LingXia {} {}", mode.as_str(), bundle_name),
        organization: "LingXia".to_string(),
        country: "CN".to_string(),
    };
    let (key_pem, csr_pem) = keygen::generate_ec_csr(&subject)?;
    std::fs::write(&paths.key_path, &key_pem)
        .with_context(|| format!("Failed to write {}", paths.key_path.display()))?;
    std::fs::write(&paths.csr_path, &csr_pem)
        .with_context(|| format!("Failed to write {}", paths.csr_path.display()))?;
    Ok((key_pem, csr_pem))
}

fn load_signing_state(path: &Path) -> Result<SigningState> {
    if !path.exists() {
        return Ok(SigningState::default());
    }

    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("Failed to parse {}", path.display()))
}

fn save_signing_state(path: &Path, state: &SigningState) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("Invalid signing state path: {}", path.display()))?;
    std::fs::create_dir_all(parent)
        .with_context(|| format!("Failed to create {}", parent.display()))?;
    let json = serde_json::to_string_pretty(state).context("Failed to serialize signing state")?;
    std::fs::write(path, json).with_context(|| format!("Failed to write {}", path.display()))
}

fn private_key_matches_certificate(private_key_pem: &str, cert_bytes: &[u8]) -> Result<bool> {
    let private_key = PKey::private_key_from_pem(private_key_pem.as_bytes())
        .context("Invalid local signing private key")?;
    let certs = parse_x509_chain(cert_bytes)?;

    for cert in certs {
        let cert_key = cert
            .public_key()
            .context("Failed to read public key from AGC certificate")?;
        if private_key.public_eq(&cert_key) {
            return Ok(true);
        }
    }

    Ok(false)
}

fn parse_x509_chain(cert_bytes: &[u8]) -> Result<Vec<X509>> {
    if cert_bytes.starts_with(b"-----BEGIN") {
        let certs = X509::stack_from_pem(cert_bytes).context("Invalid PEM certificate chain")?;
        if certs.is_empty() {
            return Err(anyhow!("PEM certificate chain is empty"));
        }
        Ok(certs)
    } else {
        Ok(vec![
            X509::from_der(cert_bytes).context("Invalid DER certificate")?,
        ])
    }
}

fn write_pkcs12(
    private_key_pem: &str,
    cert_bytes: &[u8],
    password: &str,
    output_path: &Path,
) -> Result<()> {
    let private_key = PKey::private_key_from_pem(private_key_pem.as_bytes())
        .context("Invalid local signing key")?;
    let certs = parse_x509_chain(cert_bytes).context("Invalid signing certificate chain")?;
    let cert = certs
        .iter()
        .find(|candidate| {
            candidate
                .public_key()
                .map(|pubkey| private_key.public_eq(&pubkey))
                .unwrap_or(false)
        })
        .cloned()
        .ok_or_else(|| anyhow!("No certificate in chain matches local signing private key"))?;

    let mut builder = Pkcs12::builder();
    builder.name(PKCS12_ALIAS).pkey(&private_key).cert(&cert);
    let p12 = builder
        .build2(password)
        .context("Failed to create PKCS#12 archive")?;
    let der = p12.to_der().context("Failed to encode PKCS#12 archive")?;

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    std::fs::write(output_path, der)
        .with_context(|| format!("Failed to write {}", output_path.display()))
}

fn generate_keystore_password() -> String {
    let mut bytes = [0u8; 24];
    rand::rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn build_device_name(udid: &str) -> String {
    let suffix = udid.chars().take(8).collect::<String>();
    format!("LingXia-{suffix}")
}

fn unique_values(values: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for value in values {
        if !out.iter().any(|existing| existing == value) {
            out.push(value.clone());
        }
    }
    out
}

fn profile_matches(
    profile: &ProvisionInfo,
    app: &AppIdInfo,
    cert: &CertInfo,
    required_device_ids: &[String],
    required_acl_permissions: &[String],
) -> bool {
    if profile.app_id != app.app_id {
        return false;
    }
    if !profile.cert_id.is_empty() {
        if profile.cert_id != cert.id {
            return false;
        }
    } else if !profile.cert_name.is_empty() {
        if profile.cert_name != cert.cert_name {
            return false;
        }
    } else {
        return false;
    }

    required_device_ids
        .iter()
        .all(|id| profile.device_ids.iter().any(|d| d == id))
        && acl_permissions_match(&profile.acl_permissions, required_acl_permissions)
}

fn acl_permissions_match(existing: &[String], required: &[String]) -> bool {
    use std::collections::HashSet;

    let existing_set = existing
        .iter()
        .map(|permission| permission.trim())
        .filter(|permission| !permission.is_empty())
        .collect::<HashSet<_>>();
    let required_set = required
        .iter()
        .map(|permission| permission.trim())
        .filter(|permission| !permission.is_empty())
        .collect::<HashSet<_>>();

    required_set.is_subset(&existing_set)
}

fn profile_name_for(bundle_name: &str, mode: SigningMode) -> String {
    use std::fmt::Write as _;

    let compact_bundle = sanitize_for_path(bundle_name);
    let base = format!("LingXia_{compact_bundle}_{}", mode.as_str());
    if base.len() <= 64 {
        return base;
    }

    let mut hasher = Sha256::new();
    hasher.update(base.as_bytes());
    let digest = hasher.finalize();
    let mut digest_hex = String::with_capacity(64);
    for byte in digest {
        write!(&mut digest_hex, "{byte:02x}").expect("writing digest to String should not fail");
    }

    format!("LingXia_{}_{}", mode.as_str(), &digest_hex[..32])
}

fn sanitize_for_path(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    out
}

fn is_certificate_limit_error(err: &anyhow::Error) -> bool {
    let msg = err.to_string().to_ascii_lowercase();
    msg.contains("certificate number exceeds limit") || msg.contains("exceeds limit")
}

fn reclaim_api_certificate_slot(
    client: &AgcConnectClient,
    token: &AgcToken,
    certs: &[CertInfo],
    state: &mut SigningState,
) -> Result<bool> {
    let candidate = certificate_reclaim_candidate(certs, state.cert_id.as_deref());
    let Some(candidate) = candidate else {
        return Ok(false);
    };

    client
        .delete_certificates(token, vec![candidate.id.clone()])
        .with_context(|| {
            format!(
                "Failed to delete old LingXia certificate `{}`",
                candidate.id
            )
        })?;

    state.cert_id = None;
    state.profile_id = None;

    Ok(true)
}

fn certificate_reclaim_candidate<'a>(
    certs: &'a [CertInfo],
    state_cert_id: Option<&str>,
) -> Option<&'a CertInfo> {
    if let Some(state_cert_id) = state_cert_id
        && let Some(cert) = certs.iter().find(|cert| cert.id == state_cert_id)
    {
        return Some(cert);
    }

    certs
        .iter()
        .filter(|cert| is_lingxia_managed_certificate_name(&cert.cert_name))
        .min_by(|left, right| {
            left.cert_name
                .cmp(&right.cert_name)
                .then_with(|| left.id.cmp(&right.id))
        })
}

fn is_lingxia_managed_certificate_name(name: &str) -> bool {
    (name.starts_with("lingxia_debug_") || name.starts_with("lingxia_release_"))
        && name.ends_with(".cer")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cert(id: &str, name: &str) -> CertInfo {
        CertInfo {
            id: id.to_string(),
            cert_name: name.to_string(),
            cert_type: 1,
            cert_download_url: format!("https://example.com/{id}.cer"),
        }
    }

    #[test]
    fn reclaim_candidate_prefers_state_certificate() {
        let certs = vec![
            cert("old", "lingxia_debug_20240101000000.cer"),
            cert("state", "custom_user_cert.cer"),
        ];

        let candidate = certificate_reclaim_candidate(&certs, Some("state")).unwrap();

        assert_eq!(candidate.id, "state");
    }

    #[test]
    fn reclaim_candidate_uses_oldest_lingxia_managed_certificate() {
        let certs = vec![
            cert("user", "custom_user_cert.cer"),
            cert("new", "lingxia_debug_20250101000000.cer"),
            cert("old", "lingxia_debug_20240101000000.cer"),
        ];

        let candidate = certificate_reclaim_candidate(&certs, None).unwrap();

        assert_eq!(candidate.id, "old");
    }

    #[test]
    fn reclaim_candidate_ignores_user_certificates_without_state() {
        let certs = vec![cert("user", "custom_user_cert.cer")];

        assert!(certificate_reclaim_candidate(&certs, None).is_none());
    }
}
