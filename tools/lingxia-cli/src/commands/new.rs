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
use anyhow::{Context, Result, anyhow, bail};
use colored::Colorize;
use dialoguer::{Confirm, Select, theme::ColorfulTheme};
use std::path::PathBuf;

use self::config_files::generate_config_file;
use self::lxapp_scaffold::{
    create_lxapp_from_template, create_lxapp_project, ensure_custom_template_target_parent,
};
use self::native::{create_project, create_rust_library};
use self::prompts::{
    gather_lxapp_dir_name, gather_lxapp_framework, gather_lxapp_id, gather_native_app_service_mode,
    gather_native_project_info, gather_product_name, gather_project_name, gather_project_type,
};
use self::types::{AppServiceMode, ProjectType};
use crate::commands::template_provider::{self, InstalledTemplate};

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
    template: Option<String>,
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

    let project_type = if template.is_some() && project_type.is_none() {
        ProjectType::LxApp
    } else {
        gather_project_type(project_type)?
    };

    if !matches!(project_type, ProjectType::LxApp) && template.is_some() {
        bail!("--template is only supported for standalone lxapp projects");
    }

    let provider = if matches!(project_type, ProjectType::LxApp) {
        select_template_provider(template.as_deref(), yes)?
    } else {
        None
    };
    let user_template = provider
        .as_ref()
        .map(template_provider::template_directory)
        .transpose()?;
    if let Some(path) = user_template.as_deref() {
        ensure_custom_template_target_parent(path, &std::env::current_dir()?)?;
    }
    let name = gather_project_name(name)?;

    if matches!(project_type, ProjectType::LxApp) {
        // A lightweight lxapp keeps a single name: the project name doubles as
        // the display name. Only the appId is separately editable.
        let product_name = name.clone();
        let default_app_id = provider
            .as_ref()
            .and_then(|provider| provider.manifest.defaults.app_id.as_deref())
            .map(|pattern| pattern.replace("{{PROJECT_NAME}}", &name))
            .unwrap_or_else(|| self::types::default_lxapp_app_id(&name));
        let app_id = gather_lxapp_id(&default_app_id, yes)?;
        let framework = if let Some(provider) = provider.as_ref() {
            provider.manifest.framework.clone()
        } else {
            gather_lxapp_framework(yes)?
        };
        let current_dir = std::env::current_dir()?;
        let target_dir = current_dir.join(&name);
        if target_dir.exists() {
            bail!("Directory '{}' already exists", target_dir.display());
        }
        if let Some(provider) = provider.as_ref() {
            println!(
                "  {} LxApp template: {} ({})",
                "✓".green(),
                provider.manifest.name,
                provider.commit.get(..7).unwrap_or(&provider.commit)
            );
        }
        let staging_root = tempfile::Builder::new()
            .prefix(".lingxia-new-")
            .tempdir_in(&current_dir)?;
        let staged_dir = staging_root.path().join(&name);
        create_lxapp_from_template(
            &staged_dir,
            &name,
            &app_id,
            &product_name,
            &framework,
            AppServiceMode::Enabled,
            &versions,
            &scaffold_versions.bridge,
            &scaffold_versions.types,
            user_template.as_deref(),
        )?;
        if let Some(provider) = provider.as_ref() {
            template_provider::run_create(provider, &staged_dir)?;
            template_provider::write_project_lock(provider, &staged_dir)?;
        }
        setup_ai_tooling(&staged_dir, yes);
        std::fs::rename(&staged_dir, &target_dir).with_context(|| {
            format!(
                "Failed to activate generated project at {}",
                target_dir.display()
            )
        })?;

        println!();
        println!("{}", "Project created successfully!".green().bold());
        println!();
        println!("{}", "Next steps:".bold());
        println!("  cd {}", name);
        println!("  lingxia dev");
        println!();
        return Ok(());
    }

    let product_name = gather_product_name(&name, yes)?;
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
    println!("  lingxia dev");
    println!();
    setup_ai_tooling(&config.target_dir, yes);

    Ok(())
}

fn select_template_provider(name: Option<&str>, yes: bool) -> Result<Option<InstalledTemplate>> {
    if let Some(name) = name {
        if name == "minimal" {
            return Ok(None);
        }
        return template_provider::resolve_for_new(name).map(Some);
    }
    if yes {
        return Ok(None);
    }
    let installed = template_provider::list_installed()?;
    if installed.is_empty() {
        return Ok(None);
    }
    let mut labels = vec!["Minimal".to_string()];
    labels.extend(
        installed
            .iter()
            .map(|template| template.manifest.name.clone()),
    );
    let selected = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Template")
        .items(&labels)
        .default(0)
        .interact()?;
    if selected == 0 {
        return Ok(None);
    }
    let selected = installed
        .get(selected - 1)
        .ok_or_else(|| anyhow!("Invalid template selection"))?;
    template_provider::resolve_for_new(&selected.slug).map(Some)
}

/// Set up AI tooling (the LingXia agent skill) in the freshly created project.
/// Opt-out: installs by default, including in non-interactive/`--yes` mode. A
/// declined prompt, a missing `npx`, or a failed install never fails
/// `lingxia new` — we fall back to printing the manual one-liners.
fn setup_ai_tooling(project_dir: &std::path::Path, yes: bool) {
    let proceed = if yes {
        true
    } else {
        Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Set up AI tooling (installs the LingXia agent skill)?")
            .default(true)
            .interact()
            .unwrap_or(false)
    };

    if !proceed {
        print_manual_skill_hint();
        return;
    }

    if let Err(err) = run_skill_install(project_dir) {
        eprintln!(
            "{}",
            format!("warning: AI tooling setup did not complete: {err}").yellow()
        );
        print_manual_skill_hint();
    }
}

/// Run `npx @lingxia/skill install --user --agents-md` from the new project.
/// `--user` puts the skill body in the global `~/.claude/skills/` (shared by
/// every LingXia project, discovered by Claude Code) instead of vendoring a
/// copy per repo; `--agents-md` writes a small, committable `AGENTS.md` pointer
/// into the project for Codex (which only reads project-level AGENTS.md).
fn run_skill_install(project_dir: &std::path::Path) -> Result<()> {
    println!("{}", "Setting up AI tooling...".bold());
    let status = std::process::Command::new("npx")
        .arg("@lingxia/skill")
        .arg("install")
        .arg("--user")
        .arg("--agents-md")
        .current_dir(project_dir)
        .status()
        .context("failed to run `npx @lingxia/skill install` (is `npx` on PATH?)")?;
    if !status.success() {
        bail!("`npx @lingxia/skill install` exited with a non-zero status");
    }
    Ok(())
}

fn print_manual_skill_hint() {
    println!("{}", "AI tooling (install later):".bold());
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
