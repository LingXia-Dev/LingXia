//! iOS App Bundle packaging.
//!
//! Converts a SwiftPM library package into a runnable .app bundle.

use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Configuration for app bundle creation
pub struct AppBundleConfig {
    /// Bundle identifier (e.g., "app.lingxia.example")
    pub bundle_id: String,
    /// App display name (user-facing)
    pub app_name: String,
    /// Swift package product name from the root package
    pub swift_product_name: String,
    /// Final executable filename in the app bundle
    pub executable_name: String,
    /// Deployment target (e.g., "17.0")
    pub deployment_target: String,
    /// Path to custom Info.plist (merged with generated one)
    pub info_plist_path: Option<PathBuf>,
}

/// App bundle packager
pub struct AppBundler;

const APP_RUNNER_TARGET: &str = "LingXiaAppRunner";
const APP_BUILDER_PACKAGE: &str = "LingXiaAppBundleBuilder";

impl AppBundler {
    /// Create an iOS .app bundle from a SwiftPM library package.
    ///
    /// # Arguments
    /// * `package_dir` - Path to the SwiftPM package (containing Package.swift)
    /// * `workspace_root` - Workspace root for LINGXIA_PROJECT_ROOT
    /// * `config` - App bundle configuration
    /// * `release` - Whether to build in release mode
    ///
    /// # Returns
    /// Path to the created .app bundle
    pub fn create_app_bundle(
        package_dir: &Path,
        workspace_root: &Path,
        config: &AppBundleConfig,
        release: bool,
    ) -> Result<PathBuf> {
        println!("{}", "Creating app bundle...".cyan());

        // 1. Create temporary executable package
        let tmp_package_dir = Self::create_executable_package(package_dir, config)?;

        // 2. Build the executable
        let build_dir =
            Self::build_executable(package_dir, &tmp_package_dir, workspace_root, release)?;

        // 3. Create .app bundle structure
        let app_bundle = Self::create_bundle_structure(package_dir, &build_dir, config)?;

        // 4. Clean up temporary package
        let _ = fs::remove_dir_all(&tmp_package_dir);

        println!(
            "  {} App bundle created → {}",
            "✓".green(),
            app_bundle.display()
        );

        Ok(app_bundle)
    }

    /// Create a temporary SwiftPM package with an executable target
    fn create_executable_package(package_dir: &Path, config: &AppBundleConfig) -> Result<PathBuf> {
        let build_dir = package_dir.join(".lingxia");
        let tmp_package_dir = build_dir.join(".tmp");

        // Clean up any existing temp package
        let _ = fs::remove_dir_all(&tmp_package_dir);
        fs::create_dir_all(&tmp_package_dir)?;

        // Create Package.swift that wraps the library as an executable.
        // Keep package/target identifiers stable and technical.
        let target_name = APP_RUNNER_TARGET;
        let package_swift = format!(
            r#"// swift-tools-version: 6.0
import PackageDescription
let package = Package(
    name: "{builder_package}",
    platforms: [
        .iOS("{deployment_target}"),
    ],
    products: [
        .executable(
            name: "{target_name}",
            targets: ["{target_name}"]
        ),
    ],
    dependencies: [
        .package(name: "RootPackage", path: "../.."),
    ],
    targets: [
        .executableTarget(
            name: "{target_name}",
            dependencies: [
                .product(name: "{swift_product_name}", package: "RootPackage"),
            ],
            linkerSettings: [
                .unsafeFlags([
                    "-Xlinker", "-rpath", "-Xlinker", "@executable_path/Frameworks",
                ]),
            ]
        )
    ]
)
"#,
            builder_package = APP_BUILDER_PACKAGE,
            swift_product_name = config.swift_product_name,
            deployment_target = config.deployment_target,
            target_name = target_name,
        );

        fs::write(tmp_package_dir.join("Package.swift"), package_swift)?;

        // Create stub source file
        let sources_dir = tmp_package_dir.join("Sources").join(target_name);
        fs::create_dir_all(&sources_dir)?;
        fs::write(sources_dir.join("stub.c"), "")?;

        Ok(tmp_package_dir)
    }

    /// Build the executable using swift build
    fn build_executable(
        package_dir: &Path,
        tmp_package_dir: &Path,
        workspace_root: &Path,
        release: bool,
    ) -> Result<PathBuf> {
        println!("  Building executable...");

        // Get iOS SDK path
        let sdk_path = get_ios_sdk_path()?;
        let build_config = if release { "release" } else { "debug" };
        let scratch_path = package_dir.join(".lingxia").join(".tmp-build");

        let mut cmd = Command::new("swift");
        cmd.current_dir(package_dir)
            .env("LINGXIA_PROJECT_ROOT", workspace_root)
            .env("LINGXIA_BUILD_CONFIG", build_config)
            .env_remove("SDKROOT")
            .args([
                "build",
                "--package-path",
                tmp_package_dir.to_str().unwrap(),
                "--scratch-path",
                scratch_path.to_str().unwrap(),
                "--triple",
                "arm64-apple-ios",
                "--sdk",
                &sdk_path,
                "--disable-automatic-resolution",
            ]);

        if release {
            cmd.args(["-c", "release"]);
        }

        let status = cmd.status().context("Failed to execute swift build")?;

        if !status.success() {
            return Err(anyhow!("Swift build failed"));
        }

        Ok(scratch_path.join("arm64-apple-ios").join(build_config))
    }

