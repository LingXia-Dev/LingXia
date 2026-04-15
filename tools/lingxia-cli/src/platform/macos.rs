//! macOS platform implementation.
//!
//! Builds and runs macOS applications using Swift Package Manager.
//! Simpler than iOS - no signing or device deployment needed.

use super::apple::{self};
use super::spm;
use super::{
    BuildArtifacts, BuildConfig, BuildProfile, Device, InstallConfig, Platform, RunConfig,
    resolve_cargo_target_dir,
};
use crate::config::MacosConfig;
use crate::permission_cache::{DEFAULT_MAX_AGE_SECONDS, PermissionCache, PermissionPlatform};
use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, ImageFormat};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

mod doctor;
pub use doctor::doctor_checks;

const MACOS_ARM_TARGET: &str = "aarch64-apple-darwin";
const MACOS_X86_TARGET: &str = "x86_64-apple-darwin";

/// macOS platform implementation
pub struct MacosPlatform;

impl MacosPlatform {
    /// Create a new macOS platform instance
    pub fn new() -> Self {
        Self
    }

    /// Get Rust target for macOS based on architecture
    fn rust_target(arch: &str) -> &'static str {
        match arch {
            "arm64" => MACOS_ARM_TARGET,
            "x86_64" => MACOS_X86_TARGET,
            _ => MACOS_ARM_TARGET, // Default to ARM
        }
    }

    fn swift_triple(arch: &str, deployment_target: &str) -> String {
        format!("{arch}-apple-macosx{deployment_target}")
    }

    /// Build Rust static library for macOS
    fn build_rust_library(
        &self,
        project_root: &Path,
        config: &BuildConfig,
        arch: &str,
        deployment_target: &str,
    ) -> Result<PathBuf> {
        let rust_target = Self::rust_target(arch);
        let is_release = matches!(config.profile, BuildProfile::Release);
        let profile_dir = config.profile.as_str();
        let cargo_target_dir = resolve_cargo_target_dir(project_root);

        if !config.build_native {
            return Ok(cargo_target_dir
                .join(rust_target)
                .join(profile_dir)
                .join("liblingxia.a"));
        }

        if config.lingxia_config.is_none() {
            return Ok(cargo_target_dir
                .join(rust_target)
                .join(profile_dir)
                .join("liblingxia.a"));
        }

        let lingxia_config = config
            .lingxia_config
            .as_ref()
            .ok_or_else(|| anyhow!("lingxia.config.json is required to build native libraries"))?;

        let rust_lib_name = lingxia_config
            .get_rust_lib_name()
            .ok_or_else(|| anyhow!("app.projectName is required in lingxia.config.json"))?;

        let rust_lib_dir = project_root.join(&rust_lib_name);

        apple::build_rust_staticlib(
            project_root,
            &rust_lib_dir,
            rust_target,
            is_release,
            Some(deployment_target),
            &config.native_features,
        )
    }

    /// Build Swift Package for macOS
    fn swift_build_and_get_bin_dir(
        &self,
        macos_dir: &Path,
        project_root: &Path,
        profile: BuildProfile,
        arch: &str,
        deployment_target: &str,
    ) -> Result<PathBuf> {
        println!("{}", "Building Swift Package for macOS...".cyan());

        let is_release = matches!(profile, BuildProfile::Release);
        let build_config = if is_release { "release" } else { "debug" };
        let triple = Self::swift_triple(arch, deployment_target);
        let cargo_target_dir = resolve_cargo_target_dir(project_root);

        let mut build_cmd = Command::new("swift");
        build_cmd
            .current_dir(macos_dir)
            .env("LINGXIA_PROJECT_ROOT", project_root)
            .env("LINGXIA_CARGO_TARGET_DIR", &cargo_target_dir)
            .env("LINGXIA_BUILD_CONFIG", build_config)
            .env("RUNNER_TARGET_TRIPLE", &triple)
            .args(["build", "--disable-sandbox", "--triple", &triple]);

        if is_release {
            build_cmd.args(["-c", "release"]);
        }

        let build_status = build_cmd
            .status()
            .context("Failed to execute swift build for macOS")?;
        if !build_status.success() {
            return Err(anyhow!("Swift build failed"));
        }

        let mut cmd = Command::new("swift");
        cmd.current_dir(macos_dir)
            .env("LINGXIA_PROJECT_ROOT", project_root)
            .env("LINGXIA_CARGO_TARGET_DIR", &cargo_target_dir)
            .env("LINGXIA_BUILD_CONFIG", build_config)
            .env("RUNNER_TARGET_TRIPLE", &triple)
            .args(["build", "--disable-sandbox", "--show-bin-path"]);
        cmd.args(["--triple", &triple]);

        if is_release {
            cmd.args(["-c", "release"]);
        }

        let output = cmd
            .output()
            .context("Failed to execute swift build --show-bin-path")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Swift build failed: {}", stderr.trim()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let bin_path = stdout
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .map(str::trim)
            .unwrap_or("");
        if bin_path.is_empty() {
            return Err(anyhow!("swift build --show-bin-path returned empty output"));
        }

        let bin_dir = PathBuf::from(bin_path);
        if !bin_dir.exists() {
            return Err(anyhow!("SwiftPM bin dir not found: {}", bin_dir.display()));
        }

        println!("  {} Swift build complete", "✓".green());
        Ok(bin_dir)
    }

    fn find_executable_in_bin_dir(
        &self,
        bin_dir: &Path,
        preferred_names: &[String],
    ) -> Result<PathBuf> {
        if !bin_dir.exists() {
            return Err(anyhow!("SwiftPM bin dir not found: {}", bin_dir.display()));
        }

        let mut executables = Vec::new();
        for entry in fs::read_dir(bin_dir)? {
            let path = entry?.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };

            // Skip libraries and build artifacts we might see next to the executable.
            if name.starts_with("lib")
                || name.ends_with(".a")
                || name.ends_with(".dylib")
                || name.ends_with(".o")
                || name.ends_with(".swiftmodule")
                || name.ends_with(".swiftdoc")
                || name.ends_with(".dSYM")
            {
                continue;
            }

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let Ok(meta) = path.metadata() else { continue };
                if meta.permissions().mode() & 0o111 == 0 {
                    continue;
                }
            }

            executables.push(path);
        }

        if executables.is_empty() {
            return Err(anyhow!("No executable found in {}", bin_dir.display()));
        }

        // Prefer explicitly configured names (and a few derived ones).
        for want in preferred_names {
            if want.trim().is_empty() {
                continue;
            }
            if let Some(found) = executables
                .iter()
                .find(|p| p.file_name().and_then(|n| n.to_str()) == Some(want.as_str()))
            {
                return Ok(found.to_path_buf());
            }
        }

        if executables.len() == 1 {
            return Ok(executables.remove(0));
        }

        executables.sort();
        Ok(executables.remove(0))
    }
}

