mod android;
mod apple;
mod config_files;
mod git;
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
use self::git::GitSetup;
use self::lxapp_scaffold::{
    create_lxapp_from_template, create_lxapp_project, ensure_custom_template_target_parent,
};
use self::native::{create_project, create_rust_library};
use self::prompts::{
    gather_control_mode, gather_lxapp_dir_name, gather_lxapp_framework, gather_lxapp_id,
    gather_main_surface, gather_native_app_service_mode, gather_native_project_info,
    gather_product_name, gather_project_name, gather_project_type, validate_native_main_platforms,
};
use self::types::{AppServiceMode, ControlMode, ProjectType};
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
    main: Option<String>,
    control: Option<String>,
    icon: Option<String>,
    template: Option<String>,
    template_args: Vec<String>,
    yes: bool,
    no_git: bool,
) -> Result<()> {
    println!("{}", "Create a new LingXia project".bold());
    println!();

    let versions = current_versions();
    let scaffold_versions = runtime::current_scaffold_versions();
    let npm_range = crate::versions::npm_compat_range();
    println!(
        "  {} Line {}  (SDK {}, Rong {})  npm/crates {}",
        "✓".green(),
        versions.lingxia_crate.cyan(),
        versions.sdk.cyan(),
        versions.rong.cyan(),
        npm_range.cyan(),
    );
    if let Some((current, latest)) = crate::update::newer_released_cli() {
        println!(
            "  {} CLI {} is behind {}. Reinstall the CLI so `lingxia new` scaffolds the current line.",
            "!".yellow(),
            current.yellow(),
            latest.green(),
        );
    }
    println!();

    let project_type = if template.is_some() && project_type.is_none() {
        ProjectType::LxApp
    } else {
        gather_project_type(project_type)?
    };

    if !matches!(project_type, ProjectType::LxApp) && template.is_some() {
        bail!("--template is only supported for standalone lxapp projects");
    }
    if matches!(project_type, ProjectType::LxApp) && (main.is_some() || control.is_some()) {
        bail!("--main and --control are only supported for native-app projects");
    }

    let provider = if matches!(project_type, ProjectType::LxApp) {
        select_template_provider(template.as_deref(), yes)?
    } else {
        None
    };
    if provider.is_none() && !template_args.is_empty() {
        bail!("Template arguments require an installed template provider");
    }
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
            .map(|pattern| {
                pattern
                    .replace("{{PROJECT_NAME}}", &name)
                    .replace("{{PROJECT_SLUG}}", &lxapp_scaffold::slugify(&name))
            })
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
            user_template.as_deref(),
        )?;
        if let Some(provider) = provider.as_ref() {
            template_provider::run_create(provider, &staged_dir, &template_args, !yes)?;
            template_provider::write_project_lock(provider, &staged_dir)?;
        }
        setup_ai_tooling(&staged_dir, yes);
        std::fs::rename(&staged_dir, &target_dir).with_context(|| {
            format!(
                "Failed to activate generated project at {}",
                target_dir.display()
            )
        })?;
        setup_git_repository(&target_dir, no_git);

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
    let main = gather_main_surface(main, yes)?;
    let platforms = if yes && platforms.is_empty() && main.is_native() {
        vec!["macos".to_string(), "windows".to_string()]
    } else {
        platforms
    };
    let config =
        gather_native_project_info(name, product_name, project_type, platforms, package_id, yes)?;
    validate_native_main_platforms(main, &config.platforms)?;
    let control = gather_control_mode(control, main, yes)?;
    let theme = ColorfulTheme::default();

    let lxapp_options = if control == ControlMode::LxApp {
        Some((gather_lxapp_dir_name(yes)?, gather_lxapp_framework(yes)?))
    } else {
        None
    };
    let app_service = if control == ControlMode::LxApp {
        gather_native_app_service_mode(yes)?
    } else {
        AppServiceMode::Disabled
    };

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
    println!("  Main:          {}", main.as_str().cyan());
    println!("  Control:       {}", control.as_str().cyan());
    if let Some((lxapp_dir_name, lxapp_framework)) = &lxapp_options {
        println!("  LxApp Name:    {}", lxapp_dir_name.cyan());
        println!("  LxApp View:    {}", lxapp_framework.cyan());
    }
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
    let lxapp_info = if let Some((lxapp_dir_name, lxapp_framework)) = &lxapp_options {
        Some(create_lxapp_project(
            &config,
            lxapp_dir_name,
            lxapp_framework,
            app_service,
            &versions,
            &scaffold_versions.bridge,
        )?)
    } else {
        None
    };
    generate_config_file(&config, lxapp_info.as_ref(), main, app_service)?;
    setup_ai_tooling(&config.target_dir, yes);
    setup_git_repository(&config.target_dir, no_git);

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
    Ok(())
}

