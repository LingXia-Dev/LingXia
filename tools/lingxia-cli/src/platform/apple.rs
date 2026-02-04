//! Common utilities for Apple platforms (iOS/macOS).
//!
//! Provides shared functionality for building, signing, and deploying
//! applications on Apple platforms.

// Submodules
pub mod anisette;
pub mod asc;
pub mod auth;
pub mod grandslam;
pub mod srp;

use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// Get a shared HTTP agent with native root certificates
pub fn http_agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        use ureq::tls::{Certificate, RootCerts, TlsConfig};

        // Load native root certificates from the system
        let native_certs = rustls_native_certs::load_native_certs();
        let certs: Vec<Certificate<'static>> = native_certs
            .certs
            .into_iter()
            .map(|c| {
                let der = c.as_ref();
                Certificate::from_der(der).to_owned()
            })
            .collect();

        ureq::Agent::config_builder()
            .tls_config(
                TlsConfig::builder()
                    .root_certs(RootCerts::from(certs))
                    .build(),
            )
            .build()
            .new_agent()
    })
}

// Rust cross-compilation target for iOS
pub const IOS_TARGET: &str = "aarch64-apple-ios";

/// Check if running on macOS
pub fn is_macos() -> bool {
    cfg!(target_os = "macos")
}

/// Ensure we're running on macOS (required for Apple platform builds)
pub fn ensure_macos() -> Result<()> {
    if !is_macos() {
        return Err(anyhow!(
            "iOS/macOS builds are only supported on macOS.\n\
             Current platform: {}",
            std::env::consts::OS
        ));
    }
    Ok(())
}

/// Check if a command is available in PATH
pub fn command_exists(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Ensure required tools are available
pub fn ensure_tools() -> Result<()> {
    let required_tools = ["swift", "codesign"];
    let mut missing = Vec::new();

    for tool in required_tools {
        if !command_exists(tool) {
            missing.push(tool);
        }
    }

    if !missing.is_empty() {
        return Err(anyhow!(
            "Missing required tools: {}\n\
             Please install Xcode and Xcode Command Line Tools.",
            missing.join(", ")
        ));
    }

    Ok(())
}

/// Build Rust static library for iOS/macOS
///
/// iOS requires static libraries (.a), not dynamic libraries (.dylib).
///
/// - `workspace_root`: The workspace root (where target/ directory is located)
/// - `rust_lib_dir`: The crate directory containing Cargo.toml
pub fn build_rust_staticlib(
    workspace_root: &Path,
    rust_lib_dir: &Path,
    target: &str,
    release: bool,
    features: &[String],
) -> Result<PathBuf> {
    println!("{}", "Compiling Rust static library...".cyan());

    let rust_manifest = rust_lib_dir.join("Cargo.toml");
    if !rust_manifest.exists() {
        return Err(anyhow!(
            "Rust library manifest not found: {}",
            rust_manifest.display()
        ));
    }

    // Build with cargo rustc --crate-type=staticlib
    let mut cmd = Command::new("cargo");
    cmd.arg("rustc")
        .arg("--crate-type=staticlib")
        .arg("--target")
        .arg(target)
        .arg("--manifest-path")
        .arg(&rust_manifest)
        .current_dir(rust_lib_dir);

    if release {
        cmd.arg("--release");
    }

    if !features.is_empty() {
        cmd.arg("--features").arg(features.join(","));
    }

    let status = cmd.status().context("Failed to execute cargo rustc")?;

    if !status.success() {
        return Err(anyhow!("Rust build failed for target: {}", target));
    }

    // Determine output path - use workspace target directory
    // (Cargo workspace outputs to workspace root's target/)
    let profile_dir = if release { "release" } else { "debug" };

    // The output library name is based on the crate name with underscores
    // We need to find the .a file and copy it to liblingxia.a
    let target_dir = workspace_root.join("target").join(target).join(profile_dir);

    // Find the static library (lib*.a) - look for liblingxia_lib.a or similar
    let lib_name = find_static_lib(&target_dir)?;
    let lib_path = target_dir.join(&lib_name);

    // Copy/rename to standard name liblingxia.a if needed
    let dest_path = target_dir.join("liblingxia.a");
    if lib_path != dest_path {
        std::fs::copy(&lib_path, &dest_path)?;
    }

    println!("  {} Static library → {}", "✓".green(), dest_path.display());

    Ok(dest_path)
}

/// Find static library in directory
fn find_static_lib(dir: &Path) -> Result<String> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("lib") && name.ends_with(".a") {
            return Ok(name);
        }
    }
    Err(anyhow!("No static library found in {}", dir.display()))
}

