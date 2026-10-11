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
pub mod env_icon;
pub mod grandslam;
pub mod keychain;
pub mod mobileprovision;
pub mod notarize;
pub mod provisioning;
pub mod settings_bundle;
pub mod signer;
pub mod srp;

use crate::commands::rust::{resolve_build_profile, run_cargo_rustc_staticlib_for_target};
use crate::platform::resolve_cargo_target_dir;
use crate::platform::set_native_client_codegen_env;
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

    let source_backed_targets = parsed_targets
        .iter()
        .filter(|target| {
            target
                .path
                .as_deref()
                .is_some_and(|path| path == "Sources" || path.starts_with("Sources/"))
        })
        .cloned()
        .collect::<Vec<_>>();
    if source_backed_targets.len() == 1 {
        let only = &source_backed_targets[0];
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

/// Active developer directory as `xcrun` resolves it (`DEVELOPER_DIR` wins over `xcode-select`).
pub fn active_developer_dir() -> Option<String> {
    let output = Command::new("xcode-select").arg("-p").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!dir.is_empty()).then_some(dir)
}

/// The Command Line Tools lack devicectl, the iOS SDK and actool; only an Xcode bundle has them.
pub fn is_full_xcode(developer_dir: &str) -> bool {
    developer_dir
        .trim_end_matches('/')
        .ends_with(".app/Contents/Developer")
}

