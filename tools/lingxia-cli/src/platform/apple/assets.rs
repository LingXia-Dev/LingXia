//! Apple asset catalog compilation utilities.

use anyhow::{Context, Result, anyhow};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy)]
pub enum AssetPlatform {
    Ios,
    Macos,
}

struct AssetCatalogPaths {
    resources_dir: PathBuf,
    info_plist_path: PathBuf,
    generated_plist_path: PathBuf,
    dependencies_path: PathBuf,
    sdk_arg: &'static str,
    platform_arg: &'static str,
    target_devices: &'static [&'static str],
}

fn asset_catalog_paths(app_bundle: &Path, platform: AssetPlatform) -> AssetCatalogPaths {
    match platform {
        AssetPlatform::Ios => AssetCatalogPaths {
            resources_dir: app_bundle.to_path_buf(),
            info_plist_path: app_bundle.join("Info.plist"),
            generated_plist_path: app_bundle.join("assetcatalog_generated_info.plist"),
            dependencies_path: app_bundle.join("assetcatalog_dependencies"),
            sdk_arg: "iphoneos",
            platform_arg: "iphoneos",
            target_devices: &["iphone", "ipad"],
        },
        AssetPlatform::Macos => {
            let contents_dir = app_bundle.join("Contents");
            AssetCatalogPaths {
                resources_dir: contents_dir.join("Resources"),
                info_plist_path: contents_dir.join("Info.plist"),
                generated_plist_path: contents_dir.join("assetcatalog_generated_info.plist"),
                dependencies_path: contents_dir.join("assetcatalog_dependencies"),
                sdk_arg: "macosx",
                platform_arg: "macosx",
                target_devices: &["mac"],
            }
        }
    }
}

/// Compile the asset catalog (Assets.xcassets) into Assets.car and place it in the app bundle.
///
/// # Arguments
/// * `resources_dir` - Path to SwiftPM target resources directory
/// * `app_bundle` - Path to the .app bundle directory
/// * `deployment_target` - Deployment target (e.g., "17.0")
pub fn compile_asset_catalog(
    resources_dir: &Path,
    app_bundle: &Path,
    deployment_target: &str,
    platform: AssetPlatform,
) -> Result<()> {
    let assets_dir = resources_dir.join("Assets.xcassets");
    if !assets_dir.exists() {
        return Ok(());
    }

    println!("  Compiling asset catalog...");

    let paths = asset_catalog_paths(app_bundle, platform);

    let mut device_args: Vec<&str> = Vec::new();
    for d in paths.target_devices {
        device_args.push("--target-device");
        device_args.push(d);
    }

    let output = Command::new("xcrun")
        .args(["--sdk", paths.sdk_arg])
        // Avoid inheriting a simulator SDKROOT from parent environment.
        .env_remove("SDKROOT")
        .args([
            "actool",
            "--output-format",
            "human-readable-text",
            "--notices",
            "--warnings",
            "--export-dependency-info",
            paths
                .dependencies_path
                .to_str()
                .context("Invalid UTF-8 in assetcatalog_dependencies path")?,
            "--output-partial-info-plist",
            paths
                .generated_plist_path
                .to_str()
                .context("Invalid UTF-8 in assetcatalog_generated_info.plist path")?,
            "--app-icon",
            "AppIcon",
        ])
        .args(&device_args)
        .args([
            "--minimum-deployment-target",
            deployment_target,
            "--platform",
            paths.platform_arg,
            "--compile",
            paths
                .resources_dir
                .to_str()
                .context("Invalid UTF-8 in resources_dir path")?,
            assets_dir
                .to_str()
                .context("Invalid UTF-8 in assets_dir path")?,
        ])
        .output()
        .context("Failed to execute xcrun actool")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}\n{stderr}");
    // actool exits 0 even when it fails, so trust its errors section over
    // the exit status.
    if !output.status.success() || combined.contains("com.apple.actool.errors") {
        // actool delegates iOS-family compiles to an agent that must run
        // inside a locally installed iOS runtime. Report that environment
        // failure distinctly so the caller can permit it for development but
        // reject a release package whose icon and OS launch face would be
        // missing.
        if matches!(platform, AssetPlatform::Ios) && is_missing_ios_runtime(&combined) {
            return Err(anyhow!(
                "actool needs the iOS platform installed; run `xcodebuild -downloadPlatform iOS`"
            ));
        }

        let details = sanitize_actool_output(&combined);
        if details.is_empty() {
            anyhow::bail!("Asset catalog compilation failed");
        }
        anyhow::bail!("Asset catalog compilation failed: {}", details);
    }

    println!("  Compiled asset catalog to Assets.car");
    Ok(())
}

