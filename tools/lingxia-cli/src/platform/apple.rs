//! Common utilities for Apple platforms (iOS/macOS).
//!
//! Provides shared functionality for building, signing, and deploying
//! applications on Apple platforms.

// Submodules
pub mod anisette;
pub mod app_bundle;
pub mod asc;
pub mod auth;
pub mod developer_services;
pub mod devicectl;
pub mod grandslam;
pub mod keychain;
pub mod provisioning;
pub mod signer;
pub mod srp;

use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// Get a shared HTTP agent with native root certificates (using rustls)
pub fn http_agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        use ureq::tls::{RootCerts, TlsConfig};

        // Load native root certificates from the system
        let native_certs = rustls_native_certs::load_native_certs();
        let certs: Vec<ureq::tls::Certificate<'static>> = native_certs
            .certs
            .into_iter()
            .map(|c| {
                let cert = ureq::tls::Certificate::from_der(c.as_ref());
                cert.to_owned()
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
/// - `deployment_target`: Optional iOS deployment target (e.g., "17.0")
pub fn build_rust_staticlib(
    workspace_root: &Path,
    rust_lib_dir: &Path,
    target: &str,
    release: bool,
    features: &[String],
    deployment_target: Option<&str>,
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

    // Set deployment target for iOS to ensure correct minimum version
    if target.contains("ios") {
        let deploy_ver = deployment_target.unwrap_or("17.0");
        cmd.env("IPHONEOS_DEPLOYMENT_TARGET", deploy_ver);
        println!("  {} iOS deployment target: {}", "ℹ".blue(), deploy_ver);
    }

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
    // e.g., lingxia-lib crate -> liblingxia_lib.a
    let target_dir = workspace_root.join("target").join(target).join(profile_dir);

    // Get the crate name from manifest
    let crate_name = get_crate_name(&rust_manifest)?;
    let lib_name = format!("lib{}.a", crate_name.replace('-', "_"));
    let lib_path = target_dir.join(&lib_name);

    if !lib_path.exists() {
        return Err(anyhow!(
            "Static library not found: {}. Expected from crate '{}'",
            lib_path.display(),
            crate_name
        ));
    }

    // Copy/rename to standard name liblingxia.a
    let dest_path = target_dir.join("liblingxia.a");
    if lib_path != dest_path {
        std::fs::copy(&lib_path, &dest_path)?;
        println!("  {} Copied {} -> liblingxia.a", "ℹ".blue(), lib_name);
    }

    println!("  {} Static library → {}", "✓".green(), dest_path.display());

    Ok(dest_path)
}

/// Get crate name from Cargo.toml
fn get_crate_name(manifest_path: &Path) -> Result<String> {
    let content = std::fs::read_to_string(manifest_path).context("Failed to read Cargo.toml")?;

    // Lightweight parse: only consider the [package] table.
    let mut in_package = false;
    for raw_line in content.lines() {
        // Strip comments for simplistic parsing.
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            in_package = line == "[package]";
            continue;
        }

        if !in_package {
            continue;
        }

        let (key, value) = match line.split_once('=') {
            Some((k, v)) => (k.trim(), v.trim()),
            None => continue,
        };
        if key != "name" {
            continue;
        }

        let name = value.trim_matches('"').trim_matches('\'').trim();
        if name.is_empty() {
            break;
        }
        return Ok(name.to_string());
    }

    Err(anyhow!(
        "Could not find package name in {}",
        manifest_path.display()
    ))
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

/// Update a generated Swift source file inside the staged SPM package to force
/// SwiftPM to relink when the external Rust static library changes.
///
/// SwiftPM doesn't reliably track changes to libraries passed via `unsafeFlags`
/// when those libraries live outside the package directory. By writing a small
/// generated `.swift` file whose contents depend on the `liblingxia.a` mtime and
/// size, we ensure a rebuild + relink when native code changes.
pub fn update_spm_rust_link_stamp(
    workspace_root: &Path,
    rust_target: &str,
    build_config: &str,
) -> Result<()> {
    let lib_path = workspace_root
        .join("target")
        .join(rust_target)
        .join(build_config)
        .join("liblingxia.a");

    let meta = std::fs::metadata(&lib_path).with_context(|| {
        format!(
            "Failed to stat Rust static library (expected at {})",
            lib_path.display()
        )
    })?;
    let size = meta.len();
    let modified = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
    let dur = modified
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let modified_secs = dur.as_secs();
    let modified_nanos = dur.subsec_nanos();

    let staged_dir = workspace_root.join("target").join("spm").join("lingxia");
    let stamp_path = staged_dir
        .join("Sources")
        .join("_LingXiaRustLinkStamp.swift");

    std::fs::create_dir_all(
        stamp_path
            .parent()
            .ok_or_else(|| anyhow!("Invalid stamp path: {}", stamp_path.display()))?,
    )?;

    let content = format!(
        "// Generated by lingxia-cli. Do not edit.\n\
         // Forces SwiftPM to relink when Rust native code changes.\n\
         internal enum _LingXiaRustLinkStamp {{\n\
         \tstatic let rustTarget = \"{rust_target}\"\n\
         \tstatic let buildConfig = \"{build_config}\"\n\
         \tstatic let libPath = \"{lib_path}\"\n\
         \tstatic let libSize: UInt64 = {size}\n\
         \tstatic let libModifiedSeconds: UInt64 = {modified_secs}\n\
         \tstatic let libModifiedNanos: UInt32 = {modified_nanos}\n\
         }}\n",
        rust_target = rust_target,
        build_config = build_config,
        lib_path = lib_path.display(),
        size = size,
        modified_secs = modified_secs,
        modified_nanos = modified_nanos
    );

    let write = match std::fs::read_to_string(&stamp_path) {
        Ok(existing) => existing != content,
        Err(_) => true,
    };
    if write {
        std::fs::write(&stamp_path, content)?;
    }

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

/// Recursively copy a directory tree.
///
/// Used by `app_bundle` and `signer` modules.
pub fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<()> {
    if !dest.exists() {
        std::fs::create_dir_all(dest)?;
    }

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let target = dest.join(entry.file_name());

        if path.is_dir() {
            copy_dir_recursive(&path, &target)?;
        } else {
            std::fs::copy(&path, &target)?;
        }
    }

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