/// Generate Swift bridge bindings by building with LINGXIA_GENERATE_BRIDGE=1
pub fn generate_swift_bridge(project_root: &Path, target: &str) -> Result<()> {
    println!("{}", "Generating Swift bridge bindings...".cyan());

    // Find the workspace root
    let workspace_root = find_workspace_root(project_root)?;

    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .arg("-p")
        .arg("lingxia")
        .arg("--target")
        .arg(target)
        .arg("--release")
        .env("LINGXIA_GENERATE_BRIDGE", "1")
        .current_dir(&workspace_root);

    let status = cmd.status().context("Failed to generate Swift bridge")?;

    if !status.success() {
        return Err(anyhow!("Swift bridge generation failed"));
    }

    println!("  {} Swift bridge bindings generated", "✓".green());
    Ok(())
}

/// Prepare SDK resources by calling the release.sh script
///
/// This stages the Apple SDK to target/spm/lingxia for local development.
pub fn prepare_sdk_resources(project_root: &Path, skip_rust: bool) -> Result<()> {
    println!("{}", "Preparing iOS SDK resources...".cyan());

    // Find the workspace root (where lingxia-sdk/release.sh is located)
    let workspace_root = find_workspace_root(project_root)?;

    let release_script = workspace_root.join("lingxia-sdk/release.sh");
    if !release_script.exists() {
        return Err(anyhow!(
            "SDK release script not found: {}\n\
             Make sure you're in a LingXia workspace with lingxia-sdk/release.sh",
            release_script.display()
        ));
    }

    let output_dir = workspace_root.join("target/sdk-dev");

    let mut cmd = Command::new("bash");
    cmd.arg(&release_script)
        .arg("--platform")
        .arg("ios")
        .arg("--ios-no-zip")
        .arg("--no-shasums")
        .arg("--out")
        .arg(&output_dir)
        .current_dir(&workspace_root);

    if skip_rust {
        cmd.env("SKIP_RUST", "true");
    }

    let status = cmd.status().context("Failed to prepare SDK resources")?;

    if !status.success() {
        return Err(anyhow!("SDK resource preparation failed"));
    }

    println!("  {} SDK resources prepared", "✓".green());
    Ok(())
}

/// Find the workspace root by looking for Cargo.toml with [workspace]
pub fn find_workspace_root(start: &Path) -> Result<PathBuf> {
    let mut current = start.to_path_buf();

    loop {
        let cargo_toml = current.join("Cargo.toml");
        if cargo_toml.exists() {
            if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
                if content.contains("[workspace]") {
                    return Ok(current);
                }
            }
        }

        if !current.pop() {
            break;
        }
    }

    Err(anyhow!(
        "Could not find workspace root from: {}",
        start.display()
    ))
}

/// Sign an app bundle using codesign
pub fn sign_app_bundle(app_path: &Path, identity: &str, entitlements: Option<&Path>) -> Result<()> {
    println!("{}", "Signing app bundle...".cyan());

    let mut cmd = Command::new("codesign");
    cmd.arg("--force")
        .arg("--sign")
        .arg(identity)
        .arg("--timestamp=none");

    if let Some(ent_path) = entitlements {
        cmd.arg("--entitlements").arg(ent_path);
    }

    cmd.arg(app_path);

    let status = cmd.status().context("Failed to execute codesign")?;

    if !status.success() {
        return Err(anyhow!("Code signing failed"));
    }

    println!("  {} App signed with identity: {}", "✓".green(), identity);
    Ok(())
}