/// The signature actool prints when the iOS platform (simulator runtime)
/// is not installed — distinct from genuine catalog errors.
fn is_missing_ios_runtime(actool_output: &str) -> bool {
    actool_output.contains("No available simulator runtimes")
        || actool_output.contains("Platform Not Installed")
}

pub fn is_missing_ios_runtime_error(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .to_string()
            .contains("actool needs the iOS platform installed")
    })
}

/// Preserve a configured cover when Xcode has the device SDK but no simulator
/// runtime. actool still emits the legacy icon PNGs and partial icon plist in
/// that state, but cannot create Assets.car. A loose image named by
/// UILaunchScreen gives the OS the same full-bleed cover; removing the
/// unresolved catalog color prevents it from painting white over the image.
pub fn install_ios_launch_fallback(app_bundle: &Path) -> Result<()> {
    let source = app_bundle.join(crate::splash::APPLE_BUNDLE_IMAGE);
    if !source.is_file() {
        anyhow::bail!("No loose splash cover is available at {}", source.display());
    }
    let destination = app_bundle.join(format!("{}.png", crate::splash::APPLE_IMAGE_ASSET));
    fs::copy(&source, &destination).with_context(|| {
        format!(
            "Failed to install raw iOS launch cover {}",
            destination.display()
        )
    })?;

    let info_path = app_bundle.join("Info.plist");
    let mut info: plist::Dictionary =
        plist::from_file(&info_path).context("Failed to read Info.plist for launch fallback")?;
    let launch = info
        .get_mut("UILaunchScreen")
        .and_then(plist::Value::as_dictionary_mut)
        .context("Info.plist has no UILaunchScreen dictionary")?;
    launch.remove("UIColorName");
    plist::to_file_xml(&info_path, &info).context("Failed to write Info.plist launch fallback")?;
    Ok(())
}

fn sanitize_actool_output(raw: &str) -> String {
    raw.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(8)
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" | ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_launch_fallback_names_the_cover_and_removes_the_catalog_color() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(crate::splash::APPLE_BUNDLE_IMAGE), b"png").unwrap();
        let mut launch = plist::Dictionary::new();
        launch.insert(
            "UIImageName".into(),
            crate::splash::APPLE_IMAGE_ASSET.into(),
        );
        launch.insert(
            "UIColorName".into(),
            crate::splash::APPLE_COLOR_ASSET.into(),
        );
        let mut info = plist::Dictionary::new();
        info.insert("UILaunchScreen".into(), launch.into());
        plist::to_file_xml(dir.path().join("Info.plist"), &info).unwrap();

        install_ios_launch_fallback(dir.path()).unwrap();

        assert!(
            dir.path()
                .join(format!("{}.png", crate::splash::APPLE_IMAGE_ASSET))
                .is_file()
        );
        let info: plist::Dictionary = plist::from_file(dir.path().join("Info.plist")).unwrap();
        let launch = info["UILaunchScreen"].as_dictionary().unwrap();
        assert_eq!(
            launch["UIImageName"].as_string(),
            Some(crate::splash::APPLE_IMAGE_ASSET)
        );
        assert!(!launch.contains_key("UIColorName"));
    }
}

pub fn merge_assetcatalog_plist_with_platform(
    app_bundle: &Path,
    platform: AssetPlatform,
) -> Result<()> {
    let paths = asset_catalog_paths(app_bundle, platform);
    let assetcatalog_plist = paths.generated_plist_path;
    let info_plist_path = paths.info_plist_path;

    if !assetcatalog_plist.exists() {
        // No asset catalog plist to merge
        return Ok(());
    }

    // Read existing Info.plist
    let mut info: plist::Dictionary =
        plist::from_file(&info_plist_path).context("Failed to read Info.plist for merging")?;

    // Read asset catalog generated plist
    let assetcatalog: plist::Dictionary =
        plist::from_file(&assetcatalog_plist).context("Failed to read assetcatalog plist")?;

    // Merge asset catalog entries (this includes CFBundleIcons, CFBundleIcons~ipad, etc.)
    for (key, value) in assetcatalog {
        info.insert(key, value);
    }

    // Write merged Info.plist
    plist::to_file_xml(&info_plist_path, &info).context("Failed to write merged Info.plist")?;

    Ok(())
}
