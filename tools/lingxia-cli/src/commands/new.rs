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
use dialoguer::{Confirm, theme::ColorfulTheme};
use std::path::PathBuf;

use self::config_files::generate_config_file;
use self::lxapp_scaffold::{create_lxapp_from_template, create_lxapp_project};
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

/// Execute the new project command
pub fn execute(
    name: Option<String>,
    project_type: Option<String>,
    platforms: Vec<String>,
    package_id: Option<String>,
    icon: Option<String>,
    functions: bool,
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

    if matches!(project_type, ProjectType::LxApp) {
        // A lightweight lxapp keeps a single name: the project name doubles as
        // the display name. Only the appId is separately editable.
        let product_name = name.clone();
        // --functions needs the lingxiao CLI (it scaffolds + builds the worker).
        // Fail fast before creating anything — no half-worker fallback.
        if functions && !lingxiao_available() {
            return Err(anyhow!(
                "`lingxia new --functions` requires the `lingxiao` CLI on PATH — it scaffolds \
                 and builds the cloud worker.\nInstall lingxiao and retry, or omit --functions."
            ));
        }
        let app_id = gather_lxapp_id(&name, yes)?;
        let framework = gather_lxapp_framework(yes)?;
        let target_dir = std::env::current_dir()?.join(&name);
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
        )?;
        if functions {
            scaffold_functions(&target_dir)?;
        }

        println!();
        println!("{}", "Project created successfully!".green().bold());
        println!();
        println!("{}", "Next steps:".bold());
        println!("  cd {}", name);
        println!("  lingxia dev");
        println!();
        print_ai_skill_tip();
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

/// `--functions`: lay the typed-cloud-functions overlay onto a fresh lxapp.
/// lingxia owns the contract + sample impl + mock + `functions.json` + home
/// variant; the worker *structure* (lingxiao.toml / tsconfig / src/index.ts /
/// package.json) is scaffolded by `lingxiao new` — which is required (the caller
/// checks availability first, so this only fails if the run itself errors).
fn scaffold_functions(target_dir: &std::path::Path) -> Result<()> {
    let overlay = locate_templates_dir()?.join("lxapp-functions");
    let server = target_dir.join("server");

    // Worker structure: lingxiao owns it (never hand-mirror its scaffold).
    run_lingxiao_new(&server)?;
    // Drop lingxiao's placeholder fn; we ship a coherent `hello` sample.
    let _ = std::fs::remove_file(server.join("src/functions/main.ts"));

    // lingxia's overlay: build-ready sample + mock + config + home variant.
    copy_dir_all(&overlay.join("server"), &server)?;
    std::fs::copy(
        overlay.join("functions.json"),
        target_dir.join("functions.json"),
    )?;
    // Home variant: swap the `greet` action body to call the cloud function
    // (the View is untouched).
    std::fs::copy(
        overlay.join("pages/home/index.ts"),
        target_dir.join("pages/home/index.ts"),
    )?;

    println!(
        "  {} Cloud functions: server/ worker (via lingxiao new) + functions.json",
        "✓".green()
    );
    Ok(())
}

/// Whether the `lingxiao` CLI is on PATH (it scaffolds + builds the worker).
fn lingxiao_available() -> bool {
    std::process::Command::new("lingxiao")
        .arg("--help")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
}

/// Scaffold the worker structure via `lingxiao new <server>`.
fn run_lingxiao_new(server: &std::path::Path) -> Result<()> {
    let status = std::process::Command::new("lingxiao")
        .arg("new")
        .arg(server)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .context("failed to run `lingxiao new` (is the lingxiao CLI installed?)")?;
    if !status.success() {
        bail!("`lingxiao new {}` failed", server.display());
    }
    Ok(())
}

/// Recursively copy `src` into `dst` (creating `dst`).
fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}
