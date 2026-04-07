//! Common utilities for Apple platforms (iOS/macOS).
//!
//! Provides shared functionality for building, signing, and deploying
//! applications on Apple platforms.

// Submodules
pub mod anisette;
pub mod app_bundle;
pub mod asc;
pub mod assets;
pub mod auth;
pub mod capabilities;
pub mod developer_services;
pub mod devicectl;
pub mod grandslam;
pub mod keychain;
pub mod provisioning;
pub mod signer;
pub mod srp;

use crate::commands::rust::{resolve_build_profile, run_cargo_rustc_staticlib_for_target};
use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Get a shared HTTP agent with native root certificates (using rustls)
pub fn http_agent() -> &'static ureq::Agent {
    crate::http_client::shared_native_roots_agent()
}

// Rust cross-compilation target for iOS
pub const IOS_TARGET: &str = "aarch64-apple-ios";

/// Check if running on macOS
pub fn is_macos() -> bool {
    cfg!(target_os = "macos")
}

#[derive(Debug, Clone)]
struct SwiftPmTargetDecl {
    name: String,
    path: Option<String>,
}

#[derive(Debug, Clone)]
struct SwiftPmTargetSelection {
    name: String,
    path: Option<String>,
}

fn parse_swiftpm_targets(package_dir: &Path) -> Result<Vec<SwiftPmTargetDecl>> {
    const TARGET_PREFIXES: [&str; 2] = [".target(", ".executableTarget("];

    let manifest_path = package_dir.join("Package.swift");
    if !manifest_path.exists() {
        return Ok(Vec::new());
    }

    let manifest = fs::read_to_string(&manifest_path)
        .with_context(|| format!("Failed to read {}", manifest_path.display()))?;

    let mut out = Vec::new();
    let mut cursor = 0usize;
    while cursor < manifest.len() {
        let mut next_start: Option<usize> = None;
        let mut matched_prefix = "";

        for prefix in TARGET_PREFIXES {
            if let Some(rel) = manifest[cursor..].find(prefix) {
                let abs = cursor + rel;
                if next_start.is_none_or(|best| abs < best) {
                    next_start = Some(abs);
                    matched_prefix = prefix;
                }
            }
        }

        let Some(start) = next_start else {
            break;
        };
        let open_paren = start + matched_prefix.len() - 1;
        let Some(end) = find_matching_paren(&manifest, open_paren) else {
            break;
        };
        let block = &manifest[start..=end];

        if let Some(name) = find_swift_named_string(block, "name") {
            let path = find_swift_named_string(block, "path");
            out.push(SwiftPmTargetDecl { name, path });
        }

        cursor = end + 1;
    }

    Ok(out)
}

fn find_matching_paren(text: &str, open_paren: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut i = open_paren;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut in_line_comment = false;
    let mut block_comment_depth = 0usize;

    while i < bytes.len() {
        let b = bytes[i];
        let next = bytes.get(i + 1).copied();

        if in_line_comment {
            if b == b'\n' {
                in_line_comment = false;
            }
            i += 1;
            continue;
        }

        if block_comment_depth > 0 {
            if b == b'/' && next == Some(b'*') {
                block_comment_depth += 1;
                i += 2;
                continue;
            }
            if b == b'*' && next == Some(b'/') {
                block_comment_depth -= 1;
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }

        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }

        if b == b'/' && next == Some(b'/') {
            in_line_comment = true;
            i += 2;
            continue;
        }
        if b == b'/' && next == Some(b'*') {
            block_comment_depth = 1;
            i += 2;
            continue;
        }
        if b == b'"' {
            in_string = true;
            i += 1;
            continue;
        }

        if b == b'(' {
            depth += 1;
        } else if b == b')' {
            if depth == 0 {
                return None;
            }
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }

        i += 1;
    }

    None
}

fn find_swift_named_string(block: &str, label: &str) -> Option<String> {
    let needle = format!("{label}:");
    let rel = block.find(&needle)?;
    let value_start = rel + needle.len();
    let rest = &block[value_start..];
    let first_quote = rest.find('"')?;
    let after_quote = &rest[first_quote + 1..];
    let end_quote = after_quote.find('"')?;
    Some(after_quote[..end_quote].to_string())
}

