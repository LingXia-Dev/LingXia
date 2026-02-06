//! Code signing for iOS applications.
//!
//! Uses macOS `codesign` command to sign iOS app bundles.

use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::{NamedTempFile, TempDir};

/// Code signer for iOS applications.
///
/// Uses the macOS `codesign` command to sign app bundles with
/// a certificate and entitlements.
pub struct Signer;

impl Signer {
    /// Sign an iOS app bundle.
    ///
    /// # Arguments
    /// * `app_path` - Path to the .app bundle
    /// * `identity` - Signing identity (SHA-1 fingerprint or name like "Apple Development: ...")
    /// * `profile_data` - mobileprovision file content
    /// * `entitlements` - Optional entitlements plist content
    /// * `bundle_id` - Optional new bundle ID (will update Info.plist)
    pub fn sign(
        app_path: &Path,
        identity: &str,
        profile_data: &[u8],
        entitlements: Option<&[u8]>,
        bundle_id: Option<&str>,
    ) -> Result<()> {
        Self::sign_with_keychain(
            app_path,
            identity,
            profile_data,
            entitlements,
            bundle_id,
            None,
        )
    }

    /// Sign an iOS app bundle using a specific keychain.
    ///
    /// # Arguments
    /// * `app_path` - Path to the .app bundle
    /// * `identity` - Signing identity (SHA-1 fingerprint or name like "Apple Development: ...")
    /// * `profile_data` - mobileprovision file content
    /// * `entitlements` - Optional entitlements plist content
    /// * `bundle_id` - Optional new bundle ID (will update Info.plist)
    /// * `keychain_path` - Optional keychain path (uses default if None)
    pub fn sign_with_keychain(
        app_path: &Path,
        identity: &str,
        profile_data: &[u8],
        entitlements: Option<&[u8]>,
        bundle_id: Option<&str>,
        keychain_path: Option<&Path>,
    ) -> Result<()> {
        println!("{}", "Signing app bundle...".cyan());

        // Validate app bundle exists
        if !app_path.exists() || !app_path.is_dir() {
            return Err(anyhow!("App bundle not found: {}", app_path.display()));
        }

        // 1. Embed the provisioning profile
        Self::embed_provisioning_profile(app_path, profile_data)?;

        // 2. Update bundle ID if specified
        if let Some(new_bundle_id) = bundle_id {
            Self::update_bundle_id(app_path, new_bundle_id)?;
        }

        // 3. Create entitlements file if provided
        let entitlements_file = if let Some(ent_data) = entitlements {
            let mut tmp =
                NamedTempFile::new().context("Failed to create entitlements temp file")?;
            use std::io::Write;
            tmp.write_all(ent_data)
                .context("Failed to write entitlements")?;
            Some(tmp)
        } else {
            None
        };
        let entitlements_path = entitlements_file.as_ref().map(|f: &NamedTempFile| f.path());

        // 4. Sign frameworks first (if any)
        Self::sign_frameworks(app_path, identity, keychain_path)?;

        // 5. Sign app extensions (if any)
        Self::sign_extensions(app_path, identity, entitlements_path, keychain_path)?;

        // 6. Sign the main app bundle
        Self::codesign(app_path, identity, entitlements_path, keychain_path)?;

        println!("  {} App signed successfully", "✓".green());
        Ok(())
    }

    /// Embed the provisioning profile into the app bundle
    fn embed_provisioning_profile(app_path: &Path, profile_data: &[u8]) -> Result<()> {
        let profile_path = app_path.join("embedded.mobileprovision");
        fs::write(&profile_path, profile_data)
            .context("Failed to write embedded.mobileprovision")?;
        println!("  {} Embedded provisioning profile", "✓".green());
        Ok(())
    }

