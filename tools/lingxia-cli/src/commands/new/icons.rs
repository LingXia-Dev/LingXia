use super::android;
use super::locate_templates_dir;
use super::types::{DEFAULT_ICON_BACKGROUND_COLOR, Platform, ProjectConfig};
use crate::appicon;
use crate::path_completion::FilePathCompleter;
use anyhow::Result;
use colored::Colorize;
use dialoguer::{Confirm, Input, theme::ColorfulTheme};
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

    // Determine icon configuration
    let icon_config = match (icon, yes) {
        // User provided explicit icon path
        (Some(path), _) => Some((path, DEFAULT_ICON_BACKGROUND_COLOR.to_string())),
        // Auto mode (-y): use default icon if it exists
        (None, true) => project_icon_path.exists().then(|| {
            (
                project_icon_path.to_string_lossy().to_string(),
                DEFAULT_ICON_BACKGROUND_COLOR.to_string(),
            )
        }),
        // Interactive mode: ask user, default to using the bundled icon
        (None, false) => {
            println!();
            let configure_icon = Confirm::with_theme(theme)
                .with_prompt("Do you want to configure an app icon?")
                .default(true)
                .interact()?;

            if !configure_icon {
                None
            } else {
                let default_path = if project_icon_path.exists() {
                    "./AppIcon.png".to_string()
                } else {
                    String::new()
                };
                let path: String = Input::with_theme(theme)
                    .with_prompt("Path to app icon (PNG, recommended 1024x1024)")
                    .with_initial_text(default_path)
                    .completion_with(&FilePathCompleter::new())
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

                Some((path, appicon::normalize_android_color(&background_color)?))
            }
        }
    };

    // Generate app icons if icon path is provided, otherwise remove Android icon references.
    if let Some((icon_path, background_color)) = icon_config {
        generate_app_icons(config, &icon_path, &background_color)?;
    } else if config.platforms.contains(&Platform::Android) {
        android::remove_android_icon_references(&config.target_dir)?;
    }

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
        eprintln!(
            "{} Icon file not found: {}",
            "Warning:".yellow(),
            icon_path.display()
        );
        eprintln!("Skipping icon generation.");
        return Ok(());
    }

    println!("  Generating app icons...");

    for platform in &config.platforms {
        match platform {
            Platform::Android => {
                let res_dir = config.target_dir.join("android/app/src/main/res");
                if !res_dir.exists() {
                    eprintln!(
                        "{} Android res directory not found: {}",
                        "Warning:".yellow(),
                        res_dir.display()
                    );
                    eprintln!("Skipping Android icon generation.");
                    continue;
                }
                appicon::generate_android_icons(&icon_path, &res_dir, background_color)?;
            }
            Platform::Ios => {
                eprintln!(
                    "{} iOS icon generation not yet implemented",
                    "Warning:".yellow()
                );
            }
            Platform::Harmony => {
                eprintln!(
                    "{} HarmonyOS icon generation not yet implemented",
                    "Warning:".yellow()
                );
            }
        }
    }

    Ok(())
}
