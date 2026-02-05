//! Keychain management for code signing certificates.
//!
//! Uses macOS `security` command to manage certificates and identities
//! in the system keychain or a temporary keychain for signing.

use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use sha1::Digest;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Keychain manager for code signing operations.
///
/// Manages certificates and private keys needed for iOS code signing.
/// Can work with the system keychain or create temporary keychains.
pub struct KeychainManager {
    /// Path to the keychain file (None = use default/login keychain)
    keychain_path: Option<PathBuf>,
    /// Password for the keychain (if using a custom keychain)
    keychain_password: Option<String>,
}

impl KeychainManager {
    /// Create a new KeychainManager using the default login keychain
    pub fn new() -> Self {
        Self {
            keychain_path: None,
            keychain_password: None,
        }
    }

    /// Create a temporary keychain for signing
    ///
    /// This is useful for CI/CD environments where you don't want to
    /// modify the login keychain.
    #[allow(dead_code)]
    pub fn create_temporary(name: &str, password: &str) -> Result<Self> {
        let keychain_path = std::env::temp_dir().join(format!("{}.keychain-db", name));

        // Delete existing keychain if it exists
        if keychain_path.exists() {
            let _ = Command::new("security")
                .args(["delete-keychain", keychain_path.to_str().unwrap()])
                .output();
        }

        // Create new keychain
        let status = Command::new("security")
            .args([
                "create-keychain",
                "-p",
                password,
                keychain_path.to_str().unwrap(),
            ])
            .status()
            .context("Failed to create temporary keychain")?;

        if !status.success() {
            return Err(anyhow!("Failed to create keychain"));
        }

        // Unlock the keychain
        let status = Command::new("security")
            .args([
                "unlock-keychain",
                "-p",
                password,
                keychain_path.to_str().unwrap(),
            ])
            .status()
            .context("Failed to unlock keychain")?;

        if !status.success() {
            return Err(anyhow!("Failed to unlock keychain"));
        }

        // Set keychain settings (no auto-lock)
        let _ = Command::new("security")
            .args([
                "set-keychain-settings",
                "-t",
                "3600",
                "-u",
                keychain_path.to_str().unwrap(),
            ])
            .output();

        // Add to search list
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

        keychains.insert(0, keychain_path.to_str().unwrap().to_string());

        let _ = Command::new("security")
            .arg("list-keychains")
            .arg("-d")
            .arg("user")
            .arg("-s")
            .args(&keychains)
            .output();

        Ok(Self {
            keychain_path: Some(keychain_path),
            keychain_password: Some(password.to_string()),
        })
    }

    /// Import a certificate and private key into the keychain
    ///
    /// The certificate should be in DER or PEM format.
    /// The private key should be in PEM format.
    ///
    /// Returns the SHA-1 fingerprint of the imported certificate.
    pub fn import_identity(&self, cert_path: &Path, key_path: &Path) -> Result<String> {
        use rsa::pkcs8::DecodePrivateKey;

        // Read certificate (PEM or DER)
        let cert_data = std::fs::read(cert_path).context("Failed to read certificate")?;
        let cert_der = if cert_data.starts_with(b"-----BEGIN") {
            // PEM format - extract DER
            let pem_str = String::from_utf8_lossy(&cert_data);
            let pem = pem_str
                .lines()
                .filter(|l| !l.starts_with("-----"))
                .collect::<String>();
            base64::Engine::decode(&base64::prelude::BASE64_STANDARD, &pem)
                .context("Failed to decode PEM certificate")?
        } else {
            cert_data
        };

        let expected_sha1 = sha1_hex_upper(&cert_der);

        // Read private key (PEM)
        let key_pem = std::fs::read_to_string(key_path).context("Failed to read private key")?;
        let _rsa_key =
            rsa::RsaPrivateKey::from_pkcs8_pem(&key_pem).context("Failed to parse private key")?;

        // Import cert + key directly (avoids requiring `openssl` on PATH).
        self.import_file(cert_path)
            .context("Failed to import certificate into keychain")?;
        self.import_file(key_path)
            .context("Failed to import private key into keychain")?;
        self.configure_codesign_access();

        Ok(expected_sha1)
    }