impl Platform for MacosPlatform {
    fn build(&self, config: &BuildConfig) -> Result<BuildArtifacts> {
        apple::ensure_macos()?;

        let macos_config = config
            .lingxia_config
            .as_ref()
            .and_then(|c| c.macos.as_ref());

        let host_arch = if cfg!(target_arch = "aarch64") {
            "arm64"
        } else {
            "x86_64"
        };
        let arch = config.macos_arch.as_deref().unwrap_or(host_arch);
        if arch != "arm64" && arch != "x86_64" {
            return Err(anyhow!(
                "Unsupported macOS arch '{}'. Supported values: arm64, x86_64",
                arch
            ));
        }

        // Resolve macOS project directory
        let macos_dir = resolve_macos_dir(&config.project_root, macos_config)?;
        let sdk_root = config.project_root.clone();
        let info_plist_path = macos_dir.join("Info.plist");
        let standalone_defaults = if config.lingxia_config.is_none() {
            spm::read_package_info_defaults(&info_plist_path).ok()
        } else {
            None
        };

        println!(
            "{} Building macOS app from {}",
            "[macOS]".cyan(),
            macos_dir.display()
        );

        let bundle_id = macos_config
            .and_then(|c| c.bundle_id.clone())
            .or_else(|| {
                config
                    .lingxia_config
                    .as_ref()
                    .and_then(|c| c.ios.as_ref())
                    .map(|c| c.bundle_id.clone())
            })
            .or_else(|| {
                standalone_defaults
                    .as_ref()
                    .and_then(|d| d.bundle_id.clone())
            })
            .unwrap_or_else(|| "com.example.app".to_string());
        let granted_entitlements =
            load_cached_apple_entitlements(PermissionPlatform::Macos, &bundle_id);

        if let Err(err) = warn_missing_restricted_apple_entitlements(&granted_entitlements, "macOS")
        {
            eprintln!("{} {}", "Warning:".yellow(), err);
        }

        if apple::capabilities::sync_macos_capability_files(&macos_dir, &granted_entitlements)? {
            println!(
                "{} Synced macOS capability metadata (Info.plist/App.entitlements)",
                "[macOS]".cyan()
            );
        }

        let deployment_target = macos_config
            .and_then(|c| c.deployment_target.clone())
            .unwrap_or_else(|| "14.0".to_string());

        // Build Rust static library
        let native_lib_path =
            self.build_rust_library(&config.project_root, config, arch, &deployment_target)?;
        if config.build_native && config.lingxia_config.is_some() {
            let rust_target = Self::rust_target(arch);
            apple::update_spm_rust_link_stamp(
                &config.project_root,
                &sdk_root,
                rust_target,
                config.profile.as_str(),
            )?;
        }

        // Build Swift Package and get bin dir
        let mut bin_dir = self.swift_build_and_get_bin_dir(
            &macos_dir,
            &config.project_root,
            config.profile,
            arch,
            &deployment_target,
        )?;

        let mut preferred = Vec::new();
        if let Some(macos) = &macos_config
            && let Some(ref name) = macos.executable_name
        {
            preferred.push(name.clone());
        }
        if let Some(cfg) = &config.lingxia_config
            && let Some(app) = cfg.app.as_ref()
        {
            preferred.push(app.project_name.clone());
        }
        if let Some(dir_name) = macos_dir.file_name().and_then(|n| n.to_str()) {
            preferred.push(dir_name.to_string());
        }

        let mut executable_path = self.find_executable_in_bin_dir(&bin_dir, &preferred)?;
        if config.build_native && executable_needs_native_relink(&executable_path, &native_lib_path)
        {
            println!(
                "  {} Swift executable is older than native library; forcing relink",
                "ℹ".blue()
            );
            let _ = fs::remove_dir_all(macos_dir.join(".build"));
            bin_dir = self.swift_build_and_get_bin_dir(
                &macos_dir,
                &config.project_root,
                config.profile,
                arch,
                &deployment_target,
            )?;
            executable_path = self.find_executable_in_bin_dir(&bin_dir, &preferred)?;
        }

        let product_name = config
            .lingxia_config
            .as_ref()
            .and_then(|c| c.app.as_ref())
            .map(|a| a.product_name.clone())
            .or_else(|| {
                standalone_defaults
                    .as_ref()
                    .and_then(|d| d.product_name.clone())
            })
            .unwrap_or_else(|| {
                executable_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("app")
                    .to_string()
            });

        let product_version = config
            .lingxia_config
            .as_ref()
            .and_then(|c| c.app.as_ref())
            .map(|a| a.product_version.clone())
            .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());

