//! iOS platform implementation.
//!
//! Builds, signs, and deploys iOS applications using Swift Package Manager.

use super::apple::{self, IOS_TARGET, find_workspace_root};
use super::{
    BuildArtifacts, BuildConfig, BuildProfile, Device, InstallConfig, Platform, RunConfig,
};
use crate::config::IosConfig;
use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// iOS resources directory relative path within Swift Package
pub const IOS_RESOURCES_REL_PATH: &str = "Sources/lxapp/Resources";

/// iOS platform implementation
pub struct IosPlatform;

impl IosPlatform {
    /// Create a new iOS platform instance
    pub fn new() -> Self {
        Self
    }

    /// Resolve the iOS Swift Package directory
    ///
    /// Supports both layouts:
    /// - Multi-platform layout: `{projectRoot}/ios/{packageName}/` (contains Package.swift)
    /// - Configured path: `{projectRoot}/{swiftPackagePath}/`
    fn resolve_ios_dir(
        &self,
        project_root: &Path,
        ios_config: Option<&IosConfig>,
    ) -> Result<PathBuf> {
        // 1. Check configured path first
        if let Some(config) = ios_config {
            if let Some(ref pkg_path) = config.swift_package_path {
                let configured_dir = project_root.join(pkg_path);
                if configured_dir.join("Package.swift").exists() {
                    return Ok(configured_dir);
                }
            }
        }

        // 2. Check multi-platform layout: {projectRoot}/ios/*/Package.swift
        let ios_dir = project_root.join("ios");
        if ios_dir.exists() && ios_dir.is_dir() {
            for entry in fs::read_dir(&ios_dir)? {
                let path = entry?.path();
                if path.is_dir() && path.join("Package.swift").exists() {
                    return Ok(path);
                }
            }
        }

        // 3. Check root directory
        if project_root.join("Package.swift").exists() {
            return Ok(project_root.to_path_buf());
        }

        Err(anyhow!(
            "iOS Swift Package not found.\n\
             Expected Package.swift in:\n\
             - {}/ios/<package>/\n\
             - {} (configured via ios.swiftPackagePath)\n\
             - {} (root)",
            project_root.display(),
            ios_config
                .and_then(|c| c.swift_package_path.as_deref())
                .unwrap_or("<not configured>"),
            project_root.display()
        ))
    }

    /// Build Rust static library for iOS
    ///
    /// - `project_root`: Where to find the Rust library (e.g., examples/)
    /// - `workspace_root`: Where to output the built library (e.g., workspace target/)
    fn build_rust_library(
        &self,
        project_root: &Path,
        workspace_root: &Path,
        config: &BuildConfig,
    ) -> Result<PathBuf> {
        if !config.build_native {
            // Return expected path even if not building
            let profile_dir = config.profile.as_str();
            return Ok(workspace_root
                .join("target")
                .join(IOS_TARGET)
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
            IOS_TARGET,
            matches!(config.profile, BuildProfile::Release),
            &config.features,
        )
    }

    /// Prepare app resources (app.json, homelxapp, etc.)
    fn prepare_app_resources(
        &self,
        project_root: &Path,
        ios_dir: &Path,
        config: &BuildConfig,
    ) -> Result<()> {
        println!("{}", "Preparing app resources...".cyan());

        // Find the Resources directory
        let resources_dir = ios_dir.join(IOS_RESOURCES_REL_PATH);
        fs::create_dir_all(&resources_dir)?;

        // Clear existing resources
        if resources_dir.exists() {
            for entry in fs::read_dir(&resources_dir)? {
                let path = entry?.path();
                if path.is_dir() {
                    fs::remove_dir_all(&path)?;
                } else {
                    fs::remove_file(&path)?;
                }
            }
        }

        // Generate app.json
        if let Some(ref lingxia_config) = config.lingxia_config {
            self.write_app_json(lingxia_config, &resources_dir)?;
        }

        // Build and copy home LxApp
        if let Some(ref lingxia_config) = config.lingxia_config {
            if let Some(ref app) = lingxia_config.app {
                self.build_and_copy_homelxapp(project_root, &app.home_lxapp_id, &resources_dir)?;
            }
        }

        println!(
            "  {} Resources prepared → {}",
            "✓".green(),
            resources_dir.display()
        );
        Ok(())
    }

    /// Write app.json configuration file
    fn write_app_json(
        &self,
        config: &crate::config::LingXiaConfig,
        resources_dir: &Path,
    ) -> Result<()> {
        use serde::Serialize;

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct AppJson<'a> {
            product_name: &'a str,
            product_version: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            api_server: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            api_key: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            api_secret: Option<String>,
            #[serde(rename = "homeLxAppID")]
            home_lxapp_id: &'a str,
            #[serde(rename = "homeLxAppVersion")]
            home_lxapp_version: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            cache_max_age_days: Option<u64>,
        }

        let app = config
            .app
            .as_ref()
            .ok_or_else(|| anyhow!("Missing app settings in lingxia.config.json"))?;

        let app_json = AppJson {
            product_name: &app.product_name,
            product_version: &app.product_version,
            api_server: app.api_server.as_deref().filter(|s| !s.trim().is_empty()),
            api_key: std::env::var("LINGXIA_API_KEY")
                .ok()
                .filter(|s| !s.trim().is_empty()),
            api_secret: std::env::var("LINGXIA_API_SECRET")
                .ok()
                .filter(|s| !s.trim().is_empty()),
            home_lxapp_id: &app.home_lxapp_id,
            home_lxapp_version: &app.home_lxapp_version,
            cache_max_age_days: app.cache_max_age_days,
        };

        let app_json_path = resources_dir.join("app.json");
        fs::write(&app_json_path, serde_json::to_string_pretty(&app_json)?)?;

        Ok(())
    }