    /// Create the .app bundle structure
    fn create_bundle_structure(
        package_dir: &Path,
        build_dir: &Path,
        config: &AppBundleConfig,
    ) -> Result<PathBuf> {
        let target_name = APP_RUNNER_TARGET;
        let app_name = format!("{}.app", config.app_name);

        // Create app bundle in .lingxia/ directory
        let output_dir = package_dir.join(".lingxia");
        fs::create_dir_all(&output_dir)?;

        let app_bundle = output_dir.join(&app_name);

        // Clean up existing bundle
        let _ = fs::remove_dir_all(&app_bundle);
        fs::create_dir_all(&app_bundle)?;

        // Copy executable
        let exe_src = build_dir.join(target_name);
        let exe_dst = app_bundle.join(&config.executable_name);
        if exe_src.exists() {
            fs::copy(&exe_src, &exe_dst)?;
        } else {
            return Err(anyhow!("Executable not found: {}", exe_src.display()));
        }

        // Copy resource bundles
        Self::copy_resource_bundles(build_dir, &app_bundle)?;

        // Copy frameworks (if any)
        Self::copy_frameworks(build_dir, &app_bundle)?;

        // Generate Info.plist
        Self::generate_info_plist(package_dir, &app_bundle, config)?;

        Ok(app_bundle)
    }

    /// Copy resource bundles (*.bundle) from build directory
    fn copy_resource_bundles(build_dir: &Path, app_bundle: &Path) -> Result<()> {
        for entry in fs::read_dir(build_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "bundle").unwrap_or(false) {
                let dest = app_bundle.join(path.file_name().unwrap());
                super::copy_dir_recursive(&path, &dest)?;
            }
        }
        Ok(())
    }

    /// Copy frameworks from build directory
    fn copy_frameworks(build_dir: &Path, app_bundle: &Path) -> Result<()> {
        let frameworks_dir = app_bundle.join("Frameworks");

        for entry in fs::read_dir(build_dir)? {
            let entry = entry?;
            let path = entry.path();

            // Copy .framework directories
            if path.extension().map(|e| e == "framework").unwrap_or(false) {
                // Check if it's a dynamic framework (not static)
                let fw_name = path.file_stem().unwrap().to_str().unwrap();
                let binary = path.join(fw_name);
                if binary.exists() && !is_static_archive(&binary)? {
                    fs::create_dir_all(&frameworks_dir)?;
                    let dest = frameworks_dir.join(path.file_name().unwrap());
                    super::copy_dir_recursive(&path, &dest)?;
                }
            }

            // Copy .dylib files
            if path.extension().map(|e| e == "dylib").unwrap_or(false) {
                fs::create_dir_all(&frameworks_dir)?;
                let dest = frameworks_dir.join(path.file_name().unwrap());
                fs::copy(&path, &dest)?;
            }
        }

        Ok(())
    }

    /// Generate Info.plist for the app bundle
    fn generate_info_plist(
        package_dir: &Path,
        app_bundle: &Path,
        config: &AppBundleConfig,
    ) -> Result<()> {
        let mut info: HashMap<String, plist::Value> = HashMap::new();

        // Required fields
        info.insert("CFBundleInfoDictionaryVersion".into(), "6.0".into());
        info.insert("CFBundleDevelopmentRegion".into(), "en".into());
        info.insert("CFBundleVersion".into(), "1".into());
        info.insert("CFBundleShortVersionString".into(), "1.0.0".into());
        info.insert(
            "MinimumOSVersion".into(),
            config.deployment_target.clone().into(),
        );
        info.insert("CFBundleIdentifier".into(), config.bundle_id.clone().into());
        info.insert("CFBundleName".into(), config.app_name.clone().into());
        info.insert(
            "CFBundleExecutable".into(),
            config.executable_name.clone().into(),
        );
        info.insert("CFBundleDisplayName".into(), config.app_name.clone().into());
        info.insert("CFBundlePackageType".into(), "APPL".into());

        // iOS required fields
        info.insert(
            "UIRequiredDeviceCapabilities".into(),
            plist::Value::Array(vec!["arm64".into()]),
        );
        info.insert("LSRequiresIPhoneOS".into(), true.into());
        info.insert(
            "CFBundleSupportedPlatforms".into(),
            plist::Value::Array(vec!["iPhoneOS".into()]),
        );
        info.insert(
            "UIDeviceFamily".into(),
            plist::Value::Array(vec![1.into(), 2.into()]),
        );
        info.insert(
            "UISupportedInterfaceOrientations".into(),
            plist::Value::Array(vec!["UIInterfaceOrientationPortrait".into()]),
        );
        info.insert(
            "UILaunchScreen".into(),
            plist::Value::Dictionary(plist::Dictionary::new()),
        );

        // Merge with custom Info.plist if provided
        if let Some(ref plist_path) = config.info_plist_path {
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

        // Write Info.plist
        let info_plist_path = app_bundle.join("Info.plist");
        let dict: plist::Dictionary = info.into_iter().collect();
        plist::to_file_xml(info_plist_path, &dict).context("Failed to write Info.plist")?;

        Ok(())
    }
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

/// Check if a binary file is a static archive
fn is_static_archive(path: &Path) -> Result<bool> {
    use std::io::Read;

    let mut file = fs::File::open(path)?;
    let mut magic = [0u8; 8];

    if file.read_exact(&mut magic).is_ok() {
        // Static archive magic bytes: "!<arch>\n" or "!<thin>\n"
        return Ok(&magic == b"!<arch>\n" || &magic == b"!<thin>\n");
    }

    Ok(false)
}