    /// Update the bundle identifier in Info.plist
    fn update_bundle_id(app_path: &Path, bundle_id: &str) -> Result<()> {
        let info_plist = app_path.join("Info.plist");
        if !info_plist.exists() {
            return Err(anyhow!("Info.plist not found in app bundle"));
        }

        // Use PlistBuddy to update the bundle ID
        let status = Command::new("/usr/libexec/PlistBuddy")
            .args([
                "-c",
                &format!("Set :CFBundleIdentifier {}", bundle_id),
                info_plist.to_str().unwrap(),
            ])
            .status()
            .context("Failed to update bundle ID")?;

        if !status.success() {
            return Err(anyhow!("Failed to update bundle ID in Info.plist"));
        }

        println!("  {} Updated bundle ID to {}", "✓".green(), bundle_id);
        Ok(())
    }

    /// Sign embedded frameworks
    fn sign_frameworks(
        app_path: &Path,
        identity: &str,
        keychain_path: Option<&Path>,
    ) -> Result<()> {
        let frameworks_dir = app_path.join("Frameworks");
        if !frameworks_dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(&frameworks_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path
                .extension()
                .map(|e| e == "framework" || e == "dylib")
                .unwrap_or(false)
            {
                Self::codesign(&path, identity, None, keychain_path)?;
            }
        }

        Ok(())
    }

    /// Sign app extensions
    fn sign_extensions(
        app_path: &Path,
        identity: &str,
        entitlements: Option<&Path>,
        keychain_path: Option<&Path>,
    ) -> Result<()> {
        for dir_name in &["PlugIns", "Extensions"] {
            let ext_dir = app_path.join(dir_name);
            if !ext_dir.exists() {
                continue;
            }

            for entry in fs::read_dir(&ext_dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.extension().map(|e| e == "appex").unwrap_or(false) {
                    // Sign frameworks in extension first
                    Self::sign_frameworks(&path, identity, keychain_path)?;
                    // Then sign the extension
                    Self::codesign(&path, identity, entitlements, keychain_path)?;
                }
            }
        }

        Ok(())
    }

    /// Execute codesign command
    fn codesign(
        path: &Path,
        identity: &str,
        entitlements: Option<&Path>,
        keychain_path: Option<&Path>,
    ) -> Result<()> {
        let mut cmd = Command::new("codesign");
        cmd.arg("--force")
            .arg("--sign")
            .arg(identity)
            .arg("--timestamp=none")
            .arg("--generate-entitlement-der");

        if let Some(kc_path) = keychain_path {
            cmd.arg("--keychain").arg(kc_path);
        }

        if let Some(ent_path) = entitlements {
            cmd.arg("--entitlements").arg(ent_path);
        }

        cmd.arg(path);

        let output = cmd.output().context("Failed to execute codesign")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!(
                "codesign failed for {}: {}",
                path.display(),
                stderr
            ));
        }

        Ok(())
    }

    /// Verify the signature of an app bundle
    #[allow(dead_code)]
    pub fn verify(app_path: &Path) -> Result<bool> {
        let output = Command::new("codesign")
            .args(["--verify", "--deep", "--strict"])
            .arg(app_path)
            .output()
            .context("Failed to verify signature")?;

        Ok(output.status.success())
    }

    /// Get information about the signature
    #[allow(dead_code)]
    pub fn get_signature_info(app_path: &Path) -> Result<SignatureInfo> {
        let output = Command::new("codesign")
            .args(["-dv", "--verbose=4"])
            .arg(app_path)
            .output()
            .context("Failed to get signature info")?;

        // codesign outputs to stderr
        let stderr = String::from_utf8_lossy(&output.stderr);

        let mut info = SignatureInfo::default();

        for line in stderr.lines() {
            if let Some((key, value)) = line.split_once('=') {
                match key.trim() {
                    "Identifier" => info.identifier = Some(value.to_string()),
                    "TeamIdentifier" => info.team_identifier = Some(value.to_string()),
                    "Authority" => {
                        if info.authority.is_none() {
                            info.authority = Some(value.to_string());
                        }
                    }
                    "Signed Time" => info.signed_time = Some(value.to_string()),
                    _ => {}
                }
            }
        }

        Ok(info)
    }

    /// Remove signature from an app bundle (useful for re-signing)
    #[allow(dead_code)]
    pub fn remove_signature(app_path: &Path) -> Result<()> {
        let status = Command::new("codesign")
            .args(["--remove-signature"])
            .arg(app_path)
            .status()
            .context("Failed to remove signature")?;

        if !status.success() {
            return Err(anyhow!(
                "Failed to remove signature from {}",
                app_path.display()
            ));
        }

        Ok(())
    }
}