/// Find signing identity from team ID
///
/// Searches for an Apple Development or iPhone Developer certificate
/// that matches the given team ID.
pub fn find_signing_identity(team_id: &str) -> Result<String> {
    let output = Command::new("security")
        .args(["find-identity", "-v", "-p", "codesigning"])
        .output()
        .context("Failed to list signing identities")?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Look for a valid identity with the team ID
    for line in stdout.lines() {
        if line.contains(team_id)
            && (line.contains("Apple Development")
                || line.contains("iPhone Developer")
                || line.contains("Apple Distribution"))
        {
            // Extract the identity hash (40 hex chars)
            if let Some(start) = line.find(')') {
                let after_paren = &line[start + 1..].trim();
                if let Some(quote_start) = after_paren.find('"') {
                    if let Some(quote_end) = after_paren[quote_start + 1..].find('"') {
                        return Ok(
                            after_paren[quote_start + 1..quote_start + 1 + quote_end].to_string()
                        );
                    }
                }
            }
        }
    }

    // Fallback: just use the team ID with a generic identity type
    Ok(format!("Apple Development: {}", team_id))
}

/// List connected iOS devices using ios-deploy or idevice_id
pub fn list_ios_devices() -> Result<Vec<super::Device>> {
    let mut devices = Vec::new();

    // Try ios-deploy first
    if command_exists("ios-deploy") {
        let output = Command::new("ios-deploy")
            .args(["--detect", "--timeout", "1"])
            .output();

        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                // ios-deploy output format: [....] Found UDID via USB.
                if line.contains("Found") && line.contains("via") {
                    if let Some(udid_start) = line.find("Found ") {
                        let rest = &line[udid_start + 6..];
                        if let Some(udid_end) = rest.find(' ') {
                            let udid = rest[..udid_end].to_string();
                            devices.push(super::Device {
                                id: udid,
                                name: None,
                                device_type: super::DeviceType::Physical,
                                online: true,
                            });
                        }
                    }
                }
            }
        }
    }

    // Fallback to idevice_id (libimobiledevice)
    if devices.is_empty() && command_exists("idevice_id") {
        let output = Command::new("idevice_id").arg("-l").output();

        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let udid = line.trim();
                if !udid.is_empty() {
                    devices.push(super::Device {
                        id: udid.to_string(),
                        name: None,
                        device_type: super::DeviceType::Physical,
                        online: true,
                    });
                }
            }
        }
    }

    Ok(devices)
}

/// Install app to iOS device using ios-deploy
pub fn install_with_ios_deploy(app_path: &Path, device_id: Option<&str>) -> Result<()> {
    if !command_exists("ios-deploy") {
        return Err(anyhow!(
            "ios-deploy not found. Install with: brew install ios-deploy"
        ));
    }

    println!("{}", "Installing to device...".cyan());

    let mut cmd = Command::new("ios-deploy");
    cmd.arg("--bundle").arg(app_path);

    if let Some(id) = device_id {
        cmd.arg("--id").arg(id);
    }

    let status = cmd.status().context("Failed to execute ios-deploy")?;

    if !status.success() {
        return Err(anyhow!("Installation failed"));
    }

    println!("  {} App installed", "✓".green());
    Ok(())
}

/// Run app on iOS device using ios-deploy
pub fn run_with_ios_deploy(bundle_id: &str, device_id: Option<&str>) -> Result<()> {
    if !command_exists("ios-deploy") {
        return Err(anyhow!(
            "ios-deploy not found. Install with: brew install ios-deploy"
        ));
    }

    println!("{}", "Launching app...".cyan());

    let mut cmd = Command::new("ios-deploy");
    cmd.arg("--justlaunch").arg("--bundle_id").arg(bundle_id);

    if let Some(id) = device_id {
        cmd.arg("--id").arg(id);
    }

    let status = cmd.status().context("Failed to execute ios-deploy")?;

    if !status.success() {
        return Err(anyhow!("Failed to launch app"));
    }

    println!("  {} App launched", "✓".green());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_macos() {
        // This will be true when running tests on macOS
        #[cfg(target_os = "macos")]
        assert!(is_macos());

        #[cfg(not(target_os = "macos"))]
        assert!(!is_macos());
    }
}
