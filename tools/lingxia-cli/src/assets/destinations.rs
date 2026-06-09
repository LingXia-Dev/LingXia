use super::bundles::{PreparedResourceBundle, bundle_hashes, sync_resource_bundles};
use super::cache::{DestinationStamp, HostAssetsCache};
use super::hash::path_key;
use super::icons::{PreparedAppUiIcon, app_ui_icon_hashes, sync_app_ui_icons};
use super::runtime_asset::{PreparedPolyfillsAsset, PreparedRuntimeAsset};
use super::sync::{
    sync_optional_json_file, sync_polyfills_file, sync_runtime_file, write_if_changed,
};
use anyhow::Result;
use colored::Colorize;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn prepare_android_assets_root(
    assets_root: &Path,
    app_json: &str,
    app_json_hash: &str,
    ui_json: Option<&str>,
    ui_json_hash: Option<&str>,
    bundles: &[PreparedResourceBundle],
    runtime_asset: Option<&PreparedRuntimeAsset>,
    polyfills_asset: Option<&PreparedPolyfillsAsset>,
    cache: &mut HostAssetsCache,
) -> Result<()> {
    fs::create_dir_all(assets_root)?;

    let dest_key = path_key(assets_root);
    let prev = cache.destinations.get(&dest_key).cloned();

    let desired = DestinationStamp {
        app_json_hash: app_json_hash.to_string(),
        ui_json_hash: ui_json_hash.map(ToOwned::to_owned),
        bundle_hashes: bundle_hashes(bundles),
        app_ui_icon_hashes: BTreeMap::new(),
        runtime_hash: runtime_asset.map(|r| r.runtime_hash.clone()),
        polyfills_hash: polyfills_asset.map(|p| p.hash.clone()),
    };

    let mut changed = false;

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
    changed |= sync_polyfills_file(
        &assets_root.join("polyfills.es5.js"),
        polyfills_asset,
        prev.as_ref().and_then(|s| s.polyfills_hash.as_deref()),
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

pub(super) fn prepare_apple_resources_root(
    resources_dir: &Path,
    app_json: &str,
    app_json_hash: &str,
    ui_json: Option<&str>,
    ui_json_hash: Option<&str>,
    bundles: &[PreparedResourceBundle],
    app_ui_icons: &[PreparedAppUiIcon],
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
        app_ui_icon_hashes: app_ui_icon_hashes(app_ui_icons),
        runtime_hash: runtime_asset.map(|r| r.runtime_hash.clone()),
        polyfills_hash: None,
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
    changed |= sync_app_ui_icons(
        &resources_dir,
        app_ui_icons,
        prev.as_ref().map(|s| &s.app_ui_icon_hashes),
    )?;

    if changed {
        cache.destinations.insert(dest_key, desired);
    }

    Ok(())
}

pub(super) fn prepare_harmony_rawfile_root(
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
        app_ui_icon_hashes: BTreeMap::new(),
        runtime_hash: runtime_asset.map(|r| r.runtime_hash.clone()),
        polyfills_hash: None,
    };

    let mut changed = false;

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

pub(super) fn prepare_windows_assets_root(
    assets_root: &Path,
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
        format!("Preparing Windows assets -> {}", assets_root.display()).cyan()
    );

    fs::create_dir_all(assets_root)?;

    let dest_key = path_key(assets_root);
    let prev = cache.destinations.get(&dest_key).cloned();

    let desired = DestinationStamp {
        app_json_hash: app_json_hash.to_string(),
        ui_json_hash: ui_json_hash.map(ToOwned::to_owned),
        bundle_hashes: bundle_hashes(bundles),
        app_ui_icon_hashes: BTreeMap::new(),
        runtime_hash: runtime_asset.map(|r| r.runtime_hash.clone()),
        polyfills_hash: None,
    };

    let mut changed = false;

    let app_json_path = assets_root.join("app.json");
    if write_if_changed(&app_json_path, app_json.as_bytes())? {
        changed = true;
        println!("  {} app.json -> {}", "ok".green(), app_json_path.display());
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