/// Information about an app's signature
#[allow(dead_code)]
#[derive(Debug, Default)]
pub struct SignatureInfo {
    pub identifier: Option<String>,
    pub team_identifier: Option<String>,
    pub authority: Option<String>,
    pub signed_time: Option<String>,
}

/// Extract entitlements from a provisioning profile
pub fn extract_entitlements_from_profile(profile_data: &[u8]) -> Result<Vec<u8>> {
    // Provisioning profiles are CMS signed data
    // We need to extract the plist content and get the Entitlements key

    // Use security cms to decode
    let mut profile_file = NamedTempFile::new().context("Failed to create profile temp file")?;
    use std::io::Write;
    profile_file
        .write_all(profile_data)
        .context("Failed to write profile temp file")?;

    let output = Command::new("security")
        .args(["cms", "-D", "-i"])
        .arg(profile_file.path())
        .output()
        .context("Failed to decode provisioning profile")?;

    if !output.status.success() {
        return Err(anyhow!("Failed to decode provisioning profile"));
    }

    // Parse the plist
    let plist: plist::Value =
        plist::from_bytes(&output.stdout).context("Failed to parse provisioning profile plist")?;

    let dict = plist
        .as_dictionary()
        .ok_or_else(|| anyhow!("Invalid provisioning profile format"))?;

    let entitlements = dict
        .get("Entitlements")
        .ok_or_else(|| anyhow!("No Entitlements in provisioning profile"))?;

    // Serialize entitlements to plist
    let mut buf = Vec::new();
    plist::to_writer_xml(&mut buf, entitlements)?;

    Ok(buf)
}

/// Extract the team ID from a provisioning profile
#[allow(dead_code)]
pub fn extract_team_id_from_profile(profile_data: &[u8]) -> Result<String> {
    let mut profile_file = NamedTempFile::new().context("Failed to create profile temp file")?;
    use std::io::Write;
    profile_file
        .write_all(profile_data)
        .context("Failed to write profile temp file")?;

    let output = Command::new("security")
        .args(["cms", "-D", "-i"])
        .arg(profile_file.path())
        .output()
        .context("Failed to decode provisioning profile")?;

    if !output.status.success() {
        return Err(anyhow!("Failed to decode provisioning profile"));
    }

    let plist: plist::Value =
        plist::from_bytes(&output.stdout).context("Failed to parse provisioning profile plist")?;

    let dict = plist
        .as_dictionary()
        .ok_or_else(|| anyhow!("Invalid provisioning profile format"))?;

    // Try TeamIdentifier first (array)
    if let Some(team_ids) = dict.get("TeamIdentifier").and_then(|v| v.as_array())
        && let Some(first) = team_ids.first().and_then(|v| v.as_string())
    {
        return Ok(first.to_string());
    }

    // Fall back to Entitlements.com.apple.developer.team-identifier
    if let Some(ents) = dict.get("Entitlements").and_then(|v| v.as_dictionary())
        && let Some(team_id) = ents
            .get("com.apple.developer.team-identifier")
            .and_then(|v| v.as_string())
    {
        return Ok(team_id.to_string());
    }

    Err(anyhow!("Could not find team ID in provisioning profile"))
}

