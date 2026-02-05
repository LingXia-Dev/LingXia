//! Provisioning orchestration for iOS code signing.
//!
//! This module coordinates the complete provisioning workflow:
//! 1. Register device with Apple Developer Portal
//! 2. Obtain or create a development certificate
//! 3. Create or update App ID
//! 4. Generate provisioning profile
//! 5. Sign the app bundle
//!
//! Uses a temporary keychain to avoid password prompts.

use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::NamedTempFile;

use super::asc::AppStoreConnectClient;
use super::auth::{AuthCredentials, CredentialStorage};
use super::developer_services;
use super::developer_services::DeveloperServicesClient;
use super::devicectl::{ConnectedDevice, DeviceCtl};
use super::grandslam::DeviceInfo;
use super::keychain::{KeychainManager, generate_csr};
use super::signer::{Signer, extract_entitlements_from_profile};

/// Result of the provisioning process
#[derive(Debug)]
pub struct ProvisioningResult {
    /// The signing identity (certificate SHA-1 fingerprint)
    pub signing_identity: String,
    /// The provisioning profile data (mobileprovision)
    pub profile_data: Vec<u8>,
    /// The new bundle ID (may be different from original for free accounts)
    pub bundle_id: String,
    /// Entitlements extracted from the profile
    pub entitlements: Vec<u8>,
    /// Certificate data (DER format) + private key (PEM format).
    /// Present when a fresh certificate was created; absent when reusing a Keychain identity.
    pub identity_material: Option<IdentityMaterial>,
}

/// Fresh certificate + private key for signing.
#[derive(Debug)]
pub struct IdentityMaterial {
    pub cert_data: Vec<u8>,
    pub private_key: String,
}

/// Temporary keychain for signing operations
pub struct TempKeychain {
    path: PathBuf,
    password: String,
}

impl TempKeychain {
    /// Create a new temporary keychain
    pub fn new() -> Result<Self> {
        let name = format!("lingxia-{}", std::process::id());
        let path = std::env::temp_dir().join(format!("{}.keychain-db", name));
        let password = "lingxia-signing";

        // Delete existing keychain if it exists
        if path.exists() {
            let _ = Command::new("security")
                .args(["delete-keychain", path.to_str().unwrap()])
                .output();
        }

        // Create new keychain
        let status = Command::new("security")
            .args(["create-keychain", "-p", password, path.to_str().unwrap()])
            .status()
            .context("Failed to create temporary keychain")?;

        if !status.success() {
            return Err(anyhow!("Failed to create temporary keychain"));
        }

        // Unlock the keychain
        let _ = Command::new("security")
            .args(["unlock-keychain", "-p", password, path.to_str().unwrap()])
            .status();

        // Set keychain settings (no auto-lock, no timeout)
        let _ = Command::new("security")
            .args(["set-keychain-settings", path.to_str().unwrap()])
            .output();

        // Add to search list so codesign can find it
        let output = Command::new("security")
            .args(["list-keychains", "-d", "user"])
            .output()
            .context("Failed to list keychains")?;

        let existing = String::from_utf8_lossy(&output.stdout);
        let mut keychains: Vec<String> = existing
            .lines()
            .map(|l| l.trim().trim_matches('"').to_string())
            .filter(|s| !s.is_empty())
            .collect();

        keychains.insert(0, path.to_str().unwrap().to_string());

        let _ = Command::new("security")
            .arg("list-keychains")
            .arg("-d")
            .arg("user")
            .arg("-s")
            .args(&keychains)
            .output();

        Ok(Self {
            path,
            password: password.to_string(),
        })
    }

    /// Get the keychain path
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Import a P12 file into the keychain
    pub fn import_p12(&self, p12_path: &Path, p12_password: &str) -> Result<()> {
        self.security_import(p12_path, Some(p12_password))
            .context("Failed to import P12 into keychain")?;
        self.configure_codesign_access();
        Ok(())
    }

    /// Import a certificate (DER) + private key (PEM) into the keychain.
    pub fn import_identity(&self, cert_der: &[u8], key_pem: &str) -> Result<()> {
        use std::io::Write;

        let mut cert_file = NamedTempFile::new().context("Failed to create cert temp file")?;
        cert_file
            .write_all(cert_der)
            .context("Failed to write certificate")?;

        let mut key_file = NamedTempFile::new().context("Failed to create key temp file")?;
        key_file
            .write_all(key_pem.as_bytes())
            .context("Failed to write private key")?;

        self.security_import(cert_file.path(), None)
            .context("Failed to import certificate into keychain")?;
        self.security_import(key_file.path(), None)
            .context("Failed to import private key into keychain")?;

        self.configure_codesign_access();
        Ok(())
    }

