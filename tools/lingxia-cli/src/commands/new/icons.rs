use super::locate_templates_dir;
use super::types::{DEFAULT_ICON_BACKGROUND_COLOR, Platform, ProjectConfig};
use crate::appicon;
use crate::path_completion::FilePathCompleter;
use anyhow::Result;
use anyhow::anyhow;
use colored::Colorize;
use dialoguer::{Input, theme::ColorfulTheme};
use std::fs;
use std::path::Path;

pub fn configure_and_apply_icons(
    config: &ProjectConfig,
    icon: Option<String>,
    yes: bool,
    theme: &ColorfulTheme,
) -> Result<()> {
    let templates_base = locate_templates_dir()?;
    let default_icon_path = templates_base.join("AppIcon.png");
    let project_icon_path = config.target_dir.join("AppIcon.png");

    // Copy default AppIcon.png from templates to project root (do not overwrite).
    if default_icon_path.exists() && !project_icon_path.exists() {
        fs::copy(&default_icon_path, &project_icon_path)?;
    }

    // Determine icon configuration (icon is required, explicit or default).
    let (icon_path, background_color) = match (icon, yes) {
        // User provided explicit icon path
        (Some(path), _) => {
            let icon_path = path.trim().to_string();
            if icon_path.is_empty() {
                return Err(anyhow!("Icon path cannot be empty"));
            }
            if !std::path::Path::new(&icon_path).exists() {
                return Err(anyhow!("Icon file not found: {}", icon_path));
            }
            (icon_path, DEFAULT_ICON_BACKGROUND_COLOR.to_string())
        }
        // Auto mode (-y): use default icon if it exists
        (None, true) => {
            if !project_icon_path.exists() {
                return Err(anyhow!(
                    "Default AppIcon.png not found. Please pass --icon <path-to-png>."
                ));
            }
            (
                project_icon_path.to_string_lossy().to_string(),
                DEFAULT_ICON_BACKGROUND_COLOR.to_string(),
            )
        }
        // Interactive mode: ask for icon path, default to the bundled icon.
        (None, false) => {
            println!();
            let default_path = if project_icon_path.exists() {
                project_icon_path.to_string_lossy().to_string()
            } else {
                String::new()
            };
            let path: String = Input::with_theme(theme)
                .with_prompt("Path to app icon (PNG, recommended 1024x1024)")
                .with_initial_text(default_path)
                .completion_with(&FilePathCompleter::new())
                .validate_with(|input: &String| -> Result<(), String> {
                    let p = input.trim();
                    if p.is_empty() {
                        return Err("Icon path cannot be empty".to_string());
                    }
                    if !std::path::Path::new(p).exists() {
                        return Err(format!("Icon file not found: {p}"));
                    }
                    Ok(())
                })
                .interact_text()?;
            let background_color: String = Input::with_theme(theme)
                .with_prompt("Adaptive icon background color (e.g. #FFFFFF)")
                .default(DEFAULT_ICON_BACKGROUND_COLOR.to_string())
                .validate_with(|input: &String| -> Result<(), String> {
                    appicon::normalize_android_color(input)
                        .map(|_| ())
                        .map_err(|e| e.to_string())
                })
                .interact_text()?;

            (
                path.trim().to_string(),
                appicon::normalize_android_color(&background_color)?,
            )
        }
    };

    generate_app_icons(config, &icon_path, &background_color)?;

    Ok(())
}

pub fn ensure_lxapp_public_icon(target_dir: &Path) -> Result<()> {
    let public_dir = target_dir.join("public");
    fs::create_dir_all(&public_dir)?;
    let icon_dest = public_dir.join("AppIcon.png");
    if icon_dest.exists() {
        return Ok(());
    }

    // First try to copy from parent project root (for nested lxapp in native app)
    let parent_icon = target_dir.parent().and_then(|p| {
        let icon = p.join("AppIcon.png");
        icon.exists().then_some(icon)
    });

    if let Some(parent_icon) = parent_icon {
        fs::copy(&parent_icon, &icon_dest)?;
        return Ok(());
    }

    // Fall back to templates directory (for standalone lxapp)
    let templates_base = locate_templates_dir()?;
    let template_icon = templates_base.join("AppIcon.png");
    if template_icon.exists() {
        fs::copy(&template_icon, &icon_dest)?;
    }
    Ok(())
}

fn generate_app_icons(
    config: &ProjectConfig,
    icon_path: &str,
    background_color: &str,
) -> Result<()> {
    use std::path::PathBuf;

    let icon_path = PathBuf::from(icon_path);
    if !icon_path.exists() {
        return Err(anyhow!("Icon file not found: {}", icon_path.display()));
    }

    println!("  Generating app icons...");

    for platform in &config.platforms {
        match platform {
            Platform::Android => {
                // Default: no legacy Android icons.
                if let Err(e) = crate::platform::android::generate_icons(
                    &config.target_dir,
                    &icon_path,
                    background_color,
                    false,
                ) {
                    eprintln!("{} {}", "Warning:".yellow(), e);
                    eprintln!("Skipping Android icon generation.");
                }
            }
            Platform::Ios => {
                if let Err(e) = crate::platform::ios::generate_icons(
                    &config.target_dir,
                    &icon_path,
                    None,
                    Some(config.name.as_str()),
                ) {
                    eprintln!("{} {}", "Warning:".yellow(), e);
                    eprintln!("Skipping iOS icon generation.");
                }
            }
            Platform::Macos => {
                if let Err(e) = crate::platform::macos::generate_icons(
                    &config.target_dir,
                    &icon_path,
                    None,
                    Some(config.name.as_str()),
                ) {
                    eprintln!("{} {}", "Warning:".yellow(), e);
                    eprintln!("Skipping macOS icon generation.");
                }
            }
            Platform::Harmony => {
                if let Err(e) = crate::platform::harmony::generate_icons(
                    &config.target_dir,
                    &icon_path,
                    background_color,
                    None,
                ) {
                    eprintln!("{} {}", "Warning:".yellow(), e);
                    eprintln!("Skipping HarmonyOS icon generation.");
                }
            }
        }
    }

    Ok(())
}
