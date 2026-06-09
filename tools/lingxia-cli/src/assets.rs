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
    prepare_windows_assets_root,
};
use hash::sha256_hex;
#[cfg(test)]
use icons::PreparedAppUiIcon;
use icons::prepare_app_ui_icons;
#[cfg(test)]
use icons::validate_app_ui_svg_icon;
use json::{
    build_app_json_from_config, build_ui_json_from_config, build_windows_ui_json_from_config,
};
use runtime_asset::{prepare_polyfills_es5_asset, prepare_runtime_asset};
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
pub(crate) use shell_webui::APP_ID as SHELL_WEBUI_APP_ID;
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

/// Collect build-time warnings for any path-based lxapp bundle whose
/// `view.target` won't run on the Android floor declared in `lingxia.yaml`.
///
/// Background: Vite's modulepreload polyfill (emitted whenever the build is
/// not in legacy/ES5 mode) uses `for...of` over a `NodeList`, which only works
/// from Chromium 51. Android 5.x/6.0 stock WebView is older than that, so a
/// `target: 'es2015'` bundle throws `TypeError: undefined is not a function`
/// on those devices at page load.
///
/// We only inspect *path-based* bundles (source visible in this repo).
/// Package-based bundles are pre-built upstream and outside our control.
fn collect_view_target_warnings(
    project_root: &Path,
    config: &LingXiaConfig,
    android_min_sdk: Option<u32>,
) -> Vec<String> {
    let mut warnings = Vec::new();
    let Some(min_sdk) = android_min_sdk else {
        return warnings;
    };
    if min_sdk >= runtime::MODERN_WEBVIEW_MIN_SDK {
        return warnings;
    }
    let Some(resources) = config.resources.as_ref() else {
        return warnings;
    };
    for bundle in &resources.bundles {
        let Some(path) = bundle
            .path
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.is_empty())
        else {
            continue;
        };
        let bundle_dir = project_root.join(path);
        let target = match crate::lxapp::view_target_from_dir(&bundle_dir) {
            Ok(Some(t)) => t,
            // Missing target == default modern path, which is the dangerous case.
            Ok(None) => String::new(),
            // Unreadable lxapp.config.ts isn't worth failing for; skip silently.
            Err(_) => continue,
        };
        if target.eq_ignore_ascii_case("es5") {
            continue;
        }
        let displayed_target = if target.is_empty() {
            "(default, modern)".to_string()
        } else {
            format!("'{target}'")
        };
        warnings.push(format!(
            "android.minSdk = {min_sdk} but {path}/lxapp.config.ts has view.target = {displayed_target}.\n  \
             Stock WebView on Android < {threshold} (Chromium < 51) cannot iterate NodeList, \
             which Vite's modulepreload polyfill depends on. Set view.target = 'es5' to route \
             through the legacy pipeline (post-transpile to ES5, no modulepreload polyfill).",
            threshold = runtime::MODERN_WEBVIEW_MIN_SDK,
        ));
    }
    warnings
}

/// Whether any path-based lxapp bundle in the host config opts into the
/// legacy ES5 view pipeline. When true, the bundle's emitted HTML carries a
/// `<script src="lx://assets/polyfills.es5.js">` tag (see vite_html.rs /
/// vite_pipeline.rs), so the polyfills asset has to ship alongside it.
/// Decoupled from the global runtime decision: a single bundle in legacy
/// mode still needs the polyfills script even on a minSdk-modern app.
fn any_path_bundle_targets_es5(project_root: &Path, config: &LingXiaConfig) -> bool {
    let Some(resources) = config.resources.as_ref() else {
        return false;
    };
    for bundle in &resources.bundles {
        let Some(path) = bundle
            .path
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.is_empty())
        else {
            continue;
        };
        if let Ok(Some(target)) = crate::lxapp::view_target_from_dir(&project_root.join(path))
            && target.eq_ignore_ascii_case("es5")
        {
            return true;
        }
    }
    false
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
    resolved_env: &crate::config::ResolvedEnv,
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
                | platform::detector::PlatformType::Windows
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
    let app_json =
        build_app_json_from_config(config, prepared_bundles.first(), dev_ws_url, resolved_env)?;
    let app_json_hash = sha256_hex(app_json.as_bytes());
    let prepared_app_ui_icons = prepare_app_ui_icons(project_root, config)?;
    let ui_json = build_ui_json_from_config(config, &prepared_app_ui_icons)?;
    let ui_json_hash = ui_json.as_ref().map(|json| sha256_hex(json.as_bytes()));
    let windows_ui_json = build_windows_ui_json_from_config(config, &prepared_app_ui_icons)?;
    let windows_ui_json_hash = windows_ui_json
        .as_ref()
        .map(|json| sha256_hex(json.as_bytes()));

    let has_android = platforms
        .iter()
        .any(|p| matches!(p, platform::detector::PlatformType::Android));
    let has_non_android = platforms
        .iter()
        .any(|p| !matches!(p, platform::detector::PlatformType::Android));
    let android_min_sdk = config.android.as_ref().and_then(|a| a.min_sdk);
    if has_android {
        for warning in collect_view_target_warnings(project_root, config, android_min_sdk) {
            eprintln!("{}: {warning}", "warning".yellow().bold());
        }
    }
    let needs_es5_runtime = has_android
        && runtime::target_from_build_targets(build_targets, android_min_sdk)
            == runtime::RuntimeEcmaTarget::Es5;

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
    // Polyfills ship whenever a bundle's HTML references them — i.e., the
    // bundle opts into view.target = 'es5'. Independent of needs_es5_runtime:
    // an app with minSdk >= 24 can still contain a single bundle in legacy
    // mode whose HTML loads polyfills.es5.js.
    let prepared_polyfills_es5 = if has_android && any_path_bundle_targets_es5(project_root, config)
    {
        Some(prepare_polyfills_es5_asset())
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
                    prepared_polyfills_es5.as_ref(),
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
            platform::detector::PlatformType::Windows => {
                let assets_root =
                    crate::platform::windows::resolve_windows_assets_dir(project_root)?;
                prepare_windows_assets_root(
                    &assets_root,
                    &app_json,
                    &app_json_hash,
                    windows_ui_json.as_deref(),
                    windows_ui_json_hash.as_deref(),
                    &prepared_bundles,
                    &prepared_app_ui_icons,
                    prepared_runtime_es2020.as_ref(),
                    &mut cache,
                )?;
                copy_splash_asset(config, project_root, &assets_root)?;
            }
        }
    }

    cache.save(project_root)?;
    Ok(())
}