    /// Import a PKCS#12 (.p12) file into the keychain
    ///
    /// Note: This does not try to infer which identity was imported, since that can be
    /// nondeterministic in the presence of multiple identities. Callers that know the
    /// expected certificate fingerprint should compute/return it themselves.
    pub fn import_p12(&self, p12_path: &Path, password: &str) -> Result<()> {
        let mut cmd = Command::new("security");
        cmd.arg("import")
            .arg(p12_path)
            .arg("-P")
            .arg(password)
            // Allow codesign to access the key without prompts
            .args(["-T", "/usr/bin/codesign"])
            .args(["-T", "/usr/bin/security"]);

        if let Some(ref kc_path) = self.keychain_path {
            cmd.arg("-k").arg(kc_path);
        }

        let output = cmd.output().context("Failed to import P12")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Failed to import P12: {}", stderr));
        }

        self.configure_codesign_access();

        Ok(())
    }

    fn import_file(&self, path: &Path) -> Result<()> {
        let mut cmd = Command::new("security");
        cmd.arg("import")
            .arg(path)
            // Allow codesign to access the key without prompts
            .args(["-T", "/usr/bin/codesign"])
            .args(["-T", "/usr/bin/security"]);

        if let Some(ref kc_path) = self.keychain_path {
            cmd.arg("-k").arg(kc_path);
        }

        let output = cmd.output().context("Failed to import file")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("security import failed: {}", stderr));
        }

        Ok(())
    }

    fn configure_codesign_access(&self) {
        // Set key partition list to allow codesign access without prompts
        // This is required for non-interactive code signing
        if let Some(ref kc_path) = self.keychain_path {
            if let Some(ref kc_password) = self.keychain_password {
                let _ = Command::new("security")
                    .args([
                        "set-key-partition-list",
                        "-S",
                        "apple-tool:,apple:,codesign:",
                        "-s",
                        "-k",
                        kc_password,
                        kc_path.to_str().unwrap(),
                    ])
                    .output();
            }
        } else {
            // For default login keychain, try to get its path and update partition list
            // This requires the keychain to be unlocked
            let kc_output = Command::new("security")
                .args(["default-keychain", "-d", "user"])
                .output();

            if let Ok(output) = kc_output {
                let kc_path = String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .trim_matches('"')
                    .to_string();

                if !kc_path.is_empty() {
                    // Try with empty password first (works if keychain is unlocked)
                    let result = Command::new("security")
                        .args([
                            "set-key-partition-list",
                            "-S",
                            "apple-tool:,apple:,codesign:",
                            "-s",
                            "-k",
                            "",
                            &kc_path,
                        ])
                        .output();

                    // If that fails, the user will need to enter their keychain password
                    // when codesign runs, or manually unlock the keychain
                    if result.is_err() || !result.unwrap().status.success() {
                        eprintln!(
                            "  {} Could not auto-configure keychain access. \
                            You may be prompted for your keychain password.",
                            "⚠".yellow()
                        );
                    }
                }
            }
        }
    }

    /// Import certificate data directly (DER format)
    #[allow(dead_code)]
    pub fn import_certificate_der(&self, cert_data: &[u8]) -> Result<()> {
        let cert_path = std::env::temp_dir().join("lingxia_cert.der");
        std::fs::write(&cert_path, cert_data).context("Failed to write certificate")?;

        let mut cmd = Command::new("security");
        cmd.arg("import").arg(&cert_path);

        if let Some(ref kc_path) = self.keychain_path {
            cmd.arg("-k").arg(kc_path);
        }

        let output = cmd.output().context("Failed to import certificate")?;

        // Clean up
        let _ = std::fs::remove_file(&cert_path);

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Failed to import certificate: {}", stderr));
        }

        Ok(())
    }

    /// Find a signing identity by SHA-1 fingerprint
    #[allow(dead_code)]
    pub fn find_identity(&self, sha1: &str) -> Result<SigningIdentity> {
        let output = Command::new("security")
            .args(["find-identity", "-v", "-p", "codesigning"])
            .output()
            .context("Failed to list signing identities")?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        for line in stdout.lines() {
            if line.contains(sha1) {
                return parse_identity_line(line);
            }
        }

        Err(anyhow!("Signing identity not found: {}", sha1))
    }

    /// Find a signing identity by common name (partial match)
    #[allow(dead_code)]
    pub fn find_identity_by_name(&self, name: &str) -> Result<SigningIdentity> {
        let output = Command::new("security")
            .args(["find-identity", "-v", "-p", "codesigning"])
            .output()
            .context("Failed to list signing identities")?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        for line in stdout.lines() {
            if line.contains(name) {
                return parse_identity_line(line);
            }
        }

        Err(anyhow!("Signing identity not found containing: {}", name))
    }

    /// Find the most recently added signing identity
    #[allow(dead_code)]
    fn find_newest_identity(&self) -> Result<String> {
        let output = Command::new("security")
            .args(["find-identity", "-v", "-p", "codesigning"])
            .output()
            .context("Failed to list signing identities")?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Get the first valid identity (most recently added is usually first)
        for line in stdout.lines() {
            if let Ok(identity) = parse_identity_line(line) {
                return Ok(identity.sha1);
            }
        }

        Err(anyhow!("No signing identities found"))
    }

    /// List all available signing identities
    pub fn list_identities(&self) -> Result<Vec<SigningIdentity>> {
        let output = Command::new("security")
            .args(["find-identity", "-v", "-p", "codesigning"])
            .output()
            .context("Failed to list signing identities")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut identities = Vec::new();

        for line in stdout.lines() {
            if let Ok(identity) = parse_identity_line(line) {
                identities.push(identity);
            }
        }

        Ok(identities)
    }

    /// Find signing identities for a specific team ID
    #[allow(dead_code)]
    pub fn find_identities_for_team(&self, team_id: &str) -> Result<Vec<SigningIdentity>> {
        let all = self.list_identities()?;
        Ok(all
            .into_iter()
            .filter(|id| id.common_name.contains(team_id))
            .collect())
    }

    /// Delete the temporary keychain (cleanup)
    #[allow(dead_code)]
    pub fn cleanup(&self) -> Result<()> {
        if let Some(ref kc_path) = self.keychain_path {
            let status = Command::new("security")
                .args(["delete-keychain", kc_path.to_str().unwrap()])
                .status()
                .context("Failed to delete keychain")?;

            if !status.success() {
                return Err(anyhow!("Failed to delete keychain"));
            }
        }
        Ok(())
    }

    /// Get the keychain path (if using a custom keychain)
    #[allow(dead_code)]
    pub fn keychain_path(&self) -> Option<&Path> {
        self.keychain_path.as_deref()
    }
}