/// How to point the toolchain at a full Xcode, naming installed copies Spotlight finds.
pub fn select_xcode_hint() -> String {
    if std::env::var_os("DEVELOPER_DIR").is_some() {
        return "DEVELOPER_DIR overrides xcode-select; point it at Xcode.app/Contents/Developer or unset it"
            .to_string();
    }
    let output = Command::new("mdfind")
        .arg("kMDItemCFBundleIdentifier == \"com.apple.dt.Xcode\"")
        .output();
    let found: Vec<String> = output
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let quote = |p: &str| {
        if p.contains(' ') {
            format!("'{p}'")
        } else {
            p.to_string()
        }
    };
    match found.as_slice() {
        [] => "Install Xcode 15+ and run: sudo xcode-select -s /path/to/Xcode.app".to_string(),
        [app] => format!("Found {app}; run: sudo xcode-select -s {}", quote(app)),
        apps => format!(
            "Found {}; pick one and run: sudo xcode-select -s <path>",
            apps.join(", ")
        ),
    }
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
    deployment_target: Option<&str>,
    features: &[String],
    default_features: bool,
    native_client_out: Option<&Path>,
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
    let cargo_target_dir = resolve_cargo_target_dir(project_root);
    run_cargo_rustc_staticlib_for_target(
        &rust_manifest,
        rust_lib_dir,
        &cargo_target_dir,
        target,
        profile,
        |cmd| {
            if !default_features {
                cmd.arg("--no-default-features");
            }
            set_native_client_codegen_env(cmd, native_client_out);
            if !features.is_empty() {
                cmd.arg("--features").arg(features.join(","));
            }
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

    let target_dir = cargo_target_dir.join(target).join(profile_dir);
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
    let lib_path = resolve_cargo_target_dir(project_root)
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

    let staged_dir = sdk_root.join(".lingxia").join("spm").join("lingxia");
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
        if is_apple_junk_entry(&entry.file_name()) {
            continue;
        }
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

/// Install the SwiftPM resource bundles from `build_dir` into the app's resource root.
///
/// The host target's bundle is merged into `resource_root` instead of shipped as a
/// bundle: the runtime then finds host assets in the main bundle and never has to
/// infer the bundle name from Info.plist keys that re-signing may rewrite.
pub fn install_resource_bundles(
    package_dir: &Path,
    build_dir: &Path,
    host_target: &str,
    resource_root: &Path,
) -> Result<()> {
    let host_suffix = format!("_{host_target}.bundle");
    let mut host_bundle: Option<PathBuf> = None;
    for entry in fs::read_dir(build_dir)? {
        let path = entry?.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.ends_with(".bundle") {
            continue;
        }
        if name.ends_with(&host_suffix) {
            if let Some(previous) = host_bundle.replace(path.clone()) {
                return Err(anyhow!(
                    "Several resource bundles belong to target `{host_target}`: {} and {}",
                    previous.display(),
                    path.display()
                ));
            }
            continue;
        }
        copy_dir_recursive(&path, &resource_root.join(name))?;
    }

    let Some(host_bundle) = host_bundle else {
        // A missed match would ship an app without its assets and white-screen at launch.
        let declared = resolve_swiftpm_resources_dir(package_dir, Some(host_target), None, "")?;
        if declared.is_dir() {
            return Err(anyhow!(
                "Target `{host_target}` declares resources at {} but SwiftPM built no `*_{host_target}.bundle` in {}",
                declared.display(),
                build_dir.display()
            ));
        }
        return Ok(());
    };
    // `.copy("Resources")` nests the payload one level down.
    let payload = host_bundle.join("Resources");
    let payload = if payload.is_dir() {
        payload
    } else {
        host_bundle
    };
    for entry in fs::read_dir(&payload)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == "Info.plist" || is_apple_junk_entry(&name) {
            continue;
        }
        let path = entry.path();
        let target = resource_root.join(&name);
        if path.is_dir() {
            copy_dir_recursive(&path, &target)?;
        } else {
            fs::copy(&path, &target)?;
        }
    }
    Ok(())
}

fn is_apple_junk_entry(name: &std::ffi::OsStr) -> bool {
    let Some(name) = name.to_str() else {
        return false;
    };
    name == ".DS_Store" || name == "__MACOSX" || name.starts_with("._")
}
/// Identifies a CLI-owned dependency rather than an explicitly vendored SDK.
const SDK_PACKAGE_MARKER: &str = "// lingxia-sdk: managed by `lingxia build`";

/// Apply macOS deployment-target overrides only for the duration of SwiftPM.
/// The SDK dependency itself is stable and never needs manifest rewriting.
pub(crate) fn with_temporary_package_manifest<T>(
    package_dir: &Path,
    operation: impl FnOnce() -> Result<T>,
) -> Result<T> {
    let manifest_path = package_dir.join("Package.swift");
    let original = fs::read(&manifest_path)
        .with_context(|| format!("Failed to read Package.swift: {}", manifest_path.display()))?;

    let result = operation();
    let restore = (|| -> Result<()> {
        let current = fs::read(&manifest_path).ok();
        if current.as_deref() != Some(original.as_slice()) {
            fs::write(&manifest_path, &original).with_context(|| {
                format!(
                    "Failed to restore Package.swift: {}",
                    manifest_path.display()
                )
            })?;
        }
        Ok(())
    })();

    match (result, restore) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(error),
        (Err(error), Err(restore_error)) => Err(error).context(format!(
            "Build failed and Package.swift could not be restored: {restore_error:#}"
        )),
    }
}

/// Keep SwiftPM's link target aligned with the deployment target selected from
/// `lingxia.yaml`. `swift build --triple` chooses the architecture, but SwiftPM
/// still takes the minimum macOS version from `Package.swift`.
pub(crate) fn sync_macos_deployment_target(
    package_dir: &Path,
    deployment_target: &str,
) -> Result<()> {
    if deployment_target.is_empty()
        || !deployment_target
            .split('.')
            .all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
    {
        return Err(anyhow!(
            "Invalid macOS deployment target `{deployment_target}`"
        ));
    }

    let manifest_path = package_dir.join("Package.swift");
    let original = fs::read_to_string(&manifest_path)
        .with_context(|| format!("Failed to read Package.swift: {}", manifest_path.display()))?;
    let replacement = match deployment_target {
        "10.10" => ".macOS(.v10_10)".to_string(),
        "10.11" => ".macOS(.v10_11)".to_string(),
        "10.12" => ".macOS(.v10_12)".to_string(),
        "10.13" => ".macOS(.v10_13)".to_string(),
        "10.14" => ".macOS(.v10_14)".to_string(),
        "10.15" => ".macOS(.v10_15)".to_string(),
        "11.0" => ".macOS(.v11)".to_string(),
        "12.0" => ".macOS(.v12)".to_string(),
        "13.0" => ".macOS(.v13)".to_string(),
        "14.0" => ".macOS(.v14)".to_string(),
        "15.0" => ".macOS(.v15)".to_string(),
        _ => format!(".macOS(\"{deployment_target}\")"),
    };

    let mut rewritten = String::with_capacity(original.len());
    let mut matches = 0;
    for line in original.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("//")
            && let Some(start) = line.find(".macOS(")
            && let Some(relative_end) = line[start..].find(')')
        {
            let end = start + relative_end + 1;
            rewritten.push_str(&line[..start]);
            rewritten.push_str(&replacement);
            rewritten.push_str(&line[end..]);
            rewritten.push('\n');
            matches += 1;
            continue;
        }
        rewritten.push_str(line);
        rewritten.push('\n');
    }

    if matches != 1 {
        return Err(anyhow!(
            "Expected exactly one macOS platform declaration in {}, found {matches}",
            manifest_path.display()
        ));
    }
    if rewritten != original {
        fs::write(&manifest_path, rewritten).with_context(|| {
            format!("Failed to write Package.swift: {}", manifest_path.display())
        })?;
    }
    Ok(())
}