        let app_project_name = config
            .lingxia_config
            .as_ref()
            .and_then(|c| c.app.as_ref())
            .map(|a| a.project_name.as_str())
            .or_else(|| executable_path.file_name().and_then(|n| n.to_str()));
        let resources_dir = get_resources_dir(&macos_dir, macos_config, app_project_name)?;

        let info_plist = if info_plist_path.exists() {
            Some(info_plist_path)
        } else {
            None
        };

        let app_path = create_macos_app_bundle(
            &macos_dir,
            &bin_dir,
            &executable_path,
            &product_name,
            &product_version,
            &bundle_id,
            &deployment_target,
            info_plist.as_ref(),
        )?;

        if let Err(err) = apple::assets::compile_asset_catalog(
            &resources_dir,
            &app_path,
            &deployment_target,
            apple::assets::AssetPlatform::Macos,
        ) {
            eprintln!(
                "  {} Asset catalog compilation failed: {}",
                "Warning:".yellow(),
                err
            );
        }
        if let Err(err) = apple::assets::merge_assetcatalog_plist_with_platform(
            &app_path,
            apple::assets::AssetPlatform::Macos,
        ) {
            eprintln!(
                "  {} Failed to merge asset catalog plist: {}",
                "Warning:".yellow(),
                err
            );
        }
        ensure_sdk_resource_bundles_at_app_root(&bin_dir, &app_path)?;

        let update_zip_path = if config.package {
            Some(create_update_zip(
                &app_path,
                &config.project_root,
                &product_version,
            )?)
        } else {
            None
        };

        let dmg_path = if config.dmg {
            Some(create_dmg(&app_path, &config.project_root)?)
        } else {
            None
        };

