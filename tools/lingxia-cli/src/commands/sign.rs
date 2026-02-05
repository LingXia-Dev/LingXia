//! Sign command implementation.
//!
//! Signs an iOS app bundle with provisioning and code signing.

use anyhow::{Result, anyhow};
use colored::Colorize;
use std::env;
use std::path::{Path, PathBuf};

use crate::platform::apple::provisioning;
use crate::platform::apple::signer;

/// Execute the sign command
///
/// Signs an iOS app bundle with automatic provisioning.
pub fn execute(
    app_path: Option<String>,
    device: Option<String>,
    output: Option<String>,
) -> Result<()> {
    // Ensure we're on macOS
    crate::platform::apple::ensure_macos()?;

    let project_root = env::current_dir()?;

    // Determine app path
    let app_path = if let Some(path) = app_path {
        PathBuf::from(path)
    } else {
        // Try to auto-detect the app bundle
        find_app_bundle(&project_root)?
    };

    if !app_path.exists() {
        return Err(anyhow!("App bundle not found: {}", app_path.display()));
    }

    println!("{} Signing {}", "[iOS]".cyan(), app_path.display());

    // Run provisioning and signing
    let result = provisioning::sign_app(&app_path, device.as_deref())?;

    println!();
    println!("{}", "Signing Summary:".green().bold());
    println!("  Bundle ID: {}", result.bundle_id);
    println!("  Team ID: {}", result.team_id);
    println!("  Identity: {}", result.signing_identity);

    // Create IPA if output path specified
    if let Some(output_path) = output {
        let output_path = PathBuf::from(output_path);
        let ipa_path = signer::create_ipa(&app_path, &output_path)?;
        println!("  IPA: {}", ipa_path.display());
    }

    println!();
    println!("{} App signed successfully", "✓".green());

    Ok(())
}

/// Find the app bundle in the project
fn find_app_bundle(project_root: &Path) -> Result<PathBuf> {
    // Check for iOS Swift Package build output
    let ios_dir = project_root.join("ios");
    if ios_dir.exists() {
        for entry in std::fs::read_dir(&ios_dir)? {
            let path = entry?.path();
            if path.is_dir() {
                let build_dir = path.join(".build/arm64-apple-ios");
                if build_dir.exists() {
                    // Check release first, then debug
                    for profile in &["release", "debug"] {
                        let profile_dir = build_dir.join(profile);
                        if profile_dir.exists()
                            && let Some(app) = find_app_in_dir(&profile_dir)?
                        {
                            return Ok(app);
                        }
                    }
                }
            }
        }
    }

    // Check xtool/ directories (created by AppBundler)
    if ios_dir.exists() {
        for entry in std::fs::read_dir(&ios_dir)? {
            let path = entry?.path();
            if path.is_dir() {
                let xtool_dir = path.join("xtool");
                if xtool_dir.exists()
                    && let Some(app) = find_app_in_dir(&xtool_dir)?
                {
                    return Ok(app);
                }
            }
        }
    }

    Err(anyhow!(
        "Could not find app bundle. Please specify the path with --app <path>"
    ))
}

/// Find .app bundle in a directory
fn find_app_in_dir(dir: &Path) -> Result<Option<PathBuf>> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().map(|e| e == "app").unwrap_or(false) {
            return Ok(Some(path));
        }
    }
    Ok(None)
}