/// Fixed SDK dependency in the generated iOS/macOS manifests. The link lives
/// under the host project's ignored `.lingxia/`, not in either source package.
const SDK_PACKAGE_PATH: &str = "../.lingxia/sdk/apple";

// Normalize layout without treating comments or spaces inside strings as code.
fn package_declarations(manifest: &str) -> Vec<String> {
    let mut source = String::new();
    let mut chars = manifest.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                source.push(c);
                while let Some(c) = chars.next() {
                    source.push(c);
                    if c == '\\' {
                        if let Some(escaped) = chars.next() {
                            source.push(escaped);
                        }
                    } else if c == '"' {
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'/') => {
                chars.next();
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut depth = 1;
                while let Some(c) = chars.next() {
                    if c == '/' && chars.peek() == Some(&'*') {
                        chars.next();
                        depth += 1;
                    } else if c == '*' && chars.peek() == Some(&'/') {
                        chars.next();
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                }
            }
            c if c.is_whitespace() => {}
            c => source.push(c),
        }
    }
    source
        .split(".package(")
        .skip(1)
        .filter_map(|rest| rest.split_once(')').map(|(args, _)| args.to_owned()))
        .collect()
}

fn uses_sdk_link(manifest: &str) -> bool {
    package_declarations(manifest).iter().any(|args| {
        args.contains("name:\"lingxia\"") && args.contains(&format!("path:\"{SDK_PACKAGE_PATH}\""))
    })
}

fn sdk_link_path(package_dir: &Path) -> PathBuf {
    package_dir.join(SDK_PACKAGE_PATH)
}

/// Prepare the project-local link without writing Package.swift. Updating an
/// existing link is atomic on the Apple build host; never replace user files.
pub(crate) fn prepare_sdk_package_link(package_dir: &Path, sdk_dir: &Path) -> Result<()> {
    let manifest = fs::read_to_string(package_dir.join("Package.swift"))?;
    if sdk_package_is_hand_wired(package_dir) {
        return Ok(());
    }
    if !uses_sdk_link(&manifest) {
        return Err(anyhow!(
            "{} must declare .package(name: \"lingxia\", path: \"{SDK_PACKAGE_PATH}\") and its lingxia product dependency",
            package_dir.join("Package.swift").display()
        ));
    }
    let sdk_dir = sdk_dir
        .canonicalize()
        .context("Resolve Apple SDK directory")?;
    if !sdk_dir.join("Package.swift").is_file() {
        return Err(anyhow!(
            "Apple SDK has no Package.swift: {}",
            sdk_dir.display()
        ));
    }
    let link = sdk_link_path(package_dir);
    match fs::symlink_metadata(&link) {
        Ok(metadata) if !is_sdk_directory_link(&link, &metadata) => {
            return Err(anyhow!(
                "Refusing to replace non-symlink SDK path: {}",
                link.display()
            ));
        }
        Ok(_) if link.canonicalize().ok().as_ref() == Some(&sdk_dir) => return Ok(()),
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let parent = link.parent().expect("SDK link has a parent");
    fs::create_dir_all(parent)?;
    let staging = tempfile::tempdir_in(parent)?;
    let staged_link = staging.path().join("apple");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&sdk_dir, &staged_link)?;
    #[cfg(windows)]
    {
        create_windows_sdk_link(&sdk_dir, &staged_link)?;
        if fs::symlink_metadata(&link).is_ok() {
            fs::remove_dir(&link)?;
        }
    }
    fs::rename(&staged_link, &link)
        .with_context(|| format!("Install Apple SDK link: {}", link.display()))?;
    Ok(())
}