    fn security_import(&self, path: &Path, password: Option<&str>) -> Result<()> {
        let mut cmd = Command::new("security");
        cmd.arg("import")
            .arg(path.to_str().unwrap())
            .arg("-k")
            .arg(self.path.to_str().unwrap())
            .args(["-T", "/usr/bin/codesign"])
            .args(["-T", "/usr/bin/security"]);

        if let Some(pw) = password {
            cmd.arg("-P").arg(pw);
        }

        let output = cmd.output().context("Failed to run security import")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("security import failed: {}", stderr));
        }

        Ok(())
    }

    fn configure_codesign_access(&self) {
        // Set partition list to allow codesign without prompts
        let _ = Command::new("security")
            .args([
                "set-key-partition-list",
                "-S",
                "apple-tool:,apple:,codesign:",
                "-s",
                "-k",
                &self.password,
                self.path.to_str().unwrap(),
            ])
            .output();
    }
}

impl Drop for TempKeychain {
    fn drop(&mut self) {
        let self_path = self.path.to_str().unwrap_or("");

        // Remove this keychain from the search list
        let output = Command::new("security")
            .args(["list-keychains", "-d", "user"])
            .output();

        if let Ok(output) = output {
            let existing = String::from_utf8_lossy(&output.stdout);
            let keychains: Vec<String> = existing
                .lines()
                .map(|l| l.trim().trim_matches('"').to_string())
                .filter(|s| !s.is_empty() && s != self_path)
                .collect();

            let _ = Command::new("security")
                .arg("list-keychains")
                .arg("-d")
                .arg("user")
                .arg("-s")
                .args(&keychains)
                .output();
        }

        // Delete the keychain
        let _ = Command::new("security")
            .args(["delete-keychain", self_path])
            .output();
    }
}

/// Provisioning context for iOS app signing
pub struct ProvisioningContext {
    /// Credential storage
    credentials: CredentialStorage,
    /// Device info for API calls
    device_info: DeviceInfo,
    /// Target device UDID
    target_device_udid: String,
    /// Target device name
    target_device_name: String,
}

impl ProvisioningContext {
    /// Create a new provisioning context
    pub fn new(target_device: &ConnectedDevice) -> Result<Self> {
        let credentials = CredentialStorage::new()?;
        let device_info = DeviceInfo::default_macos();

        let udid = target_device
            .udid()
            .ok_or_else(|| anyhow!("Device UDID not available"))?
            .to_string();

        let name = target_device.name().unwrap_or("iOS Device").to_string();

        Ok(Self {
            credentials,
            device_info,
            target_device_udid: udid,
            target_device_name: name,
        })
    }

    /// Create from a device UDID
    #[allow(dead_code)]
    pub fn from_udid(udid: &str) -> Result<Self> {
        let device = DeviceCtl::get_device(udid)?;
        Self::new(&device)
    }

    /// Run the complete provisioning workflow
    ///
    /// Returns the provisioning result needed for signing.
    pub fn provision(&self, original_bundle_id: &str) -> Result<ProvisioningResult> {
        println!("{}", "Starting provisioning workflow...".cyan());

        // Load credentials
        let creds = self.credentials.load()?.ok_or_else(|| {
            anyhow!("No Apple credentials found. Run 'lingxia auth login' first.")
        })?;

        match creds {
            AuthCredentials::AppleId {
                adsid,
                app_token,
                team_id,
                ..
            } => self.provision_with_apple_id(&adsid, &app_token, &team_id, original_bundle_id),
            AuthCredentials::AppStoreConnect {
                key_id,
                issuer_id,
                private_key_path,
                team_id,
            } => self.provision_with_asc(
                &key_id,
                &issuer_id,
                &private_key_path,
                &team_id,
                original_bundle_id,
            ),
        }
    }

