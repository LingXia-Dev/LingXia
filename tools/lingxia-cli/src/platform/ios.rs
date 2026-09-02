//! iOS platform implementation.
//!
//! Builds, signs, and deploys iOS applications using Swift Package Manager.

use super::apple::{self, IOS_TARGET};
use super::spm;
use super::{
    BuildArtifacts, BuildConfig, BuildProfile, Device, InstallConfig, Platform, RunConfig,
    native_client_out_for_host_project, resolve_cargo_target_dir, resolve_lingxia_target_dir,
};
use crate::config::IosConfig;
use crate::permission_cache::{DEFAULT_MAX_AGE_SECONDS, PermissionCache, PermissionPlatform};
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

    /// Resolve the iOS-specific section of the project config, if any.
    fn ios_config<'a>(&self, config: &'a BuildConfig) -> Option<&'a IosConfig> {
        config.lingxia_config.as_ref().and_then(|c| c.ios.as_ref())
    }

    /// Build Rust static library for iOS
    ///
    /// - `project_root`: Where to find the Rust library (e.g., examples/)
    /// - output is always under `{project_root}/target`
    /// - `ios_config`: iOS configuration for deployment target
    fn do_build_rust_library(
        &self,
        project_root: &Path,
        config: &BuildConfig,
        ios_config: Option<&IosConfig>,
    ) -> Result<PathBuf> {
        let is_release = matches!(config.profile, BuildProfile::Release);
        let profile_dir = config.profile.as_str();
        let cargo_target_dir = resolve_cargo_target_dir(project_root);

        if !config.build_native {
            // Return expected path even if not building
            return Ok(cargo_target_dir
                .join(IOS_TARGET)
                .join(profile_dir)
                .join("liblingxia.a"));
        }

        if config.lingxia_config.is_none() {
            return Ok(cargo_target_dir
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
        let native_client_out =
            native_client_out_for_host_project(project_root, lingxia_config, config.framework)?;

        // Get deployment target from config
        let deployment_target = ios_config.and_then(|c| c.deployment_target.as_deref());

        apple::build_rust_staticlib(
            project_root,
            &rust_lib_dir,
            IOS_TARGET,
            is_release,
            deployment_target,
            &config.native_features,
            config.native_default_features,
            native_client_out.as_deref(),
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
        let cargo_target_dir = resolve_cargo_target_dir(project_root);

        // Note: We intentionally don't set SDKROOT as it would affect manifest compilation.
        // The --sdk flag is sufficient for cross-compilation to iOS.
        let mut cmd = Command::new("swift");
        cmd.current_dir(ios_dir)
            .env("LINGXIA_PROJECT_ROOT", project_root)
            .env("LINGXIA_CARGO_TARGET_DIR", &cargo_target_dir)
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

        // Get bundle ID and other config. Apply env-version package suffixes
        // here without touching the source Info.plist on disk.
        let base_bundle_id = ios_config
            .map(|c| c.bundle_id.clone())
            .unwrap_or_else(|| "com.example.app".to_string());
        let bundle_id = match config.resolved_env.effective_package_id_suffix() {
            Some(suffix) => format!("{base_bundle_id}{suffix}"),
            None => base_bundle_id,
        };

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
        let bundle_name = app_config.project_name.clone();
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

        let splash = config
            .lingxia_config
            .as_ref()
            .and_then(|c| c.splash.as_ref())
            .map(|splash| crate::splash::ResolvedSplash::resolve(&config.project_root, splash))
            .transpose()?;
        let bundle_config = AppBundleConfig {
            bundle_id,
            bundle_name,
            app_name,
            swift_product_name,
            executable_name,
            deployment_target,
            info_plist_path: info_plist,
            splash_background: splash.as_ref().map(|s| s.background().to_string()),
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
    /// Searches in the target directory where AppBundler places the .app.
    fn find_app_bundle(
        &self,
        project_root: &Path,
        _profile: Option<BuildProfile>,
    ) -> Result<PathBuf> {
        let output_dir = resolve_lingxia_target_dir(project_root).join("ios");
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
}

impl Platform for IosPlatform {
    fn build(&self, config: &BuildConfig) -> Result<BuildArtifacts> {
        // Ensure we're on macOS
        apple::ensure_macos()?;
        apple::ensure_tools()?;

        let ios_config = self.ios_config(config);

        // Resolve iOS project directory
        let ios_dir = resolve_ios_dir(&config.project_root, ios_config)?;

        // SDK/runtime/native artifacts are scoped to this host project.
        let sdk_root = config.project_root.clone();

        println!(
            "{} Building iOS app from {}",
            "[iOS]".cyan(),
            ios_dir.display()
        );

        let bundle_id = ios_config
            .map(|c| c.bundle_id.clone())
            .unwrap_or_else(|| "com.example.app".to_string());
        let granted_entitlements =
            load_cached_apple_entitlements(PermissionPlatform::Ios, &bundle_id);

        if let Err(err) = warn_missing_restricted_apple_entitlements(&granted_entitlements, "iOS") {
            eprintln!("{} {}", "Warning:".yellow(), err);
        }

        let app_link_hosts = config
            .lingxia_config
            .as_ref()
            .and_then(|config| config.app_links.as_ref())
            .map(|app_links| app_links.hosts.as_slice())
            .unwrap_or(&[]);
        if apple::capabilities::sync_ios_capability_files(
            &ios_dir,
            &granted_entitlements,
            app_link_hosts,
        )? {
            println!(
                "{} Synced iOS capability metadata (Info.plist/App.entitlements)",
                "[iOS]".cyan()
            );
        }

        // Build Rust static library + refresh SwiftPM relink stamp.
        // Skipped when the orchestrator already ran Phase 1 via
        // `build_rust_library`.
        if !config.skip_native_build {
            self.do_build_rust_library(&config.project_root, config, ios_config)?;
            if config.build_native && config.lingxia_config.is_some() {
                apple::update_spm_rust_link_stamp(
                    &config.project_root,
                    &sdk_root,
                    IOS_TARGET,
                    config.profile.as_str(),
                )?;
            }
        }

        let app_path = apple::with_temporary_package_manifest(&ios_dir, || {
            // Point the build-time manifest at the cached Apple SDK (no-op
            // in-workspace), then restore it after every SwiftPM consumer has
            // finished with the package.
            apple::ensure_sdk_package_dependency(&config.project_root, &ios_dir)?;

            // Build Swift Package (library dependencies first).
            self.swift_build(&ios_dir, &config.project_root, config.profile)?;

            // Create .app bundle using AppBundler (converts library to an
            // executable app) and consume any sibling Packet Tunnel product.
            let app_path =
                self.create_app_bundle(&ios_dir, &config.project_root, config, ios_config)?;
            embed_packet_tunnel_extension_if_present(&ios_dir, &app_path, config.profile)?;
            Ok(app_path)
        })?;

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
        // For developer/preview env, point actool at a staging copy of
        // Assets.xcassets whose AppIcon.appiconset has each PNG composited
        // with a circular D/P badge — same visual language as the Android
        // launcher overlay. Source xcassets is never mutated.
        let staging_base = resolve_lingxia_target_dir(&config.project_root).join("ios");
        let env_staged = match apple::env_icon::prepare_overlay_resources_dir(
            &staging_base,
            &resources_dir,
            config.resolved_env.version,
            0.0,
            true,
        ) {
            Ok(staged) => staged,
            Err(err) => {
                eprintln!(
                    "  {} Skipping env app-icon overlay: {}",
                    "Warning:".yellow(),
                    err
                );
                None
            }
        };
        // Splash assets go into a staged catalog copy too (reusing the env
        // staging when it exists), keeping the source xcassets untouched.
        let splash_config = config
            .lingxia_config
            .as_ref()
            .and_then(|c| c.splash.as_ref());
        let resources_for_compile = match splash_config {
            Some(splash_config) => {
                let resolved =
                    crate::splash::ResolvedSplash::resolve(&config.project_root, splash_config)?;
                // Also install the images as plain bundle resources: the
                // runtime overlay reads those, so it survives an actool
                // failure that would leave the compiled catalog missing.
                crate::splash::install_apple_bundle_images(&app_path, &resolved)?;
                // The OS launch face, composed the way the overlay composes
                // it. Without Interface Builder only the ground can be drawn
                // and the art arrives on the app's first frame instead — the
                // Android beat, fine for a dev build and not for a shipped one.
                let face = crate::splash::install_apple_launch_screen(
                    &app_path,
                    &resolved,
                    &deployment_target,
                )?;
                if face == crate::splash::AppleLaunchFace::Ground {
                    if matches!(
                        config.resolved_env.version,
                        crate::config::EnvVersion::Release
                    ) {
                        anyhow::bail!(
                            "Release iOS build requires a compiled launch storyboard; \
                             `ibtool` needs the iOS platform installed — run \
                             `xcodebuild -downloadPlatform iOS`"
                        );
                    }
                    eprintln!(
                        "  {} No launch storyboard: `ibtool` needs the iOS platform installed, so\n     \
                         the OS launch frame is the splash background alone and the art arrives\n     \
                         with the app's first frame. Fix with: xcodebuild -downloadPlatform iOS",
                        "Warning:".yellow()
                    );
                }
                crate::splash::stage_apple_splash_resources(
                    &staging_base,
                    &resources_dir,
                    env_staged,
                    &resolved,
                )?
            }
            None => env_staged.unwrap_or_else(|| resources_dir.clone()),
        };
        if let Err(err) = apple::assets::compile_asset_catalog(
            &resources_for_compile,
            &app_path,
            &deployment_target,
            apple::assets::AssetPlatform::Ios,
        ) {
            if matches!(
                config.resolved_env.version,
                crate::config::EnvVersion::Release
            ) {
                return Err(err).context(
                    "Release iOS build requires a compiled asset catalog; without it the app icon and OS launch face are missing",
                );
            } else {
                eprintln!(
                    "  {} Asset catalog compilation failed: {}",
                    "Warning:".yellow(),
                    err
                );
                // The launch face is a bundle resource of its own, so this
                // costs the app its icon and nothing else.
                eprintln!(
                    "  {} No Assets.car: the app icon is missing.",
                    "Warning:".yellow()
                );
            }
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
            apple::provisioning::sign_app(&app_path, None, app_link_hosts)?;
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

    fn build_rust_library(&self, config: &BuildConfig) -> Result<()> {
        let ios_config = self.ios_config(config);
        self.do_build_rust_library(&config.project_root, config, ios_config)?;
        if config.build_native && config.lingxia_config.is_some() {
            apple::update_spm_rust_link_stamp(
                &config.project_root,
                &config.project_root,
                IOS_TARGET,
                config.profile.as_str(),
            )?;
        }
        Ok(())
    }

    fn hoists_native_build(&self) -> bool {
        true
    }

    fn install(&self, config: &InstallConfig) -> Result<()> {
        apple::ensure_macos()?;

        let host_config = crate::config::LingXiaConfig::load(&config.project_root).ok();
        let app_link_hosts = host_config
            .as_ref()
            .and_then(|config| config.app_links.as_ref())
            .map(|app_links| app_links.hosts.clone())
            .unwrap_or_default();
        // Determine app path
        let app_path = if let Some(ref path) = config.artifact_path {
            path.clone()
        } else {
            self.find_app_bundle(&config.project_root, None)?
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
        apple::provisioning::sign_app(&app_path, Some(&device_identifier), &app_link_hosts)?;

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
        apple::devicectl::launch_app(
            &config.package_id,
            config.device_id.as_deref(),
            config.restart,
        )
    }

    fn list_devices(&self) -> Result<Vec<Device>> {
        // Use devicectl (Xcode 15+).
        apple::devicectl::list_devices()
    }
}

/// Embed the convention-based Packet Tunnel product when the host declares
/// `ios/PacketTunnel/Info.plist`.
///
/// SwiftPM builds the sibling executable target during `swift build`; this step
/// gives that executable the `.appex` structure Xcode would normally create.
/// Projects without the directory keep the existing build path unchanged.
fn embed_packet_tunnel_extension_if_present(
    ios_dir: &Path,
    app_path: &Path,
    profile: BuildProfile,
) -> Result<()> {
    let source_dir = ios_dir.join("PacketTunnel");
    let source_plist = source_dir.join("Info.plist");
    if !source_plist.is_file() {
        return Ok(());
    }

    let build_config = if matches!(profile, BuildProfile::Release) {
        "release"
    } else {
        "debug"
    };
    let executable_name = "PacketTunnel";
    let executable = ios_dir
        .join(".build")
        .join("arm64-apple-ios")
        .join(build_config)
        .join(executable_name);
    if !executable.is_file() {
        return Err(anyhow!(
            "Packet Tunnel source exists, but SwiftPM did not build the `{executable_name}` executable product at {}",
            executable.display()
        ));
    }

    let main_plist = plist::Value::from_file(app_path.join("Info.plist"))
        .context("Failed to read the generated iOS app Info.plist")?;
    let main = main_plist
        .as_dictionary()
        .ok_or_else(|| anyhow!("Generated iOS app Info.plist is not a dictionary"))?;
    let app_bundle_id = plist_string(main, "CFBundleIdentifier")?;

    let mut extension_plist = plist::Value::from_file(&source_plist)
        .with_context(|| format!("Failed to read {}", source_plist.display()))?;
    let extension = extension_plist
        .as_dictionary_mut()
        .ok_or_else(|| anyhow!("{} is not a dictionary", source_plist.display()))?;
    extension.insert(
        "CFBundleIdentifier".into(),
        plist::Value::String(format!("{app_bundle_id}.PacketTunnel")),
    );
    extension.insert(
        "CFBundleExecutable".into(),
        plist::Value::String(executable_name.into()),
    );
    for key in [
        "CFBundleVersion",
        "CFBundleShortVersionString",
        "MinimumOSVersion",
    ] {
        if let Some(value) = main.get(key) {
            extension.insert(key.into(), value.clone());
        }
    }
    if let Some(plist::Value::Dictionary(nsextension)) = extension.get_mut("NSExtension") {
        nsextension.insert(
            "NSExtensionPrincipalClass".into(),
            plist::Value::String("PacketTunnel.PacketTunnelProvider".into()),
        );
    }

    let destination = app_path.join("PlugIns").join("PacketTunnel.appex");
    if destination.exists() {
        fs::remove_dir_all(&destination)?;
    }
    fs::create_dir_all(&destination)?;
    fs::copy(&executable, destination.join(executable_name)).with_context(|| {
        format!(
            "Failed to copy Packet Tunnel executable from {}",
            executable.display()
        )
    })?;
    extension_plist
        .to_file_xml(destination.join("Info.plist"))
        .context("Failed to write Packet Tunnel Info.plist")?;
    let entitlements = source_dir.join("PacketTunnel.entitlements");
    if entitlements.is_file() {
        fs::copy(&entitlements, destination.join("PacketTunnel.entitlements"))?;
    }
    println!("  {} Embedded PacketTunnel.appex", "✓".green());
    Ok(())
}

fn plist_string<'a>(dictionary: &'a plist::Dictionary, key: &str) -> Result<&'a str> {
    dictionary
        .get(key)
        .and_then(plist::Value::as_string)
        .ok_or_else(|| anyhow!("Generated iOS app Info.plist has no {key}"))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embeds_packet_tunnel_with_environment_specific_identity() {
        let root = tempfile::tempdir().unwrap();
        let ios = root.path().join("ios");
        let packet_tunnel = ios.join("PacketTunnel");
        let binary_dir = ios.join(".build/arm64-apple-ios/debug");
        let app = root.path().join("Farshore.app");
        fs::create_dir_all(&packet_tunnel).unwrap();
        fs::create_dir_all(&binary_dir).unwrap();
        fs::create_dir_all(&app).unwrap();
        fs::write(binary_dir.join("PacketTunnel"), b"mach-o").unwrap();
        fs::write(
            packet_tunnel.join("PacketTunnel.entitlements"),
            b"entitlements",
        )
        .unwrap();

        plist::Value::Dictionary(plist::Dictionary::from_iter([
            (
                String::from("CFBundleIdentifier"),
                plist::Value::String("com.example.app.dev".into()),
            ),
            (
                String::from("CFBundleVersion"),
                plist::Value::String("42".into()),
            ),
        ]))
        .to_file_xml(app.join("Info.plist"))
        .unwrap();
        plist::Value::Dictionary(plist::Dictionary::from_iter([
            (
                String::from("CFBundleIdentifier"),
                plist::Value::String("stale.identifier".into()),
            ),
            (
                String::from("NSExtension"),
                plist::Value::Dictionary(plist::Dictionary::new()),
            ),
        ]))
        .to_file_xml(packet_tunnel.join("Info.plist"))
        .unwrap();

        embed_packet_tunnel_extension_if_present(&ios, &app, BuildProfile::Debug).unwrap();

        let appex = app.join("PlugIns/PacketTunnel.appex");
        let embedded = plist::Value::from_file(appex.join("Info.plist")).unwrap();
        let embedded = embedded.as_dictionary().unwrap();
        assert_eq!(
            embedded["CFBundleIdentifier"].as_string(),
            Some("com.example.app.dev.PacketTunnel")
        );
        assert_eq!(
            embedded["CFBundleExecutable"].as_string(),
            Some("PacketTunnel")
        );
        assert_eq!(embedded["CFBundleVersion"].as_string(), Some("42"));
        assert!(appex.join("PacketTunnel").is_file());
        assert!(appex.join("PacketTunnel.entitlements").is_file());
    }
}