        Ok(BuildArtifacts::MacOs {
            app_path,
            update_zip_path,
            dmg_path,
        })
    }

    fn install(&self, _config: &InstallConfig) -> Result<()> {
        // macOS apps don't need installation - they run directly
        println!(
            "{} macOS apps run directly, no installation needed",
            "ℹ".blue()
        );
        Ok(())
    }

    fn uninstall(&self, _package_id: &str, _device_id: Option<&str>) -> Result<()> {
        Err(anyhow!("Uninstall is not supported for macOS apps"))
    }

    fn run(&self, _config: &RunConfig) -> Result<()> {
        Err(anyhow!(
            "macOS apps run directly from the build output.\n\
             Use 'lingxia dev --platform macos' for the full build-and-run workflow."
        ))
    }

    fn list_devices(&self) -> Result<Vec<Device>> {
        // macOS runs on the local machine
        Ok(vec![Device {
            id: "localhost".to_string(),
            name: Some("This Mac".to_string()),
            device_type: super::DeviceType::Physical,
            online: true,
        }])
    }
}

fn load_cached_apple_entitlements(platform: PermissionPlatform, bundle_id: &str) -> Vec<String> {
    let Ok(cache) = PermissionCache::load() else {
        return Vec::new();
    };
    let current = cache
        .get(platform, bundle_id, Some(DEFAULT_MAX_AGE_SECONDS))
        .unwrap_or_default();
    if !current.is_empty() {
        return current;
    }
    if matches!(platform, PermissionPlatform::Macos) {
        return cache
            .get(
                PermissionPlatform::Ios,
                bundle_id,
                Some(DEFAULT_MAX_AGE_SECONDS),
            )
            .unwrap_or_default();
    }
    current
}

fn executable_needs_native_relink(executable_path: &Path, native_lib_path: &Path) -> bool {
    let Ok(executable_modified) = fs::metadata(executable_path).and_then(|m| m.modified()) else {
        return false;
    };
    let Ok(native_modified) = fs::metadata(native_lib_path).and_then(|m| m.modified()) else {
        return false;
    };
    native_modified > executable_modified
}

fn warn_missing_restricted_apple_entitlements(
    granted_entitlements: &[String],
    platform_label: &str,
) -> Result<()> {
    let missing = apple::capabilities::missing_restricted_apple_entitlements(granted_entitlements);
    if missing.is_empty() {
        return Ok(());
    }

    Err(anyhow!(
        "{platform_label} restricted permissions not verified yet: {}.\n\
LingXia will not inject these entitlements until approval is confirmed.",
        missing.join(", ")
    ))
}

#[allow(clippy::too_many_arguments)]
fn create_macos_app_bundle(
    macos_dir: &Path,
    bin_dir: &Path,
    executable_path: &Path,
    product_name: &str,
    product_version: &str,
    bundle_id: &str,
    deployment_target: &str,
    info_plist_path: Option<&PathBuf>,
) -> Result<PathBuf> {
    let app_name = format!("{}.app", product_name);
    let output_dir = macos_dir.join(".lingxia");
    fs::create_dir_all(&output_dir)?;

    let app_bundle = output_dir.join(&app_name);
    let contents_dir = app_bundle.join("Contents");
    let macos_exec_dir = contents_dir.join("MacOS");
    let resources_dir = contents_dir.join("Resources");
    let frameworks_dir = contents_dir.join("Frameworks");

    let _ = fs::remove_dir_all(&app_bundle);
    fs::create_dir_all(&macos_exec_dir)?;
    fs::create_dir_all(&resources_dir)?;

    let executable_name = executable_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow!("Invalid executable name: {}", executable_path.display()))?;
    let exe_dst = macos_exec_dir.join(executable_name);
    fs::copy(executable_path, &exe_dst)?;

    // Copy all SwiftPM resource bundles into Contents/Resources so host
    // runtime asset loading can resolve app.json and other package resources
    // using standard macOS bundle semantics.
    for entry in fs::read_dir(bin_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map(|e| e == "bundle").unwrap_or(false) {
            let dest = resources_dir.join(path.file_name().unwrap());
            apple::copy_dir_recursive(&path, &dest)?;
        }
    }

    // Copy frameworks and dylibs into Contents/Frameworks
    for entry in fs::read_dir(bin_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map(|e| e == "framework").unwrap_or(false) {
            fs::create_dir_all(&frameworks_dir)?;
            let dest = frameworks_dir.join(path.file_name().unwrap());
            apple::copy_dir_recursive(&path, &dest)?;
        }
        if path.extension().map(|e| e == "dylib").unwrap_or(false) {
            fs::create_dir_all(&frameworks_dir)?;
            let dest = frameworks_dir.join(path.file_name().unwrap());
            fs::copy(&path, &dest)?;
        }
    }

    copy_info_plist_localizations(macos_dir, &resources_dir)?;

    generate_macos_info_plist(
        macos_dir,
        &contents_dir,
        product_name,
        product_version,
        bundle_id,
        deployment_target,
        executable_name,
        info_plist_path,
    )?;

    Ok(app_bundle)
}

