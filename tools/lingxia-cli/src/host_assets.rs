use crate::config::{HOST_CONFIG_FILE, LXAPP_BUILD_CONFIG_FILE, LingXiaConfig};
use crate::lxapp;
use crate::platform::{self, BuildProfile};
use crate::runtime;
use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

const CACHE_VERSION: u32 = 1;

pub(crate) fn prepare_host_assets(
    project_root: &Path,
    config: &LingXiaConfig,
    build_profile: BuildProfile,
    platforms: &[platform::detector::PlatformType],
    build_targets: &[String],
    _explicit_platforms: bool,
) -> Result<()> {
    let mut cache = HostAssetsCache::load(project_root);
    let app_json = build_app_json_from_config(config)?;
    let app_json_hash = sha256_hex(app_json.as_bytes());
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

    let prepared_lxapp_assets = if needs_embedded_lxapp {
        prepare_embedded_lxapp_assets(project_root, config, build_profile, &mut cache)?
    } else {
        None
    };
    let has_android = platforms
        .iter()
        .any(|p| matches!(p, platform::detector::PlatformType::Android));
    let has_non_android = platforms
        .iter()
        .any(|p| !matches!(p, platform::detector::PlatformType::Android));
    let needs_es5_runtime = has_android
        && runtime::target_from_build_targets(build_targets) == runtime::RuntimeEcmaTarget::Es5;

    // Only Android can require ES5 runtime (armv7). Other platforms should stay on ES2020.
    let prepared_runtime_es5 = if needs_embedded_lxapp && needs_es5_runtime {
        Some(prepare_runtime_asset(
            project_root,
            config,
            runtime::RuntimeEcmaTarget::Es5,
        )?)
    } else {
        None
    };
    let prepared_runtime_es2020 = if needs_embedded_lxapp && (has_non_android || !needs_es5_runtime)
    {
        Some(prepare_runtime_asset(
            project_root,
            config,
            runtime::RuntimeEcmaTarget::Es2020,
        )?)
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
                    prepared_lxapp_assets.as_ref(),
                    prepared_runtime_es5
                        .as_ref()
                        .or(prepared_runtime_es2020.as_ref()),
                    &mut cache,
                )?;
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
                    prepared_lxapp_assets.as_ref(),
                    prepared_runtime_es2020.as_ref(),
                    &mut prepared_resource_roots,
                    &mut cache,
                )?;
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
                    prepared_lxapp_assets.as_ref(),
                    prepared_runtime_es2020.as_ref(),
                    &mut prepared_resource_roots,
                    &mut cache,
                )?;
            }
            platform::detector::PlatformType::Harmony => {
                let rawfile_root =
                    crate::platform::harmony::resolve_harmony_rawfile_dir(project_root)?;
                prepare_harmony_rawfile_root(
                    &rawfile_root,
                    &app_json,
                    &app_json_hash,
                    prepared_lxapp_assets.as_ref(),
                    prepared_runtime_es2020.as_ref(),
                    &mut cache,
                )?;
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
    lxapp_assets: Option<&PreparedLxAppAssets>,
    runtime_asset: Option<&PreparedRuntimeAsset>,
    cache: &mut HostAssetsCache,
) -> Result<()> {
    fs::create_dir_all(assets_root)?;

    let dest_key = path_key(assets_root);
    let prev = cache.destinations.get(&dest_key).cloned();

    if let Some(prev) = &prev
        && let (Some(prev_name), Some(next)) = (
            prev.asset_name.as_deref(),
            lxapp_assets.map(|a| a.asset_name.as_str()),
        )
        && prev_name != next
    {
        let old_dir = assets_root.join(prev_name);
        if old_dir.exists() {
            fs::remove_dir_all(&old_dir)
                .with_context(|| format!("Failed to remove {}", old_dir.display()))?;
        }
    }

    let desired = DestinationStamp {
        app_json_hash: app_json_hash.to_string(),
        dist_hash: lxapp_assets.map(|a| a.dist_hash.clone()),
        asset_name: lxapp_assets.map(|a| a.asset_name.clone()),
        runtime_hash: runtime_asset.map(|r| r.runtime_hash.clone()),
    };

    let mut changed = false;

    // app.json
    let app_json_path = assets_root.join("app.json");
    if write_if_changed(&app_json_path, app_json.as_bytes())? {
        changed = true;
        println!("  {} app.json → {}", "✓".green(), app_json_path.display());
    }
    changed |= sync_runtime_file(
        &assets_root.join("runtime.js"),
        runtime_asset,
        prev.as_ref().and_then(|s| s.runtime_hash.as_deref()),
    )?;

    // LxApp dist
    if let Some(lxapp_assets) = lxapp_assets {
        let target_dir = assets_root.join(&lxapp_assets.asset_name);
        if prev.as_ref() != Some(&desired) || !target_dir.exists() {
            if target_dir.exists() {
                fs::remove_dir_all(&target_dir)
                    .with_context(|| format!("Failed to remove {}", target_dir.display()))?;
            }
            copy_dir_recursive(&lxapp_assets.dist_dir, &target_dir)?;
            println!("  {} LxApp assets → {}", "✓".green(), target_dir.display());
            changed = true;
        }
    } else if let Some(prev) = &prev {
        // LxApp was previously embedded but is now unavailable; remove stale directory.
        if let Some(prev_name) = &prev.asset_name {
            let stale = assets_root.join(prev_name);
            if stale.exists() {
                fs::remove_dir_all(&stale)
                    .with_context(|| format!("Failed to remove {}", stale.display()))?;
                changed = true;
            }
        }
    }

    if changed {
        cache.destinations.insert(dest_key, desired);
    }

    Ok(())
}