    /// Provision using Apple ID (free or paid account via GrandSlam)
    fn provision_with_apple_id(
        &self,
        adsid: &str,
        app_token: &str,
        team_id: &str,
        original_bundle_id: &str,
    ) -> Result<ProvisioningResult> {
        // Get fresh anisette data
        let mut anisette_provider = super::anisette::OmnisetteProvider::new();
        let anisette = anisette_provider.fetch_anisette_data()?;

        // Determine whether the selected team is free (Personal Team) or paid.
        // Default to paid if the team cannot be found, since bundle-id rewriting is only
        // required for free teams and can be disruptive for paid teams.
        let teams = developer_services::list_teams(adsid, app_token, &self.device_info, &anisette)?;
        let is_free_team = teams
            .iter()
            .find(|t| t.id == team_id)
            .map(|t| t.is_free())
            .unwrap_or(false);

        let client =
            DeveloperServicesClient::new(adsid, app_token, team_id, &self.device_info, &anisette);

        // 1. Register device
        println!("{}", "Step 1/4: Registering device...".cyan());
        self.ensure_device_registered(&client)?;

        // 2. Ensure certificate
        println!("{}", "Step 2/4: Ensuring certificate...".cyan());
        let (cert_id, signing_identity, identity_material) =
            self.ensure_certificate(&client, is_free_team)?;

        // 3. Ensure App ID
        println!("{}", "Step 3/4: Ensuring App ID...".cyan());
        let (app_id_id, new_bundle_id) =
            self.ensure_app_id(&client, original_bundle_id, team_id, is_free_team)?;

        // 4. Create provisioning profile
        println!("{}", "Step 4/4: Creating provisioning profile...".cyan());
        let profile_data =
            self.create_profile(&client, &app_id_id, &cert_id, &new_bundle_id, is_free_team)?;

        // Extract entitlements
        let entitlements = extract_entitlements_from_profile(&profile_data)?;

        println!("  {} Provisioning complete", "✓".green());

        Ok(ProvisioningResult {
            signing_identity,
            profile_data,
            bundle_id: new_bundle_id,
            entitlements,
            identity_material,
        })
    }

    /// Provision using App Store Connect API (paid accounts only)
    fn provision_with_asc(
        &self,
        key_id: &str,
        issuer_id: &str,
        private_key_path: &str,
        team_id: &str,
        original_bundle_id: &str,
    ) -> Result<ProvisioningResult> {
        let client = AppStoreConnectClient::new(key_id, issuer_id, private_key_path, team_id)?;

        // 1. Register device
        println!("{}", "Step 1/4: Registering device...".cyan());
        self.ensure_device_registered_asc(&client)?;

        // 2. Ensure certificate
        println!("{}", "Step 2/4: Ensuring certificate...".cyan());
        let (cert_id, signing_identity, identity_material) =
            self.ensure_certificate_asc(&client)?;

        // 3. Ensure Bundle ID
        println!("{}", "Step 3/4: Ensuring Bundle ID...".cyan());
        let bundle_id_record = self.ensure_bundle_id_asc(&client, original_bundle_id)?;

        // 4. Create provisioning profile
        println!("{}", "Step 4/4: Creating provisioning profile...".cyan());
        let profile_data = self.create_profile_asc(&client, &bundle_id_record.id, &cert_id)?;

        // Extract entitlements
        let entitlements = extract_entitlements_from_profile(&profile_data)?;

        println!("  {} Provisioning complete", "✓".green());

        Ok(ProvisioningResult {
            signing_identity,
            profile_data,
            bundle_id: original_bundle_id.to_string(),
            entitlements,
            identity_material,
        })
    }

    // =========================================================================
    // Apple ID (Developer Services) Implementation
    // =========================================================================

    fn ensure_device_registered(&self, client: &DeveloperServicesClient) -> Result<()> {
        // Check if device is already registered
        let devices = client.list_devices()?;
        let already_registered = devices.iter().any(|d| d.udid == self.target_device_udid);

        if already_registered {
            println!("  {} Device already registered", "✓".green());
            return Ok(());
        }

        // Register the device
        client.add_device(&self.target_device_udid, &self.target_device_name)?;
        println!(
            "  {} Device registered: {}",
            "✓".green(),
            self.target_device_name
        );
        Ok(())
    }