/// Create a new bundle ID based on the team ID and original bundle ID
pub fn generate_new_bundle_id(original_id: &str, team_id: &str) -> String {
    // For free Apple Developer accounts, "Personal Team" apps commonly get a bundle ID
    // prefixed by the Team ID. We keep it deterministic to allow re-signing updates.
    //
    // Strategy: drop the first two segments of the original bundle id (e.g. com.example.*)
    // and keep the rest as a hyphenated suffix.
    let parts: Vec<&str> = original_id.split('.').filter(|p| !p.is_empty()).collect();
    let suffix = if parts.len() >= 3 {
        parts[2..].join("-")
    } else if parts.len() == 2 {
        parts[1].to_string()
    } else {
        parts.first().copied().unwrap_or("app").to_string()
    };

    format!("com.{}.{}", team_id, suffix)
}

/// Package an app bundle into an IPA file
pub fn create_ipa(app_path: &Path, output_path: &Path) -> Result<PathBuf> {
    println!("{}", "Creating IPA...".cyan());

    let temp_dir = TempDir::new().context("Failed to create IPA temp dir")?;
    let payload_dir = temp_dir.path().join("Payload");
    fs::create_dir_all(&payload_dir).context("Failed to create Payload dir")?;

    // Copy app to Payload
    let app_name = app_path
        .file_name()
        .ok_or_else(|| anyhow!("Invalid app path"))?;
    let dest_app = payload_dir.join(app_name);
    super::copy_dir_recursive(app_path, &dest_app)?;

    // Determine output path
    let ipa_path = if output_path.extension().map(|e| e == "ipa").unwrap_or(false) {
        output_path.to_path_buf()
    } else {
        let app_stem = app_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("app");
        output_path.join(format!("{}.ipa", app_stem))
    };

    // Create IPA using zip crate
    create_ipa_zip(temp_dir.path(), &ipa_path)?;

    println!("  {} IPA created: {}", "✓".green(), ipa_path.display());
    Ok(ipa_path)
}

/// Create IPA zip file from temp directory containing Payload folder
fn create_ipa_zip(temp_dir: &Path, ipa_path: &Path) -> Result<()> {
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    if let Some(parent) = ipa_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).context("Failed to create IPA output directory")?;
    }

    let file = fs::File::create(ipa_path).context("Failed to create IPA file")?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o755);

    let payload_dir = temp_dir.join("Payload");
    add_dir_to_zip(&mut zip, &payload_dir, "Payload", options)?;

    zip.finish().context("Failed to finalize IPA")?;
    Ok(())
}

/// Recursively add a directory to a zip archive
fn add_dir_to_zip<W: std::io::Write + std::io::Seek>(
    zip: &mut zip::ZipWriter<W>,
    src_dir: &Path,
    prefix: &str,
    options: zip::write::SimpleFileOptions,
) -> Result<()> {
    use std::io::{Read, Write};

    for entry in fs::read_dir(src_dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = format!("{}/{}", prefix, entry.file_name().to_string_lossy());

        if path.is_dir() {
            zip.add_directory(&name, options)?;
            add_dir_to_zip(zip, &path, &name, options)?;
        } else {
            // Preserve executable permissions for binaries
            let metadata = path.metadata()?;
            #[cfg(unix)]
            let file_options = {
                use std::os::unix::fs::PermissionsExt;
                let mode = metadata.permissions().mode();
                options.unix_permissions(mode)
            };
            #[cfg(not(unix))]
            let file_options = options;

            zip.start_file(&name, file_options)?;
            let mut file = fs::File::open(&path)?;
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)?;
            zip.write_all(&buffer)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_new_bundle_id() {
        assert_eq!(
            generate_new_bundle_id("com.example.myapp", "ABCD1234EF"),
            "com.ABCD1234EF.myapp"
        );

        assert_eq!(
            generate_new_bundle_id("org.company.app.name", "TEAMID"),
            "com.TEAMID.app-name"
        );
    }
}
