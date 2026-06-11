use crate::appicon;
use crate::config::{HOST_CONFIG_FILE, LingXiaConfig, has_host_config};
use crate::platform;
use crate::platform::detector::PlatformType;
use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use std::path::PathBuf;

const DEFAULT_ICON_BACKGROUND_COLOR: &str = "#FFFFFF";

/// Execute the icon command to generate or update app icons
pub fn execute(
    icon_path: String,
    platform: Option<String>,
    background_color: Option<String>,
    legacy: bool,
    foreground: Option<String>,
) -> Result<()> {
    println!("{}", "Generate/Update App Icons".bold());
    println!();

    // Check if icon file exists
    let current_dir = std::env::current_dir()?;
    let icon_path = current_dir.join(&icon_path);
    if !icon_path.exists() {
        return Err(anyhow!("Icon file not found: {:?}", icon_path));
    }

    let foreground_path = match &foreground {
        Some(p) => {
            let path = current_dir.join(p);
            if !path.exists() {
                return Err(anyhow!("Foreground icon file not found: {:?}", path));
            }
            Some(path)
        }
        None => None,
    };

    let context = resolve_icon_context(&current_dir)?;

    // Determine target platform(s)
    let platforms: Vec<String> = if let Some(p) = platform {
        vec![p.to_lowercase()]
    } else {
        context
            .config
            .as_ref()
            .and_then(|cfg| cfg.app.as_ref())
            .as_ref()
            .map(|app| app.platforms.clone())
            .or_else(|| context.inferred_platform.as_ref().map(|p| vec![p.as_str().to_string()]))
            .ok_or_else(|| {
                anyhow!(
                    "Failed to determine target platform. Pass --platform or run this command from a LingXia host project or Apple Swift Package directory."
                )
            })?
    };

    if platforms.is_empty() {
        return Err(anyhow!(
            "No platforms specified. Please specify a platform using --platform or configure app.platforms in lingxia.yaml"
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
    if legacy {
        println!("  Legacy support:   {}", "enabled (minSdk < 26)".cyan());
    }
    if let Some(p) = &foreground_path {
        println!("  Foreground:       {}", p.display().to_string().cyan());
    }
    println!();

    let mut generated_count = 0;
    let app_project_name = context
        .config
        .as_ref()
        .and_then(|cfg| cfg.app.as_ref())
        .map(|a| a.project_name.as_str());

    for platform_name in platforms {
        match platform_name.as_str() {
            "android" => {
                println!("{}", "Generating Android icons...".bold());
                match platform::android::generate_icons(
                    &context.project_root,
                    &icon_path,
                    &bg_color,
                    legacy,
                    foreground_path.as_deref(),
                ) {
                    Ok(()) => generated_count += 1,
                    Err(e) => {
                        eprintln!("  {} {}", "Warning:".yellow(), e);
                        eprintln!("  Skipping Android icon generation.");
                    }
                }
            }
            "ios" => {
                println!("{}", "Generating iOS icons...".bold());
                match platform::ios::generate_icons(
                    &context.project_root,
                    &icon_path,
                    context.config.as_ref().and_then(|cfg| cfg.ios.as_ref()),
                    app_project_name,
                ) {
                    Ok(()) => generated_count += 1,
                    Err(e) => {
                        eprintln!("  {} {}", "Warning:".yellow(), e);
                        eprintln!("  Skipping iOS icon generation.");
                    }
                }
            }
            "macos" => {
                println!("{}", "Generating macOS icons...".bold());
                match platform::macos::generate_icons(
                    &context.project_root,
                    &icon_path,
                    context.config.as_ref().and_then(|cfg| cfg.macos.as_ref()),
                    app_project_name,
                ) {
                    Ok(()) => generated_count += 1,
                    Err(e) => {
                        eprintln!("  {} {}", "Warning:".yellow(), e);
                        eprintln!("  Skipping macOS icon generation.");
                    }
                }
            }
            "harmony" | "harmonyos" => {
                println!("{}", "Generating HarmonyOS icons...".bold());
                match platform::harmony::generate_icons(
                    &context.project_root,
                    &icon_path,
                    &bg_color,
                    context.config.as_ref().and_then(|cfg| cfg.harmony.as_ref()),
                    foreground_path.as_deref(),
                ) {
                    Ok(()) => generated_count += 1,
                    Err(e) => {
                        eprintln!("  {} {}", "Warning:".yellow(), e);
                        eprintln!("  Skipping HarmonyOS icon generation.");
                    }
                }
            }
            _ => {
                eprintln!(
                    "  {} Unknown platform: {}",
                    "Warning:".yellow(),
                    platform_name
                );
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

struct IconCommandContext {
    project_root: PathBuf,
    config: Option<LingXiaConfig>,
    inferred_platform: Option<PlatformType>,
}

fn resolve_icon_context(current_dir: &std::path::Path) -> Result<IconCommandContext> {
    if has_host_config(current_dir) {
        let config = LingXiaConfig::load(current_dir).context(format!(
            "Failed to load {}. Are you in a LingXia project directory?",
            HOST_CONFIG_FILE
        ))?;
        return Ok(IconCommandContext {
            project_root: current_dir.to_path_buf(),
            config: Some(config),
            inferred_platform: None,
        });
    }

    if let Some(ctx) =
        platform::spm::find_apple_swift_package_context(current_dir, HOST_CONFIG_FILE)?
    {
        let config = LingXiaConfig::load(&ctx.host_project_root).context(format!(
            "Failed to load {} from the host project for this Apple Swift Package.",
            HOST_CONFIG_FILE
        ))?;
        return Ok(IconCommandContext {
            project_root: ctx.host_project_root,
            config: Some(config),
            inferred_platform: Some(ctx.inferred_platform),
        });
    }

    if let Some(host_root) =
        platform::detector::find_host_project_root(current_dir, HOST_CONFIG_FILE)
        && let Ok(inferred_platform) = platform::detector::detect_platform_type(current_dir)
    {
        let config = LingXiaConfig::load(&host_root).context(format!(
            "Failed to load {} from the detected host project.",
            HOST_CONFIG_FILE
        ))?;
        return Ok(IconCommandContext {
            project_root: host_root,
            config: Some(config),
            inferred_platform: Some(inferred_platform),
        });
    }

    if let Some(inferred_platform) =
        platform::spm::detect_local_apple_swift_package_platform(current_dir)?
    {
        return Ok(IconCommandContext {
            project_root: current_dir.to_path_buf(),
            config: None,
            inferred_platform: Some(inferred_platform),
        });
    }

    Err(anyhow!(
        "Failed to load {}. Are you in a LingXia project directory?\n\
         Supported icon targets without host config: local Apple Swift Packages (ios, macos).",
        HOST_CONFIG_FILE
    ))
}
