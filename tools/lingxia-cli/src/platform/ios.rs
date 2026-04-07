//! iOS platform implementation.
//!
//! Builds, signs, and deploys iOS applications using Swift Package Manager.

use super::apple::{self, IOS_TARGET};
use super::spm;
use super::{
    BuildArtifacts, BuildConfig, BuildProfile, Device, InstallConfig, Platform, RunConfig,
};
use crate::config::IosConfig;
use crate::permission_cache::{DEFAULT_MAX_AGE_SECONDS, PermissionCache, PermissionPlatform};
use crate::sdk::{self, SdkPlatform};
use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

mod doctor;
pub use doctor::doctor_checks;

/// iOS platform implementation
pub struct IosPlatform;

impl IosPlatform {
    /// Create a new iOS platform instance
    pub fn new() -> Self {
        Self
    }

    /// Build Rust static library for iOS
    ///
    /// - `project_root`: Where to find the Rust library (e.g., examples/)
    /// - output is always under `{project_root}/target`
    /// - `ios_config`: iOS configuration for deployment target
    fn build_rust_library(
        &self,
        project_root: &Path,
        config: &BuildConfig,
        ios_config: Option<&IosConfig>,
    ) -> Result<PathBuf> {
        let is_release = matches!(config.profile, BuildProfile::Release);
        let profile_dir = config.profile.as_str();

        if !config.build_native {
            // Return expected path even if not building
            return Ok(project_root
                .join("target")
                .join(IOS_TARGET)
                .join(profile_dir)
                .join("liblingxia.a"));
        }

        if config.lingxia_config.is_none() {
            return Ok(project_root
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

        // Get deployment target from config
        let deployment_target = ios_config.and_then(|c| c.deployment_target.as_deref());

        apple::build_rust_staticlib(
            project_root,
            &rust_lib_dir,
            IOS_TARGET,
            is_release,
            &config.features,
            deployment_target,
        )
    }

    /// Build Swift Package (library only, for dependency compilation)
    fn swift_build(
        &self,
        ios_dir: &Path,
        project_root: &Path,
        profile: BuildProfile,
    ) -> Result<()> {
        println!("{}", "Building Swift Package...".cyan());

        // Get the iOS SDK path using xcrun
        let sdk_path = get_ios_sdk_path()?;

        let is_release = matches!(profile, BuildProfile::Release);
        let build_config = if is_release { "release" } else { "debug" };

        // Note: We intentionally don't set SDKROOT as it would affect manifest compilation.
        // The --sdk flag is sufficient for cross-compilation to iOS.
        let mut cmd = Command::new("swift");
        cmd.current_dir(ios_dir)
            .env("LINGXIA_PROJECT_ROOT", project_root)
            .env("LINGXIA_BUILD_CONFIG", build_config)
            // Clear any existing SDKROOT to ensure manifest compiles correctly
            .env_remove("SDKROOT")
            .args([
                "build",
                "--disable-sandbox",
                "--triple",
                "arm64-apple-ios",
                "--sdk",
                &sdk_path,
            ]);

        if is_release {
            cmd.arg("-c").arg("release");
        }

        let status = cmd.status().context("Failed to execute swift build")?;

        if !status.success() {
            return Err(anyhow!("Swift build failed"));
        }

        println!("  {} Swift build complete", "✓".green());
        Ok(())
    }

    /// Create .app bundle using the AppBundler
    fn create_app_bundle(
        &self,
        ios_dir: &Path,
        project_root: &Path,
        config: &BuildConfig,
        ios_config: Option<&IosConfig>,
    ) -> Result<PathBuf> {
        use apple::app_bundle::{AppBundleConfig, AppBundler};

        // Get bundle ID and other config
        let bundle_id = ios_config
            .map(|c| c.bundle_id.clone())
            .unwrap_or_else(|| "com.example.app".to_string());

        let app_config = config
            .lingxia_config
            .as_ref()
            .and_then(|c| c.app.as_ref())
            .ok_or_else(|| {
                anyhow!(
                    "Missing app config in lingxia.config.json. \
                     iOS bundle build requires app.projectName and app.productName."
                )
            })?;
        let app_name = app_config.product_name.clone();
        let swift_product_name = apple::resolve_swiftpm_target_name(
            ios_dir,
            ios_config.and_then(|c| c.target_name.as_deref()),
            Some(app_config.project_name.as_str()),
            "ios",
        )?;
        let executable_name = app_config.project_name.clone();

        let deployment_target = ios_config
            .and_then(|c| c.deployment_target.clone())
            .unwrap_or_else(|| "17.0".to_string());

        // Look for Info.plist in the package directory
        let info_plist_path = ios_dir.join("Info.plist");
        let info_plist = if info_plist_path.exists() {
            Some(info_plist_path)
        } else {
            None
        };

        let bundle_config = AppBundleConfig {
            bundle_id,
            app_name,
            swift_product_name,
            executable_name,
            deployment_target,
            info_plist_path: info_plist,
        };

        AppBundler::create_app_bundle(
            ios_dir,
            project_root,
            &bundle_config,
            matches!(config.profile, BuildProfile::Release),
        )
    }

    /// Find the .app bundle in build output.
    ///
    /// Searches in `.lingxia/` directory where AppBundler places the .app.
    fn find_app_bundle(&self, ios_dir: &Path, _profile: Option<BuildProfile>) -> Result<PathBuf> {
        let output_dir = ios_dir.join(".lingxia");
        if output_dir.exists() {
            for entry in fs::read_dir(&output_dir)? {
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

    fn ensure_apple_sdk(&self, config: &BuildConfig) -> Result<()> {
        if config.lingxia_config.is_none() {
            return Ok(());
        }
        let lingxia_config = config
            .lingxia_config
            .as_ref()
            .ok_or_else(|| anyhow!("lingxia.config.json is required to resolve SDK version"))?;
        let rust_lib_name = lingxia_config
            .get_rust_lib_name()
            .ok_or_else(|| anyhow!("app.projectName is required in lingxia.config.json"))?;
        let sdk_version =
            sdk::resolve_sdk_version_from_rust_manifest(&config.project_root, &rust_lib_name)?;
        sdk::ensure_sdk(&config.project_root, SdkPlatform::Apple, &sdk_version)?;
        Ok(())
    }
}

impl Platform for IosPlatform {
    fn build(&self, config: &BuildConfig) -> Result<BuildArtifacts> {
        // Ensure we're on macOS
        apple::ensure_macos()?;
        apple::ensure_tools()?;

        let ios_config = config.lingxia_config.as_ref().and_then(|c| c.ios.as_ref());

        // Resolve iOS project directory
        let ios_dir = resolve_ios_dir(&config.project_root, ios_config)?;

        // SDK/runtime/native artifacts are scoped to this host project.
        let sdk_root = config.project_root.clone();

        println!(
            "{} Building iOS app from {}",
            "[iOS]".cyan(),
            ios_dir.display()
        );

        self.ensure_apple_sdk(config)?;

        let bundle_id = ios_config
            .map(|c| c.bundle_id.clone())
            .unwrap_or_else(|| "com.example.app".to_string());
        let granted_entitlements =
            load_cached_apple_entitlements(PermissionPlatform::Ios, &bundle_id);

        if let Err(err) = warn_missing_restricted_apple_entitlements(&granted_entitlements, "iOS") {
            eprintln!("{} {}", "Warning:".yellow(), err);
        }

        if apple::capabilities::sync_ios_capability_files(&ios_dir, &granted_entitlements)? {
            println!(
                "{} Synced iOS capability metadata (Info.plist/App.entitlements)",
                "[iOS]".cyan()
            );
        }

        // Build Rust static library
        // Use host project root for both crate discovery and target output.
        self.build_rust_library(&config.project_root, config, ios_config)?;
        if config.build_native && config.lingxia_config.is_some() {
            apple::update_spm_rust_link_stamp(
                &config.project_root,
                &sdk_root,
                IOS_TARGET,
                config.profile.as_str(),
            )?;
        }

        // Build Swift Package (library dependencies first)
        self.swift_build(&ios_dir, &config.project_root, config.profile)?;

        // Create .app bundle using AppBundler (converts library to executable app)
        let app_path =
            self.create_app_bundle(&ios_dir, &config.project_root, config, ios_config)?;

        // Compile asset catalog (includes AppIcon) and merge generated plist
        let deployment_target = ios_config
            .and_then(|c| c.deployment_target.clone())
            .unwrap_or_else(|| "17.0".to_string());
        let app_project_name = config
            .lingxia_config
            .as_ref()
            .and_then(|c| c.app.as_ref())
            .map(|a| a.project_name.as_str());
        let resources_dir = get_resources_dir(&ios_dir, ios_config, app_project_name)?;
        if let Err(err) = apple::assets::compile_asset_catalog(
            &resources_dir,
            &app_path,
            &deployment_target,
            apple::assets::AssetPlatform::Ios,
        ) {
            eprintln!(
                "  {} Asset catalog compilation failed: {}",
                "Warning:".yellow(),
                err
            );
        }
        if let Err(err) = apple::assets::merge_assetcatalog_plist_with_platform(
            &app_path,
            apple::assets::AssetPlatform::Ios,
        ) {
            eprintln!(
                "  {} Failed to merge asset catalog plist: {}",
                "Warning:".yellow(),
                err
            );
        }

        let ipa_path = if config.ipa {
            apple::provisioning::sign_app(&app_path, None)?;
            let app_name = app_path
                .file_stem()
                .and_then(|n| n.to_str())
                .ok_or_else(|| anyhow!("Invalid app bundle name: {}", app_path.display()))?;
            let ipa_output_dir = config.project_root.join("dist").join("ios");
            fs::create_dir_all(&ipa_output_dir).with_context(|| {
                format!(
                    "Failed to create iOS distribution directory: {}",
                    ipa_output_dir.display()
                )
            })?;
            let ipa_path = ipa_output_dir.join(format!("{app_name}.ipa"));
            let ipa_path = apple::signer::create_ipa(&app_path, &ipa_path)?;
            println!("{} IPA → {}", "✓".green(), ipa_path.display());
            Some(ipa_path)
        } else {
            None
        };

        Ok(BuildArtifacts::Ios { app_path, ipa_path })
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
            let ios_dir = resolve_ios_dir(&config.project_root, ios_config.as_ref())?;
            self.find_app_bundle(&ios_dir, None)?
        };

        if !app_path.exists() {
            return Err(anyhow!("App bundle not found at: {}", app_path.display()));
        }

        let device_identifier = if let Some(device_id) = config.device_id.as_deref() {
            device_id.to_string()
        } else {
            apple::devicectl::DeviceCtl::wait_for_device(30)?.identifier
        };

        // Sign the app before installing
        apple::provisioning::sign_app(&app_path, Some(&device_identifier))?;

        if config.reinstall {
            let bundle_id = read_bundle_id(&app_path).ok();
            if let Some(bundle_id) = bundle_id {
                if let Err(err) =
                    apple::devicectl::uninstall_app(&bundle_id, Some(&device_identifier))
                {
                    eprintln!(
                        "{} failed to uninstall {} before install: {}",
                        "Warning:".yellow(),
                        bundle_id,
                        err
                    );
                }
            } else {
                eprintln!(
                    "{} could not resolve iOS bundle id for --reinstall; continuing install",
                    "Warning:".yellow()
                );
            }
        }

        apple::devicectl::install_app(&app_path, Some(&device_identifier))
    }

    fn uninstall(&self, package_id: &str, device_id: Option<&str>) -> Result<()> {
        apple::devicectl::uninstall_app(package_id, device_id)
    }

    fn run(&self, config: &RunConfig) -> Result<()> {
        apple::devicectl::launch_app(&config.package_id, config.device_id.as_deref())
    }

    fn list_devices(&self) -> Result<Vec<Device>> {
        // Use devicectl (Xcode 15+).
        apple::devicectl::list_devices()
    }
}

fn load_cached_apple_entitlements(platform: PermissionPlatform, bundle_id: &str) -> Vec<String> {
    let Ok(cache) = PermissionCache::load() else {
        return Vec::new();
    };
    cache
        .get(platform, bundle_id, Some(DEFAULT_MAX_AGE_SECONDS))
        .unwrap_or_default()
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

/// Resolve the iOS Swift Package directory.
///
/// Expects Package.swift in: `{projectRoot}/ios/`
pub(crate) fn resolve_ios_dir(
    project_root: &Path,
    _ios_config: Option<&IosConfig>,
) -> Result<PathBuf> {
    spm::resolve_apple_swift_package_dir(project_root, "ios", None, "iOS")
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

/// Read the bundle ID from a built/signed iOS app bundle.
pub fn read_bundle_id(app_path: &Path) -> Result<String> {
    apple::provisioning::read_bundle_id(&app_path.join("Info.plist"))
}

/// Generate iOS app icons
///
/// # Arguments
/// * `project_root` - Project root directory
/// * `source_icon` - Path to source icon image
/// * `ios_config` - Optional iOS configuration from lingxia.config.json
/// * `app_project_name` - Optional app project name (used for SwiftPM target inference)
pub fn generate_icons(
    project_root: &Path,
    source_icon: &Path,
    ios_config: Option<&crate::config::IosConfig>,
    app_project_name: Option<&str>,
) -> Result<()> {
    let ios_dir = resolve_ios_dir(project_root, ios_config)?;
    let resources_dir = get_resources_dir(&ios_dir, ios_config, app_project_name)?;
    crate::appicon::generate_ios_icons(source_icon, &resources_dir)
}

/// Get the resources directory path for an iOS Swift Package
pub fn get_resources_dir(
    ios_dir: &Path,
    ios_config: Option<&crate::config::IosConfig>,
    app_project_name: Option<&str>,
) -> Result<PathBuf> {
    apple::resolve_swiftpm_resources_dir(
        ios_dir,
        ios_config.and_then(|c| c.target_name.as_deref()),
        app_project_name,
        "ios",
    )
}
