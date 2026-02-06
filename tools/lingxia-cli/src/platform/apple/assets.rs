//! Apple asset catalog compilation utilities.

use anyhow::{Context, Result};
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
                platform_arg: "macosx",
                target_devices: &["mac"],
            }
        }
    }
}

/// Compile the asset catalog (Assets.xcassets) into Assets.car and place it in the app bundle.
///
/// # Arguments
/// * `apple_dir` - Path to the Apple Swift Package directory (ios/ or macos/)
/// * `app_bundle` - Path to the .app bundle directory
/// * `deployment_target` - Deployment target (e.g., "17.0")
/// * `target_name` - SwiftPM target name that contains Resources/
pub fn compile_asset_catalog(
    apple_dir: &Path,
    app_bundle: &Path,
    deployment_target: &str,
    target_name: &str,
    platform: AssetPlatform,
) -> Result<()> {
    let assets_dir = apple_dir
        .join("Sources")
        .join(target_name)
        .join("Resources/Assets.xcassets");
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

    let status = Command::new("xcrun")
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
        .status()
        .context("Failed to execute xcrun actool")?;

    if !status.success() {
        anyhow::bail!("Asset catalog compilation failed");
    }

    println!("  Compiled asset catalog to Assets.car");
    Ok(())
}

/// Merge assetcatalog_generated_info.plist into the main Info.plist.
pub fn merge_assetcatalog_plist(app_bundle: &Path) -> Result<()> {
    merge_assetcatalog_plist_with_platform(app_bundle, AssetPlatform::Ios)
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