fn setup_git_repository(project_dir: &std::path::Path, no_git: bool) {
    if no_git {
        return;
    }
    match git::initialize_repository(project_dir) {
        Ok(GitSetup::Initialized) => println!(
            "  {} Git: initialized `main` with an initial commit",
            "✓".green()
        ),
        Ok(GitSetup::SkippedParentWorktree) => println!(
            "{}",
            "  Git: skipped because the project is inside an existing worktree".yellow()
        ),
        Ok(GitSetup::SkippedExistingRepository) => println!(
            "{}",
            "  Git: skipped because the template already initialized a repository".yellow()
        ),
        Err(error) => eprintln!(
            "{}",
            format!("warning: Git initialization did not complete: {error:#}").yellow()
        ),
    }
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
/// declined prompt or a failed install never fails `lingxia new` — we fall back
/// to printing the manual one-liners.
fn setup_ai_tooling(project_dir: &std::path::Path, yes: bool) {
    // The skill body is shared by every project. Once it is in the home
    // directory there is nothing to decide: the rest is the project's own
    // AGENTS.md pointer, which belongs to the scaffold like any other file.
    let already_installed = crate::commands::skill::user_install_exists();
    let proceed = if yes || already_installed {
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

/// Write the skill this binary carries into `~/.claude/skills/` (shared by
/// every LingXia project, discovered by Claude Code) instead of vendoring a
/// copy per repo, plus a small, committable `AGENTS.md` pointer in the project
/// for tools that only read a project-level AGENTS.md.
fn run_skill_install(project_dir: &std::path::Path) -> Result<()> {
    println!("{}", "Setting up AI tooling...".bold());
    crate::commands::skill::install_for_new_project(project_dir)
}

fn print_manual_skill_hint() {
    println!("{}", "AI tooling (install later):".bold());
    println!(
        "  {}              # for Claude Code / Anthropic Skills",
        "lingxia skill install".cyan()
    );
    println!(
        "  {}  # for Codex CLI / AGENTS.md tools",
        "lingxia skill install --agents-md".cyan()
    );
    println!();
}

// Platform-specific helpers are in `commands/new/*`.

#[cfg(test)]
mod native_main_scaffold_tests {
    use super::types::{LxAppInfo, MainSurface, Platform, ProjectConfig};
    use super::*;
    use crate::config::{
        EnvVersion, LingXiaConfig, ResolvedEnv, ResourceBundleConfig, ResourceBundleType,
        ResourcesConfig,
    };
    use crate::platform::BuildProfile;
    use crate::platform::detector::PlatformType;

    #[test]
    fn windows_native_terminal_accepts_an_explicit_resource() {
        let temp = tempfile::tempdir().unwrap();
        let target_dir = temp.path().join("terminal-host");
        let config = ProjectConfig {
            name: "terminal-host".to_string(),
            product_name: "Terminal Host".to_string(),
            project_type: ProjectType::NativeApp,
            platforms: vec![Platform::Windows],
            package_id: "com.example.terminal".to_string(),
            app_link_hosts: Vec::new(),
            target_dir: target_dir.clone(),
        };
        let versions = current_versions();

        create_project(&config, &versions).unwrap();
        let windows_manifest =
            std::fs::read_to_string(target_dir.join("windows").join("Cargo.toml")).unwrap();
        assert!(!windows_manifest.contains("{{WINDOWS_RS_REV}}"));
        assert!(windows_manifest.contains(&format!("rev = \"{}\"", windows::WINDOWS_RS_REV)));
        create_rust_library(&config, &versions, AppServiceMode::Disabled).unwrap();
        generate_config_file(
            &config,
            Option::<&LxAppInfo>::None,
            MainSurface::Terminal,
            AppServiceMode::Disabled,
        )
        .unwrap();

        assert!(!target_dir.join(DEFAULT_LXAPP_DIR_NAME).exists());
        // `lingxia new` always seeds the project-root icon; the interactive
        // icon step is bypassed here.
        std::fs::write(target_dir.join("AppIcon.png"), b"png-bytes").unwrap();
        let settings_dir = target_dir.join("terminal-settings-fixture");
        std::fs::create_dir_all(settings_dir.join("pages/settings")).unwrap();
        std::fs::write(
            settings_dir.join("lxapp.json"),
            r#"{
                "appId": "com.example.settings",
                "name": "Settings",
                "version": "0.0.0",
                "logic": false,
                "security": {
                    "network": { "trustedDomains": [] },
                    "privileges": []
                },
                "pages": [{
                    "name": "settings",
                    "path": "pages/settings/index.html"
                }]
            }"#,
        )
        .unwrap();
        std::fs::write(settings_dir.join("lxapp.config.ts"), "export default {};\n").unwrap();
        std::fs::write(
            settings_dir.join("pages/settings/index.html"),
            "<!doctype html><title>Settings</title>\n",
        )
        .unwrap();

        let mut host_config = LingXiaConfig::load(&target_dir).unwrap();
        host_config.resources = Some(ResourcesConfig {
            bundles: vec![ResourceBundleConfig {
                bundle_type: ResourceBundleType::Lxapp,
                app_id: "com.example.settings".to_string(),
                path: Some("terminal-settings-fixture".to_string()),
                package: None,
                version: None,
            }],
        });
        crate::host_assets::prepare_configured_host_assets(
            &target_dir,
            &host_config,
            BuildProfile::Debug,
            None,
            None,
            false,
            &[PlatformType::Windows],
            &[],
            true,
            None,
            &ResolvedEnv {
                version: EnvVersion::Developer,
                lingxia_server: "https://api.example.com".to_string(),
                package_id_suffix: Some(".dev".to_string()),
            },
        )
        .unwrap();

        let assets_dir = crate::platform::windows::resolve_windows_assets_dir(&target_dir).unwrap();
        let app_json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(assets_dir.join("app.json")).unwrap()).unwrap();
        assert!(app_json.get("homeAppId").is_none());
        assert!(app_json.get("homeAppVersion").is_none());

        let ui_json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(assets_dir.join("ui.json")).unwrap()).unwrap();
        assert_eq!(ui_json["launch"]["initialSurface"], "terminal");
        let surfaces = ui_json["surfaces"].as_array().unwrap();
        assert_eq!(surfaces.len(), 1);
        assert_eq!(surfaces[0]["role"], "main");
        assert_eq!(surfaces[0]["content"]["kind"], "native");
        assert_eq!(surfaces[0]["content"]["name"], "terminal");

        let mut asset_names = std::fs::read_dir(&assets_dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        asset_names.sort();
        assert_eq!(
            asset_names,
            [
                "AppIcon.png",
                "app.json",
                "bridge-runtime.js",
                "com.example.settings",
                "icons",
                "ui.json"
            ]
        );
        assert_eq!(
            std::fs::read(assets_dir.join("AppIcon.png")).unwrap(),
            b"png-bytes"
        );
    }
}
