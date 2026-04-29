use crate::config::LingXiaConfig;
use crate::lxapp::ProjectFramework;
use crate::platform::{self, BuildProfile};
use crate::runtime;
use anyhow::{Context, Result, anyhow};
use bundles::{
    prepare_home_app_bundle, prepare_resource_lxapp_bundles, prepare_shell_webui_bundle,
};
use cache::HostAssetsCache;
pub(crate) use clean::clean_configured_host_assets;
use colored::Colorize;
use destinations::{
    prepare_android_assets_root, prepare_apple_resources_root, prepare_harmony_rawfile_root,
};
use hash::sha256_hex;
#[cfg(test)]
use icons::PreparedAppUiIcon;
use icons::prepare_app_ui_icons;
#[cfg(test)]
use icons::validate_app_ui_svg_icon;
use json::{build_app_json_from_config, build_ui_json_from_config};
use runtime_asset::prepare_runtime_asset;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use sync::write_if_changed;

#[path = "assets/bundles.rs"]
mod bundles;
#[path = "assets/cache.rs"]
mod cache;
#[path = "assets/clean.rs"]
mod clean;
#[path = "assets/destinations.rs"]
mod destinations;
#[path = "assets/hash.rs"]
mod hash;
#[path = "assets/icons.rs"]
mod icons;
#[path = "assets/json.rs"]
mod json;
#[path = "assets/runtime.rs"]
mod runtime_asset;
#[path = "assets/shell_webui.rs"]
mod shell_webui;
#[path = "assets/sync.rs"]
mod sync;
#[cfg(test)]
#[path = "assets/tests.rs"]
mod tests;
#[path = "assets/ui.rs"]
mod ui;

fn is_png_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("png"))
        .unwrap_or(false)
}

fn copy_splash_asset(config: &LingXiaConfig, project_root: &Path, dest_dir: &Path) -> Result<()> {
    let splash_path = match config.splash_path() {
        Some(path) => path,
        None => return Ok(()),
    };
    let src = project_root.join(splash_path);
    if !is_png_path(&src) {
        anyhow::bail!(
            "Invalid ui.launch.splash.path '{}': splash image must be a PNG file",
            splash_path
        );
    }
    if !src.exists() {
        anyhow::bail!("Splash image not found: {}", src.display());
    }
    let dest = dest_dir.join("splash.png");
    let src_bytes = fs::read(&src)
        .with_context(|| format!("Failed to read splash image: {}", src.display()))?;
    if write_if_changed(&dest, &src_bytes)? {
        println!("  {} splash.png → {}", "✓".green(), dest.display());
    }
    Ok(())
}