fn prepare_apple_resources_root(
    resources_dir: &Path,
    app_json: &str,
    app_json_hash: &str,
    lxapp_assets: Option<&PreparedLxAppAssets>,
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
        dist_hash: lxapp_assets.map(|a| a.dist_hash.clone()),
        asset_name: lxapp_assets.map(|_| "homelxapp".to_string()),
        runtime_hash: runtime_asset.map(|r| r.runtime_hash.clone()),
    };

    let mut changed = false;

    let app_json_path = resources_dir.join("app.json");
    if write_if_changed(&app_json_path, app_json.as_bytes())? {
        changed = true;
        println!("  {} app.json → {}", "✓".green(), app_json_path.display());
    }
    changed |= sync_runtime_file(
        &resources_dir.join("runtime.js"),
        runtime_asset,
        prev.as_ref().and_then(|s| s.runtime_hash.as_deref()),
    )?;

    if let Some(lxapp_assets) = lxapp_assets {
        let target_dir = resources_dir.join("homelxapp");
        if prev.as_ref() != Some(&desired) || !target_dir.exists() {
            if target_dir.exists() {
                fs::remove_dir_all(&target_dir)?;
            }
            copy_dir_recursive(&lxapp_assets.dist_dir, &target_dir)?;
            println!("  {} LxApp assets → {}", "✓".green(), target_dir.display());
            changed = true;
        }
    } else if let Some(prev) = &prev
        && prev.dist_hash.is_some()
    {
        let stale = resources_dir.join("homelxapp");
        if stale.exists() {
            fs::remove_dir_all(&stale)?;
            changed = true;
        }
    }

    if changed {
        cache.destinations.insert(dest_key, desired);
    }

    Ok(())
}