fn ensure_sdk_resource_bundles_at_app_root(bin_dir: &Path, app_bundle: &Path) -> Result<()> {
    for entry in fs::read_dir(bin_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.extension().is_some_and(|e| e == "bundle") {
            continue;
        }
        let Some(bundle_name) = path.file_name() else {
            return Err(anyhow!("Invalid bundle path: {}", path.display()));
        };
        let bundle_name_str = bundle_name.to_string_lossy();
        if bundle_name_str != "lingxia_lingxia.bundle"
            && bundle_name_str != "LingXia_LingXia.bundle"
        {
            continue;
        }
        let dest = app_bundle.join(bundle_name);
        let _ = fs::remove_dir_all(&dest);
        apple::copy_dir_recursive(&path, &dest)?;
    }
    Ok(())
}

fn copy_info_plist_localizations(source_root: &Path, resources_dir: &Path) -> Result<()> {
    for entry in fs::read_dir(source_root)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.ends_with(".lproj") {
            continue;
        }
        let src_strings = path.join("InfoPlist.strings");
        if !src_strings.exists() {
            continue;
        }
        let dest_dir = resources_dir.join(name);
        fs::create_dir_all(&dest_dir)?;
        let dest_strings = dest_dir.join("InfoPlist.strings");
        fs::copy(&src_strings, &dest_strings).with_context(|| {
            format!(
                "Failed to copy InfoPlist.strings from {} to {}",
                src_strings.display(),
                dest_strings.display()
            )
        })?;
    }
    Ok(())
}

fn create_dmg(app_path: &Path, project_root: &Path) -> Result<PathBuf> {
    let app_name = app_path
        .file_stem()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow!("Invalid app bundle name: {}", app_path.display()))?;
    let app_bundle_name = app_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow!("Invalid app bundle path: {}", app_path.display()))?;
    let dmg_output_dir = project_root.join("dist").join("macos");
    fs::create_dir_all(&dmg_output_dir).with_context(|| {
        format!(
            "Failed to create macOS distribution directory: {}",
            dmg_output_dir.display()
        )
    })?;
    let dmg_path = dmg_output_dir.join(format!("{app_name}.dmg"));

    if dmg_path.exists() {
        fs::remove_file(&dmg_path)
            .with_context(|| format!("Failed to remove existing {}", dmg_path.display()))?;
    }

    println!("  Packaging DMG...");
    let temp_dir = tempfile::tempdir().context("Failed to create temporary directory for DMG")?;
    let stage_dir = temp_dir.path().join("stage");
    fs::create_dir_all(&stage_dir)?;
    let staged_app = stage_dir.join(app_bundle_name);
    apple::copy_dir_recursive(app_path, &staged_app)?;

    // Create Applications symlink for drag-to-install
    let apps_link = stage_dir.join("Applications");
    if apps_link.exists() {
        fs::remove_file(&apps_link).context("Failed to remove existing Applications link")?;
    }
    std::os::unix::fs::symlink("/Applications", &apps_link)
        .context("Failed to create /Applications symlink in DMG staging directory")?;

    // Verify symlink was created
    if !apps_link.exists() || apps_link.read_link().is_err() {
        eprintln!(
            "  {} Applications symlink verification failed",
            "Warning:".yellow()
        );
    } else {
        println!("  {} Created Applications symlink", "✓".green());
    }
    let rw_dmg_path = temp_dir.path().join(format!("{app_name}-rw.dmg"));
    let status = Command::new("hdiutil")
        .arg("create")
        .arg("-quiet")
        .arg("-volname")
        .arg(app_name)
        .arg("-srcfolder")
        .arg(&stage_dir)
        .arg("-ov")
        .arg("-format")
        .arg("UDRW")
        .arg(&rw_dmg_path)
        .status()
        .context("Failed to execute hdiutil create for temporary DMG")?;
    if !status.success() {
        anyhow::bail!("Failed to create temporary writable DMG");
    }

    let mount_point = temp_dir.path().join("mnt");
    fs::create_dir_all(&mount_point)?;
    let attach_status = Command::new("hdiutil")
        .arg("attach")
        .arg("-quiet")
        .arg(&rw_dmg_path)
        .arg("-readwrite")
        .arg("-noverify")
        .arg("-noautoopen")
        .arg("-mountpoint")
        .arg(&mount_point)
        .status()
        .context("Failed to mount temporary DMG")?;
    if !attach_status.success() {
        anyhow::bail!("Failed to mount temporary DMG");
    }

    // Give the system time to register the disk with Finder before AppleScript
    thread::sleep(Duration::from_millis(2000));

    if let Err(err) = configure_dmg_layout(app_name, app_bundle_name) {
        eprintln!(
            "  {} Failed to configure DMG Finder layout: {}",
            "Warning:".yellow(),
            err
        );
        eprintln!(
            "  {} DMG still includes an 'Applications' link for drag-to-install.",
            "Info:".blue()
        );
    }

    // Give Finder a moment to persist the .DS_Store before detach.
    thread::sleep(Duration::from_millis(500));
    detach_mount(&mount_point)?;

    let converted_base = temp_dir.path().join("release");
    let convert_status = Command::new("hdiutil")
        .arg("convert")
        .arg("-quiet")
        .arg(&rw_dmg_path)
        .arg("-ov")
        .arg("-format")
        .arg("UDZO")
        .arg("-o")
        .arg(&converted_base)
        .status()
        .context("Failed to convert DMG to compressed format")?;
    if !convert_status.success() {
        anyhow::bail!("Failed to convert DMG to compressed format");
    }
    let converted_dmg = converted_base.with_extension("dmg");
    if !converted_dmg.exists() {
        return Err(anyhow!(
            "Converted DMG not found at expected path: {}",
            converted_dmg.display()
        ));
    }
    fs::rename(&converted_dmg, &dmg_path)
        .with_context(|| format!("Failed to move DMG to {}", dmg_path.display()))?;

    println!("  {} DMG → {}", "✓".green(), dmg_path.display());
    Ok(dmg_path)
}

