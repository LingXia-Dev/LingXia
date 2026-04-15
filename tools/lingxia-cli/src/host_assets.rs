use crate::config::{
    HOST_CONFIG_FILE, LXAPP_BUILD_CONFIG_FILE, LingXiaConfig, ResourceBundleConfig,
    ResourceBundleType,
};
use crate::lxapp;
use crate::lxapp::ProjectFramework;
use crate::platform::{self, BuildProfile};
use crate::runtime;
use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const CACHE_VERSION: u32 = 1;

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

    let prepared_bundles = prepare_resource_bundles(
        project_root,
        config,
        build_profile,
        framework_override,
        progress_override,
        &mut cache,
    )?;
    let home_bundle = configured_home_bundle(config, &prepared_bundles);
    if let Some(home_lxapp_id) = config
        .app
        .as_ref()
        .and_then(|app| app.home_lxapp_id.as_deref())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        && home_bundle.is_none()
    {
        return Err(anyhow!(
            "homeLxAppID '{}' is configured but no matching bundle was prepared.\n\
Add the home LxApp project to resources.bundles and ensure its lxapp.json appId matches.",
            home_lxapp_id
        ));
    }
    let app_json = build_app_json_from_config(config, home_bundle)?;
    let app_json_hash = sha256_hex(app_json.as_bytes());
    let ui_json = build_ui_json_from_config(config)?;
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

fn prepare_android_assets_root(
    assets_root: &Path,
    app_json: &str,
    app_json_hash: &str,
    ui_json: Option<&str>,
    ui_json_hash: Option<&str>,
    bundles: &[PreparedResourceBundle],
    runtime_asset: Option<&PreparedRuntimeAsset>,
    cache: &mut HostAssetsCache,
) -> Result<()> {
    fs::create_dir_all(assets_root)?;

    let dest_key = path_key(assets_root);
    let prev = cache.destinations.get(&dest_key).cloned();

    let desired = DestinationStamp {
        app_json_hash: app_json_hash.to_string(),
        ui_json_hash: ui_json_hash.map(ToOwned::to_owned),
        bundle_hashes: bundle_hashes(bundles),
        runtime_hash: runtime_asset.map(|r| r.runtime_hash.clone()),
    };

    let mut changed = false;

    // app.json
    let app_json_path = assets_root.join("app.json");
    if write_if_changed(&app_json_path, app_json.as_bytes())? {
        changed = true;
        println!("  {} app.json → {}", "✓".green(), app_json_path.display());
    }
    changed |= sync_optional_json_file(
        &assets_root.join("ui.json"),
        ui_json,
        prev.as_ref().and_then(|s| s.ui_json_hash.as_deref()),
        "ui.json",
    )?;
    changed |= sync_runtime_file(
        &assets_root.join("bridge-runtime.js"),
        runtime_asset,
        prev.as_ref().and_then(|s| s.runtime_hash.as_deref()),
    )?;
    changed |= sync_resource_bundles(
        assets_root,
        bundles,
        prev.as_ref().map(|s| &s.bundle_hashes),
    )?;

    if changed {
        cache.destinations.insert(dest_key, desired);
    }

    Ok(())
}

fn prepare_apple_resources_root(
    resources_dir: &Path,
    app_json: &str,
    app_json_hash: &str,
    ui_json: Option<&str>,
    ui_json_hash: Option<&str>,
    bundles: &[PreparedResourceBundle],
    runtime_asset: Option<&PreparedRuntimeAsset>,
    prepared_roots: &mut HashSet<PathBuf>,
    cache: &mut HostAssetsCache,
) -> Result<()> {
    let resources_dir = resources_dir.to_path_buf();
    if !prepared_roots.insert(resources_dir.clone()) {
        return Ok(());
    }

    println!(
        "{}",
        format!("Preparing resources → {}", resources_dir.display()).cyan()
    );

    fs::create_dir_all(&resources_dir)?;
    let dest_key = path_key(&resources_dir);
    let prev = cache.destinations.get(&dest_key).cloned();

    let desired = DestinationStamp {
        app_json_hash: app_json_hash.to_string(),
        ui_json_hash: ui_json_hash.map(ToOwned::to_owned),
        bundle_hashes: bundle_hashes(bundles),
        runtime_hash: runtime_asset.map(|r| r.runtime_hash.clone()),
    };

    let mut changed = false;

    let app_json_path = resources_dir.join("app.json");
    if write_if_changed(&app_json_path, app_json.as_bytes())? {
        changed = true;
        println!("  {} app.json → {}", "✓".green(), app_json_path.display());
    }
    changed |= sync_optional_json_file(
        &resources_dir.join("ui.json"),
        ui_json,
        prev.as_ref().and_then(|s| s.ui_json_hash.as_deref()),
        "ui.json",
    )?;
    changed |= sync_runtime_file(
        &resources_dir.join("bridge-runtime.js"),
        runtime_asset,
        prev.as_ref().and_then(|s| s.runtime_hash.as_deref()),
    )?;
    changed |= sync_resource_bundles(
        &resources_dir,
        bundles,
        prev.as_ref().map(|s| &s.bundle_hashes),
    )?;

    if changed {
        cache.destinations.insert(dest_key, desired);
    }

    Ok(())
}