    /// Build and copy home LxApp to resources directory
    fn build_and_copy_homelxapp(
        &self,
        project_root: &Path,
        lxapp_id: &str,
        resources_dir: &Path,
    ) -> Result<()> {
        let lxapp_dir = project_root.join(lxapp_id);
        if !lxapp_dir.exists() {
            println!(
                "  {} Home LxApp '{}' not found, skipping",
                "⚠".yellow(),
                lxapp_id
            );
            return Ok(());
        }

        println!("  Building home LxApp '{}'...", lxapp_id);

        let status = Command::new("npm")
            .args(["run", "build"])
            .current_dir(&lxapp_dir)
            .status()
            .context("Failed to build LxApp")?;

        if !status.success() {
            return Err(anyhow!("LxApp build failed"));
        }

        // Copy dist to resources
        let dist_dir = lxapp_dir.join("dist");
        if !dist_dir.exists() {
            return Err(anyhow!("LxApp dist directory not found after build"));
        }

        let target_dir = resources_dir.join("homelxapp");
        copy_dir_recursive(&dist_dir, &target_dir)?;

        Ok(())
    }

    /// Build Swift Package
    fn swift_build(
        &self,
        ios_dir: &Path,
        project_root: &Path,
        profile: BuildProfile,
    ) -> Result<()> {
        println!("{}", "Building Swift Package...".cyan());

        // Get the iOS SDK path using xcrun
        let sdk_path = get_ios_sdk_path()?;

        // Note: We intentionally don't set SDKROOT as it would affect manifest compilation.
        // The --sdk flag is sufficient for cross-compilation to iOS.
        let mut cmd = Command::new("swift");
        cmd.current_dir(ios_dir)
            .env("LINGXIA_PROJECT_ROOT", project_root)
            // Clear any existing SDKROOT to ensure manifest compiles correctly
            .env_remove("SDKROOT")
            .args(["build", "--triple", "arm64-apple-ios", "--sdk", &sdk_path]);

        if matches!(profile, BuildProfile::Release) {
            cmd.arg("-c").arg("release");
        }

        let status = cmd.status().context("Failed to execute swift build")?;

        if !status.success() {
            return Err(anyhow!("Swift build failed"));
        }

        println!("  {} Swift build complete", "✓".green());
        Ok(())
    }

    /// Find the .app bundle in build output.
    ///
    /// If `profile` is given, looks only in that profile directory.
    /// Otherwise, checks release first, then debug.
    fn find_app_bundle(&self, ios_dir: &Path, profile: Option<BuildProfile>) -> Result<PathBuf> {
        let base = ios_dir.join(".build/arm64-apple-ios");

        let dirs: Vec<PathBuf> = match profile {
            Some(p) => vec![base.join(p.as_str())],
            None => vec![base.join("release"), base.join("debug")],
        };

        for dir in &dirs {
            if !dir.exists() {
                continue;
            }
            for entry in fs::read_dir(dir)? {
                let path = entry?.path();
                if path.extension().map(|e| e == "app").unwrap_or(false) {
                    return Ok(path);
                }
            }
        }

        Err(anyhow!(
            "No .app bundle found. Build the project first with 'lingxia build --platform ios'"
        ))
    }
}

