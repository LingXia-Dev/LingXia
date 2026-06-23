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
mod template_assets;
mod types;
mod validation;
mod windows;

use crate::runtime;
use crate::versions::current_versions;
use anyhow::Result;
use colored::Colorize;
use dialoguer::{Confirm, theme::ColorfulTheme};
use std::path::PathBuf;

use self::config_files::generate_config_file;
use self::lxapp_scaffold::{create_lxapp_from_template, create_lxapp_project};
use self::native::{create_project, create_rust_library};
use self::prompts::{
    gather_lxapp_dir_name, gather_lxapp_framework, gather_native_app_service_mode,
    gather_native_project_info, gather_product_name, gather_project_name, gather_project_type,
};
use self::types::{AppServiceMode, ProjectType};

/// Directory name for the native Rust library crate scaffolded by `lingxia new`.
/// Named for the layer (native Rust) rather than the project; recorded in
/// `lingxia.yaml` as `app.rustLibDir` so builds resolve it explicitly rather
/// than re-deriving it.
pub(super) const RUST_LIB_DIR_NAME: &str = "native";

/// Default directory name for the scaffolded lxapp. Named for what it is (an
/// lxapp), matching the `lxapp.json`/`lxapp.ts` it contains. The lxapp directory
/// name doubles as its `appId`, so this is also the default home appId.
pub(super) const DEFAULT_LXAPP_DIR_NAME: &str = "lxapp";

/// Locate the extracted embedded template assets directory.
pub(super) fn locate_templates_dir() -> Result<PathBuf> {
    template_assets::locate_templates_dir()
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

    let versions = current_versions();
    let scaffold_versions = runtime::current_scaffold_versions();
    println!(
        "  {} SDK: {}, Rong: {}, LingXia crate: {}, Bridge: {}, Types: {}",
        "✓".green(),
        versions.sdk.cyan(),
        versions.rong.cyan(),
        versions.lingxia_crate.cyan(),
        scaffold_versions.bridge.cyan(),
        scaffold_versions.types.cyan(),
    );
    println!();

    let project_type = gather_project_type(project_type)?;
    let name = gather_project_name(name)?;
    let product_name = gather_product_name(&name, yes)?;

    if matches!(project_type, ProjectType::LxApp) {
        let framework = gather_lxapp_framework(yes)?;
        let target_dir = std::env::current_dir()?.join(&name);
        create_lxapp_from_template(
            &target_dir,
            &name,
            &product_name,
            &framework,
            AppServiceMode::Enabled,
            &versions,
            &scaffold_versions.bridge,
            &scaffold_versions.types,
        )?;

        println!();
        println!("{}", "Project created successfully!".green().bold());
        println!();
        println!("{}", "Next steps:".bold());
        println!("  cd {}", name);
        println!("  lingxia build");
        println!();
        print_ai_skill_tip();
        return Ok(());
    }

    let config =
        gather_native_project_info(name, product_name, project_type, platforms, package_id, yes)?;
    let theme = ColorfulTheme::default();

    let lxapp_dir_name = gather_lxapp_dir_name(yes)?;
    let lxapp_framework = gather_lxapp_framework(yes)?;
    let app_service = gather_native_app_service_mode(yes)?;

    println!();
    println!("{}", "Project Configuration:".bold());
    println!("  Name:          {}", config.name.cyan());
    if config.product_name != config.name {
        println!("  Product:       {}", config.product_name.cyan());
    }
    println!("  Type:          {}", config.project_type.as_str().cyan());
    let platform_list = config
        .platforms
        .iter()
        .map(|p| p.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    println!("  Platforms:     {}", platform_list.cyan());
    println!("  Package ID:    {}", config.package_id.cyan());
    println!("  LxApp Name:    {}", lxapp_dir_name.cyan());
    println!("  LxApp View:    {}", lxapp_framework.cyan());
    println!("  AppService:    {}", app_service.label().cyan());
    println!(
        "  Directory:     {}",
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
    create_rust_library(&config, &versions, app_service)?;
    icons::configure_and_apply_icons(&config, icon, yes, &theme)?;
    let lxapp_info = create_lxapp_project(
        &config,
        &lxapp_dir_name,
        &lxapp_framework,
        app_service,
        &versions,
        &scaffold_versions.bridge,
        &scaffold_versions.types,
    )?;
    generate_config_file(&config, &lxapp_info, app_service)?;

    println!();
    println!("{}", "Project created successfully!".green().bold());
    println!();
    println!(
        "{}",
        format!(
            "Note: in {} -> [storage], set cacheMaxSizeMB=0 to disable usercache size enforcement.",
            crate::config::HOST_CONFIG_FILE
        )
        .yellow()
    );
    println!();
    println!("{}", "Next steps:".bold());
    println!("  cd {}", config.name);
    println!("  lingxia build");
    println!();
    print_ai_skill_tip();

    Ok(())
}

fn print_ai_skill_tip() {
    println!("{}", "AI tooling (optional):".bold());
    println!(
        "  {}              # for Claude Code / Anthropic Skills",
        "npx @lingxia/skill install".cyan()
    );
    println!(
        "  {}  # for Codex CLI / AGENTS.md tools",
        "npx @lingxia/skill install --agents-md".cyan()
    );
    println!();
}

// Platform-specific helpers are in `commands/new/*`.