fn resolve_swiftpm_target(
    package_dir: &Path,
    configured: Option<&str>,
    app_project_name: Option<&str>,
) -> Result<SwiftPmTargetSelection> {
    let parsed_targets = parse_swiftpm_targets(package_dir)?;

    if let Some(name) = configured {
        if let Some(found) = parsed_targets.iter().find(|t| t.name == name) {
            return Ok(SwiftPmTargetSelection {
                name: found.name.clone(),
                path: found.path.clone(),
            });
        }
        return Ok(SwiftPmTargetSelection {
            name: name.to_string(),
            path: None,
        });
    }

    if let Some(name) = app_project_name {
        if let Some(found) = parsed_targets.iter().find(|t| t.name == name) {
            return Ok(SwiftPmTargetSelection {
                name: found.name.clone(),
                path: found.path.clone(),
            });
        }
        let candidate = package_dir.join("Sources").join(name);
        if candidate.is_dir() {
            return Ok(SwiftPmTargetSelection {
                name: name.to_string(),
                path: None,
            });
        }
    }

    if parsed_targets.len() == 1 {
        let only = &parsed_targets[0];
        return Ok(SwiftPmTargetSelection {
            name: only.name.clone(),
            path: only.path.clone(),
        });
    }

    let sources_dir = package_dir.join("Sources");
    if sources_dir.is_dir() {
        let mut candidates = Vec::new();
        for entry in fs::read_dir(&sources_dir)? {
            let path = entry?.path();
            if path.is_dir()
                && let Some(name) = path.file_name().and_then(|n| n.to_str())
            {
                candidates.push(name.to_string());
            }
        }
        if candidates.len() == 1 {
            return Ok(SwiftPmTargetSelection {
                name: candidates.remove(0),
                path: None,
            });
        }
    }

    Err(anyhow!(
        "Cannot determine SwiftPM target name from directory: {:?}. \
         Please set 'targetName' in lingxia.config.json for this Apple platform.",
        package_dir
    ))
}

/// Resolve SwiftPM target name for Apple resource locations.
///
/// Resolution order:
/// 1. Explicit config (targetName)
/// 2. App project name if it matches a SwiftPM target name
/// 3. Single SwiftPM target in Package.swift
/// 4. Single directory under Sources/
pub fn resolve_swiftpm_target_name(
    package_dir: &Path,
    configured: Option<&str>,
    app_project_name: Option<&str>,
    _platform_label: &str,
) -> Result<String> {
    Ok(resolve_swiftpm_target(package_dir, configured, app_project_name)?.name)
}

/// Resolve the effective SwiftPM resources directory for an Apple target.
///
/// For targets with `path: "..."`, resources live under `<path>/Resources`.
/// Otherwise this falls back to `Sources/<targetName>/Resources`.
pub fn resolve_swiftpm_resources_dir(
    package_dir: &Path,
    configured: Option<&str>,
    app_project_name: Option<&str>,
    _platform_label: &str,
) -> Result<PathBuf> {
    let target = resolve_swiftpm_target(package_dir, configured, app_project_name)?;
    if let Some(path) = target.path.as_deref() {
        return Ok(package_dir.join(path).join("Resources"));
    }
    Ok(package_dir
        .join("Sources")
        .join(target.name)
        .join("Resources"))
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
/// - `project_root`: Host project root (where target/ directory is located)
/// - `rust_lib_dir`: The crate directory containing Cargo.toml
/// - `deployment_target`: Optional iOS deployment target (e.g., "17.0")
pub fn build_rust_staticlib(
    project_root: &Path,
    rust_lib_dir: &Path,
    target: &str,
    release: bool,
    features: &[String],
    deployment_target: Option<&str>,
) -> Result<PathBuf> {
    println!("{}", "Compiling native static library...".cyan());

    let rust_manifest = rust_lib_dir.join("Cargo.toml");
    if !rust_manifest.exists() {
        return Err(anyhow!(
            "Rust library manifest not found: {}",
            rust_manifest.display()
        ));
    }

    let profile = resolve_build_profile(release);
    run_cargo_rustc_staticlib_for_target(
        &rust_manifest,
        rust_lib_dir,
        &project_root.join("target"),
        target,
        profile,
        features,
        |cmd| {
            if target.contains("ios") {
                let deploy_ver = deployment_target.unwrap_or("17.0");
                cmd.env("IPHONEOS_DEPLOYMENT_TARGET", deploy_ver);
                println!("  {} iOS deployment target: {}", "ℹ".blue(), deploy_ver);
            } else if target.contains("darwin")
                && let Some(deploy_ver) = deployment_target
            {
                cmd.env("MACOSX_DEPLOYMENT_TARGET", deploy_ver);
                println!("  {} macOS deployment target: {}", "ℹ".blue(), deploy_ver);
            }
        },
    )?;

    // Determine output path - force host project's target directory.
    let profile_dir = if release { "release" } else { "debug" };

    let target_dir = project_root.join("target").join(target).join(profile_dir);
    let dest_path = target_dir.join("liblingxia.a");
    if !dest_path.exists() {
        return Err(anyhow!(
            "Static library not found: {}. Expected fixed library name 'liblingxia.a'",
            dest_path.display()
        ));
    }

    println!("  {} Native library → {}", "✓".green(), dest_path.display());

    Ok(dest_path)
}

/// Update a generated Swift source file inside the staged SPM package to force
/// SwiftPM to relink when the external Rust static library changes.
///
/// SwiftPM doesn't reliably track changes to libraries passed via `unsafeFlags`
/// when those libraries live outside the package directory. By writing a small
/// generated `.swift` file whose contents depend on the `liblingxia.a` mtime and
/// size, we ensure a rebuild + relink when native code changes.
pub fn update_spm_rust_link_stamp(
    project_root: &Path,
    sdk_root: &Path,
    rust_target: &str,
    build_config: &str,
) -> Result<()> {
    let lib_path = project_root
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

    let staged_dir = sdk_root.join("target").join("spm").join("lingxia");
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