fn prepare_harmony_rawfile_root(
    rawfile_root: &Path,
    app_json: &str,
    app_json_hash: &str,
    ui_json: Option<&str>,
    ui_json_hash: Option<&str>,
    bundles: &[PreparedResourceBundle],
    runtime_asset: Option<&PreparedRuntimeAsset>,
    cache: &mut HostAssetsCache,
) -> Result<()> {
    println!(
        "{}",
        format!("Preparing HarmonyOS rawfile → {}", rawfile_root.display()).cyan()
    );

    fs::create_dir_all(rawfile_root)?;

    let dest_key = path_key(rawfile_root);
    let prev = cache.destinations.get(&dest_key).cloned();

    let desired = DestinationStamp {
        app_json_hash: app_json_hash.to_string(),
        ui_json_hash: ui_json_hash.map(ToOwned::to_owned),
        bundle_hashes: bundle_hashes(bundles),
        runtime_hash: runtime_asset.map(|r| r.runtime_hash.clone()),
    };

    let mut changed = false;

    // app.json
    let app_json_path = rawfile_root.join("app.json");
    if write_if_changed(&app_json_path, app_json.as_bytes())? {
        changed = true;
        println!("  {} app.json → {}", "✓".green(), app_json_path.display());
    }
    changed |= sync_optional_json_file(
        &rawfile_root.join("ui.json"),
        ui_json,
        prev.as_ref().and_then(|s| s.ui_json_hash.as_deref()),
        "ui.json",
    )?;
    changed |= sync_runtime_file(
        &rawfile_root.join("bridge-runtime.js"),
        runtime_asset,
        prev.as_ref().and_then(|s| s.runtime_hash.as_deref()),
    )?;
    changed |= sync_resource_bundles(
        rawfile_root,
        bundles,
        prev.as_ref().map(|s| &s.bundle_hashes),
    )?;

    if changed {
        cache.destinations.insert(dest_key, desired);
    }

    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct DestinationStamp {
    app_json_hash: String,
    ui_json_hash: Option<String>,
    bundle_hashes: BTreeMap<String, String>,
    runtime_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LxAppBuildStamp {
    inputs_hash: String,
    dist_hash: String,
    asset_name: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct HostAssetsCache {
    version: u32,
    lxapp_builds: HashMap<String, LxAppBuildStamp>,
    destinations: HashMap<String, DestinationStamp>,
}

impl HostAssetsCache {
    fn load(project_root: &Path) -> Self {
        let path = cache_path(project_root);
        let data = match fs::read(&path) {
            Ok(d) => d,
            Err(_) => return Self::default_v1(),
        };
        match serde_json::from_slice::<HostAssetsCache>(&data) {
            Ok(cache) if cache.version == CACHE_VERSION => cache,
            _ => Self::default_v1(),
        }
    }

    fn save(&mut self, project_root: &Path) -> Result<()> {
        let path = cache_path(project_root);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        self.version = CACHE_VERSION;
        fs::write(path, serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }

    fn default_v1() -> Self {
        Self {
            version: CACHE_VERSION,
            lxapp_builds: HashMap::new(),
            destinations: HashMap::new(),
        }
    }
}

struct PreparedResourceBundle {
    dist_dir: PathBuf,
    asset_name: String,
    dist_hash: String,
    version: String,
}

struct ResourceBundlePlan {
    bundle_dir: PathBuf,
    bundle_type: ResourceBundleType,
    asset_name: String,
    output_dir: PathBuf,
    version: String,
}

struct PreparedRuntimeAsset {
    bytes: Vec<u8>,
    runtime_hash: String,
}

fn prepare_runtime_asset(target: runtime::RuntimeEcmaTarget) -> PreparedRuntimeAsset {
    let resolved = runtime::embedded_runtime(target);
    println!(
        "  {} bridge-runtime.js ({}) ← {}",
        "✓".green(),
        target.as_str(),
        resolved.source
    );

    PreparedRuntimeAsset {
        bytes: resolved.bytes.to_vec(),
        runtime_hash: resolved.hash,
    }
}

fn prepare_resource_bundles(
    project_root: &Path,
    config: &LingXiaConfig,
    build_profile: BuildProfile,
    framework_override: Option<ProjectFramework>,
    progress_override: Option<&str>,
    cache: &mut HostAssetsCache,
) -> Result<Vec<PreparedResourceBundle>> {
    let app = config
        .app
        .as_ref()
        .ok_or_else(|| anyhow!("Missing app settings in {}", HOST_CONFIG_FILE))?;

    let configured_bundles = config
        .resources
        .as_ref()
        .and_then(|resources| resources.bundles.as_ref())
        .cloned();

    let bundle_entries = if let Some(entries) = configured_bundles {
        entries
    } else {
        println!(
            "{} No resource bundles configured, skipping bundle preparation",
            "ℹ".blue()
        );
        return Ok(Vec::new());
    };
    let configured_home_lxapp_id = app
        .home_lxapp_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    println!("{}", "Preparing resource bundles...".bold());

    let mut prepared = Vec::new();
    for bundle in bundle_entries {
        let (bundle_path, bundle_type, explicit_target) = match bundle {
            ResourceBundleConfig::Path(path) => (path, ResourceBundleType::Lxapp, None),
            ResourceBundleConfig::Detailed(detail) => {
                (detail.path, detail.bundle_type, detail.target)
            }
        };
        let bundle_dir = project_root.join(&bundle_path);
        if !bundle_dir.exists() {
            return Err(anyhow!(
                "Configured resource bundle directory not found: {}",
                bundle_dir.display()
            ));
        }

        let plan = match bundle_type {
            ResourceBundleType::Lxapp => {
                let lxapp_json = bundle_dir.join("lxapp.json");
                let lxapp_build_config = bundle_dir.join(LXAPP_BUILD_CONFIG_FILE);
                if !lxapp_json.exists() || !lxapp_build_config.exists() {
                    return Err(anyhow!(
                        "Configured lxapp bundle must contain lxapp.json and {}: {}",
                        LXAPP_BUILD_CONFIG_FILE,
                        bundle_dir.display()
                    ));
                }
                let metadata = read_lxapp_metadata(&lxapp_json)?;
                ResourceBundlePlan {
                    bundle_dir: bundle_dir.clone(),
                    bundle_type,
                    asset_name: metadata.app_id,
                    output_dir: bundle_dir.join("dist"),
                    version: metadata.version,
                }
            }
            ResourceBundleType::Npm => {
                let package_json = bundle_dir.join("package.json");
                if !package_json.exists() {
                    return Err(anyhow!(
                        "Configured npm bundle must contain package.json: {}",
                        bundle_dir.display()
                    ));
                }
                let target = explicit_target
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| {
                        anyhow!(
                            "Configured npm bundle requires target: {}",
                            bundle_dir.display()
                        )
                    })?;
                ResourceBundlePlan {
                    bundle_dir: bundle_dir.clone(),
                    bundle_type,
                    asset_name: target.to_string(),
                    output_dir: bundle_dir.join("dist"),
                    version: "0.0.0".to_string(),
                }
            }
        };

        let cache_key = format!(
            "{}|{}|{}",
            path_key(&plan.bundle_dir),
            build_profile.as_str(),
            plan.bundle_type.as_str()
        );
        let inputs_hash = hash_tree(
            &plan.bundle_dir,
            &["dist", "node_modules", ".git", ".lingxia"],
        )?;
        let mut needs_build = true;

        if let Some(stamp) = cache.lxapp_builds.get(&cache_key)
            && stamp.inputs_hash == inputs_hash
            && plan.output_dir.exists()
        {
            needs_build = false;
        }

        if needs_build {
            println!("  {} {}", "Building bundle...".cyan(), bundle_dir.display());
            match plan.bundle_type {
                ResourceBundleType::Lxapp => {
                    let mut args = vec!["build".to_string()];
                    if matches!(build_profile, BuildProfile::Release) {
                        args.push("--release".to_string());
                    }
                    if configured_home_lxapp_id.as_deref() == Some(plan.asset_name.as_str())
                        && let Some(framework) = framework_override
                    {
                        args.push("--framework".to_string());
                        args.push(framework.as_str().to_string());
                    }
                    if let Some(progress) = progress_override {
                        args.push("--progress".to_string());
                        args.push(progress.to_string());
                    }
                    lxapp::run_in_dir(&args, &plan.bundle_dir)?
                }
                ResourceBundleType::Npm => run_npm_bundle_build(&plan.bundle_dir, build_profile)?,
            }
        } else {
            println!(
                "  {} bundle unchanged, skip build: {}",
                "✓".green(),
                plan.bundle_dir.display()
            );
        }

        if !plan.output_dir.exists() {
            return Err(anyhow!(
                "Bundle build output not found: {}",
                plan.output_dir.display()
            ));
        }

        let dist_hash = hash_tree(&plan.output_dir, &[])?;
        cache.lxapp_builds.insert(
            cache_key,
            LxAppBuildStamp {
                inputs_hash,
                dist_hash: dist_hash.clone(),
                asset_name: plan.asset_name.clone(),
            },
        );

        prepared.push(PreparedResourceBundle {
            dist_dir: plan.output_dir,
            asset_name: plan.asset_name,
            dist_hash,
            version: plan.version,
        });
    }

    Ok(prepared)
}

fn build_app_json_from_config(
    config: &LingXiaConfig,
    home_bundle: Option<&PreparedResourceBundle>,
) -> Result<String> {
    let app = config
        .app
        .as_ref()
        .ok_or_else(|| anyhow!("Missing app settings in {}", HOST_CONFIG_FILE))?;
    let api_server = app.api_server.as_deref();

    let mut obj = serde_json::Map::new();
    obj.insert(
        "productName".to_string(),
        serde_json::json!(app.product_name),
    );
    obj.insert(
        "productVersion".to_string(),
        serde_json::json!(app.product_version),
    );

    if let Some(api_server) = api_server.filter(|s| !s.is_empty()) {
        obj.insert("apiServer".to_string(), serde_json::json!(api_server));
    }
    if let Some(lingxia_id) = app.lingxia_id.as_deref().filter(|s| !s.is_empty()) {
        obj.insert("lingxiaId".to_string(), serde_json::json!(lingxia_id));
    }

    if let Some(home_bundle) = home_bundle
        && let Some(home_lxapp_id) = app.home_lxapp_id.as_deref().filter(|s| !s.is_empty())
    {
        obj.insert("homeLxAppID".to_string(), serde_json::json!(home_lxapp_id));
        obj.insert(
            "homeLxAppVersion".to_string(),
            serde_json::json!(home_bundle.version.as_str()),
        );
    }
    if let Some(max_age) = app.cache_max_age_days {
        obj.insert("cacheMaxAgeDays".to_string(), serde_json::json!(max_age));
    }
    if let Some(max_size_mb) = app.cache_max_size_mb {
        obj.insert("cacheMaxSizeMB".to_string(), serde_json::json!(max_size_mb));
    }

    Ok(serde_json::to_string_pretty(&serde_json::Value::Object(
        obj,
    ))?)
}

fn build_ui_json_from_config(config: &LingXiaConfig) -> Result<Option<String>> {
    let Some(ui) = config.ui.as_ref() else {
        return Ok(None);
    };
    Ok(Some(serde_json::to_string_pretty(ui)?))
}

struct LxAppMetadata {
    app_id: String,
    version: String,
}

fn configured_home_bundle<'a>(
    config: &LingXiaConfig,
    bundles: &'a [PreparedResourceBundle],
) -> Option<&'a PreparedResourceBundle> {
    let home_lxapp_id = config
        .app
        .as_ref()
        .and_then(|app| app.home_lxapp_id.as_deref())
        .map(str::trim)
        .filter(|s| !s.is_empty())?;
    bundles
        .iter()
        .find(|bundle| bundle.asset_name == home_lxapp_id)
}

fn run_npm_bundle_build(bundle_dir: &Path, build_profile: BuildProfile) -> Result<()> {
    let status = Command::new("npm")
        .arg("run")
        .arg("build")
        .env("LINGXIA_BUILD_PROFILE", build_profile.as_str())
        .env(
            "NODE_ENV",
            if matches!(build_profile, BuildProfile::Release) {
                "production"
            } else {
                "development"
            },
        )
        .current_dir(bundle_dir)
        .status()
        .with_context(|| format!("Failed to run npm build in {}", bundle_dir.display()))?;
    if !status.success() {
        return Err(anyhow!(
            "npm run build failed for resource bundle {}",
            bundle_dir.display()
        ));
    }
    Ok(())
}

fn sync_optional_json_file(
    json_path: &Path,
    json_contents: Option<&str>,
    prev_json_hash: Option<&str>,
    label: &str,
) -> Result<bool> {
    if let Some(json_contents) = json_contents {
        if write_if_changed(json_path, json_contents.as_bytes())? {
            println!("  {} {} → {}", "✓".green(), label, json_path.display());
            return Ok(true);
        }
        return Ok(false);
    }

    if prev_json_hash.is_some() && json_path.exists() {
        fs::remove_file(json_path)
            .with_context(|| format!("Failed to remove {}", json_path.display()))?;
        println!(
            "  {} remove stale {} → {}",
            "✓".green(),
            label,
            json_path.display()
        );
        return Ok(true);
    }

    Ok(false)
}

fn bundle_hashes(bundles: &[PreparedResourceBundle]) -> BTreeMap<String, String> {
    bundles
        .iter()
        .map(|bundle| (bundle.asset_name.clone(), bundle.dist_hash.clone()))
        .collect()
}

fn sync_resource_bundles(
    target_root: &Path,
    bundles: &[PreparedResourceBundle],
    prev_hashes: Option<&BTreeMap<String, String>>,
) -> Result<bool> {
    let desired_hashes = bundle_hashes(bundles);
    let mut changed = false;

    if let Some(prev_hashes) = prev_hashes {
        for prev_name in prev_hashes.keys() {
            if !desired_hashes.contains_key(prev_name) {
                let stale_dir = target_root.join(prev_name);
                if stale_dir.exists() {
                    fs::remove_dir_all(&stale_dir)
                        .with_context(|| format!("Failed to remove {}", stale_dir.display()))?;
                    changed = true;
                }
            }
        }
    }

    for bundle in bundles {
        let target_dir = target_root.join(&bundle.asset_name);
        let prev_hash = prev_hashes.and_then(|hashes| hashes.get(&bundle.asset_name));
        if prev_hash == Some(&bundle.dist_hash) && target_dir.exists() {
            continue;
        }
        if target_dir.exists() {
            fs::remove_dir_all(&target_dir)
                .with_context(|| format!("Failed to remove {}", target_dir.display()))?;
        }
        copy_dir_recursive(&bundle.dist_dir, &target_dir)?;
        println!(
            "  {} bundle {} → {}",
            "✓".green(),
            bundle.asset_name,
            target_dir.display()
        );
        changed = true;
    }

    Ok(changed)
}

fn read_lxapp_metadata(path: &Path) -> Result<LxAppMetadata> {
    let content = fs::read_to_string(path)?;
    let value: serde_json::Value = serde_json::from_str(&content)?;
    let app_id = value
        .get("appId")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow!("appId missing in {}", path.display()))?;
    let version = value
        .get("version")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow!("version missing in {}", path.display()))?;
    Version::parse(version).map_err(|_| {
        anyhow!(
            "version in {} must be a semantic version (major.minor.patch)",
            path.display()
        )
    })?;

    Ok(LxAppMetadata {
        app_id: app_id.to_string(),
        version: version.to_string(),
    })
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<()> {
    if !dest.exists() {
        fs::create_dir_all(dest)?;
    }

    let mut entries: Vec<_> = fs::read_dir(src)?.collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let target = dest.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(&path, &target)?;
        } else {
            fs::copy(&path, &target)?;
        }
    }
    Ok(())
}

