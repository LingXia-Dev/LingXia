use crate::appicon;
use crate::config::LingXiaConfig;
use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use std::path::PathBuf;

const DEFAULT_ICON_BACKGROUND_COLOR: &str = "#FFFFFF";

/// Execute the icon command to generate or update app icons
pub fn execute(
    icon_path: String,
    platform: Option<String>,
    background_color: Option<String>,
) -> Result<()> {
    println!("{}", "Generate/Update App Icons".bold());
    println!();

    // Check if icon file exists
    let icon_path = PathBuf::from(&icon_path);
    if !icon_path.exists() {
        return Err(anyhow!("Icon file not found: {:?}", icon_path));
    }

    // Load project configuration
    let current_dir = std::env::current_dir()?;
    let config = LingXiaConfig::load(&current_dir)
        .context("Failed to load lingxia.config.json. Are you in a LingXia project directory?")?;

    // Determine target platform(s)
    let platforms: Vec<String> = if let Some(p) = platform {
        vec![p.to_lowercase()]
    } else {
        config
            .app
            .as_ref()
            .map(|a| a.platforms.clone())
            .ok_or_else(|| anyhow!("Missing app section in lingxia.config.json"))?
    };

    if platforms.is_empty() {
        return Err(anyhow!(
            "No platforms specified. Please specify a platform using --platform or configure app.platforms in lingxia.config.json"
        ));
    }

    let bg_color = appicon::normalize_android_color(
        background_color
            .as_deref()
            .unwrap_or(DEFAULT_ICON_BACKGROUND_COLOR),
    )?;

    println!(
        "  Icon source:      {}",
        icon_path.display().to_string().cyan()
    );
    println!("  Target platform:  {}", platforms.join(", ").cyan());
    println!("  Background color: {}", bg_color.cyan());
    println!();

    let mut generated_count = 0;

    for platform in platforms {
        match platform.as_str() {
            "android" => {
                println!("{}", "Generating Android icons...".bold());
                let android_res = current_dir.join("android/app/src/main/res");

                if !android_res.exists() {
                    eprintln!(
                        "  {} Android res directory not found: {:?}",
                        "Warning:".yellow(),
                        android_res
                    );
                    eprintln!("  Skipping Android icon generation.");
                    continue;
                }

                appicon::generate_android_icons(&icon_path, &android_res, &bg_color)?;
                generated_count += 1;
            }
            "ios" => {
                println!("{}", "Generating iOS icons...".bold());
                eprintln!(
                    "  {} iOS icon generation not yet implemented",
                    "Warning:".yellow()
                );
            }
            "harmony" | "harmonyos" => {
                println!("{}", "Generating HarmonyOS icons...".bold());
                eprintln!(
                    "  {} HarmonyOS icon generation not yet implemented",
                    "Warning:".yellow()
                );
            }
            _ => {
                eprintln!("  {} Unknown platform: {}", "Warning:".yellow(), platform);
            }
        }
    }

    if generated_count > 0 {
        println!();
        println!("{}", "Icons generated successfully!".green().bold());
    } else {
        println!();
        println!("{}", "No icons were generated.".yellow());
    }

    Ok(())
}
