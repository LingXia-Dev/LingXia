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
use anyhow::{Context, Result, bail};
use colored::Colorize;
use dialoguer::{Confirm, theme::ColorfulTheme};
use std::path::{Path, PathBuf};

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

/// Resolve the optional user-level standalone lxapp template.
///
/// The directory is a complete React lxapp template repository and replaces
/// the embedded standalone template as one unit.
fn locate_user_lxapp_template_dir() -> Result<Option<PathBuf>> {
    let Some(home) = dirs::home_dir() else {
        return Ok(None);
    };
    locate_user_lxapp_template_dir_from(&home)
}

fn locate_user_lxapp_template_dir_from(home: &Path) -> Result<Option<PathBuf>> {
    let root = user_lxapp_template_root(home);
    match std::fs::symlink_metadata(&root) {
        Ok(_) => validate_lxapp_template_dir(&root).map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error)
            .with_context(|| format!("Failed to inspect LxApp template: {}", root.display())),
    }
}

fn validate_lxapp_template_dir(root: &Path) -> Result<PathBuf> {
    if !root.exists() {
        bail!("Custom LxApp template does not exist: {}", root.display());
    }
    if !root.is_dir() {
        bail!(
            "Custom LxApp template is not a directory: {}",
            root.display()
        );
    }
    for required in ["package.json", "lxapp.json"] {
        if !root.join(required).is_file() {
            bail!(
                "Custom LxApp template is missing {}",
                root.join(required).display()
            );
        }
    }

    root.canonicalize()
        .with_context(|| format!("Failed to resolve LxApp template: {}", root.display()))
}

fn user_lxapp_template_root(home: &Path) -> PathBuf {
    home.join(".lingxia").join("templates").join("lxapp")
}

/// Execute the new project command
pub fn execute(
    name: Option<String>,
    project_type: Option<String>,
    platforms: Vec<String>,
    package_id: Option<String>,
    icon: Option<String>,
    template: Option<PathBuf>,
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

    let user_template = if matches!(project_type, ProjectType::LxApp) {
        match template.as_deref() {
            Some(path) => Some(validate_lxapp_template_dir(path)?),
            None => locate_user_lxapp_template_dir()?,
        }
    } else {
        None
    };
    if let Some(template) = user_template.as_deref() {
        ensure_custom_template_target_parent(template, &std::env::current_dir()?)?;
    }
    let name = gather_project_name(name)?;

    if matches!(project_type, ProjectType::LxApp) {
        // A lightweight lxapp keeps a single name: the project name doubles as
        // the display name. Only the appId is separately editable.
        let product_name = name.clone();
        let app_id = gather_lxapp_id(&self::types::default_lxapp_app_id(&name), yes)?;
        let framework = if user_template.is_some() {
            "react".to_string()
        } else {
            gather_lxapp_framework(yes)?
        };
        let target_dir = std::env::current_dir()?.join(&name);
        if let Some(template) = user_template.as_deref() {
            println!("  {} LxApp template: {}", "✓".green(), template.display());
        }
        create_lxapp_from_template(
            &target_dir,
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

        println!();
        println!("{}", "Project created successfully!".green().bold());
        println!();
        println!("{}", "Next steps:".bold());
        println!("  cd {}", name);
        println!("  lingxia dev");
        println!();
        setup_ai_tooling(&target_dir, yes);
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

#[cfg(test)]
mod user_template_tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn missing_user_template_uses_builtin() {
        let home = tempdir().unwrap();
        assert_eq!(
            locate_user_lxapp_template_dir_from(home.path()).unwrap(),
            None
        );
    }

    #[test]
    fn resolves_complete_user_template() {
        let home = tempdir().unwrap();
        let template = user_lxapp_template_root(home.path());
        std::fs::create_dir_all(&template).unwrap();
        for file in ["package.json", "lxapp.json"] {
            std::fs::write(template.join(file), "template").unwrap();
        }

        assert_eq!(
            locate_user_lxapp_template_dir_from(home.path()).unwrap(),
            Some(template.canonicalize().unwrap())
        );
    }

    #[test]
    fn rejects_incomplete_user_template() {
        let home = tempdir().unwrap();
        std::fs::create_dir_all(user_lxapp_template_root(home.path())).unwrap();

        let error = locate_user_lxapp_template_dir_from(home.path()).unwrap_err();
        assert!(error.to_string().contains("package.json"));
    }

    #[test]
    fn validates_explicit_template_root() {
        let root = tempdir().unwrap();
        for file in ["package.json", "lxapp.json"] {
            std::fs::write(root.path().join(file), "template").unwrap();
        }

        assert_eq!(
            validate_lxapp_template_dir(root.path()).unwrap(),
            root.path().canonicalize().unwrap()
        );
    }

    #[test]
    fn rejects_missing_explicit_template() {
        let root = tempdir().unwrap();
        let missing = root.path().join("missing");

        let error = validate_lxapp_template_dir(&missing).unwrap_err();

        assert!(error.to_string().contains("does not exist"));
    }
}
