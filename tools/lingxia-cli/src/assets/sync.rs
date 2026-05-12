use super::runtime_asset::{PreparedPolyfillsAsset, PreparedRuntimeAsset};
use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

pub(super) fn write_if_changed(path: &Path, bytes: &[u8]) -> Result<bool> {
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

pub(super) fn sync_optional_json_file(
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

pub(super) fn sync_runtime_file(
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
        println!(
            "  {} remove stale bridge-runtime.js → {}",
            "✓".green(),
            runtime_path.display()
        );
        return Ok(true);
    }

    Ok(false)
}

pub(super) fn sync_polyfills_file(
    polyfills_path: &Path,
    polyfills_asset: Option<&PreparedPolyfillsAsset>,
    prev_polyfills_hash: Option<&str>,
) -> Result<bool> {
    if let Some(polyfills_asset) = polyfills_asset {
        if write_if_changed(polyfills_path, &polyfills_asset.bytes)? {
            println!(
                "  {} polyfills.es5.js → {}",
                "✓".green(),
                polyfills_path.display()
            );
            return Ok(true);
        }
        return Ok(false);
    }

    if prev_polyfills_hash.is_some() && polyfills_path.exists() {
        fs::remove_file(polyfills_path)
            .with_context(|| format!("Failed to remove {}", polyfills_path.display()))?;
        println!(
            "  {} remove stale polyfills.es5.js → {}",
            "✓".green(),
            polyfills_path.display()
        );
        return Ok(true);
    }

    Ok(false)
}