fn write_if_changed(path: &Path, bytes: &[u8]) -> Result<bool> {
    if let Ok(existing) = fs::read(path)
        && existing == bytes
    {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes)?;
    Ok(true)
}

fn sync_runtime_file(
    runtime_path: &Path,
    runtime_asset: Option<&PreparedRuntimeAsset>,
    prev_runtime_hash: Option<&str>,
) -> Result<bool> {
    if let Some(runtime_asset) = runtime_asset {
        if write_if_changed(runtime_path, &runtime_asset.bytes)? {
            println!(
                "  {} bridge-runtime.js → {}",
                "✓".green(),
                runtime_path.display()
            );
            return Ok(true);
        }
        return Ok(false);
    }

    if prev_runtime_hash.is_some() && runtime_path.exists() {
        fs::remove_file(runtime_path)
            .with_context(|| format!("Failed to remove {}", runtime_path.display()))?;
        return Ok(true);
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::{build_app_json_from_config, build_ui_json_from_config, is_png_path};
    use crate::config::{HostAppConfig, LingXiaConfig};
    use std::path::Path;

    #[test]
    fn png_path_check_accepts_png_case_insensitively() {
        assert!(is_png_path(Path::new("splash.png")));
        assert!(is_png_path(Path::new("SPLASH.PNG")));
        assert!(is_png_path(Path::new("assets/launch.PnG")));
    }

    #[test]
    fn png_path_check_rejects_non_png_extensions() {
        assert!(!is_png_path(Path::new("splash.jpg")));
        assert!(!is_png_path(Path::new("splash.jpeg")));
        assert!(!is_png_path(Path::new("splash.webp")));
        assert!(!is_png_path(Path::new("splash")));
    }

    #[test]
    fn generated_app_json_excludes_ui_fields() {
        let config = LingXiaConfig {
            app: Some(HostAppConfig {
                project_name: "demo".into(),
                product_name: "Demo".into(),
                product_version: "1.2.3".into(),
                api_server: Some("http://127.0.0.1:8080".into()),
                lingxia_id: Some("demo".into()),
                platforms: vec!["macos".into()],
                home_lxapp_id: Some("demo-home".into()),
                cache_max_age_days: Some(7),
                cache_max_size_mb: Some(64),
            }),
            android: None,
            ios: None,
            macos: None,
            harmony: None,
            ui: Some(serde_json::json!({
                "launch": { "initialSurface": "main" },
                "surfaces": [],
                "activators": []
            })),
            resources: None,
        };

        let app_json = build_app_json_from_config(&config, None).unwrap();
        let value: serde_json::Value = serde_json::from_str(&app_json).unwrap();

        assert!(value.get("ui").is_none());
        assert!(value.get("panels").is_none());
        assert!(value.get("splashTimeout").is_none());
    }

    #[test]
    fn generated_ui_json_matches_ui_section() {
        let ui = serde_json::json!({
            "launch": { "initialSurface": "main" },
            "surfaces": [{
                "id": "main",
                "presentation": { "style": "window" },
                "content": { "kind": "lxapp", "appId": "demo-home" }
            }],
            "activators": []
        });
        let config = LingXiaConfig {
            app: None,
            android: None,
            ios: None,
            macos: None,
            harmony: None,
            ui: Some(ui.clone()),
            resources: None,
        };

        let ui_json = build_ui_json_from_config(&config).unwrap().unwrap();
        let value: serde_json::Value = serde_json::from_str(&ui_json).unwrap();
        assert_eq!(value, ui);
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(bytes);
    hex_lower(&hasher.finalize())
}

fn hash_tree(root: &Path, ignore_dir_names: &[&str]) -> Result<String> {
    let mut hasher = sha2::Sha256::new();
    hash_tree_inner(root, root, &mut hasher, ignore_dir_names)?;
    Ok(hex_lower(&hasher.finalize()))
}

fn hash_tree_inner(
    root: &Path,
    current: &Path,
    hasher: &mut sha2::Sha256,
    ignore_dir_names: &[&str],
) -> Result<()> {
    let mut entries: Vec<_> = fs::read_dir(current)
        .with_context(|| format!("Failed to read {}", current.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        let file_name_str: &str = file_name.as_ref();
        if path.is_dir() {
            if ignore_dir_names.contains(&file_name_str) {
                continue;
            }
            hash_tree_inner(root, &path, hasher, ignore_dir_names)?;
        } else if path.is_file() {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");

            hasher.update(rel.as_bytes());
            hasher.update([0]);

            let data =
                fs::read(&path).with_context(|| format!("Failed to read {}", path.display()))?;
            hasher.update((data.len() as u64).to_le_bytes());
            hasher.update([0]);
            hasher.update(&data);
            hasher.update([0]);
        }
    }

    Ok(())
}

fn cache_path(project_root: &Path) -> PathBuf {
    project_root
        .join(".lingxia")
        .join("host-assets")
        .join("cache.json")
}

fn path_key(path: &Path) -> String {
    match path.canonicalize() {
        Ok(p) => p.to_string_lossy().to_string(),
        Err(_) => path.to_string_lossy().to_string(),
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(out, "{:02x}", b);
    }
    out
}
