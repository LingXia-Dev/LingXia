//! Apple asset catalog compilation utilities.

use anyhow::{Context, Result, anyhow};
use std::ffi::OsString;
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

    let mut args: Vec<OsString> = [
        "--output-format",
        "human-readable-text",
        "--notices",
        "--warnings",
        "--export-dependency-info",
    ]
    .map(OsString::from)
    .to_vec();
    args.push(paths.dependencies_path.clone().into());
    args.push("--output-partial-info-plist".into());
    args.push(paths.generated_plist_path.clone().into());
    args.extend(["--app-icon", "AppIcon"].map(OsString::from));
    args.extend(device_args.iter().map(OsString::from));
    args.extend(["--minimum-deployment-target", deployment_target].map(OsString::from));
    args.extend(["--platform", paths.platform_arg, "--compile"].map(OsString::from));
    args.push(paths.resources_dir.clone().into());
    args.push(assets_dir.into());

    let mut result = run_actool(xcrun_actool(platform), &args);
    // actool renders iOS-family assets in an agent that runs inside the
    // simulator runtime. Without one, use Xcode's own macOS asset runtime
    // through a prepared tree rather than requiring the multi-GB download.
    if matches!(platform, AssetPlatform::Ios)
        && result
            .as_ref()
            .is_err_and(|out| is_missing_ios_runtime(out))
    {
        let developer_dir = ensure_runtime_free_developer_dir()?;
        result = run_actool(runtime_free_actool(&developer_dir), &args);
        if result.is_ok() {
            println!("  Compiled asset catalog without the iOS simulator runtime");
        }
    }

    if let Err(combined) = result {
        // Reported distinctly so the caller can permit it for development
        // but reject a release package whose icon and OS launch face would
        // be missing.
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

/// actool's output when it failed. It exits 0 on many failures, so its
/// errors section counts over the exit status.
fn run_actool(mut command: Command, args: &[OsString]) -> std::result::Result<(), String> {
    let output = command
        .args(args)
        .output()
        .map_err(|err| format!("Failed to execute actool: {err}"))?;
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if !output.status.success() || combined.contains("com.apple.actool.errors") {
        return Err(combined);
    }
    Ok(())
}

/// Xcode's `actool` for the platform's SDK.
fn xcrun_actool(platform: AssetPlatform) -> Command {
    let sdk = match platform {
        AssetPlatform::Ios => "iphoneos",
        AssetPlatform::Macos => "macosx",
    };
    let mut command = Command::new("xcrun");
    command.args(["--sdk", sdk, "actool"]);
    // Avoid inheriting a simulator SDKROOT from the parent environment.
    command.env_remove("SDKROOT");
    command
}

/// `actool` from a prepared tree, rendering iOS assets in its lightweight
/// asset runtime instead of the simulator runtime.
fn runtime_free_actool(developer_dir: &Path) -> Command {
    let mut command = Command::new(developer_dir.join("usr/bin/actool"));
    command
        .env("XCODE_DEVELOPER_DIR_PATH", developer_dir)
        .env(
            "SDKROOT",
            developer_dir.join("Platforms/iPhoneOS.platform/Developer/SDKs/iPhoneOS.sdk"),
        )
        .args(["--lightweight-asset-runtime-mode", "enabled"]);
    command
}

/// The `actool` commands an iOS compile tries, in order: Xcode's own, then
/// the runtime-free tree when one is already prepared. For `lingxia doctor`.
pub(crate) fn ios_actool_commands() -> Vec<Command> {
    let mut commands = vec![xcrun_actool(AssetPlatform::Ios)];
    if let (Ok(tree), Some(contents)) = (runtime_free_tree_root(), active_xcode_contents())
        && let Some(developer_dir) = prepared_developer_dir(&tree, &contents)
    {
        commands.push(runtime_free_actool(&developer_dir));
    }
    commands
}

/// `<Xcode>.app/Contents` of the selected Xcode.
fn active_xcode_contents() -> Option<PathBuf> {
    let developer_dir = super::active_developer_dir()?;
    Path::new(&developer_dir).parent().map(Path::to_path_buf)
}

fn runtime_free_tree_root() -> Result<PathBuf> {
    Ok(crate::state_root::lingxia_dir()?.join("apple/actool/xcode-tree"))
}

/// The tree's developer dir when it is complete and made from this Xcode.
fn prepared_developer_dir(tree: &Path, xcode_contents: &Path) -> Option<PathBuf> {
    let developer_dir = tree.join("Contents/Developer");
    let runtime = developer_dir.join("Platforms/iPhoneOS.platform/System/AssetRuntime");
    if !developer_dir.join("usr/bin/actool").is_file() || !runtime.is_dir() {
        return None;
    }
    let source_version = fs::read(xcode_contents.join("version.plist")).ok()?;
    let tree_version = fs::read(tree.join("Contents/version.plist")).ok()?;
    (source_version == tree_version).then_some(developer_dir)
}

/// The runtime-free tree for the selected Xcode, prepared once per Xcode
/// version. It is built beside its final path and renamed into place, so a
/// concurrent build never sees it half made.
fn ensure_runtime_free_developer_dir() -> Result<PathBuf> {
    let contents = active_xcode_contents()
        .context("xcode-select has no Xcode developer directory; install Xcode")?;
    let tree = runtime_free_tree_root()?;
    if let Some(dir) = prepared_developer_dir(&tree, &contents) {
        return Ok(dir);
    }

    println!(
        "  Preparing an iOS asset compiler that needs no simulator runtime: an APFS clone\n  of Xcode at {} (about two minutes, once per Xcode version; safe to delete)...",
        tree.display()
    );
    let parent = tree.parent().context("asset compiler path has no parent")?;
    fs::create_dir_all(parent)?;
    let staging = parent.join(format!("xcode-tree.{}.tmp", std::process::id()));
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    fs::create_dir_all(&staging)?;
    let built = build_runtime_free_tree(&contents, &staging.join("Contents"));
    if let Err(err) = built {
        fs::remove_dir_all(&staging).ok();
        return Err(err);
    }

    // Move a stale tree aside first: rename cannot replace a non-empty dir.
    let stale = parent.join(format!("xcode-tree.{}.old", std::process::id()));
    if tree.exists() {
        fs::rename(&tree, &stale).ok();
    }
    if fs::rename(&staging, &tree).is_err() {
        // Another build installed its tree first; use that one.
        fs::remove_dir_all(&staging).ok();
    }
    fs::remove_dir_all(&stale).ok();
    prepared_developer_dir(&tree, &contents)
        .context("the iOS asset compiler tree is incomplete after preparation")
}

/// An APFS clone of `Xcode.app/Contents` with an `AssetRuntime` added to its
/// iPhoneOS platform: Xcode's macOS asset runtime plus the iOS asset catalog
/// frameworks. `actool` resolves its developer dir from where its files
/// really are, so links back into Xcode lead it back to Xcode's own
/// platform; a clone is the whole tree at the cost of its metadata.
fn build_runtime_free_tree(xcode_contents: &Path, dest_contents: &Path) -> Result<()> {
    let status = Command::new("cp")
        .arg("-cR")
        .arg(xcode_contents)
        .arg(dest_contents)
        .status()
        .context("failed to spawn cp")?;
    if !status.success() {
        anyhow::bail!(
            "could not clone Xcode into {} (it needs the same APFS volume as Xcode); \
             run `xcodebuild -downloadPlatform iOS` instead",
            dest_contents.display()
        );
    }

    let platform = dest_contents.join("Developer/Platforms/iPhoneOS.platform");
    let runtime = platform.join("System/AssetRuntime");
    let frameworks = runtime.join("System/Library/Frameworks");
    let private_frameworks = runtime.join("System/Library/PrivateFrameworks");
    fs::create_dir_all(&frameworks)?;
    fs::create_dir_all(&private_frameworks)?;
    let mac_runtime =
        xcode_contents.join("Developer/Platforms/MacOSX.platform/System/AssetRuntime");
    link_children(&mac_runtime.join("System/Library/Frameworks"), &frameworks)?;
    link_children(
        &mac_runtime.join("System/Library/PrivateFrameworks"),
        &private_frameworks,
    )?;
    let catalog_frameworks = xcode_contents.join("Frameworks/AssetRuntime");
    for name in [
        "AssetCatalogFoundation.framework",
        "AssetCatalogCocoaTouchFoundation.framework",
    ] {
        force_symlink(
            &catalog_frameworks.join(name),
            &private_frameworks.join(name),
        )?;
    }
    let environment = xcode_contents.join(
        "Developer/Platforms/MacOSX.platform/Library/Developer/AssetRuntime/RuntimeEnvironment.plist",
    );
    let environment_dir = platform.join("Library/Developer/AssetRuntime");
    fs::create_dir_all(&environment_dir)?;
    for dest in [
        runtime.join("RuntimeEnvironment.plist"),
        environment_dir.join("RuntimeEnvironment.plist"),
    ] {
        fs::copy(&environment, dest)
            .context("Xcode's macOS AssetRuntime environment is missing")?;
    }
    Ok(())
}

fn link_children(source_dir: &Path, dest_dir: &Path) -> Result<()> {
    let entries = fs::read_dir(source_dir)
        .with_context(|| format!("Xcode directory is missing: {}", source_dir.display()))?;
    for entry in entries {
        let entry = entry?;
        force_symlink(&entry.path(), &dest_dir.join(entry.file_name()))?;
    }
    Ok(())
}

fn force_symlink(target: &Path, link: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        if let Ok(meta) = fs::symlink_metadata(link) {
            if meta.is_dir() {
                fs::remove_dir_all(link)?;
            } else {
                fs::remove_file(link)?;
            }
        }
        std::os::unix::fs::symlink(target, link)
            .with_context(|| format!("failed to link {} -> {}", link.display(), target.display()))
    }
    #[cfg(not(unix))]
    {
        let _ = (target, link);
        anyhow::bail!("iOS asset catalogs can only be compiled on macOS");
    }
}

