//! iOS platform implementation.
//!
//! Builds, signs, and deploys iOS applications using Swift Package Manager.

use super::apple::{self, IOS_TARGET};
use super::spm;
use super::{
    BuildArtifacts, BuildConfig, BuildProfile, Device, InstallConfig, Platform, RunConfig,
    native_client_out_for_host_project, resolve_cargo_target_dir,
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

        // External user projects don't have the Apple SDK in their source tree,
        // so fetch the published source package (verified, cached) and rewrite
        // the app's Package.swift to depend on it via a local `.package(path:)`.
        // The SDK uses `unsafeFlags`, so it can ONLY be a local path dependency
        // — a remote URL is rejected by SwiftPM. Inside the workspace the
        // committed Package.swift already points at the local SDK source.
        if !super::is_inside_lingxia_workspace(&config.project_root) {
            let version = crate::sdk_cache::sdk_version();
            let sdk_dir =
                crate::sdk_cache::ensure_sdk(crate::sdk_cache::SdkPlatform::Apple, &version)?;
            inject_sdk_package_dependency(&ios_dir, &sdk_dir)?;
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
        // For developer/preview env, point actool at a staging copy of
        // Assets.xcassets whose AppIcon.appiconset has each PNG composited
        // with a circular D/P badge — same visual language as the Android
        // launcher overlay. Source xcassets is never mutated.
        let resources_for_compile = match apple::env_icon::prepare_overlay_resources_dir(
            &ios_dir,
            &resources_dir,
            config.resolved_env.version,
        ) {
            Ok(Some(staging)) => staging,
            Ok(None) => resources_dir.clone(),
            Err(err) => {
                eprintln!(
                    "  {} Skipping env app-icon overlay: {}",
                    "Warning:".yellow(),
                    err
                );
                resources_dir.clone()
            }
        };
        if let Err(err) = apple::assets::compile_asset_catalog(
            &resources_for_compile,
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
        let ios_config = host_config.and_then(|c| c.ios);

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

/// Sentinel marking the CLI-managed SDK package line in a generated
/// `Package.swift`. Lets repeated builds / version bumps converge instead of
/// appending duplicate dependencies.
const SDK_PACKAGE_MARKER: &str = "// lingxia-sdk: managed by `lingxia build`";

/// Idempotently rewrite the app's `Package.swift` so it depends on the cached
/// LingXia Apple SDK via a local `.package(path:)`.
///
/// Two insertions, both keyed off markers so the operation is convergent:
///   1. `dependencies:` gets `.package(name: "lingxia", path: "<abs>")`.
///   2. the app target's `dependencies:` gets
///      `.product(name: "lingxia", package: "lingxia")`.
///
/// The template (`templates/ios-native/Package.swift`) ships commented-out TODO
/// placeholders for both; we replace those on the first build and replace our
/// own previously-written line on subsequent builds (handles version/path drift).
fn inject_sdk_package_dependency(ios_dir: &Path, sdk_dir: &Path) -> Result<()> {
    let manifest_path = ios_dir.join("Package.swift");
    let original = fs::read_to_string(&manifest_path).with_context(|| {
        format!(
            "Failed to read iOS Package.swift: {}",
            manifest_path.display()
        )
    })?;

    let abs = sdk_dir
        .canonicalize()
        .unwrap_or_else(|_| sdk_dir.to_path_buf());
    let abs_str = abs.to_string_lossy().replace('\\', "/");

    let package_line =
        format!(".package(name: \"lingxia\", path: \"{abs_str}\"), {SDK_PACKAGE_MARKER}");
    let product_line =
        format!(".product(name: \"lingxia\", package: \"lingxia\"), {SDK_PACKAGE_MARKER}");

    let mut rewritten = String::with_capacity(original.len() + 256);
    let mut inserted_package = false;
    let mut inserted_product = false;

    for line in original.lines() {
        let trimmed = line.trim();
        let indent = &line[..line.len() - line.trim_start().len()];

        // Replace an existing CLI-managed package line (path/version drift) or
        // the template's TODO placeholder.
        if trimmed.contains(SDK_PACKAGE_MARKER) && trimmed.contains(".package(") {
            rewritten.push_str(indent);
            rewritten.push_str(&package_line);
            rewritten.push('\n');
            inserted_package = true;
            continue;
        }
        if !inserted_package
            && trimmed.starts_with("// Add the LingXia Swift package dependency here")
        {
            rewritten.push_str(indent);
            rewritten.push_str(&package_line);
            rewritten.push('\n');
            inserted_package = true;
            continue;
        }

        // Replace an existing CLI-managed product line or the template TODO.
        if trimmed.contains(SDK_PACKAGE_MARKER) && trimmed.contains(".product(") {
            rewritten.push_str(indent);
            rewritten.push_str(&product_line);
            rewritten.push('\n');
            inserted_product = true;
            continue;
        }
        if !inserted_product
            && trimmed.starts_with("// .product(name: \"lingxia\", package: \"lingxia\")")
        {
            rewritten.push_str(indent);
            rewritten.push_str(&product_line);
            rewritten.push('\n');
            inserted_product = true;
            continue;
        }

        rewritten.push_str(line);
        rewritten.push('\n');
    }

    if !inserted_package || !inserted_product {
        return Err(anyhow!(
            "Could not locate LingXia dependency placeholders in {}\n  \
             Expected the generated template's TODO comments in `dependencies:` and the app target.",
            manifest_path.display()
        ));
    }

    if rewritten != original {
        fs::write(&manifest_path, &rewritten).with_context(|| {
            format!(
                "Failed to write iOS Package.swift: {}",
                manifest_path.display()
            )
        })?;
    }
    Ok(())
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
    use tempfile::TempDir;

    const TEMPLATE_PACKAGE_SWIFT: &str = r#"// swift-tools-version: 6.0
import PackageDescription
let package = Package(
    name: "demo",
    dependencies: [
        // Add the LingXia Swift package dependency here before building.
    ],
    targets: [
        .target(
            name: "demo",
            dependencies: [
                // .product(name: "lingxia", package: "lingxia"),
            ],
            path: "Sources"
        ),
    ]
)
"#;

    fn write_manifest(ios_dir: &Path, body: &str) {
        fs::write(ios_dir.join("Package.swift"), body).unwrap();
    }

    #[test]
    fn inject_replaces_template_placeholders() {
        let ios = TempDir::new().unwrap();
        write_manifest(ios.path(), TEMPLATE_PACKAGE_SWIFT);
        let sdk = TempDir::new().unwrap();

        inject_sdk_package_dependency(ios.path(), sdk.path()).unwrap();
        let out = fs::read_to_string(ios.path().join("Package.swift")).unwrap();

        assert!(out.contains(".package(name: \"lingxia\", path:"));
        assert!(out.contains(".product(name: \"lingxia\", package: \"lingxia\")"));
        assert!(out.contains(SDK_PACKAGE_MARKER));
        // Placeholders consumed.
        assert!(!out.contains("// Add the LingXia Swift package dependency here"));
    }

    #[test]
    fn inject_is_idempotent_and_converges_on_path_change() {
        let ios = TempDir::new().unwrap();
        write_manifest(ios.path(), TEMPLATE_PACKAGE_SWIFT);
        let sdk_a = TempDir::new().unwrap();

        inject_sdk_package_dependency(ios.path(), sdk_a.path()).unwrap();
        let first = fs::read_to_string(ios.path().join("Package.swift")).unwrap();

        // Re-running with the same SDK dir is a no-op.
        inject_sdk_package_dependency(ios.path(), sdk_a.path()).unwrap();
        let again = fs::read_to_string(ios.path().join("Package.swift")).unwrap();
        assert_eq!(first, again);

        // A new SDK path replaces the managed line rather than duplicating it.
        let sdk_b = TempDir::new().unwrap();
        inject_sdk_package_dependency(ios.path(), sdk_b.path()).unwrap();
        let third = fs::read_to_string(ios.path().join("Package.swift")).unwrap();
        let package_lines = third
            .lines()
            .filter(|l| l.contains(".package(name: \"lingxia\""))
            .count();
        let product_lines = third
            .lines()
            .filter(|l| l.contains(".product(name: \"lingxia\""))
            .count();
        assert_eq!(package_lines, 1, "exactly one managed package line");
        assert_eq!(product_lines, 1, "exactly one managed product line");
    }
}