    /// Returns (cert_id, SHA-1 fingerprint, optional identity material).
    fn ensure_certificate(
        &self,
        client: &DeveloperServicesClient,
        is_free_team: bool,
    ) -> Result<(String, String, Option<IdentityMaterial>)> {
        // For free teams (Personal Team), Apple typically allows only one iOS Development
        // certificate at a time. Creating a new one often fails with 7460, so try to reuse
        // an existing Keychain identity first.
        if is_free_team {
            if let Some(existing) = Self::try_match_existing_certificate(client)? {
                println!("  {} Using existing certificate from Keychain", "✓".green());
                return Ok((existing.0, existing.1, None));
            }
        }

        // Otherwise, try to create a fresh certificate (best for non-interactive signing).
        println!("  Creating new development certificate...");

        // Generate CSR
        let (csr_content, private_key) = generate_csr("LingXia Development")?;

        match client.submit_development_csr(&csr_content) {
            Ok(new_cert) => {
                // Decode certificate content (DER)
                let cert_content = new_cert
                    .certificate_content
                    .as_ref()
                    .ok_or_else(|| anyhow!("No certificate content in response"))?;

                let cert_data = base64_decode(cert_content)?;
                let sha1 = sha1_hex_upper(&cert_data);

                println!("  {} Created new certificate", "✓".green());
                Ok((
                    new_cert.id,
                    sha1,
                    Some(IdentityMaterial {
                        cert_data,
                        private_key,
                    }),
                ))
            }
            Err(e) => {
                let msg = e.to_string();
                if !msg.contains("failed (7460)") {
                    return Err(e);
                }

                println!(
                    "  {} Apple portal refused a new certificate (7460), trying to reuse an existing one from Keychain...",
                    "!".yellow()
                );

                if let Some(existing) = Self::try_match_existing_certificate(client)? {
                    println!("  {} Using existing certificate from Keychain", "✓".green());
                    return Ok((existing.0, existing.1, None));
                }

                Err(anyhow!(
                    "Unable to create a new iOS Development certificate (7460) and could not match an existing portal certificate to a Keychain identity.\n\
Tip: Open Xcode → Settings/Preferences → Accounts → Manage Certificates and remove/revoke the existing iOS Development certificate or clear any pending request, then retry.\n\
Tip: Ensure your login keychain contains an \"Apple Development\" identity for team {}.",
                    client.team_id
                ))
            }
        }
    }

    fn try_match_existing_certificate(
        client: &DeveloperServicesClient,
    ) -> Result<Option<(String, String)>> {
        let certs = client
            .list_certificates()
            .context("Failed to list existing certificates")?;

        let mut cert_id_by_sha1: HashMap<String, String> = HashMap::new();
        for cert in &certs {
            let Some(ref cert_content) = cert.certificate_content else {
                continue;
            };
            let der = match base64_decode(cert_content) {
                Ok(d) => d,
                Err(_) => continue,
            };
            cert_id_by_sha1.insert(sha1_hex_upper(&der), cert.id.clone());
        }

        let keychain = KeychainManager::new();
        let identities = keychain
            .list_identities()
            .context("Failed to list code signing identities from Keychain")?;

        // Best-effort: match Keychain identity SHA-1 to a portal certificate SHA-1.
        let mut matched: Option<(String, String)> = None; // (cert_id, identity_sha1)

        for identity in identities.iter().filter(|id| id.is_development()) {
            let sha1 = identity.sha1.to_ascii_uppercase();
            if let Some(cert_id) = cert_id_by_sha1.get(&sha1) {
                let prefer = identity.team_id() == Some(client.team_id);
                if matched.is_none() || prefer {
                    matched = Some((cert_id.clone(), sha1));
                    if prefer {
                        break;
                    }
                }
            }
        }

        if matched.is_some() {
            return Ok(matched);
        }

        // Heuristic fallback: if the portal only shows one dev cert and the keychain has a
        // single dev identity for this team, assume they correspond.
        if cert_id_by_sha1.is_empty() && certs.len() == 1 {
            let mut team_identities = identities
                .iter()
                .filter(|id| id.is_development())
                .filter(|id| id.team_id() == Some(client.team_id));

            if let (Some(identity), None) = (team_identities.next(), team_identities.next()) {
                println!(
                    "  {} Portal did not include certificate content; assuming the only dev certificate matches the only Keychain identity for team {}",
                    "!".yellow(),
                    client.team_id
                );
                return Ok(Some((
                    certs[0].id.clone(),
                    identity.sha1.to_ascii_uppercase(),
                )));
            }
        }

        Ok(None)
    }