fn create_update_zip(
    app_path: &Path,
    project_root: &Path,
    product_version: &str,
) -> Result<PathBuf> {
    let app_name = app_path
        .file_stem()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow!("Invalid app bundle name: {}", app_path.display()))?;
    let output_dir = project_root.join("dist").join("macos");
    fs::create_dir_all(&output_dir).with_context(|| {
        format!(
            "Failed to create macOS distribution directory: {}",
            output_dir.display()
        )
    })?;

    let zip_path = output_dir.join(format!("{app_name}-{product_version}-macos.zip"));
    if zip_path.exists() {
        fs::remove_file(&zip_path)
            .with_context(|| format!("Failed to remove existing {}", zip_path.display()))?;
    }

    println!("  Packaging macOS update ZIP...");
    let parent_dir = app_path.parent().ok_or_else(|| {
        anyhow!(
            "Failed to determine app bundle parent directory for {}",
            app_path.display()
        )
    })?;
    let app_bundle_name = app_path.file_name().ok_or_else(|| {
        anyhow!(
            "Failed to determine app bundle name for {}",
            app_path.display()
        )
    })?;
    let status = Command::new("/usr/bin/ditto")
        .current_dir(parent_dir)
        .args(["-c", "-k", "--sequesterRsrc", "--keepParent"])
        .arg(app_bundle_name)
        .arg(&zip_path)
        .status()
        .context("Failed to execute ditto for macOS update ZIP")?;
    if !status.success() {
        anyhow::bail!("Failed to package macOS update ZIP");
    }

    println!("  {} Update ZIP → {}", "✓".green(), zip_path.display());
    Ok(zip_path)
}

fn configure_dmg_layout(volume_name: &str, app_bundle_name: &str) -> Result<()> {
    let volume_name = escape_applescript_string(volume_name);
    let app_bundle_name = escape_applescript_string(app_bundle_name);
    let script = format!(
        r#"
tell application "Finder"
    set dmgDisk to disk "{volume_name}"
    open dmgDisk
    delay 0.5
    set dmgWindow to container window of dmgDisk
    set current view of dmgWindow to icon view
    set toolbar visible of dmgWindow to false
    set statusbar visible of dmgWindow to false
    set bounds of dmgWindow to {{120, 120, 780, 500}}
    tell icon view options of dmgWindow
        set arrangement to not arranged
        set icon size to 128
        set text size to 14
    end tell
    -- Position app and Applications link (symlink should already exist)
    set position of item "{app_bundle_name}" of dmgWindow to {{180, 220}}
    if exists item "Applications" of dmgWindow then
        set position of item "Applications" of dmgWindow to {{500, 220}}
    end if
    update without registering applications
    delay 0.5
    close dmgWindow
end tell
"#
    );

    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .context("Failed to execute osascript for DMG layout")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            anyhow::bail!("osascript returned non-zero status while setting DMG layout");
        }
        anyhow::bail!(
            "osascript returned non-zero status while setting DMG layout: {}",
            stderr
        );
    }
    Ok(())
}