fn sanitize_actool_output(raw: &str) -> String {
    raw.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("dyld["))
        .take(8)
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" | ")
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

#[cfg(test)]
mod tests {
    use super::*;

    fn xcode_with_version(root: &Path, version: &str) -> PathBuf {
        let contents = root.join("Xcode.app/Contents");
        fs::create_dir_all(&contents).unwrap();
        fs::write(contents.join("version.plist"), version).unwrap();
        contents
    }

    fn tree_with_version(root: &Path, version: &str) -> PathBuf {
        let tree = root.join("tree");
        let developer = tree.join("Contents/Developer");
        fs::create_dir_all(developer.join("usr/bin")).unwrap();
        fs::create_dir_all(developer.join("Platforms/iPhoneOS.platform/System/AssetRuntime"))
            .unwrap();
        fs::write(developer.join("usr/bin/actool"), "").unwrap();
        fs::write(tree.join("Contents/version.plist"), version).unwrap();
        tree
    }

    #[test]
    fn prepared_tree_is_used_only_for_its_xcode_version() {
        let tmp = tempfile::tempdir().unwrap();
        let tree = tree_with_version(tmp.path(), "26.0");

        let same = xcode_with_version(&tmp.path().join("a"), "26.0");
        assert_eq!(
            prepared_developer_dir(&tree, &same),
            Some(tree.join("Contents/Developer"))
        );
        let newer = xcode_with_version(&tmp.path().join("b"), "26.1");
        assert_eq!(prepared_developer_dir(&tree, &newer), None);
    }

    #[test]
    fn incomplete_tree_is_not_prepared() {
        let tmp = tempfile::tempdir().unwrap();
        let tree = tree_with_version(tmp.path(), "26.0");
        let xcode = xcode_with_version(&tmp.path().join("a"), "26.0");
        fs::remove_dir_all(
            tree.join("Contents/Developer/Platforms/iPhoneOS.platform/System/AssetRuntime"),
        )
        .unwrap();
        assert_eq!(prepared_developer_dir(&tree, &xcode), None);
    }

    #[test]
    fn only_a_missing_runtime_falls_back() {
        assert!(is_missing_ios_runtime(
            "error: No available simulator runtimes for platform iphonesimulator."
        ));
        assert!(!is_missing_ios_runtime(
            "error: The stickers icon set has an unassigned child."
        ));
    }

    #[test]
    fn dyld_noise_is_dropped_from_failures() {
        let raw = "dyld[1]: Library not loaded\nerror: bad catalog\n";
        assert_eq!(sanitize_actool_output(raw), "error: bad catalog");
    }
}
