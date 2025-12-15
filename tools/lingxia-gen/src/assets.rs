use anyhow::{Context, Result};
use clap::Args;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Args, Debug)]
pub struct AssetsConfig {
    /// Path to the source assets directory
    #[arg(short, long, default_value = "lingxia-sdk/resources/assets")]
    pub input: PathBuf,

    /// Path to output Android assets (src/main/assets/lingxia-core)
    #[arg(long)]
    pub android_out: Option<PathBuf>,

    /// Path to output iOS assets (Resources/lingxia-core)
    #[arg(long)]
    pub ios_out: Option<PathBuf>,

    /// Path to output HarmonyOS assets (resources/rawfile/lingxia-core)
    #[arg(long)]
    pub harmony_out: Option<PathBuf>,
}

pub fn run(config: AssetsConfig) -> Result<()> {
    println!("Syncing assets from: {:?}", config.input);

    if !config.input.exists() {
        println!(
            "Warning: Input directory does not exist: {:?}",
            config.input
        );
        return Ok(());
    }

    if let Some(path) = &config.android_out {
        sync_dir(&config.input, path)?;
        println!("Synced Android assets to: {:?}", path);
    }

    if let Some(path) = &config.ios_out {
        sync_dir(&config.input, path)?;
        println!("Synced iOS assets to: {:?}", path);
    }

    if let Some(path) = &config.harmony_out {
        sync_dir(&config.input, path)?;
        println!("Synced HarmonyOS assets to: {:?}", path);
    }

    Ok(())
}

/// Check if a file should be included in the runtime assets
fn should_include_file(path: &Path) -> bool {
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

    // Exclude documentation and metadata files
    if file_name.eq_ignore_ascii_case("README.md")
        || file_name.eq_ignore_ascii_case("LICENSE")
        || file_name.starts_with('.')
    {
        return false;
    }

    // Only include runtime asset files
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("html")
            | Some("js")
            | Some("css")
            | Some("json")
            | Some("svg")
            | Some("png")
            | Some("jpg")
            | Some("jpeg")
    )
}

fn sync_dir(src: &Path, dst: &Path) -> Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst).context("Failed to create destination directory")?;
    }

    for entry in WalkDir::new(src) {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            continue;
        }

        // Skip non-runtime files
        if !should_include_file(path) {
            continue;
        }

        let relative_path = path.strip_prefix(src)?;
        let dest_path = dst.join(relative_path);

        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::copy(path, &dest_path)?;
    }
    Ok(())
}