    fn ensure_app_id(
        &self,
        client: &DeveloperServicesClient,
        original_bundle_id: &str,
        team_id: &str,
        is_free_team: bool,
    ) -> Result<(String, String)> {
        let new_bundle_id = if is_free_team {
            // For free accounts, we need to create a unique bundle ID
            super::signer::generate_new_bundle_id(original_bundle_id, team_id)
        } else {
            original_bundle_id.to_string()
        };

        // Check if App ID already exists
        let app_ids = client.list_app_ids()?;
        if let Some(existing) = app_ids.iter().find(|a| a.identifier == new_bundle_id) {
            println!("  {} Using existing App ID: {}", "✓".green(), new_bundle_id);
            return Ok((existing.id.clone(), new_bundle_id));
        }

        // Create new App ID
        let app_name = format!(
            "LingXia {}",
            original_bundle_id.split('.').last().unwrap_or("App")
        );
        let app_id = client.add_app_id(&new_bundle_id, &app_name)?;
        println!("  {} Created App ID: {}", "✓".green(), new_bundle_id);
        Ok((app_id.id, new_bundle_id))
    }

    fn create_profile(
        &self,
        client: &DeveloperServicesClient,
        app_id_id: &str,
        cert_id: &str,
        bundle_id: &str,
        cleanup_existing: bool,
    ) -> Result<Vec<u8>> {
        // Include only the target device in the profile.
        let devices = client.list_devices()?;
        let target_device_id = devices
            .iter()
            .find(|d| d.udid == self.target_device_udid)
            .map(|d| d.id.clone())
            .ok_or_else(|| anyhow!("Device not found on portal: {}", self.target_device_udid))?;

        // Free teams have tight limits; delete only profiles created by this tool for this app.
        if cleanup_existing {
            let prefix = profile_name_prefix(bundle_id);
            let profiles = client.list_provisioning_profiles()?;
            for profile in profiles {
                if profile.name.starts_with(&prefix) {
                    let _ = client.delete_provisioning_profile(&profile.id);
                }
            }
        }

        // Create new profile
        let profile_name = format!(
            "{}{}",
            profile_name_prefix(bundle_id),
            chrono::Utc::now().timestamp()
        );
        let device_ids = [target_device_id.as_str()];
        let profile = client.create_provisioning_profile(
            &profile_name,
            app_id_id,
            &[cert_id],
            &device_ids,
        )?;

        // Download the profile
        let profile_data = client.download_provisioning_profile(&profile.id)?;
        println!(
            "  {} Created provisioning profile: {}",
            "✓".green(),
            profile_name
        );
        Ok(profile_data)
    }

    // =========================================================================
    // App Store Connect API Implementation
    // =========================================================================

    fn ensure_device_registered_asc(&self, client: &AppStoreConnectClient) -> Result<()> {
        let devices = client.list_devices()?;
        let already_registered = devices
            .iter()
            .any(|d| d.attributes.udid.as_deref() == Some(&self.target_device_udid));

        if already_registered {
            println!("  {} Device already registered", "✓".green());
            return Ok(());
        }

        client.register_device(
            &self.target_device_name,
            &self.target_device_udid,
            super::asc::DevicePlatform::Ios,
        )?;
        println!(
            "  {} Device registered: {}",
            "✓".green(),
            self.target_device_name
        );
        Ok(())
    }

    fn ensure_certificate_asc(
        &self,
        client: &AppStoreConnectClient,
    ) -> Result<(String, String, Option<IdentityMaterial>)> {
        // Always create a new certificate for signing
        // This avoids needing to access the user's keychain
        println!("  Creating new development certificate...");
        let (csr_content, private_key) = generate_csr("LingXia Development")?;

        let new_cert =
            client.create_certificate(&csr_content, super::asc::CertificateType::IosDevelopment)?;

        let cert_content = new_cert
            .attributes
            .certificate_content
            .as_ref()
            .ok_or_else(|| anyhow!("No certificate content"))?;

        let cert_data = base64_decode(cert_content)?;
        let sha1 = sha1_hex_upper(&cert_data);

        println!("  {} Created new certificate", "✓".green());
        Ok((
            new_cert.id,
            sha1,
            Some(IdentityMaterial {
                cert_data,
                private_key,
            }),
        ))
    }

    fn ensure_bundle_id_asc(
        &self,
        client: &AppStoreConnectClient,
        bundle_id: &str,
    ) -> Result<super::asc::BundleId> {
        // Check if exists
        if let Some(existing) = client.find_bundle_id(bundle_id)? {
            println!("  {} Using existing Bundle ID: {}", "✓".green(), bundle_id);
            return Ok(existing);
        }

        // Create new
        let name = bundle_id.replace('.', " ");
        let bundle =
            client.create_bundle_id(bundle_id, &name, super::asc::BundleIdPlatform::Ios)?;
        println!("  {} Created Bundle ID: {}", "✓".green(), bundle_id);
        Ok(bundle)
    }