impl Drop for KeychainManager {
    fn drop(&mut self) {
        // Don't automatically clean up - let the user decide
        // self.cleanup() can be called explicitly if needed
    }
}

impl Default for KeychainManager {
    fn default() -> Self {
        Self::new()
    }
}

/// A signing identity found in the keychain
#[derive(Debug, Clone)]
pub struct SigningIdentity {
    /// SHA-1 fingerprint of the certificate
    pub sha1: String,
    /// Common name of the certificate (e.g., "Apple Development: John Doe (TEAMID)")
    pub common_name: String,
}

impl SigningIdentity {
    /// Extract the team ID from the common name
    pub fn team_id(&self) -> Option<&str> {
        // Format: "Apple Development: Name (TEAMID)" or similar
        if let Some(start) = self.common_name.rfind('(') {
            if let Some(end) = self.common_name.rfind(')') {
                if start < end {
                    return Some(&self.common_name[start + 1..end]);
                }
            }
        }
        None
    }

    /// Check if this is a development certificate
    pub fn is_development(&self) -> bool {
        self.common_name.contains("Development") || self.common_name.contains("Developer")
    }

    /// Check if this is a distribution certificate
    #[allow(dead_code)]
    pub fn is_distribution(&self) -> bool {
        self.common_name.contains("Distribution")
    }
}

