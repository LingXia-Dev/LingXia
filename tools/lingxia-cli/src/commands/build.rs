use crate::commands::rust::resolve_build_profile;
use crate::config::{HOST_CONFIG_FILE, LXAPP_BUILD_CONFIG_FILE, LingXiaConfig};
use crate::host_assets::prepare_configured_host_assets;
use crate::lxapp;
use crate::platform::detector::PlatformType;
use crate::platform::{self, BuildArtifacts, BuildConfig};
use anyhow::{Result, anyhow};
use colored::Colorize;
use std::path::{Path, PathBuf};
use std::{env, fs};

pub struct BuildExecuteOptions {
    pub release: bool,
    pub build_native: bool,
    pub abis: Vec<String>,
    pub macos_arch: Option<String>,
    pub framework: Option<String>,
    pub progress: Option<String>,
    pub platforms: Vec<String>,
    pub all_platforms: bool,
    pub ipa: bool,
    pub dmg: bool,
    pub package: bool,
    /// Raw `--env` value from CLI.
    pub env_version: Option<String>,
}

/// Execute the build command
///
/// Builds the project using the detected platform's build system.
/// Supports debug and release profiles and multi-target builds.
pub fn execute(options: BuildExecuteOptions) -> Result<()> {
    let BuildExecuteOptions {
        release,
        build_native,
        abis,
        macos_arch,
        framework,
        progress,
        platforms,
        all_platforms,
        ipa,
        dmg,
        package,
        env_version,
    } = options;

    // Detect project root (current directory)
    let current_dir = env::current_dir()?;
    let mut project_root = current_dir.clone();
    let mut inferred_platform_from_subdir = None;
    let mut standalone_apple_swift_package = false;
    let lxapp_json_exists = current_dir.join("lxapp.json").exists();
    let lxplugin_json_exists = current_dir.join("lxplugin.json").exists();

    let host_config_exists = current_dir.join(HOST_CONFIG_FILE).exists();

    // LxApp or LxPlugin project (no host config)
    if (lxapp_json_exists || lxplugin_json_exists) && !host_config_exists {
        if package && !release {
            return Err(anyhow!(
                "Packaging requires a release build for LxApp/LxPlugin projects."
            ));
        }
        let mut args = vec!["build".to_string()];
        if release {
            args.push("--release".to_string());
        }
        if package {
            args.push("--package".to_string());
        }
        if let Some(framework) = framework.as_deref() {
            args.push("--framework".to_string());
            args.push(framework.to_string());
        }
        if let Some(progress) = progress.as_deref() {
            args.push("--progress".to_string());
            args.push(progress.to_string());
        }

        return lxapp::run(&args);
    }

    if !host_config_exists {
        if let Some(ctx) =
            platform::spm::find_apple_swift_package_context(&current_dir, HOST_CONFIG_FILE)?
        {
            println!(
                "{} Detected Apple Swift Package in {}",
                "ℹ".blue(),
                current_dir.display()
            );
            println!(
                "  {} Host project: {}",
                "•".cyan(),
                ctx.host_project_root.display()
            );
            println!(
                "  {} Default platform: {}",
                "•".cyan(),
                ctx.inferred_platform.as_str()
            );
            println!();

            project_root = ctx.host_project_root;
            inferred_platform_from_subdir = Some(ctx.inferred_platform);
        } else if let Some(host_root) =
            platform::detector::find_host_project_root(&current_dir, HOST_CONFIG_FILE)
        {
            if let Ok(inferred_platform) = platform::detector::detect_platform_type(&current_dir) {
                println!(
                    "{} Detected {} project in {}",
                    "ℹ".blue(),
                    inferred_platform.as_str(),
                    current_dir.display()
                );
                println!("  {} Host project: {}", "•".cyan(), host_root.display());
                println!();

                project_root = host_root;
                inferred_platform_from_subdir = Some(inferred_platform);
            } else {
                return Err(anyhow!(
                    "No config file found in {}.\n\
                     Expected one of:\n\
                     - {} (native host project)\n\
                     - lxapp.json + {} (LxApp project)",
                    current_dir.display(),
                    HOST_CONFIG_FILE,
                    LXAPP_BUILD_CONFIG_FILE
                ));
            }
        } else {
            if let Some(inferred_platform) =
                platform::spm::detect_local_apple_swift_package_platform(&current_dir)?
            {
                println!(
                    "{} Detected standalone Apple Swift Package in {}",
                    "ℹ".blue(),
                    current_dir.display()
                );
                println!("  {} Platform: {}", "•".cyan(), inferred_platform.as_str());
                println!(
                    "  {} lingxia.yaml: not required for standalone Swift Package builds",
                    "•".cyan()
                );
                println!();

                project_root = current_dir.clone();
                inferred_platform_from_subdir = Some(inferred_platform);
                standalone_apple_swift_package = true;
            } else {
                return Err(anyhow!(
                    "No config file found in {}.\n\
                     Expected one of:\n\
                     - {} (native host project)\n\
                     - lxapp.json + {} (LxApp project)\n\
                     Tip: run from a host project, one of its platform subdirectories, or a standalone Apple Swift Package.",
                    current_dir.display(),
                    HOST_CONFIG_FILE,
                    LXAPP_BUILD_CONFIG_FILE
                ));
            }
        }
    }

    if standalone_apple_swift_package {
        return build_standalone_apple_swift_package(
            &project_root,
            inferred_platform_from_subdir,
            build_native,
            release,
            macos_arch,
            platforms,
            all_platforms,
            ipa,
            dmg,
            package,
        );
    }

    // Host/native build
    if package && !release {
        return Err(anyhow!("Packaging requires a release build."));
    }
    let config = LingXiaConfig::load(&project_root)?;

    let app = config.app.as_ref().ok_or_else(|| {
        anyhow!(
            "Missing app section in {}.\n\
             Please configure app.productName/app.productVersion/app.platforms.",
            HOST_CONFIG_FILE
        )
    })?;

    // Determine platforms from config (no auto-detection/fallback).
    let available_platforms: Vec<platform::detector::PlatformType> = app
        .platforms
        .iter()
        .map(|p| p.parse())
        .collect::<Result<Vec<_>, _>>()?;

    if available_platforms.is_empty() {
        return Err(anyhow!(
            "No platform configured in lingxia.config.json.\n\
             Set app.platforms to include at least one of: android, ios, macos, harmony"
        ));
    }

    // Determine which platforms to build.
    let mut requested_platforms = platforms;
    if requested_platforms.is_empty()
        && !all_platforms
        && let Some(inferred_platform) = inferred_platform_from_subdir.as_ref()
    {
        requested_platforms.push(inferred_platform.as_str().to_string());
    }
    let (platforms_to_build, constrained_platforms): (Vec<platform::detector::PlatformType>, bool) =
        if !requested_platforms.is_empty() {
            let mut selected = Vec::new();
            for p in requested_platforms {
                let platform_type: platform::detector::PlatformType = p.parse()?;
                if !available_platforms.contains(&platform_type) {
                    let configured = available_platforms
                        .iter()
                        .map(|p| p.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    return Err(anyhow!(
                        "Platform '{}' is not configured in {} (app.platforms).\n\
Configured platforms: {}",
                        platform_type.as_str(),
                        HOST_CONFIG_FILE,
                        configured
                    ));
                }
                if !selected.contains(&platform_type) {
                    selected.push(platform_type);
                }
            }
            (selected, true)
        } else if all_platforms || available_platforms.len() == 1 {
            (available_platforms.clone(), true)
        } else {
            let available = available_platforms
                .iter()
                .map(|p| p.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(anyhow!(
                "Multiple platforms are configured: {available}\n\
Specify one with `--platform <name>` or build all with `--all-platforms`."
            ));
        };

    // If the user explicitly asked to build iOS/macOS, fail fast on non-macOS hosts
    // (Apple tooling requires macOS).
    if constrained_platforms
        && platforms_to_build.iter().any(|p| {
            matches!(
                p,
                platform::detector::PlatformType::Ios | platform::detector::PlatformType::MacOs
            )
        })
    {
        crate::platform::apple::ensure_macos().map_err(|e| {
            anyhow!(
                "{}\nTip: on non-macOS hosts, pass `--platform android` to build only Android.",
                e
            )
        })?;
    }

    // Parse build profile (cargo-like): debug unless explicitly set to release.
    let build_profile = resolve_build_profile(release);
    // Resolve environment-version independently from debug/release profile.
    let resolved_env = resolve_build_env(&config, env_version.as_deref())?;
    println!(
        "{} Build env: {}{}",
        "ℹ".blue(),
        resolved_env.version,
        if env_version.is_some() {
            " (--env)"
        } else {
            " (default)"
        }
    );

    let has_android = platforms_to_build
        .iter()
        .any(|p| matches!(p, platform::detector::PlatformType::Android));
    let build_targets = if has_android {
        crate::platform::android_abis::resolve_android_targets_from_abis(&abis)?
    } else {
        if !abis.is_empty() {
            println!(
                "{} Ignoring --abis because Android is not in selected platforms",
                "ℹ".blue()
            );
        }
        Vec::new()
    };

    let framework_override = lxapp::parse_framework_override(framework.as_deref())?;

    // Create platforms and build configs once.
    let mut platform_builds: Vec<(Box<dyn platform::Platform>, BuildConfig, PlatformType)> =
        Vec::new();
    for platform_type in &platforms_to_build {
        let platform = match platform::detector::create_platform(platform_type) {
            Ok(p) => p,
            Err(e) => {
                if constrained_platforms {
                    return Err(e);
                }
                eprintln!(
                    "{} Skipping {}: {}",
                    "⚠".yellow(),
                    platform_type.as_str(),
                    e
                );
                continue;
            }
        };
        let build_config = BuildConfig {
            project_root: project_root.clone(),
            profile: build_profile,
            build_native,
            targets: if matches!(platform_type, PlatformType::Android) {
                build_targets.clone()
            } else {
                Vec::new()
            },
            lingxia_config: Some(config.clone()),
            ipa: ipa && matches!(platform_type, PlatformType::Ios),
            package: package && matches!(platform_type, PlatformType::MacOs),
            dmg: dmg && matches!(platform_type, PlatformType::MacOs),
            macos_arch: if matches!(platform_type, PlatformType::MacOs) {
                macos_arch.clone()
            } else {
                None
            },
            framework: framework_override,
            native_features: config.native_features_for_platform(platform_type.as_str()),
            native_default_features: config.native_default_features_enabled(),
            resolved_env: resolved_env.clone(),
            skip_native_build: false,
        };
        platform_builds.push((platform, build_config, platform_type.clone()));
    }

    // Phase 1: Build the native Rust library first so that cargo build.rs
    // checks (e.g. muke-lib's cloud-types drift check) fail fast before the
    // slow lxapp asset build runs. Platforms that don't hoist their native
    // build (default `Platform::hoists_native_build() == false`) are no-ops
    // here and run native inline in Phase 3 as before.
    for (platform, build_config, _) in &platform_builds {
        platform.build_rust_library(build_config)?;
    }

    // Phase 2: Build lxapp assets.
    prepare_configured_host_assets(
        &project_root,
        &config,
        build_profile,
        framework_override,
        progress.as_deref(),
        &platforms_to_build,
        &build_targets,
        constrained_platforms,
        None,
        &resolved_env,
    )?;

    // Phase 3: Build platform packages (Gradle / Swift / etc.).
    // For platforms that hoisted their native build into Phase 1 we set
    // skip_native_build so `build` doesn't re-invoke cargo. Platforms that
    // didn't opt in (Phase 1 was a no-op for them) keep the flag false and
    // build native inline as before.
    let mut all_artifacts = Vec::new();
    for (platform, mut build_config, platform_type) in platform_builds {
        build_config.skip_native_build = platform.hoists_native_build();
        let artifacts = platform.build(&build_config)?;
        if package {
            stage_package_artifact(&project_root, &platform_type, &artifacts)?;
        }
        all_artifacts.push((platform_type, artifacts));
    }

    if all_artifacts.is_empty() {
        return Err(anyhow!("No supported platforms to build."));
    }

    // Print build summary
    println!();
    for (platform_type, artifacts) in &all_artifacts {
        println!(
            "{} {} → {}",
            "✓".green(),
            platform_type.as_str(),
            artifacts.path().display()
        );
    }

    Ok(())
}

fn stage_package_artifact(
    project_root: &Path,
    platform_type: &PlatformType,
    artifacts: &BuildArtifacts,
) -> Result<Option<PathBuf>> {
    if !matches!(platform_type, PlatformType::Android | PlatformType::Harmony) {
        return Ok(None);
    }

    let source = artifacts.path();
    if !source.is_file() {
        return Ok(None);
    }

    let file_name = source
        .file_name()
        .ok_or_else(|| anyhow!("Invalid artifact path: {}", source.display()))?;
    let dist_dir = project_root.join("dist").join(platform_type.as_str());
    fs::create_dir_all(&dist_dir)?;
    let dest = dist_dir.join(file_name);

    if source != dest {
        fs::copy(source, &dest).map_err(|err| {
            anyhow!(
                "Failed to stage package artifact {} -> {}: {}",
                source.display(),
                dest.display(),
                err
            )
        })?;
    }

    println!("{} package → {}", "✓".green(), dest.display());
    Ok(Some(dest))
}

/// Resolve the active environment for a build/dev/package invocation.
///
/// `--env <name>` chooses the env; omitted defaults to `Developer` (callers
/// like `package` override the default before getting here). env-version is
/// a build-time property with built-in defaults — no yaml block is required.
pub(crate) fn resolve_build_env(
    config: &LingXiaConfig,
    requested: Option<&str>,
) -> Result<crate::config::ResolvedEnv> {
    let version = requested
        .map(crate::config::EnvVersion::parse_cli)
        .transpose()?
        .unwrap_or(crate::config::EnvVersion::Developer);
    config.resolve_env(version)
}

#[cfg(test)]
mod tests {
    use super::{resolve_build_env, stage_package_artifact};
    use crate::config::{EnvVersion, LingXiaConfig};
    use crate::platform::BuildArtifacts;
    use crate::platform::detector::PlatformType;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn package_stages_android_apk_under_dist_android() {
        let temp = TempDir::new().unwrap();
        let source = temp
            .path()
            .join("android/app/build/outputs/apk/release/app-release.apk");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, b"apk").unwrap();

        let artifacts = BuildArtifacts::Android {
            apk_path: source.clone(),
        };
        let staged = stage_package_artifact(temp.path(), &PlatformType::Android, &artifacts)
            .unwrap()
            .unwrap();

        assert_eq!(staged, temp.path().join("dist/android/app-release.apk"));
        assert_eq!(fs::read(staged).unwrap(), b"apk");
    }

    #[test]
    fn package_does_not_restage_macos_artifact() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("dist/macos/Demo-1.0.0-macos.zip");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, b"zip").unwrap();

        let artifacts = BuildArtifacts::MacOs {
            app_path: temp.path().join("macos/.build/Demo.app"),
            update_zip_path: Some(source),
            dmg_path: None,
        };

        assert!(
            stage_package_artifact(temp.path(), &PlatformType::MacOs, &artifacts)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn omitted_env_defaults_to_developer_with_builtin_suffix() {
        let config = LingXiaConfig::new_android("demo", "com.example.demo", "demo");

        let resolved = resolve_build_env(&config, None).unwrap();

        assert_eq!(resolved.version, EnvVersion::Developer);
        assert_eq!(resolved.effective_package_id_suffix(), Some(".dev"));
    }

    #[test]
    fn explicit_env_release_clears_suffix() {
        let config = LingXiaConfig::new_android("demo", "com.example.demo", "demo");

        let release = resolve_build_env(&config, Some("release")).unwrap();
        let preview = resolve_build_env(&config, Some("preview")).unwrap();

        assert_eq!(release.version, EnvVersion::Release);
        assert_eq!(release.effective_package_id_suffix(), None);
        assert_eq!(preview.version, EnvVersion::Preview);
        assert_eq!(preview.effective_package_id_suffix(), Some(".preview"));
    }
}

fn build_standalone_apple_swift_package(
    project_root: &std::path::Path,
    inferred_platform: Option<PlatformType>,
    build_native: bool,
    release: bool,
    macos_arch: Option<String>,
    platforms: Vec<String>,
    all_platforms: bool,
    ipa: bool,
    dmg: bool,
    package: bool,
) -> Result<()> {
    if package && !release {
        return Err(anyhow!(
            "Packaging requires a release build for standalone Apple Swift Package projects."
        ));
    }

    let inferred_platform = inferred_platform
        .ok_or_else(|| anyhow!("Failed to infer platform for standalone Apple Swift Package"))?;

    let build_profile = resolve_build_profile(release);
    let mut requested_platforms = platforms;
    if requested_platforms.is_empty() && !all_platforms {
        requested_platforms.push(inferred_platform.as_str().to_string());
    }

    let platforms_to_build = if !requested_platforms.is_empty() {
        let mut selected = Vec::new();
        for p in requested_platforms {
            let platform_type: PlatformType = p.parse()?;
            if !matches!(platform_type, PlatformType::MacOs) {
                return Err(anyhow!(
                    "Standalone Apple Swift Package without {} only supports macos builds, got '{}'",
                    HOST_CONFIG_FILE,
                    platform_type.as_str()
                ));
            }
            if !selected.contains(&platform_type) {
                selected.push(platform_type);
            }
        }
        selected
    } else {
        vec![inferred_platform]
    };

    crate::platform::apple::ensure_macos()?;

    let mut all_artifacts = Vec::new();
    for platform_type in platforms_to_build {
        let platform = platform::detector::create_platform(&platform_type)?;
        let build_config = BuildConfig {
            project_root: project_root.to_path_buf(),
            profile: build_profile,
            build_native,
            targets: Vec::new(),
            lingxia_config: None,
            ipa: ipa && matches!(platform_type, PlatformType::Ios),
            package: package && matches!(platform_type, PlatformType::MacOs),
            dmg: dmg && matches!(platform_type, PlatformType::MacOs),
            macos_arch: if matches!(platform_type, PlatformType::MacOs) {
                macos_arch.clone()
            } else {
                None
            },
            framework: None,
            native_features: if matches!(platform_type, PlatformType::MacOs) {
                vec!["shell-runtime".to_string(), "webview-input".to_string()]
            } else {
                Vec::new()
            },
            native_default_features: true,
            // Standalone Apple SwiftPM builds have no `lingxia.yaml` to draw
            // env config from; they always run as the default release env.
            resolved_env: crate::config::ResolvedEnv {
                version: crate::config::EnvVersion::Release,
                lingxia_server: String::new(),
                package_id_suffix: None,
            },
            skip_native_build: false,
        };

        let artifacts = platform.build(&build_config)?;
        all_artifacts.push((platform_type, artifacts));
    }

    println!();
    for (platform_type, artifacts) in &all_artifacts {
        println!(
            "{} {} → {}",
            "✓".green(),
            platform_type.as_str(),
            artifacts.path().display()
        );
    }

    Ok(())
}