    fn create_profile_asc(
        &self,
        client: &AppStoreConnectClient,
        bundle_id: &str,
        cert_id: &str,
    ) -> Result<Vec<u8>> {
        // Include only the target device.
        let devices = client.list_devices()?;
        let target_device_id = devices
            .iter()
            .find(|d| d.attributes.udid.as_deref() == Some(self.target_device_udid.as_str()))
            .map(|d| d.id.clone())
            .ok_or_else(|| {
                anyhow!(
                    "Device not found on App Store Connect: {}",
                    self.target_device_udid
                )
            })?;

        // Create profile
        let profile_name = format!(
            "{}{}",
            profile_name_prefix(bundle_id),
            chrono::Utc::now().timestamp()
        );
        let profile = client.create_profile(
            &profile_name,
            super::asc::ProfileType::IosAppDevelopment,
            bundle_id,
            &[cert_id.to_string()],
            &[target_device_id],
        )?;

        // Download
        let profile_data = client.download_profile(&profile.id)?;
        println!(
            "  {} Created provisioning profile: {}",
            "✓".green(),
            profile_name
        );
        Ok(profile_data)
    }
}

fn profile_name_prefix(bundle_id: &str) -> String {
    use sha1::Digest;
    let digest = sha1::Sha1::digest(bundle_id.as_bytes());
    let short = digest
        .iter()
        .take(4)
        .map(|b| format!("{:02X}", b))
        .collect::<String>();
    format!("LingXia Dev {} ", short)
}

/// High-level function to sign an app with automatic provisioning
pub fn sign_app(app_path: &Path, device_udid: Option<&str>) -> Result<ProvisioningResult> {
    // Get device
    let device = if let Some(udid) = device_udid {
        DeviceCtl::get_device(udid)?
    } else {
        println!("Waiting for device...");
        DeviceCtl::wait_for_device(30)?
    };

    println!("Using device: {}", device.description().cyan());

    // Read bundle ID from app
    let info_plist = app_path.join("Info.plist");
    let bundle_id = read_bundle_id(&info_plist)?;

    // Create provisioning context
    let ctx = ProvisioningContext::new(&device)?;

    // Run provisioning
    let result = ctx.provision(&bundle_id)?;

    if let Some(ref material) = result.identity_material {
        // Create temporary keychain for signing (avoids password prompts)
        let temp_keychain = TempKeychain::new().context("Failed to create temporary keychain")?;

        // Import cert + key into the temp keychain
        temp_keychain.import_identity(&material.cert_data, &material.private_key)?;

        // Sign the app using temporary keychain
        Signer::sign_with_keychain(
            app_path,
            &result.signing_identity,
            &result.profile_data,
            Some(&result.entitlements),
            Some(&result.bundle_id),
            Some(temp_keychain.path()),
        )?;
        // temp_keychain is automatically cleaned up when dropped
    } else {
        // Provisioning reused a Keychain identity; sign using the default keychain.
        Signer::sign(
            app_path,
            &result.signing_identity,
            &result.profile_data,
            Some(&result.entitlements),
            Some(&result.bundle_id),
        )?;
    }
    Ok(result)
}

/// Read bundle ID from Info.plist
pub fn read_bundle_id(info_plist: &Path) -> Result<String> {
    let plist: plist::Value =
        plist::from_file(info_plist).map_err(|e| anyhow!("Failed to read Info.plist: {}", e))?;

    let dict = plist
        .as_dictionary()
        .ok_or_else(|| anyhow!("Invalid Info.plist format"))?;

    dict.get("CFBundleIdentifier")
        .and_then(|v: &plist::Value| v.as_string())
        .map(|s: &str| s.to_string())
        .ok_or_else(|| anyhow!("CFBundleIdentifier not found in Info.plist"))
}

/// Decode base64 string
fn base64_decode(data: &str) -> Result<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(data)
        .context("Failed to decode base64")
}

fn sha1_hex_upper(data: &[u8]) -> String {
    use sha1::Digest;
    let digest = sha1::Sha1::digest(data);
    digest.iter().map(|b| format!("{:02X}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_decode() {
        let encoded = "SGVsbG8gV29ybGQ=";
        let decoded = base64_decode(encoded).unwrap();
        assert_eq!(decoded, b"Hello World");
    }
}
