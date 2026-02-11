mod android;
mod apple;
mod config_files;
mod harmony;
mod icons;
mod ios;
mod lxapp_scaffold;
mod macos;
mod native;
mod prompts;
mod template;
mod types;
mod validation;

use crate::runtime;
use crate::versions::fetch_latest_versions;
use anyhow::{Result, anyhow};
use colored::Colorize;
use dialoguer::{Confirm, theme::ColorfulTheme};
use indicatif::{ProgressBar, ProgressStyle};
use std::env;
use std::io::{IsTerminal, Write};
use std::path::PathBuf;

use self::config_files::generate_config_file;
use self::lxapp_scaffold::{create_lxapp_from_template, create_lxapp_project};
use self::native::{create_project, create_rust_library};
use self::prompts::{
    gather_lxapp_dir_name, gather_lxapp_framework, gather_native_project_info, gather_product_name,
    gather_project_name, gather_project_type,
};
use self::types::ProjectType;

/// Locate the templates directory via LINGXIA_TEMPLATES_DIR environment variable.
pub(super) fn locate_templates_dir() -> Result<PathBuf> {
    let path = env::var("LINGXIA_TEMPLATES_DIR")
        .map(PathBuf::from)
        .map_err(|_| anyhow!("LINGXIA_TEMPLATES_DIR not set"))?;

    if path.exists() {
        Ok(path)
    } else {
        Err(anyhow!("Templates directory not found: {}", path.display()))
    }
}

/// Execute the new project command
pub fn execute(
    name: Option<String>,
    project_type: Option<String>,
    platforms: Vec<String>,
    package_id: Option<String>,
    icon: Option<String>,
    yes: bool,
) -> Result<()> {
    println!("{}", "Create a new LingXia project".bold());
    println!();

    // Fetch LingXia versions from GitHub
    // Use spinner only in TTY (interactive terminal), skip in CI/non-TTY to avoid log pollution
    let is_tty = std::io::stdout().is_terminal();
    let spinner: Option<ProgressBar> = if is_tty {
        let sp = ProgressBar::new_spinner();
        sp.set_style(
            ProgressStyle::default_spinner()
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                .template("{spinner:.cyan} {msg}")
                .expect("Invalid spinner template"),
        );
        sp.set_message("Fetching SDK information...");
        sp.enable_steady_tick(std::time::Duration::from_millis(80));
        Some(sp)
    } else {
        print!("  Fetching SDK information...");
        std::io::stdout().flush().ok();
        None
    };

    let fetch_result = fetch_latest_versions();
    if let Some(sp) = spinner {
        sp.finish_and_clear();
    } else if fetch_result.is_ok() {
        println!(" done");
    } else {
        println!(" failed");
    }

    let versions = fetch_result?;
    let web_runtime_version = runtime::fetch_latest_runtime_version()
        .map_err(|e| anyhow!("Failed to fetch latest @lingxia/web-runtime version: {e}"))?;
    println!(
        "  {} SDK: {}, Rong: {}, LingXia crate: {}, Runtime: {}",
        "✓".green(),
        versions.sdk.cyan(),
        versions.rong.cyan(),
        versions.lingxia_crate.cyan(),
        web_runtime_version.cyan()
    );
    println!();

    let project_type = gather_project_type(project_type)?;
    let name = gather_project_name(name)?;
    let product_name = gather_product_name(&name, yes)?;

    if matches!(project_type, ProjectType::LxApp) {
        let framework = gather_lxapp_framework(yes)?;
        let target_dir = std::env::current_dir()?.join(&name);
        create_lxapp_from_template(&target_dir, &name, &product_name, &framework, &versions)?;

        println!();
        println!("{}", "Project created successfully!".green().bold());
        println!();
        println!("{}", "Next steps:".bold());
        println!("  cd {}", name);
        println!("  lingxia build");
        println!();
        return Ok(());
    }

    let config =
        gather_native_project_info(name, product_name, project_type, platforms, package_id, yes)?;
    let theme = ColorfulTheme::default();

    println!();
    println!("{}", "Project Configuration:".bold());
    println!("  Name:        {}", config.name.cyan());
    if config.product_name != config.name {
        println!("  Product:     {}", config.product_name.cyan());
    }
    println!("  Type:        {}", config.project_type.as_str().cyan());
    let platform_list = config
        .platforms
        .iter()
        .map(|p| p.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    println!("  Platforms:   {}", platform_list.cyan());
    println!("  Package ID:  {}", config.package_id.cyan());
    println!(
        "  Directory:   {}",
        config.target_dir.display().to_string().cyan()
    );
    println!();

    if !yes {
        let confirmed = Confirm::with_theme(&theme)
            .with_prompt("Create project?")
            .default(true)
            .interact()?;

        if !confirmed {
            println!("{}", "Project creation cancelled.".yellow());
            return Ok(());
        }
    }

    create_project(&config, &versions)?;
    create_rust_library(&config, &versions)?;
    icons::configure_and_apply_icons(&config, icon, yes, &theme)?;

    let lxapp_dir_name = gather_lxapp_dir_name(yes)?;
    let lxapp_framework = gather_lxapp_framework(yes)?;
    let lxapp_info = create_lxapp_project(&config, &lxapp_dir_name, &lxapp_framework, &versions)?;
    generate_config_file(&config, &lxapp_info, &web_runtime_version)?;

    println!();
    println!("{}", "Project created successfully!".green().bold());
    println!();
    println!("{}", "Next steps:".bold());
    println!("  cd {}", config.name);
    println!("  lingxia build");
    println!();

    Ok(())
}

// Platform-specific helpers are in `commands/new/*`.