impl Platform for IosPlatform {
    fn build(&self, config: &BuildConfig) -> Result<BuildArtifacts> {
        // Ensure we're on macOS
        apple::ensure_macos()?;
        apple::ensure_tools()?;

        let ios_config = config.lingxia_config.as_ref().and_then(|c| c.ios.as_ref());

        // Resolve iOS project directory
        let ios_dir = self.resolve_ios_dir(&config.project_root, ios_config)?;

        // Find the workspace root for SDK and bridge generation
        let workspace_root = find_workspace_root(&config.project_root)?;

        println!(
            "{} Building iOS app from {}",
            "[iOS]".cyan(),
            ios_dir.display()
        );

        // Generate Swift bridge if needed (for development builds)
        if config.build_native {
            apple::generate_swift_bridge(&workspace_root, IOS_TARGET)?;
        }

        // Prepare SDK resources
        apple::prepare_sdk_resources(&workspace_root, !config.build_native)?;

        // Build Rust static library
        // Note: Use config.project_root for Rust library location (e.g., examples/lingxia-lib)
        // but workspace_root for output target directory
        self.build_rust_library(&config.project_root, &workspace_root, config)?;

        // Prepare app resources (app.json, homelxapp)
        self.prepare_app_resources(&config.project_root, &ios_dir, config)?;

        // Build Swift Package
        self.swift_build(&ios_dir, &workspace_root, config.profile)?;

        // Find the app bundle
        let app_path = match self.find_app_bundle(&ios_dir, Some(config.profile)) {
            Ok(path) => path,
            Err(_) => {
                println!();
                println!(
                    "{} Swift Package built successfully, but no .app bundle was produced.",
                    "ℹ".blue()
                );
                println!("  This is normal for library packages.");
                println!(
                    "  For a full app bundle, ensure your Package.swift defines an executable product."
                );
                println!();

                ios_dir
                    .join(".build/arm64-apple-ios")
                    .join(config.profile.as_str())
            }
        };

        // TODO: Signing will be implemented separately (requires team_id from local config)

        Ok(BuildArtifacts::Ios { app_path })
    }

    fn install(&self, config: &InstallConfig) -> Result<()> {
        apple::ensure_macos()?;

        let ios_config = crate::config::LingXiaConfig::load(&config.project_root)
            .ok()
            .and_then(|c| c.ios);

        // Determine app path
        let app_path = if let Some(ref path) = config.artifact_path {
            path.clone()
        } else {
            let ios_dir = self.resolve_ios_dir(&config.project_root, ios_config.as_ref())?;
            self.find_app_bundle(&ios_dir, None)?
        };

        if !app_path.exists() {
            return Err(anyhow!("App bundle not found at: {}", app_path.display()));
        }

        apple::install_with_ios_deploy(&app_path, config.device_id.as_deref())
    }

    fn run(&self, config: &RunConfig) -> Result<()> {
        apple::ensure_macos()?;

        apple::run_with_ios_deploy(&config.package_id, config.device_id.as_deref())
    }

    fn list_devices(&self) -> Result<Vec<Device>> {
        apple::list_ios_devices()
    }

    fn name(&self) -> &str {
        "ios"
    }
}

/// Recursively copy a directory
fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<()> {
    if !dest.exists() {
        fs::create_dir_all(dest)?;
    }

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let target = dest.join(entry.file_name());

        if path.is_dir() {
            copy_dir_recursive(&path, &target)?;
        } else {
            fs::copy(&path, &target)?;
        }
    }

    Ok(())
}

/// Get the iOS SDK path using xcrun
fn get_ios_sdk_path() -> Result<String> {
    let output = Command::new("xcrun")
        .args(["--sdk", "iphoneos", "--show-sdk-path"])
        .output()
        .context("Failed to get iOS SDK path")?;

    if !output.status.success() {
        return Err(anyhow!(
            "Failed to find iOS SDK. Make sure Xcode is installed."
        ));
    }

    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        return Err(anyhow!(
            "iOS SDK path is empty. Make sure Xcode is properly configured."
        ));
    }

    Ok(path)
}
