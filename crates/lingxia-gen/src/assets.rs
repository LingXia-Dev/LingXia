use anyhow::{Context, Result};
use clap::Args;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Args, Debug)]
pub struct AssetsConfig {
    /// Optional path to additional static assets directory
    #[arg(short, long)]
    pub input: Option<PathBuf>,

    /// Path to runtime assets directory (packages/lingxia-bridge/dist)
    #[arg(long, default_value = "packages/lingxia-bridge/dist")]
    pub runtime_input: PathBuf,

    /// Path to output Android assets (e.g. src/main/assets/lingxia-bridge)
    #[arg(long)]
    pub android_out: Option<PathBuf>,

    /// Path to output Apple assets (e.g. Resources/lingxia-bridge)
    #[arg(long = "apple-out")]
    pub apple_out: Option<PathBuf>,

    /// Path to output HarmonyOS assets (e.g. resources/rawfile/lingxia-bridge)
    #[arg(long)]
    pub harmony_out: Option<PathBuf>,
}

pub fn run(config: AssetsConfig) -> Result<()> {
    let input_exists = config.input.as_ref().is_some_and(|p| p.exists());
    if let Some(input) = &config.input {
        if input_exists {
            println!("Syncing extra assets from: {:?}", input);
        } else {
            println!("Warning: Input directory does not exist: {:?}", input);
        }
    }

    let mut runtime_exists = config.runtime_input.exists();
    if runtime_exists {
        println!("Syncing runtime assets from: {:?}", config.runtime_input);
    } else {
        println!(
            "Warning: Runtime assets directory does not exist: {:?}",
            config.runtime_input
        );
    }

    if input_exists
        && runtime_exists
        && let Some(input) = &config.input
        && let (Ok(input_path), Ok(runtime_path)) =
            (input.canonicalize(), config.runtime_input.canonicalize())
        && input_path == runtime_path
    {
        println!("Runtime assets path matches input; skipping duplicate sync.");
        runtime_exists = false;
    }

    if let Some(path) = &config.android_out {
        if input_exists {
            sync_dir(config.input.as_ref().expect("input exists"), path)?;
        }
        if runtime_exists {
            sync_dir(&config.runtime_input, path)?;
        }
        println!("Synced Android assets to: {:?}", path);
    }

    if let Some(path) = &config.apple_out {
        if input_exists {
            sync_dir(config.input.as_ref().expect("input exists"), path)?;
        }
        if runtime_exists {
            sync_dir(&config.runtime_input, path)?;
        }
        println!("Synced Apple assets to: {:?}", path);
    }

    if let Some(path) = &config.harmony_out {
        if input_exists {
            sync_dir(config.input.as_ref().expect("input exists"), path)?;
        }
        if runtime_exists {
            sync_dir(&config.runtime_input, path)?;
        }
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
