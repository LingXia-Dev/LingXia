use crate::config::{HOST_CONFIG_FILE, LingXiaConfig, ResourceBundleConfig, ResourceBundleType};
use crate::host_assets;
use crate::lxapp::Project;
use crate::platform::detector::PlatformType;
use crate::platform::{self, resolve_cargo_target_dir};
use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub fn execute() -> Result<()> {
    let current_dir = env::current_dir()?;
    match discover_clean_context(&current_dir)? {
        CleanContext::Host { root } => clean_host_project(&root),
        CleanContext::LxApp { root } => clean_lxapp_project(&root),
        CleanContext::StandaloneAppleSwiftPackage { root } => clean_standalone_swift_package(&root),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CleanContext {
    Host { root: PathBuf },
    LxApp { root: PathBuf },
    StandaloneAppleSwiftPackage { root: PathBuf },
}

fn discover_clean_context(current_dir: &Path) -> Result<CleanContext> {
    let has_host_config = current_dir.join(HOST_CONFIG_FILE).exists();
    let has_lxapp_manifest = current_dir.join("lxapp.json").exists();
    let has_lxplugin_manifest = current_dir.join("lxplugin.json").exists();

    if (has_lxapp_manifest || has_lxplugin_manifest) && !has_host_config {
        return Ok(CleanContext::LxApp {
            root: current_dir.to_path_buf(),
        });
    }

    if has_host_config {
        return Ok(CleanContext::Host {
            root: current_dir.to_path_buf(),
        });
    }

    if let Some(ctx) =
        platform::spm::find_apple_swift_package_context(current_dir, HOST_CONFIG_FILE)?
    {
        return Ok(CleanContext::Host {
            root: ctx.host_project_root,
        });
    }

    if let Some(host_root) =
        platform::detector::find_host_project_root(current_dir, HOST_CONFIG_FILE)
    {
        return Ok(CleanContext::Host { root: host_root });
    }

    if platform::spm::detect_local_apple_swift_package_platform(current_dir)?.is_some() {
        return Ok(CleanContext::StandaloneAppleSwiftPackage {
            root: current_dir.to_path_buf(),
        });
    }

    Err(anyhow!(
        "No cleanable LingXia project found in {}.\n\
         Run from a host project with {}, an lxapp/lxplugin project, a host platform subdirectory, or a standalone Apple Swift Package.",
        current_dir.display(),
        HOST_CONFIG_FILE
    ))
}

fn clean_host_project(project_root: &Path) -> Result<()> {
    println!("{}", "Cleaning LingXia host project...".cyan());
    println!("  {} {}", "Project".dimmed(), project_root.display());

    let config = LingXiaConfig::load(project_root).with_context(|| {
        format!(
            "Failed to read {}",
            project_root.join(HOST_CONFIG_FILE).display()
        )
    })?;
    let mut removed = Vec::new();

    for path in host_assets::clean_configured_host_assets(project_root)? {
        print_removed(&path);
        removed.push(path);
    }

    remove_path(&project_root.join("dist"), &mut removed)?;
    remove_path(&project_root.join(".lingxia"), &mut removed)?;
    clean_cargo_target(project_root, &mut removed)?;

    clean_configured_resource_bundles(project_root, &config, &mut removed)?;
    clean_host_platform_dirs(project_root, &config, &mut removed)?;

    print_done(removed.len());
    Ok(())
}

fn clean_lxapp_project(project_root: &Path) -> Result<()> {
    println!("{}", "Cleaning LingXia lxapp project...".cyan());
    println!("  {} {}", "Project".dimmed(), project_root.display());

    let mut removed = Vec::new();
    if let Ok(project) = Project::discover(project_root, None) {
        remove_path(&project.output_dir, &mut removed)?;
    } else {
        if project_root.join("lxapp.json").exists() {
            remove_path(&project_root.join("dist"), &mut removed)?;
        }
        if project_root.join("lxplugin.json").exists() {
            remove_path(&project_root.join("dist-plugin"), &mut removed)?;
        }
    }

    remove_path(&project_root.join("node_modules"), &mut removed)?;
    remove_path(&project_root.join(".lingxia"), &mut removed)?;
    clean_legacy_lxapp_view_dirs(project_root, &mut removed)?;

    print_done(removed.len());
    Ok(())
}

fn clean_standalone_swift_package(project_root: &Path) -> Result<()> {
    println!("{}", "Cleaning standalone Apple Swift Package...".cyan());
    println!("  {} {}", "Project".dimmed(), project_root.display());

    let mut removed = Vec::new();
    remove_path(&project_root.join(".build"), &mut removed)?;
    remove_path(&project_root.join(".lingxia"), &mut removed)?;

    print_done(removed.len());
    Ok(())
}

fn clean_configured_resource_bundles(
    project_root: &Path,
    config: &LingXiaConfig,
    removed: &mut Vec<PathBuf>,
) -> Result<()> {
    let Some(resources) = config.resources.as_ref() else {
        return Ok(());
    };
    let Some(bundles) = resources.bundles.as_ref() else {
        return Ok(());
    };

    for bundle in bundles {
        let (path, bundle_type) = match bundle {
            ResourceBundleConfig::Path(path) => (path.as_str(), ResourceBundleType::Lxapp),
            ResourceBundleConfig::Detailed(detail) => (detail.path.as_str(), detail.bundle_type),
        };
        match bundle_type {
            ResourceBundleType::Lxapp | ResourceBundleType::Npm => {
                remove_path(&project_root.join(path).join("dist"), removed)?;
            }
        }
    }

    Ok(())
}

fn clean_host_platform_dirs(
    project_root: &Path,
    config: &LingXiaConfig,
    removed: &mut Vec<PathBuf>,
) -> Result<()> {
    for platform in configured_platforms(config) {
        match platform {
            PlatformType::Android => clean_android(project_root, removed)?,
            PlatformType::Ios => clean_ios(project_root, config, removed)?,
            PlatformType::MacOs => clean_macos(project_root, config, removed)?,
            PlatformType::Harmony => clean_harmony(project_root, config, removed)?,
        }
    }
    Ok(())
}

fn clean_cargo_target(project_root: &Path, removed: &mut Vec<PathBuf>) -> Result<()> {
    let target_dir = resolve_cargo_target_dir(project_root);
    if !target_dir.exists() {
        return Ok(());
    }
    if path_is_under_or_equal(project_root, &target_dir) {
        remove_path(&target_dir, removed)?;
    } else {
        println!(
            "  {} shared Cargo target outside project root: {}",
            "skipped".dimmed(),
            target_dir.display()
        );
    }
    Ok(())
}

fn configured_platforms(config: &LingXiaConfig) -> Vec<PlatformType> {
    config
        .app
        .as_ref()
        .map(|app| {
            app.platforms
                .iter()
                .filter_map(|platform| platform.parse().ok())
                .collect()
        })
        .unwrap_or_default()
}

fn clean_android(project_root: &Path, removed: &mut Vec<PathBuf>) -> Result<()> {
    let android_dir = platform::detector::resolve_android_dir(project_root);
    remove_path(&android_dir.join("build"), removed)?;
    remove_path(&android_dir.join("app/build"), removed)?;
    remove_path(&android_dir.join("app/src/main/jniLibs"), removed)?;
    Ok(())
}

fn clean_ios(
    project_root: &Path,
    config: &LingXiaConfig,
    removed: &mut Vec<PathBuf>,
) -> Result<()> {
    if let Ok(ios_dir) = platform::ios::resolve_ios_dir(project_root, config.ios.as_ref()) {
        remove_path(&ios_dir.join(".build"), removed)?;
        remove_path(&ios_dir.join(".lingxia"), removed)?;
    }
    Ok(())
}

fn clean_macos(
    project_root: &Path,
    config: &LingXiaConfig,
    removed: &mut Vec<PathBuf>,
) -> Result<()> {
    if let Ok(macos_dir) = platform::macos::resolve_macos_dir(project_root, config.macos.as_ref()) {
        remove_path(&macos_dir.join(".build"), removed)?;
        remove_path(&macos_dir.join(".lingxia"), removed)?;
    }
    Ok(())
}

fn clean_harmony(
    project_root: &Path,
    config: &LingXiaConfig,
    removed: &mut Vec<PathBuf>,
) -> Result<()> {
    if let Ok(harmony_dir) =
        platform::harmony::resolve_harmony_dir(project_root, config.harmony.as_ref())
    {
        remove_path(&harmony_dir.join("build"), removed)?;
        remove_path(&harmony_dir.join("entry/build"), removed)?;
        let native_lib = harmony_dir.join("entry/libs/arm64-v8a/liblingxia.so");
        remove_path(&native_lib, removed)?;
        remove_empty_parent_dirs_until(&harmony_dir, native_lib.parent());
    }
    Ok(())
}

fn clean_legacy_lxapp_view_dirs(project_root: &Path, removed: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(project_root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(".lingxia-view-") {
            remove_path(&entry.path(), removed)?;
        }
    }
    Ok(())
}

fn remove_path(path: &Path, removed: &mut Vec<PathBuf>) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    if path.is_dir() {
        fs::remove_dir_all(path).with_context(|| format!("Failed to remove {}", path.display()))?;
    } else {
        fs::remove_file(path).with_context(|| format!("Failed to remove {}", path.display()))?;
    }
    print_removed(path);
    removed.push(path.to_path_buf());
    Ok(())
}

fn remove_empty_parent_dirs_until(root: &Path, start: Option<&Path>) {
    let Ok(root) = root.canonicalize() else {
        return;
    };
    let mut current = start.map(Path::to_path_buf);
    while let Some(dir) = current {
        let Ok(canonical) = dir.canonicalize() else {
            break;
        };
        if canonical == root || !canonical.starts_with(&root) {
            break;
        }
        let is_empty = fs::read_dir(&canonical)
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(false);
        if !is_empty || fs::remove_dir(&canonical).is_err() {
            break;
        }
        current = canonical.parent().map(Path::to_path_buf);
    }
}

fn path_is_under_or_equal(root: &Path, path: &Path) -> bool {
    let Ok(root) = root.canonicalize() else {
        return false;
    };
    let Ok(path) = path.canonicalize() else {
        return false;
    };
    path == root || path.starts_with(root)
}

fn print_removed(path: &Path) {
    println!("  {} {}", "removed".green(), path.display());
}

fn print_done(removed_count: usize) {
    if removed_count == 0 {
        println!("  {}", "Already clean".dimmed());
    } else {
        println!("  {} removed {} paths", "✓".green(), removed_count);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    #[test]
    fn detects_lxapp_before_host_ancestor() {
        let temp = TempDir::new().unwrap();
        write(
            &temp.path().join("lingxia.yaml"),
            "app:\n  projectName: host\n  productName: Host\n  productVersion: 1.0.0\n  platforms: [macos]\n",
        );
        let lxapp = temp.path().join("apps/home");
        write(
            &lxapp.join("lxapp.json"),
            r#"{"version":"1.0.0","pages":[]}"#,
        );

        assert_eq!(
            discover_clean_context(&lxapp).unwrap(),
            CleanContext::LxApp { root: lxapp }
        );
    }

    #[test]
    fn detects_host_from_swift_package_subdir() {
        let temp = TempDir::new().unwrap();
        write(
            &temp.path().join("lingxia.yaml"),
            "app:\n  projectName: host\n  productName: Host\n  productVersion: 1.0.0\n  platforms: [macos]\n",
        );
        let macos = temp.path().join("macos");
        write(
            &macos.join("Package.swift"),
            "// swift-tools-version: 6.0\n.platforms([.macOS(.v14)])",
        );

        assert_eq!(
            discover_clean_context(&macos).unwrap(),
            CleanContext::Host {
                root: temp.path().to_path_buf()
            }
        );
    }

    #[test]
    fn detects_standalone_swift_package() {
        let temp = TempDir::new().unwrap();
        write(
            &temp.path().join("Package.swift"),
            "// swift-tools-version: 6.0\n.platforms([.macOS(.v14)])",
        );

        assert_eq!(
            discover_clean_context(temp.path()).unwrap(),
            CleanContext::StandaloneAppleSwiftPackage {
                root: temp.path().to_path_buf()
            }
        );
    }

    #[test]
    fn clean_lxapp_removes_local_build_outputs() {
        let temp = TempDir::new().unwrap();
        write(
            &temp.path().join("lxapp.json"),
            r#"{"version":"1.0.0","logic":false,"pages":["pages/home/index"]}"#,
        );
        write(&temp.path().join("pages/home/index.html"), "");
        write(&temp.path().join("dist/file.js"), "");
        write(&temp.path().join("node_modules/pkg/index.js"), "");
        write(&temp.path().join(".lingxia/view-build/html/file"), "");
        write(&temp.path().join(".lingxia-view-old/file"), "");

        clean_lxapp_project(temp.path()).unwrap();

        assert!(!temp.path().join("dist").exists());
        assert!(!temp.path().join("node_modules").exists());
        assert!(!temp.path().join(".lingxia").exists());
        assert!(!temp.path().join(".lingxia-view-old").exists());
    }

    #[test]
    fn host_clean_skips_workspace_target_outside_project_root() {
        let temp = TempDir::new().unwrap();
        write(
            &temp.path().join("Cargo.toml"),
            "[workspace]\nmembers = []\n",
        );
        write(&temp.path().join("target/debug/.keep"), "");
        let project = temp.path().join("examples");
        write(
            &project.join("lingxia.yaml"),
            "app:\n  projectName: host\n  productName: Host\n  productVersion: 1.0.0\n  platforms: []\n",
        );

        let mut removed = Vec::new();
        clean_cargo_target(&project, &mut removed).unwrap();

        assert!(temp.path().join("target/debug/.keep").exists());
        assert!(removed.is_empty());
    }
}
