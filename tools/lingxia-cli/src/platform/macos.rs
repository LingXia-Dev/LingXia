//! macOS platform implementation.
//!
//! Builds and runs macOS applications using Swift Package Manager.
//! Simpler than iOS - no signing or device deployment needed.

use super::apple::{self, find_workspace_root};
use super::{
    BuildArtifacts, BuildConfig, BuildProfile, Device, InstallConfig, Platform, RunConfig,
};
use crate::config::MacosConfig;
use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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

    /// Build Rust static library for macOS
    fn build_rust_library(
        &self,
        project_root: &Path,
        workspace_root: &Path,
        config: &BuildConfig,
        arch: &str,
    ) -> Result<PathBuf> {
        let rust_target = Self::rust_target(arch);
        let is_release = matches!(config.profile, BuildProfile::Release);
        let profile_dir = config.profile.as_str();

        if !config.build_native {
            return Ok(workspace_root
                .join("target")
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
            workspace_root,
            &rust_lib_dir,
            rust_target,
            is_release,
            &config.features,
            None, // No deployment target for macOS
        )
    }

    /// Build Swift Package for macOS
    fn swift_build_and_get_bin_dir(
        &self,
        macos_dir: &Path,
        workspace_root: &Path,
        profile: BuildProfile,
        arch: &str,
    ) -> Result<PathBuf> {
        println!("{}", "Building Swift Package for macOS...".cyan());

        let is_release = matches!(profile, BuildProfile::Release);
        let build_config = if is_release { "release" } else { "debug" };

        let mut cmd = Command::new("swift");
        cmd.current_dir(macos_dir)
            .env("LINGXIA_PROJECT_ROOT", workspace_root)
            .env("LINGXIA_BUILD_CONFIG", build_config)
            .args(["build", "--show-bin-path"]);

        // Cross-compile if target arch differs from host
        let host_arch = if cfg!(target_arch = "aarch64") {
            "arm64"
        } else {
            "x86_64"
        };
        if arch != host_arch {
            cmd.args(["--arch", arch]);
        }

        if is_release {
            cmd.args(["-c", "release"]);
        }

        let output = cmd.output().context("Failed to execute swift build")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Swift build failed: {}", stderr.trim()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let bin_path = stdout.trim();
        if bin_path.is_empty() {
            return Err(anyhow!("swift build --show-bin-path returned empty output"));
        }

        println!("  {} Swift build complete", "✓".green());
        Ok(PathBuf::from(bin_path))
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

        // Default to host architecture
        let arch = if cfg!(target_arch = "aarch64") {
            "arm64"
        } else {
            "x86_64"
        };

        // Resolve macOS project directory
        let macos_dir = resolve_macos_dir(&config.project_root, macos_config)?;
        let workspace_root = find_workspace_root(&config.project_root)?;

        println!(
            "{} Building macOS app from {}",
            "[macOS]".cyan(),
            macos_dir.display()
        );

        // Generate Swift bridge if needed
        if config.build_native {
            let rust_target = Self::rust_target(arch);
            apple::generate_swift_bridge(&workspace_root, rust_target)?;
        }

        // Prepare SDK resources
        apple::prepare_sdk_resources(&workspace_root, !config.build_native)?;

        // Build Rust static library
        self.build_rust_library(&config.project_root, &workspace_root, config, arch)?;
        if config.build_native {
            let rust_target = Self::rust_target(arch);
            apple::update_spm_rust_link_stamp(
                &workspace_root,
                rust_target,
                config.profile.as_str(),
            )?;
        }

        // Build Swift Package and get bin dir
        let bin_dir =
            self.swift_build_and_get_bin_dir(&macos_dir, &workspace_root, config.profile, arch)?;

        let mut preferred = Vec::new();
        if let Some(ref macos) = macos_config {
            if let Some(ref name) = macos.executable_name {
                preferred.push(name.clone());
            }
        }
        if let Some(ref cfg) = config.lingxia_config {
            if let Some(app) = cfg.app.as_ref() {
                preferred.push(app.project_name.clone());
            }
        }
        if let Some(dir_name) = macos_dir.file_name().and_then(|n| n.to_str()) {
            preferred.push(dir_name.to_string());
        }

        let executable_path = self.find_executable_in_bin_dir(&bin_dir, &preferred)?;

        let product_name = config
            .lingxia_config
            .as_ref()
            .and_then(|c| c.app.as_ref())
            .map(|a| a.product_name.clone())
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
            .unwrap_or_else(|| "1.0.0".to_string());

        let bundle_id = macos_config
            .and_then(|c| c.bundle_id.clone())
            .or_else(|| {
                config
                    .lingxia_config
                    .as_ref()
                    .and_then(|c| c.ios.as_ref())
                    .map(|c| c.bundle_id.clone())
            })
            .unwrap_or_else(|| "com.example.app".to_string());

        let deployment_target = macos_config
            .and_then(|c| c.deployment_target.clone())
            .unwrap_or_else(|| "14.0".to_string());

        let app_project_name = config
            .lingxia_config
            .as_ref()
            .and_then(|c| c.app.as_ref())
            .map(|a| a.project_name.as_str());
        let target_name = apple::resolve_swiftpm_target_name(
            &macos_dir,
            macos_config.and_then(|c| c.target_name.as_deref()),
            app_project_name,
            "macos",
        )?;

        let info_plist_path = macos_dir.join("Info.plist");
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
            &macos_dir,
            &app_path,
            &deployment_target,
            &target_name,
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

        Ok(BuildArtifacts::MacOs { app_path })
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

    // Copy resource bundles (*.bundle) into Contents/Resources
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
    // 1. Check standard macOS directory
    let macos_dir = project_root.join("macos");
    if macos_dir.join("Package.swift").exists() {
        return Ok(macos_dir);
    }

    // 2. Fallback to iOS directory (shared codebase)
    let ios_dir = project_root.join("ios");
    if ios_dir.join("Package.swift").exists() {
        return Ok(ios_dir);
    }

    Err(anyhow!(
        "macOS Swift Package not found.\n\
         Expected Package.swift in:\n\
         - {}/macos/\n\
         - {}/ios/ (shared)",
        project_root.display(),
        project_root.display()
    ))
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
    let target_name = apple::resolve_swiftpm_target_name(
        &macos_dir,
        macos_config.and_then(|c| c.target_name.as_deref()),
        app_project_name,
        "macos",
    )?;
    crate::appicon::generate_macos_icons(source_icon, &macos_dir, &target_name)
}

/// Get the resources directory path for a macOS Swift Package
pub fn get_resources_dir(
    macos_dir: &Path,
    macos_config: Option<&crate::config::MacosConfig>,
    app_project_name: Option<&str>,
) -> Result<PathBuf> {
    let target_name = apple::resolve_swiftpm_target_name(
        macos_dir,
        macos_config.and_then(|c| c.target_name.as_deref()),
        app_project_name,
        "macos",
    )?;

    Ok(macos_dir
        .join("Sources")
        .join(target_name)
        .join("Resources"))
}