pub(crate) fn prepare_configured_host_assets(
    project_root: &Path,
    config: &LingXiaConfig,
    build_profile: BuildProfile,
    framework_override: Option<ProjectFramework>,
    progress_override: Option<&str>,
    platforms: &[platform::detector::PlatformType],
    build_targets: &[String],
    _explicit_platforms: bool,
    dev_ws_url: Option<&str>,
) -> Result<()> {
    let mut cache = HostAssetsCache::load(project_root);
    let app_project_name = config.app.as_ref().map(|a| a.project_name.as_str());

    let needs_embedded_lxapp = platforms.iter().any(|p| {
        matches!(
            p,
            platform::detector::PlatformType::Android
                | platform::detector::PlatformType::Ios
                | platform::detector::PlatformType::MacOs
                | platform::detector::PlatformType::Harmony
        )
    });
    if !needs_embedded_lxapp {
        return Err(anyhow!(
            "No platform requires embedded home LxApp assets for this build"
        ));
    }

    let home_bundle = prepare_home_app_bundle(
        project_root,
        config,
        build_profile,
        framework_override,
        progress_override,
        &mut cache,
    )?;
    let mut prepared_bundles = vec![home_bundle];
    prepared_bundles.extend(prepare_resource_lxapp_bundles(
        project_root,
        config,
        build_profile,
        framework_override,
        progress_override,
        &mut cache,
    )?);
    if platforms.iter().any(|p| config.shell_enabled(p.as_str())) {
        prepared_bundles.push(prepare_shell_webui_bundle(
            project_root,
            config,
            build_profile,
            &mut cache,
        )?);
    }
    let app_json = build_app_json_from_config(config, prepared_bundles.first(), dev_ws_url)?;
    let app_json_hash = sha256_hex(app_json.as_bytes());
    let prepared_app_ui_icons = prepare_app_ui_icons(project_root, config)?;
    let ui_json = build_ui_json_from_config(config, &prepared_app_ui_icons)?;
    let ui_json_hash = ui_json.as_ref().map(|json| sha256_hex(json.as_bytes()));

    let has_android = platforms
        .iter()
        .any(|p| matches!(p, platform::detector::PlatformType::Android));
    let has_non_android = platforms
        .iter()
        .any(|p| !matches!(p, platform::detector::PlatformType::Android));
    let needs_es5_runtime = has_android
        && runtime::target_from_build_targets(build_targets) == runtime::RuntimeEcmaTarget::Es5;

    // Only Android can require ES5 runtime here (for armv7 builds).
    let prepared_runtime_es5 = if needs_es5_runtime {
        Some(prepare_runtime_asset(runtime::RuntimeEcmaTarget::Es5))
    } else {
        None
    };
    let prepared_runtime_es2020 = if has_non_android || !needs_es5_runtime {
        Some(prepare_runtime_asset(runtime::RuntimeEcmaTarget::Es2020))
    } else {
        None
    };

    // Deduplicate resource destinations (iOS/macOS can share the same Swift package dir).
    let mut prepared_resource_roots: HashSet<PathBuf> = HashSet::new();

    for platform in platforms {
        match platform {
            platform::detector::PlatformType::Android => {
                let assets_root = platform::detector::resolve_android_assets_dir(project_root);
                prepare_android_assets_root(
                    &assets_root,
                    &app_json,
                    &app_json_hash,
                    ui_json.as_deref(),
                    ui_json_hash.as_deref(),
                    &prepared_bundles,
                    prepared_runtime_es5
                        .as_ref()
                        .or(prepared_runtime_es2020.as_ref()),
                    &mut cache,
                )?;
                copy_splash_asset(config, project_root, &assets_root)?;
            }
            platform::detector::PlatformType::Ios => {
                if !crate::platform::apple::is_macos() {
                    // Keep parity with platform detection: iOS builds are skipped on non-macOS hosts.
                    continue;
                }
                let ios_dir =
                    crate::platform::ios::resolve_ios_dir(project_root, config.ios.as_ref())?;
                let resources_dir = crate::platform::ios::get_resources_dir(
                    &ios_dir,
                    config.ios.as_ref(),
                    app_project_name,
                )?;
                prepare_apple_resources_root(
                    &resources_dir,
                    &app_json,
                    &app_json_hash,
                    ui_json.as_deref(),
                    ui_json_hash.as_deref(),
                    &prepared_bundles,
                    &prepared_app_ui_icons,
                    prepared_runtime_es2020.as_ref(),
                    &mut prepared_resource_roots,
                    &mut cache,
                )?;
                copy_splash_asset(config, project_root, &resources_dir)?;
            }
            platform::detector::PlatformType::MacOs => {
                if !crate::platform::apple::is_macos() {
                    // Keep parity with platform detection: macOS builds are skipped on non-macOS hosts.
                    continue;
                }
                let macos_dir =
                    crate::platform::macos::resolve_macos_dir(project_root, config.macos.as_ref())?;
                let resources_dir = crate::platform::macos::get_resources_dir(
                    &macos_dir,
                    config.macos.as_ref(),
                    app_project_name,
                )?;
                prepare_apple_resources_root(
                    &resources_dir,
                    &app_json,
                    &app_json_hash,
                    ui_json.as_deref(),
                    ui_json_hash.as_deref(),
                    &prepared_bundles,
                    &prepared_app_ui_icons,
                    prepared_runtime_es2020.as_ref(),
                    &mut prepared_resource_roots,
                    &mut cache,
                )?;
                copy_splash_asset(config, project_root, &resources_dir)?;
            }
            platform::detector::PlatformType::Harmony => {
                let rawfile_root =
                    crate::platform::harmony::resolve_harmony_rawfile_dir(project_root)?;
                prepare_harmony_rawfile_root(
                    &rawfile_root,
                    &app_json,
                    &app_json_hash,
                    ui_json.as_deref(),
                    ui_json_hash.as_deref(),
                    &prepared_bundles,
                    prepared_runtime_es2020.as_ref(),
                    &mut cache,
                )?;
                copy_splash_asset(config, project_root, &rawfile_root)?;
            }
        }
    }

    cache.save(project_root)?;
    Ok(())
}