fn prepare_harmony_rawfile_root(
    rawfile_root: &Path,
    app_json: &str,
    app_json_hash: &str,
    lxapp_assets: Option<&PreparedLxAppAssets>,
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

    // Clean up old LxApp directory if asset name changed
    if let Some(prev) = &prev
        && let (Some(prev_name), Some(next)) = (
            prev.asset_name.as_deref(),
            lxapp_assets.map(|a| a.asset_name.as_str()),
        )
        && prev_name != next
    {
        let old_dir = rawfile_root.join(prev_name);
        if old_dir.exists() {
            fs::remove_dir_all(&old_dir)
                .with_context(|| format!("Failed to remove {}", old_dir.display()))?;
        }
    }

    let desired = DestinationStamp {
        app_json_hash: app_json_hash.to_string(),
        dist_hash: lxapp_assets.map(|a| a.dist_hash.clone()),
        asset_name: lxapp_assets.map(|a| a.asset_name.clone()),
        runtime_hash: runtime_asset.map(|r| r.runtime_hash.clone()),
    };

    let mut changed = false;

    // app.json
    let app_json_path = rawfile_root.join("app.json");
    if write_if_changed(&app_json_path, app_json.as_bytes())? {
        changed = true;
        println!("  {} app.json → {}", "✓".green(), app_json_path.display());
    }
    changed |= sync_runtime_file(
        &rawfile_root.join("runtime.js"),
        runtime_asset,
        prev.as_ref().and_then(|s| s.runtime_hash.as_deref()),
    )?;

    // LxApp dist
    if let Some(lxapp_assets) = lxapp_assets {
        let target_dir = rawfile_root.join(&lxapp_assets.asset_name);
        if prev.as_ref() != Some(&desired) || !target_dir.exists() {
            if target_dir.exists() {
                fs::remove_dir_all(&target_dir)
                    .with_context(|| format!("Failed to remove {}", target_dir.display()))?;
            }
            copy_dir_recursive(&lxapp_assets.dist_dir, &target_dir)?;
            println!("  {} LxApp assets → {}", "✓".green(), target_dir.display());
            changed = true;
        }
    } else if let Some(prev) = &prev {
        // LxApp was previously embedded but is now unavailable; remove stale directory.
        if let Some(prev_name) = &prev.asset_name {
            let stale = rawfile_root.join(prev_name);
            if stale.exists() {
                fs::remove_dir_all(&stale)
                    .with_context(|| format!("Failed to remove {}", stale.display()))?;
                changed = true;
            }
        }
    }

    if changed {
        cache.destinations.insert(dest_key, desired);
    }

    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct DestinationStamp {
    app_json_hash: String,
    dist_hash: Option<String>,
    asset_name: Option<String>,
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

struct PreparedLxAppAssets {
    dist_dir: PathBuf,
    asset_name: String,
    dist_hash: String,
}

struct PreparedRuntimeAsset {
    bytes: Vec<u8>,
    runtime_hash: String,
}

fn prepare_runtime_asset(
    project_root: &Path,
    config: &LingXiaConfig,
    target: runtime::RuntimeEcmaTarget,
) -> Result<PreparedRuntimeAsset> {
    let resolved = runtime::resolve_runtime_js(project_root, config, target)
        .with_context(|| format!("Failed to resolve runtime.js ({})", target.as_str()))?;
    let bytes = fs::read(&resolved.path)
        .with_context(|| format!("Failed to read runtime file: {}", resolved.path.display()))?;

    println!(
        "  {} runtime.js ({}) ← {}",
        "✓".green(),
        target.as_str(),
        resolved.source
    );

    Ok(PreparedRuntimeAsset {
        bytes,
        runtime_hash: resolved.hash,
    })
}

fn prepare_embedded_lxapp_assets(
    project_root: &Path,
    config: &LingXiaConfig,
    build_profile: BuildProfile,
    cache: &mut HostAssetsCache,
) -> Result<Option<PreparedLxAppAssets>> {
    let Some(app) = &config.app else {
        return Ok(None);
    };

    let lxapp_dir = project_root.join(&app.home_lxapp_id);
    if !lxapp_dir.exists() {
        return Ok(None);
    }

    let lxapp_json = lxapp_dir.join("lxapp.json");
    let lxapp_build_config = lxapp_dir.join(LXAPP_BUILD_CONFIG_FILE);
    if !lxapp_json.exists() || !lxapp_build_config.exists() {
        return Err(anyhow!(
            "LxApp project must include lxapp.json and {} in {}",
            LXAPP_BUILD_CONFIG_FILE,
            lxapp_dir.display()
        ));
    }

    println!("{}", "Preparing LxApp...".bold());

    let mut args = vec!["build".to_string()];
    if matches!(build_profile, BuildProfile::Release) {
        args.push("--release".to_string());
    }
    let cache_key = format!("{}|{}", path_key(&lxapp_dir), build_profile.as_str(),);

    let inputs_hash = hash_tree(&lxapp_dir, &["dist", "node_modules", ".git", ".lingxia"])?;
    let mut needs_build = true;

    if let Some(stamp) = cache.lxapp_builds.get(&cache_key) {
        let dist_dir = lxapp_dir.join("dist");
        if stamp.inputs_hash == inputs_hash && dist_dir.exists() {
            needs_build = false;
        }
    }

    if needs_build {
        println!("  {}", "Building LxApp...".cyan());
        lxapp::run_in_dir(&args, &lxapp_dir)?;
    } else {
        println!("  {} LxApp unchanged, skip build", "✓".green());
    }

    let dist_dir = lxapp_dir.join("dist");
    if !dist_dir.exists() {
        return Err(anyhow!(
            "LxApp build output not found: {}",
            dist_dir.display()
        ));
    }

    let asset_name = resolve_lxapp_id(&lxapp_json).unwrap_or_else(|_| app.home_lxapp_id.clone());
    let dist_hash = hash_tree(&dist_dir, &[])?;

    cache.lxapp_builds.insert(
        cache_key,
        LxAppBuildStamp {
            inputs_hash,
            dist_hash: dist_hash.clone(),
            asset_name: asset_name.clone(),
        },
    );

    Ok(Some(PreparedLxAppAssets {
        dist_dir,
        asset_name,
        dist_hash,
    }))
}

fn build_app_json_from_config(config: &LingXiaConfig) -> Result<String> {
    let app = config
        .app
        .as_ref()
        .ok_or_else(|| anyhow!("Missing app settings in {}", HOST_CONFIG_FILE))?;

    // Read apiKey/apiSecret from environment variables (CI-friendly)
    let api_key = std::env::var("LINGXIA_API_KEY")
        .ok()
        .filter(|s| !s.trim().is_empty());
    let api_secret = std::env::var("LINGXIA_API_SECRET")
        .ok()
        .filter(|s| !s.trim().is_empty());

    let mut obj = serde_json::Map::new();
    obj.insert(
        "productName".to_string(),
        serde_json::json!(app.product_name),
    );
    obj.insert(
        "productVersion".to_string(),
        serde_json::json!(app.product_version),
    );

    if let Some(api_server) = app.api_server.as_ref().filter(|s| !s.trim().is_empty()) {
        obj.insert("apiServer".to_string(), serde_json::json!(api_server));
    }
    if let Some(api_key) = api_key {
        obj.insert("apiKey".to_string(), serde_json::json!(api_key));
    }
    if let Some(api_secret) = api_secret {
        obj.insert("apiSecret".to_string(), serde_json::json!(api_secret));
    }

    obj.insert(
        "homeLxAppID".to_string(),
        serde_json::json!(app.home_lxapp_id),
    );
    obj.insert(
        "homeLxAppVersion".to_string(),
        serde_json::json!(app.home_lxapp_version),
    );
    if let Some(max_age) = app.cache_max_age_days {
        obj.insert("cacheMaxAgeDays".to_string(), serde_json::json!(max_age));
    }

    Ok(serde_json::to_string_pretty(&serde_json::Value::Object(
        obj,
    ))?)
}

fn resolve_lxapp_id(path: &Path) -> Result<String> {
    let content = fs::read_to_string(path)?;
    let value: serde_json::Value = serde_json::from_str(&content)?;
    let app_id = value
        .get("appId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("appId missing in lxapp.json"))?;
    Ok(app_id.to_string())
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
            println!("  {} runtime.js → {}", "✓".green(), runtime_path.display());
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