fn detach_mount(mount_point: &Path) -> Result<()> {
    for _ in 0..5 {
        let status = Command::new("hdiutil")
            .arg("detach")
            .arg("-quiet")
            .arg(mount_point)
            .status()
            .context("Failed to execute hdiutil detach")?;
        if status.success() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(400));
    }

    let force_status = Command::new("hdiutil")
        .arg("detach")
        .arg("-quiet")
        .arg("-force")
        .arg(mount_point)
        .status()
        .context("Failed to execute forced hdiutil detach")?;
    if !force_status.success() {
        anyhow::bail!("Failed to detach mounted DMG volume");
    }
    Ok(())
}

fn escape_applescript_string(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

#[allow(clippy::too_many_arguments)]
fn generate_macos_info_plist(
    package_dir: &Path,
    contents_dir: &Path,
    product_name: &str,
    product_version: &str,
    bundle_id: &str,
    deployment_target: &str,
    executable_name: &str,
    info_plist_path: Option<&PathBuf>,
) -> Result<()> {
    let mut info: std::collections::HashMap<String, plist::Value> =
        std::collections::HashMap::new();

    info.insert("CFBundleInfoDictionaryVersion".into(), "6.0".into());
    info.insert("CFBundleDevelopmentRegion".into(), "en".into());
    info.insert("CFBundleVersion".into(), "1".into());
    info.insert(
        "CFBundleShortVersionString".into(),
        product_version.to_string().into(),
    );
    info.insert("CFBundleIdentifier".into(), bundle_id.to_string().into());
    info.insert("CFBundleName".into(), product_name.to_string().into());
    info.insert(
        "CFBundleDisplayName".into(),
        product_name.to_string().into(),
    );
    info.insert(
        "CFBundleExecutable".into(),
        executable_name.to_string().into(),
    );
    info.insert("CFBundlePackageType".into(), "APPL".into());
    info.insert(
        "CFBundleSupportedPlatforms".into(),
        plist::Value::Array(vec!["MacOSX".into()]),
    );
    info.insert(
        "LSMinimumSystemVersion".into(),
        deployment_target.to_string().into(),
    );
    info.insert("NSHighResolutionCapable".into(), true.into());

    if let Some(plist_path) = info_plist_path {
        let full_path = if plist_path.is_absolute() {
            plist_path.clone()
        } else {
            package_dir.join(plist_path)
        };
        if full_path.exists() {
            let custom: plist::Dictionary = plist::from_file(&full_path)
                .map_err(|e| anyhow!("Failed to parse Info.plist: {}", e))?;
            for (key, value) in custom {
                info.insert(key, value);
            }
        }
    }

    info.insert("CFBundleVersion".into(), "1".into());
    info.insert(
        "CFBundleShortVersionString".into(),
        product_version.to_string().into(),
    );
    info.insert("CFBundleIdentifier".into(), bundle_id.to_string().into());
    info.insert("CFBundleName".into(), product_name.to_string().into());
    info.insert(
        "CFBundleDisplayName".into(),
        product_name.to_string().into(),
    );
    info.insert(
        "CFBundleExecutable".into(),
        executable_name.to_string().into(),
    );
    info.insert(
        "LSMinimumSystemVersion".into(),
        deployment_target.to_string().into(),
    );

    let info_plist_path = contents_dir.join("Info.plist");
    let dict: plist::Dictionary = info.into_iter().collect();
    plist::to_file_xml(info_plist_path, &dict).context("Failed to write Info.plist")?;

    Ok(())
}

pub fn app_bundle_executable(app_path: &Path) -> Result<PathBuf> {
    let info_plist_path = app_path.join("Contents").join("Info.plist");
    let info: plist::Dictionary =
        plist::from_file(&info_plist_path).context("Failed to read Info.plist")?;
    let Some(plist::Value::String(name)) = info.get("CFBundleExecutable") else {
        return Err(anyhow!("CFBundleExecutable not found in Info.plist"));
    };
    Ok(app_path.join("Contents").join("MacOS").join(name))
}

/// Resolve the macOS Swift Package directory.
///
/// Expects Package.swift in:
/// - `{projectRoot}/macos/`
/// - `{projectRoot}/ios/` (shared codebase fallback)
pub(crate) fn resolve_macos_dir(
    project_root: &Path,
    _macos_config: Option<&MacosConfig>,
) -> Result<PathBuf> {
    spm::resolve_apple_swift_package_dir(project_root, "macos", Some("ios"), "macOS")
}

/// Generate macOS app icons
///
/// # Arguments
/// * `project_root` - Project root directory
/// * `source_icon` - Path to source icon image
/// * `macos_config` - Optional macOS configuration from lingxia.config.json
/// * `app_project_name` - Optional app project name (used for SwiftPM target inference)
pub fn generate_icons(
    project_root: &Path,
    source_icon: &Path,
    macos_config: Option<&crate::config::MacosConfig>,
    app_project_name: Option<&str>,
) -> Result<()> {
    let macos_dir = resolve_macos_dir(project_root, macos_config)?;
    let resources_dir = get_resources_dir(&macos_dir, macos_config, app_project_name)?;
    let normalized_icon = build_macos_icon_source(source_icon)?;
    crate::appicon::generate_macos_icons(normalized_icon.path(), &resources_dir)
}

/// Get the resources directory path for a macOS Swift Package
pub fn get_resources_dir(
    macos_dir: &Path,
    macos_config: Option<&crate::config::MacosConfig>,
    app_project_name: Option<&str>,
) -> Result<PathBuf> {
    apple::resolve_swiftpm_resources_dir(
        macos_dir,
        macos_config.and_then(|c| c.target_name.as_deref()),
        app_project_name,
        "macos",
    )
}

fn build_macos_icon_source(source_icon: &Path) -> Result<tempfile::NamedTempFile> {
    if !source_icon.exists() {
        anyhow::bail!("Source icon not found: {:?}", source_icon);
    }

    let img = image::open(source_icon).context("Failed to open source image")?;
    let (width, height) = img.dimensions();
    let canvas_size = width.max(height).max(1);

    let source_visual_ratio = estimate_nontransparent_bounds_ratio(&img);
    const TARGET_DOCK_VISUAL_RATIO: f32 = 0.73;
    let content_scale = (TARGET_DOCK_VISUAL_RATIO / source_visual_ratio).clamp(0.60, 0.92);

    let icon_size = (canvas_size as f32 * content_scale).round().max(1.0) as u32;
    let offset = (canvas_size - icon_size) / 2;
    let mut resized = img
        .resize_exact(icon_size, icon_size, FilterType::Lanczos3)
        .to_rgba8();
    apply_rounded_corner_mask(&mut resized, icon_size as f32 * 0.22);

    let mut canvas = image::RgbaImage::new(canvas_size, canvas_size);
    image::imageops::overlay(&mut canvas, &resized, offset as i64, offset as i64);
    let normalized = DynamicImage::ImageRgba8(canvas);

    let temp = tempfile::Builder::new()
        .prefix("lingxia-macos-icon-")
        .suffix(".png")
        .tempfile()
        .context("Failed to create temporary file for macOS icon generation")?;
    normalized
        .save_with_format(temp.path(), ImageFormat::Png)
        .context("Failed to write temporary normalized macOS icon")?;
    Ok(temp)
}

fn estimate_nontransparent_bounds_ratio(img: &DynamicImage) -> f32 {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut found = false;

    const ALPHA_THRESHOLD: u8 = 12;
    for y in 0..h {
        for x in 0..w {
            let a = rgba.get_pixel(x, y).0[3];
            if a <= ALPHA_THRESHOLD {
                continue;
            }
            found = true;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }

    if !found {
        return 1.0;
    }

    let bw = (max_x - min_x + 1) as f32 / w as f32;
    let bh = (max_y - min_y + 1) as f32 / h as f32;
    bw.max(bh).clamp(0.01, 1.0)
}

fn apply_rounded_corner_mask(img: &mut image::RgbaImage, radius: f32) {
    let (w, h) = img.dimensions();
    let r = radius.clamp(1.0, (w.min(h) as f32) * 0.5);
    let left = r;
    let top = r;
    let right = (w as f32) - r;
    let bottom = (h as f32) - r;

    for y in 0..h {
        for x in 0..w {
            let xf = x as f32 + 0.5;
            let yf = y as f32 + 0.5;
            let cx = if xf < left {
                left
            } else if xf > right {
                right
            } else {
                xf
            };
            let cy = if yf < top {
                top
            } else if yf > bottom {
                bottom
            } else {
                yf
            };

            let dx = xf - cx;
            let dy = yf - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist <= r - 1.0 {
                continue;
            }

            let px = img.get_pixel_mut(x, y);
            if dist >= r {
                px.0[3] = 0;
                continue;
            }

            let edge_alpha = ((r - dist) * 255.0).clamp(0.0, 255.0) as u8;
            let current_alpha = px.0[3];
            px.0[3] = ((current_alpha as u16 * edge_alpha as u16) / 255) as u8;
        }
    }
}