/// Parse a line from `security find-identity` output
fn parse_identity_line(line: &str) -> Result<SigningIdentity> {
    // Format: "  1) SHA1_HASH "Common Name""
    // or:     "  1) SHA1_HASH "Common Name" (CSSMERR_TP_CERT_REVOKED)"

    let line = line.trim();
    if line.is_empty() || !line.contains(')') {
        return Err(anyhow!("Invalid identity line"));
    }

    // Find the SHA1 hash (40 hex characters after the index)
    let parts: Vec<&str> = line.splitn(3, ' ').collect();
    if parts.len() < 3 {
        return Err(anyhow!("Invalid identity line format"));
    }

    let sha1 = parts[1].to_string();
    if sha1.len() != 40 || !sha1.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(anyhow!("Invalid SHA1 hash"));
    }

    // Extract common name (in quotes)
    let rest = parts[2];
    let common_name = if let Some(start) = rest.find('"') {
        if let Some(end) = rest[start + 1..].find('"') {
            rest[start + 1..start + 1 + end].to_string()
        } else {
            return Err(anyhow!("Invalid common name format"));
        }
    } else {
        return Err(anyhow!("Missing common name"));
    };

    Ok(SigningIdentity { sha1, common_name })
}

/// Generate a Certificate Signing Request (CSR) using rcgen
///
/// Returns (CSR content as PEM string, private key as PEM string)
pub fn generate_csr(common_name: &str) -> Result<(String, String)> {
    use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair};
    use rsa::pkcs8::EncodePrivateKey;
    use rsa::rand_core::OsRng;

    // Generate RSA 2048 key pair using OsRng (cryptographically secure)
    let rsa_key =
        rsa::RsaPrivateKey::new(&mut OsRng, 2048).context("Failed to generate RSA key")?;

    // Export private key as PEM
    let key_pem = rsa_key
        .to_pkcs8_pem(rsa::pkcs8::LineEnding::LF)
        .context("Failed to encode private key")?;

    // Create KeyPair for rcgen from the PEM
    let key_pair =
        KeyPair::from_pem(&key_pem).map_err(|e| anyhow!("Failed to create key pair: {}", e))?;

    // Build distinguished name
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, common_name);
    dn.push(DnType::OrganizationName, "LingXia");
    dn.push(DnType::CountryName, "US");

    // Generate CSR
    let mut params = CertificateParams::default();
    params.distinguished_name = dn;

    let csr = params
        .serialize_request(&key_pair)
        .map_err(|e| anyhow!("Failed to generate CSR: {}", e))?;

    let csr_pem = csr
        .pem()
        .map_err(|e| anyhow!("Failed to encode CSR: {}", e))?;
    Ok((csr_pem, key_pem.to_string()))
}

fn sha1_hex_upper(data: &[u8]) -> String {
    let digest = sha1::Sha1::digest(data);
    digest.iter().map(|b| format!("{:02X}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_identity_line() {
        let line = r#"  1) ABC123DEF456ABC123DEF456ABC123DEF456ABC1 "Apple Development: John Doe (ABCD1234EF)""#;
        let identity = parse_identity_line(line).unwrap();
        assert_eq!(identity.sha1, "ABC123DEF456ABC123DEF456ABC123DEF456ABC1");
        assert_eq!(
            identity.common_name,
            "Apple Development: John Doe (ABCD1234EF)"
        );
        assert_eq!(identity.team_id(), Some("ABCD1234EF"));
        assert!(identity.is_development());
    }

    #[test]
    fn test_parse_identity_line_invalid() {
        assert!(parse_identity_line("invalid line").is_err());
        assert!(parse_identity_line("").is_err());
    }
}