fn is_sdk_directory_link(path: &Path, metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        metadata.file_type().is_symlink() || junction::get_target(path).is_ok()
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        metadata.file_type().is_symlink()
    }
}

#[cfg(windows)]
fn create_windows_sdk_link(target: &Path, link: &Path) -> Result<()> {
    if std::os::windows::fs::symlink_dir(target, link).is_ok() {
        return Ok(());
    }
    // Native junction creation handles literal paths without shell expansion
    // and does not require Developer Mode or elevation.
    junction::create(target, link).context("Create Apple SDK directory junction")
}

pub(crate) fn sdk_package_points_at(package_dir: &Path, sdk_dir: &Path) -> bool {
    let Ok(manifest) = fs::read_to_string(package_dir.join("Package.swift")) else {
        return false;
    };
    uses_sdk_link(&manifest)
        && matches!((sdk_link_path(package_dir).canonicalize(), sdk_dir.canonicalize()),
            (Ok(actual), Ok(expected)) if actual == expected)
}

/// Explicitly vendored/source SDK dependencies remain under the app's control.
pub(crate) fn sdk_package_is_hand_wired(package_dir: &Path) -> bool {
    fs::read_to_string(package_dir.join("Package.swift")).is_ok_and(|manifest| {
        !uses_sdk_link(&manifest)
            && !manifest.contains(SDK_PACKAGE_MARKER)
            && package_declarations(&manifest)
                .iter()
                .any(|args| args.contains("name:\"lingxia\""))
    })
}

