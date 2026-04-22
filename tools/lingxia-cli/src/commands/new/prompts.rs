use super::lxapp_scaffold;
use super::types::{AppServiceMode, DEFAULT_PACKAGE_PREFIX, Platform, ProjectConfig, ProjectType};
use super::validation::{validate_package_id, validate_product_name, validate_project_name};
use anyhow::{Result, anyhow};
use dialoguer::{Input, MultiSelect, Select, theme::ColorfulTheme};

pub(super) fn gather_project_name(name: Option<String>) -> Result<String> {
    match name {
        Some(n) => {
            validate_project_name(&n)?;
            Ok(n)
        }
        None => {
            let input: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Project name")
                .validate_with(|input: &String| -> Result<(), String> {
                    validate_project_name(input).map_err(|e| e.to_string())
                })
                .interact_text()?;
            Ok(input)
        }
    }
}

pub(super) fn gather_product_name(project_name: &str, yes: bool) -> Result<String> {
    if yes {
        return Ok(project_name.to_string());
    }

    let input: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Product name")
        .with_initial_text(project_name.to_string())
        .validate_with(|input: &String| -> Result<(), String> {
            validate_product_name(input).map_err(|e| e.to_string())
        })
        .interact_text()?;
    Ok(input.trim().to_string())
}

pub(super) fn gather_project_type(project_type: Option<String>) -> Result<ProjectType> {
    Ok(match project_type.and_then(|t| ProjectType::from_str(&t)) {
        Some(t) => t,
        None => {
            let types = vec!["Native Host App", "LingXia Lightweight App (LxApp)"];
            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Project type")
                .items(&types)
                .default(0)
                .interact()?;

            match selection {
                0 => ProjectType::NativeApp,
                1 => ProjectType::LxApp,
                _ => unreachable!(),
            }
        }
    })
}

pub(super) fn gather_native_project_info(
    name: String,
    product_name: String,
    project_type: ProjectType,
    platforms: Vec<String>,
    package_id: Option<String>,
    yes: bool,
) -> Result<ProjectConfig> {
    let platforms = if !platforms.is_empty() {
        normalize_platforms(platforms)?
    } else if yes {
        vec![
            Platform::Android,
            Platform::Ios,
            Platform::Macos,
            Platform::Harmony,
        ]
    } else {
        println!("Use ↑/↓ to move, Space to select, Enter to confirm.");

        let items = vec![
            "Android",
            "iOS",
            "macOS",
            "Harmony",
            "All (Android + iOS + macOS + Harmony)",
        ];
        let defaults = vec![false, false, false, false, false];
        let selections = MultiSelect::with_theme(&ColorfulTheme::default())
            .with_prompt("Target platforms")
            .items(&items)
            .defaults(&defaults)
            .interact()?;

        if selections.is_empty() {
            return Err(anyhow!(
                "At least one platform must be selected (press Space to toggle)"
            ));
        }

        let has_all = selections.contains(&4);
        let has_specific = selections.iter().any(|idx| *idx != 4);

        if has_all && !has_specific {
            vec![
                Platform::Android,
                Platform::Ios,
                Platform::Macos,
                Platform::Harmony,
            ]
        } else {
            let mut selected = Vec::new();
            for idx in selections {
                if idx == 4 {
                    continue;
                }
                let platform = match idx {
                    0 => Platform::Android,
                    1 => Platform::Ios,
                    2 => Platform::Macos,
                    3 => Platform::Harmony,
                    _ => unreachable!(),
                };
                if !selected.contains(&platform) {
                    selected.push(platform);
                }
            }
            selected
        }
    };

    let default_package_id = format!("{}.{}", DEFAULT_PACKAGE_PREFIX, name.to_lowercase());
    let package_id = match package_id {
        Some(p) => {
            validate_package_id(&p)?;
            p
        }
        None => {
            let input: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Package ID")
                .with_initial_text(default_package_id.clone())
                .validate_with(|input: &String| -> Result<(), String> {
                    validate_package_id(input).map_err(|e| e.to_string())
                })
                .interact_text()?;
            input
        }
    };

    let target_dir = std::env::current_dir()?.join(&name);

    Ok(ProjectConfig {
        name,
        product_name,
        project_type,
        platforms,
        app_link_hosts: Vec::new(),
        package_id,
        target_dir,
    })
}

pub(super) fn gather_lxapp_dir_name(project_name: &str, yes: bool) -> Result<String> {
    let default_name = project_name.to_string();
    if yes {
        return Ok(default_name);
    }

    let name: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("LxApp (lightweight application) name")
        .with_initial_text(default_name)
        .validate_with(|input: &String| -> Result<(), String> {
            let trimmed = input.trim();
            if trimmed.is_empty() {
                return Err("LxApp directory name cannot be empty".to_string());
            }
            if trimmed.contains('/') || trimmed.contains('\\') {
                return Err("LxApp directory name cannot contain path separators".to_string());
            }
            if lxapp_scaffold::slugify(trimmed) != trimmed {
                return Err(
                    "Use lowercase letters, numbers, and dashes only (e.g. 'home-lxapp')"
                        .to_string(),
                );
            }
            Ok(())
        })
        .interact_text()?;

    Ok(name.trim().to_string())
}

pub(super) fn gather_lxapp_framework(yes: bool) -> Result<String> {
    if yes {
        return Ok("react".to_string());
    }

    let choices = vec!["React", "Vue", "HTML"];
    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Choose framework")
        .items(&choices)
        .default(0)
        .interact()?;
    Ok(choices[selection].to_lowercase())
}

pub(super) fn gather_native_app_service_mode(yes: bool) -> Result<AppServiceMode> {
    if yes {
        return Ok(AppServiceMode::Enabled);
    }

    let enabled = dialoguer::Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Enable AppService for the native host?")
        .default(true)
        .interact()?;
    Ok(if enabled {
        AppServiceMode::Enabled
    } else {
        AppServiceMode::Disabled
    })
}

fn normalize_platforms(input: Vec<String>) -> Result<Vec<Platform>> {
    if input.iter().any(|p| p.eq_ignore_ascii_case("all")) {
        return Ok(vec![
            Platform::Android,
            Platform::Ios,
            Platform::Macos,
            Platform::Harmony,
        ]);
    }

    let mut platforms = Vec::new();
    for raw in input {
        let Some(platform) = Platform::from_str(&raw) else {
            return Err(anyhow!("Unknown platform: {}", raw));
        };
        if !platforms.contains(&platform) {
            platforms.push(platform);
        }
    }
    Ok(platforms)
}
