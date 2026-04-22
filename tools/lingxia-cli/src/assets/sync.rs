use super::runtime_asset::PreparedRuntimeAsset;
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