/// Fetch the selected SDK and refresh the ignored project link. No manifest
/// mutation is needed; in-workspace and explicitly vendored SDKs stay as-is.
pub fn ensure_sdk_package_dependency(project_root: &Path, package_dir: &Path) -> Result<()> {
    if super::is_inside_lingxia_workspace(project_root) || sdk_package_is_hand_wired(package_dir) {
        return Ok(());
    }
    // Fail before a download if the manifest doesn't use the current template.
    if !uses_sdk_link(&fs::read_to_string(package_dir.join("Package.swift"))?) {
        return Err(anyhow!(
            "Use .package(name: \"lingxia\", path: \"{SDK_PACKAGE_PATH}\") in {}",
            package_dir.join("Package.swift").display()
        ));
    }
    let version = crate::sdk_cache::sdk_version();
    let sdk_dir = crate::sdk_cache::ensure_sdk(crate::sdk_cache::SdkPlatform::Apple, &version)?;
    prepare_sdk_package_link(package_dir, &sdk_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn host_resource_bundle_is_merged_into_resource_root() {
        let dir = TempDir::new().unwrap();
        let build = dir.path().join("build");
        let host = build.join("lxapp_lxapp.bundle/Resources");
        fs::create_dir_all(host.join("lxapps/home")).unwrap();
        fs::write(host.join("app.json"), "{}").unwrap();
        fs::write(host.join("lxapps/home/index.html"), "").unwrap();
        let sdk = build.join("lingxia_lingxia.bundle");
        fs::create_dir_all(&sdk).unwrap();
        fs::write(sdk.join("404.html"), "").unwrap();
        let root = dir.path().join("App.app");
        fs::create_dir_all(&root).unwrap();

        install_resource_bundles(dir.path(), &build, "lxapp", &root).unwrap();

        assert!(root.join("app.json").is_file());
        assert!(root.join("lxapps/home/index.html").is_file());
        assert!(!root.join("lxapp_lxapp.bundle").exists());
        assert!(root.join("lingxia_lingxia.bundle/404.html").is_file());
    }

    #[test]
    fn declared_host_resources_without_a_bundle_are_rejected() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("Sources/lxapp/Resources")).unwrap();
        let build = dir.path().join("build");
        fs::create_dir_all(build.join("lingxia_lingxia.bundle")).unwrap();
        assert!(install_resource_bundles(dir.path(), &build, "lxapp", dir.path()).is_err());
    }

    #[test]
    fn several_bundles_for_the_host_target_are_rejected() {
        let dir = TempDir::new().unwrap();
        let build = dir.path().join("build");
        fs::create_dir_all(build.join("a_lxapp.bundle")).unwrap();
        fs::create_dir_all(build.join("b_lxapp.bundle")).unwrap();
        assert!(install_resource_bundles(dir.path(), &build, "lxapp", dir.path()).is_err());
    }

    #[test]
    fn test_is_macos() {
        // This will be true when running tests on macOS
        #[cfg(target_os = "macos")]
        assert!(is_macos());

        #[cfg(not(target_os = "macos"))]
        assert!(!is_macos());
    }

    #[test]
    fn apple_junk_filter_is_narrow() {
        assert!(is_apple_junk_entry(std::ffi::OsStr::new(".DS_Store")));
        assert!(is_apple_junk_entry(std::ffi::OsStr::new("__MACOSX")));
        assert!(is_apple_junk_entry(std::ffi::OsStr::new("._Icon")));
        assert!(!is_apple_junk_entry(std::ffi::OsStr::new(".well-known")));
        assert!(!is_apple_junk_entry(std::ffi::OsStr::new(".config")));
    }

    #[test]
    fn sync_macos_target_rewrites_the_swiftpm_platform() {
        let pkg = TempDir::new().unwrap();
        write_manifest(pkg.path(), MACOS_TEMPLATE);

        sync_macos_deployment_target(pkg.path(), "14.0").unwrap();
        let once = fs::read_to_string(pkg.path().join("Package.swift")).unwrap();
        assert!(once.contains(".macOS(.v14)"));
        assert!(!once.contains(".macOS(.v12)"));

        sync_macos_deployment_target(pkg.path(), "14.0").unwrap();
        let twice = fs::read_to_string(pkg.path().join("Package.swift")).unwrap();
        assert_eq!(once, twice);
    }

    #[test]
    fn sync_macos_target_supports_custom_versions() {
        let pkg = TempDir::new().unwrap();
        write_manifest(pkg.path(), MACOS_TEMPLATE);

        sync_macos_deployment_target(pkg.path(), "14.2").unwrap();
        let manifest = fs::read_to_string(pkg.path().join("Package.swift")).unwrap();
        assert!(manifest.contains(".macOS(\"14.2\")"));
    }

    #[test]
    fn temporary_manifest_restores_after_success() {
        let pkg = TempDir::new().unwrap();
        write_manifest(pkg.path(), MACOS_TEMPLATE);
        let original = fs::read(pkg.path().join("Package.swift")).unwrap();

        let value = with_temporary_package_manifest(pkg.path(), || {
            sync_macos_deployment_target(pkg.path(), "14.0")?;
            Ok(42)
        })
        .unwrap();

        assert_eq!(value, 42);
        assert_eq!(
            fs::read(pkg.path().join("Package.swift")).unwrap(),
            original
        );
    }

    #[test]
    fn temporary_manifest_restores_after_error() {
        let pkg = TempDir::new().unwrap();
        write_manifest(pkg.path(), MACOS_TEMPLATE);
        let original = fs::read(pkg.path().join("Package.swift")).unwrap();

        let error = with_temporary_package_manifest(pkg.path(), || -> Result<()> {
            sync_macos_deployment_target(pkg.path(), "14.0")?;
            Err(anyhow!("swift build failed"))
        })
        .unwrap_err();

        assert!(error.to_string().contains("swift build failed"));
        assert_eq!(
            fs::read(pkg.path().join("Package.swift")).unwrap(),
            original
        );
    }

    /// The shipped templates are the real input to the injector; matching on a
    /// hand-copied replica would let template edits silently break external
    /// builds (in-workspace projects never exercise this path).
    const IOS_TEMPLATE: &str = include_str!("../../templates/ios/Package.swift");
    const MACOS_TEMPLATE: &str = include_str!("../../templates/macos/Package.swift");

    fn write_manifest(package_dir: &Path, body: &str) {
        fs::write(package_dir.join("Package.swift"), body).unwrap();
    }

    fn package_fixture() -> (TempDir, PathBuf) {
        let root = TempDir::new().unwrap();
        let package = root.path().join("macos");
        fs::create_dir(&package).unwrap();
        write_manifest(&package, MACOS_TEMPLATE);
        (root, package)
    }

    fn sdk_fixture() -> TempDir {
        let sdk = TempDir::new().unwrap();
        fs::write(sdk.path().join("Package.swift"), "// test SDK").unwrap();
        sdk
    }

    #[test]
    fn templates_use_the_stable_sdk_dependency() {
        for template in [IOS_TEMPLATE, MACOS_TEMPLATE] {
            assert!(uses_sdk_link(template));
            assert!(template.contains(".product(name: \"lingxia\", package: \"lingxia\")"));
            assert!(!template.contains("// .product"));
        }
    }

    #[test]
    fn multiline_managed_and_vendored_dependencies_are_recognized() {
        let (_root, package) = package_fixture();
        let sdk = sdk_fixture();
        let managed = MACOS_TEMPLATE.replace(
            ".package(name: \"lingxia\", path: \"../.lingxia/sdk/apple\")",
            ".package(\n name: /* SDK */ \"lingxia\",\n // relative link\n path: \"../.lingxia/sdk/apple\"\n)",
        );
        write_manifest(&package, &managed);
        assert!(uses_sdk_link(&managed));
        assert!(!sdk_package_is_hand_wired(&package));
        prepare_sdk_package_link(&package, sdk.path()).unwrap();
        assert!(sdk_package_points_at(&package, sdk.path()));
        assert_eq!(
            fs::read_to_string(package.join("Package.swift")).unwrap(),
            managed
        );
        let vendored = managed
            .replace(SDK_PACKAGE_PATH, "../vendor/apple")
            .replace(SDK_PACKAGE_MARKER, "");
        write_manifest(&package, &vendored);
        assert!(sdk_package_is_hand_wired(&package));
        assert!(!uses_sdk_link(&vendored));
        assert!(!uses_sdk_link(
            "// .package(name: \"lingxia\", path: \"../.lingxia/sdk/apple\")"
        ));
    }

    #[cfg(windows)]
    #[test]
    fn sdk_junction_can_be_recognized_retargeted_and_repaired() {
        let (root, _) = package_fixture();
        let package = root.path().join("R&D %SDK% (测试)").join("macos");
        fs::create_dir_all(&package).unwrap();
        write_manifest(&package, MACOS_TEMPLATE);
        let cache = TempDir::new().unwrap();
        let a = cache.path().join("SDK & %TARGET% (测试)");
        fs::create_dir(&a).unwrap();
        fs::write(a.join("Package.swift"), "// test SDK").unwrap();
        let link = sdk_link_path(&package);
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        junction::create(&a, &link).unwrap();
        assert!(is_sdk_directory_link(
            &link,
            &fs::symlink_metadata(&link).unwrap()
        ));
        prepare_sdk_package_link(&package, &a).unwrap();
        fs::remove_dir_all(&a).unwrap();
        let b = sdk_fixture();
        prepare_sdk_package_link(&package, b.path()).unwrap();
        assert!(sdk_package_points_at(&package, b.path()));
        assert!(b.path().join("Package.swift").is_file());
    }

    #[test]
    fn sdk_link_refresh_preserves_manifest_and_is_idempotent() {
        let (_root, package) = package_fixture();
        let original = fs::read(package.join("Package.swift")).unwrap();
        let a = sdk_fixture();
        prepare_sdk_package_link(&package, a.path()).unwrap();
        assert!(sdk_package_points_at(&package, a.path()));
        let metadata = fs::symlink_metadata(sdk_link_path(&package)).unwrap();
        prepare_sdk_package_link(&package, a.path()).unwrap();
        assert_eq!(
            metadata.modified().unwrap(),
            fs::symlink_metadata(sdk_link_path(&package))
                .unwrap()
                .modified()
                .unwrap()
        );
        let b = sdk_fixture();
        prepare_sdk_package_link(&package, b.path()).unwrap();
        assert!(sdk_package_points_at(&package, b.path()));
        assert!(!sdk_package_points_at(&package, a.path()));
        assert_eq!(fs::read(package.join("Package.swift")).unwrap(), original);
        assert!(!sdk_package_is_hand_wired(&package));
    }

    #[test]
    fn sdk_link_recovers_when_previous_cache_was_removed() {
        let (_root, package) = package_fixture();
        let a = sdk_fixture();
        prepare_sdk_package_link(&package, a.path()).unwrap();
        drop(a);
        let b = sdk_fixture();
        prepare_sdk_package_link(&package, b.path()).unwrap();
        assert!(sdk_package_points_at(&package, b.path()));
    }

    #[test]
    fn sdk_link_never_overwrites_user_files_or_directories() {
        let (_root, package) = package_fixture();
        let sdk = sdk_fixture();
        let link = sdk_link_path(&package);
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        fs::write(&link, "keep me").unwrap();
        assert!(prepare_sdk_package_link(&package, sdk.path()).is_err());
        assert_eq!(fs::read_to_string(&link).unwrap(), "keep me");
        fs::remove_file(&link).unwrap();
        fs::create_dir(&link).unwrap();
        fs::write(link.join("keep"), "keep me").unwrap();
        assert!(prepare_sdk_package_link(&package, sdk.path()).is_err());
        assert!(link.join("keep").is_file());
    }

    #[test]
    fn sdk_link_leaves_vendored_dependencies_alone() {
        let (_root, package) = package_fixture();
        let manifest = MACOS_TEMPLATE
            .replace("../.lingxia/sdk/apple", "../vendor/apple")
            .replace(SDK_PACKAGE_MARKER, "");
        write_manifest(&package, &manifest);
        assert!(sdk_package_is_hand_wired(&package));
        prepare_sdk_package_link(&package, Path::new("/not-downloaded")).unwrap();
        assert_eq!(
            fs::read_to_string(package.join("Package.swift")).unwrap(),
            manifest
        );
        assert!(!sdk_link_path(&package).exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn swiftpm_builds_through_the_link_and_observes_sdk_changes() {
        let (_root, package) = package_fixture();
        let host_manifest = r#"// swift-tools-version: 6.0
import PackageDescription
let package = Package(name: "host", dependencies: [
    .package(name: "lingxia", path: "../.lingxia/sdk/apple")
], targets: [.executableTarget(name: "host", dependencies: [
    .product(name: "lingxia", package: "lingxia")
])])
"#;
        write_manifest(&package, host_manifest);
        fs::create_dir_all(package.join("Sources/host")).unwrap();
        fs::write(
            package.join("Sources/host/main.swift"),
            "import lingxia\nprint(sdkValue)\n",
        )
        .unwrap();
        let a = sdk_fixture();
        let b = sdk_fixture();
        for (sdk, value) in [(a.path(), "first"), (b.path(), "second")] {
            fs::write(
                sdk.join("Package.swift"),
                r#"// swift-tools-version: 6.0
import PackageDescription
let package = Package(name: "lingxia", products: [.library(name: "lingxia", targets: ["lingxia"])],
    targets: [.target(name: "lingxia", swiftSettings: [.unsafeFlags(["-D", "SDK_TEST"])])])
"#,
            )
            .unwrap();
            fs::create_dir_all(sdk.join("Sources/lingxia")).unwrap();
            fs::write(
                sdk.join("Sources/lingxia/Value.swift"),
                format!("public let sdkValue = \"{value}\"\n"),
            )
            .unwrap();
            prepare_sdk_package_link(&package, sdk).unwrap();
            let output = std::process::Command::new("swift")
                .args(["run", "--package-path"])
                .arg(&package)
                .arg("host")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), value);
            assert_eq!(
                fs::read_to_string(package.join("Package.swift")).unwrap(),
                host_manifest
            );
        }
    }

    #[test]
    fn unsupported_manifest_fails_without_rewriting_it() {
        let (_root, package) = package_fixture();
        let manifest = "// swift-tools-version: 6.0\n";
        write_manifest(&package, manifest);
        assert!(prepare_sdk_package_link(&package, Path::new("/not-downloaded")).is_err());
        assert_eq!(
            fs::read_to_string(package.join("Package.swift")).unwrap(),
            manifest
        );
        assert!(!sdk_link_path(&package).exists());
    }
}
