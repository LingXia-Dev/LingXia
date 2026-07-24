use anyhow::{Context, Result, anyhow, bail};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const MANIFEST_FILE: &str = "lingxia-template.json";
const SKILL_OWNER_FILE: &str = ".lingxia-template-owner";
const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
const REMOTE_CHECK_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TemplateDefaults {
    pub app_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TemplateManifest {
    pub name: String,
    pub template: PathBuf,
    #[serde(default = "default_framework")]
    pub framework: String,
    #[serde(default)]
    pub commands: BTreeMap<String, PathBuf>,
    #[serde(default)]
    pub skills: Vec<PathBuf>,
    pub create: Option<TemplateLifecycle>,
    #[serde(default)]
    pub defaults: TemplateDefaults,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TemplateLifecycle {
    pub command: PathBuf,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct TemplateState {
    source: String,
    commit: String,
    last_checked: u64,
}

#[derive(Clone, Debug)]
pub struct InstalledTemplate {
    pub slug: String,
    pub root: PathBuf,
    pub manifest: TemplateManifest,
    pub source: String,
    pub commit: String,
}

fn default_framework() -> String {
    "react".to_string()
}

pub fn execute_add(source: &str) -> Result<()> {
    let home = require_home()?;
    let installed = add_from(&home, source)?;
    println!(
        "{}",
        format!(
            "Added template {} ({})",
            installed.manifest.name, installed.commit
        )
        .green()
    );
    Ok(())
}

pub fn execute_list() -> Result<()> {
    let home = require_home()?;
    let templates = list_from(&home)?;
    if templates.is_empty() {
        println!("No external templates installed.");
        println!("Add one with: lingxia template add <git-url>");
        return Ok(());
    }
    println!("Installed templates:");
    for template in templates {
        println!(
            "  {}  {}  {}",
            template.slug.cyan(),
            short_commit(&template.commit),
            template.source
        );
    }
    Ok(())
}

pub fn execute_update(name: Option<&str>) -> Result<()> {
    let home = require_home()?;
    let names = match name {
        Some(name) => vec![name.to_string()],
        None => list_from(&home)?
            .into_iter()
            .map(|template| template.slug)
            .collect(),
    };
    if names.is_empty() {
        println!("No external templates installed.");
        return Ok(());
    }
    for name in names {
        let before = load_from(&home, &name)?;
        let after = update_from(&home, &name, true)?;
        if before.commit == after.commit {
            println!("  {} {} is current", "✓".green(), name);
        } else {
            println!(
                "  {} {} {} → {}",
                "✓".green(),
                name,
                short_commit(&before.commit),
                short_commit(&after.commit)
            );
        }
    }
    Ok(())
}

pub fn execute_remove(name: &str) -> Result<()> {
    let home = require_home()?;
    remove_from(&home, name)?;
    println!("{}", format!("Removed template {name}").green());
    Ok(())
}

pub fn list_installed() -> Result<Vec<InstalledTemplate>> {
    list_from(&require_home()?)
}

pub fn resolve_for_new(name: &str) -> Result<InstalledTemplate> {
    let home = require_home()?;
    let current = load_from(&home, name)?;
    let state = read_state(&home, &current.slug)?;
    if now().saturating_sub(state.last_checked) < CHECK_INTERVAL.as_secs() {
        return Ok(current);
    }
    match update_from(&home, &current.slug, false) {
        Ok(updated) => Ok(updated),
        Err(error) => {
            eprintln!(
                "{}",
                format!(
                    "warning: unable to refresh template {}: {error:#}; using {}",
                    current.slug,
                    short_commit(&current.commit)
                )
                .yellow()
            );
            Ok(current)
        }
    }
}

pub fn template_directory(template: &InstalledTemplate) -> Result<PathBuf> {
    resolve_owned_path(
        &template.root,
        &template.manifest.template,
        "template directory",
    )
}

pub fn run_create(template: &InstalledTemplate, project_root: &Path) -> Result<()> {
    let Some(lifecycle) = template.manifest.create.as_ref() else {
        return Ok(());
    };
    let entry = resolve_owned_path(&template.root, &lifecycle.command, "create entry")?;
    let mut command = command_for_entry(&entry);
    let status = command
        .args(&lifecycle.args)
        .current_dir(project_root)
        .env("LINGXIA_TEMPLATE_ROOT", &template.root)
        .status()
        .with_context(|| {
            format!(
                "Failed to start create lifecycle for template {}",
                template.manifest.name
            )
        })?;
    if !status.success() {
        bail!(
            "Template {} create lifecycle failed with {status}",
            template.manifest.name
        );
    }
    Ok(())
}

pub fn write_project_lock(template: &InstalledTemplate, project_root: &Path) -> Result<()> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct ProjectTemplateLock<'a> {
        name: &'a str,
        commit: &'a str,
    }

    let directory = project_root.join(".lingxia");
    fs::create_dir_all(&directory)?;
    let bytes = serde_json::to_vec_pretty(&ProjectTemplateLock {
        name: &template.slug,
        commit: &template.commit,
    })?;
    fs::write(
        directory.join("template.json"),
        [bytes, b"\n".to_vec()].concat(),
    )?;
    Ok(())
}

fn require_home() -> Result<PathBuf> {
    dirs::home_dir().ok_or_else(|| anyhow!("Unable to determine the user home directory"))
}

fn templates_root(home: &Path) -> PathBuf {
    home.join(".lingxia").join("templates")
}

fn states_root(home: &Path) -> PathBuf {
    home.join(".lingxia").join("template-state")
}

fn state_path(home: &Path, slug: &str) -> PathBuf {
    states_root(home).join(format!("{slug}.json"))
}

fn add_from(home: &Path, source: &str) -> Result<InstalledTemplate> {
    let source = normalize_source(source)?;
    let templates = templates_root(home);
    fs::create_dir_all(&templates)?;
    let temporary = tempfile::Builder::new()
        .prefix(".template-add-")
        .tempdir_in(&templates)?;
    let checkout = temporary.path().join("checkout");
    clone_repository(&source, &checkout)?;
    let manifest = load_manifest(&checkout)?;
    let slug = slug_for_name(&manifest.name)?;
    let target = templates.join(&slug);
    if target.exists() {
        bail!("Template `{slug}` is already installed");
    }
    let commit = git_commit(&checkout)?;
    fs::rename(&checkout, &target).with_context(|| {
        format!(
            "Failed to install template {} into {}",
            manifest.name,
            target.display()
        )
    })?;
    let installed = InstalledTemplate {
        slug: slug.clone(),
        root: target,
        manifest,
        source: source.clone(),
        commit: commit.clone(),
    };
    if let Err(error) = write_state(
        home,
        &slug,
        &TemplateState {
            source,
            commit,
            last_checked: now(),
        },
    ) {
        let _ = fs::remove_dir_all(&installed.root);
        return Err(error);
    }
    if let Err(error) = sync_assets(home, &installed, None) {
        let _ = fs::remove_dir_all(&installed.root);
        let _ = fs::remove_file(state_path(home, &slug));
        return Err(error);
    }
    Ok(installed)
}

fn list_from(home: &Path) -> Result<Vec<InstalledTemplate>> {
    let root = templates_root(home);
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    for entry in fs::read_dir(&root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() || entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let slug = entry.file_name().to_string_lossy().into_owned();
        result.push(load_from(home, &slug)?);
    }
    result.sort_by(|left, right| left.slug.cmp(&right.slug));
    Ok(result)
}

fn load_from(home: &Path, name: &str) -> Result<InstalledTemplate> {
    let slug = slug_for_name(name)?;
    let root = templates_root(home).join(&slug);
    if !root.is_dir() {
        bail!("Template `{slug}` is not installed; run `lingxia template add <git-url>`");
    }
    let manifest = load_manifest(&root)?;
    let state = read_state(home, &slug)?;
    Ok(InstalledTemplate {
        slug,
        root,
        manifest,
        source: state.source,
        commit: state.commit,
    })
}

fn update_from(home: &Path, name: &str, force: bool) -> Result<InstalledTemplate> {
    let current = load_from(home, name)?;
    let state = read_state(home, &current.slug)?;
    if !force && now().saturating_sub(state.last_checked) < CHECK_INTERVAL.as_secs() {
        return Ok(current);
    }

    let remote_commit = git_remote_commit(&state.source)?;
    if remote_commit == current.commit {
        sync_assets(home, &current, Some(&current))?;
        write_state(
            home,
            &current.slug,
            &TemplateState {
                last_checked: now(),
                ..state
            },
        )?;
        return Ok(current);
    }

    let templates = templates_root(home);
    let temporary = tempfile::Builder::new()
        .prefix(".template-update-")
        .tempdir_in(&templates)?;
    let checkout = temporary.path().join("checkout");
    clone_repository(&state.source, &checkout)?;
    let manifest = load_manifest(&checkout)?;
    let slug = slug_for_name(&manifest.name)?;
    if slug != current.slug {
        bail!(
            "Template update changed its name from `{}` to `{slug}`",
            current.slug
        );
    }
    let commit = git_commit(&checkout)?;
    if commit == current.commit {
        sync_assets(home, &current, Some(&current))?;
        write_state(
            home,
            &slug,
            &TemplateState {
                last_checked: now(),
                ..state
            },
        )?;
        return Ok(current);
    }

    let backup = templates.join(format!(".{}-previous", current.slug));
    if backup.exists() {
        fs::remove_dir_all(&backup)?;
    }
    fs::rename(&current.root, &backup)?;
    if let Err(error) = fs::rename(&checkout, &current.root) {
        let _ = fs::rename(&backup, &current.root);
        return Err(error).context("Failed to activate updated template");
    }
    let updated = InstalledTemplate {
        slug: current.slug.clone(),
        root: current.root.clone(),
        manifest,
        source: state.source.clone(),
        commit: commit.clone(),
    };
    if let Err(error) = sync_assets(home, &updated, Some(&current)) {
        let _ = fs::remove_dir_all(&updated.root);
        let _ = fs::rename(&backup, &current.root);
        return Err(error);
    }
    if let Err(error) = write_state(
        home,
        &updated.slug,
        &TemplateState {
            source: state.source,
            commit,
            last_checked: now(),
        },
    ) {
        let _ = fs::remove_dir_all(&updated.root);
        let _ = fs::rename(&backup, &current.root);
        let rollback = sync_assets(home, &current, Some(&updated));
        return match rollback {
            Ok(()) => Err(error),
            Err(rollback) => Err(error).context(format!(
                "Template update rollback also failed: {rollback:#}"
            )),
        };
    }
    fs::remove_dir_all(&backup)?;
    Ok(updated)
}

fn remove_from(home: &Path, name: &str) -> Result<()> {
    let installed = load_from(home, name)?;
    validate_asset_ownership(home, &installed, Some(&installed))?;
    remove_launchers(home, &installed)?;
    for skill in &installed.manifest.skills {
        let source = resolve_owned_path(&installed.root, skill, "skill")?;
        let Some(name) = source.file_name() else {
            continue;
        };
        let target = home.join(".claude").join("skills").join(name);
        if target.exists() {
            ensure_skill_owned(&target, &installed.slug)?;
            fs::remove_dir_all(target)?;
        }
    }
    fs::remove_dir_all(installed.root)?;
    let state = state_path(home, &installed.slug);
    if state.exists() {
        fs::remove_file(state)?;
    }
    Ok(())
}

fn load_manifest(root: &Path) -> Result<TemplateManifest> {
    let path = root.join(MANIFEST_FILE);
    let bytes = fs::read(&path)
        .with_context(|| format!("Failed to read template manifest {}", path.display()))?;
    let manifest: TemplateManifest = serde_json::from_slice(&bytes)
        .with_context(|| format!("Invalid template manifest {}", path.display()))?;
    validate_manifest(root, &manifest)?;
    Ok(manifest)
}

fn validate_manifest(root: &Path, manifest: &TemplateManifest) -> Result<()> {
    slug_for_name(&manifest.name)?;
    if !matches!(manifest.framework.as_str(), "react" | "vue" | "html") {
        bail!("Template framework must be react, vue, or html");
    }
    let template = resolve_owned_path(root, &manifest.template, "template directory")?;
    if !template.is_dir() {
        bail!(
            "Template directory is not a directory: {}",
            template.display()
        );
    }
    for required in ["package.json", "lxapp.json"] {
        if !template.join(required).is_file() {
            bail!("Template directory is missing {required}");
        }
    }
    for (name, entry) in &manifest.commands {
        validate_command_name(name)?;
        validate_entry(root, entry, "command")?;
    }
    if let Some(lifecycle) = &manifest.create {
        validate_entry(root, &lifecycle.command, "create entry")?;
        if lifecycle.args.iter().any(|arg| arg.contains('\0')) {
            bail!("Template create arguments must not contain NUL bytes");
        }
    }
    for skill in &manifest.skills {
        let skill = resolve_owned_path(root, skill, "skill")?;
        if !skill.join("SKILL.md").is_file() {
            bail!(
                "Template skill is missing {}",
                skill.join("SKILL.md").display()
            );
        }
    }
    if let Some(app_id) = &manifest.defaults.app_id
        && !app_id.contains("{{PROJECT_NAME}}")
    {
        bail!("defaults.appId must contain {{PROJECT_NAME}}");
    }
    Ok(())
}

fn validate_entry(root: &Path, entry: &Path, label: &str) -> Result<()> {
    let entry = resolve_owned_path(root, entry, label)?;
    if !entry.is_file() {
        bail!("Template {label} is not a file: {}", entry.display());
    }
    Ok(())
}

fn resolve_owned_path(root: &Path, relative: &Path, label: &str) -> Result<PathBuf> {
    if relative.is_absolute() {
        bail!("Template {label} must be relative to the repository root");
    }
    let root = root.canonicalize()?;
    let path = root
        .join(relative)
        .canonicalize()
        .with_context(|| format!("Failed to resolve template {label}: {}", relative.display()))?;
    if !path.starts_with(&root) {
        bail!("Template {label} escapes the repository root");
    }
    Ok(path)
}

fn sync_assets(
    home: &Path,
    template: &InstalledTemplate,
    previous: Option<&InstalledTemplate>,
) -> Result<()> {
    validate_asset_ownership(home, template, previous)?;

    let transaction_root = home.join(".lingxia");
    fs::create_dir_all(&transaction_root)?;
    let backup = tempfile::Builder::new()
        .prefix(".template-assets-")
        .tempdir_in(&transaction_root)?;
    let targets = asset_targets(home, template, previous)?;
    let mut saved = Vec::new();
    for (index, target) in targets.iter().enumerate() {
        if target.exists() {
            let saved_path = backup.path().join(index.to_string());
            if let Err(error) = fs::rename(target, &saved_path) {
                for (original, saved) in saved.into_iter().rev() {
                    let _ = fs::rename(saved, original);
                }
                return Err(error).with_context(|| {
                    format!("Failed to stage template asset {}", target.display())
                });
            }
            saved.push((target.clone(), saved_path));
        }
    }

    let result = install_launchers(home, template).and_then(|_| install_skills(home, template));
    if let Err(error) = result {
        let mut rollback_error = None;
        for target in &targets {
            let removal = if target.is_dir() {
                fs::remove_dir_all(target)
            } else if target.exists() {
                fs::remove_file(target)
            } else {
                Ok(())
            };
            if let Err(error) = removal {
                rollback_error.get_or_insert(error);
            }
        }
        for (target, saved_path) in saved {
            if let Some(parent) = target.parent()
                && let Err(error) = fs::create_dir_all(parent)
            {
                rollback_error.get_or_insert(error);
                continue;
            }
            if let Err(error) = fs::rename(saved_path, target) {
                rollback_error.get_or_insert(error);
            }
        }
        if let Some(rollback_error) = rollback_error {
            return Err(error).context(format!(
                "Template asset rollback also failed: {rollback_error}"
            ));
        }
        return Err(error);
    }
    Ok(())
}

fn asset_targets(
    home: &Path,
    template: &InstalledTemplate,
    previous: Option<&InstalledTemplate>,
) -> Result<Vec<PathBuf>> {
    let mut targets = BTreeMap::<PathBuf, ()>::new();
    for candidate in previous.into_iter().chain(std::iter::once(template)) {
        let bin = home.join(".local").join("bin");
        for name in candidate.manifest.commands.keys() {
            targets.insert(launcher_path(&bin, name), ());
        }
        for skill in &candidate.manifest.skills {
            let name = skill
                .file_name()
                .ok_or_else(|| anyhow!("Template skill has no directory name"))?;
            targets.insert(home.join(".claude").join("skills").join(name), ());
        }
    }
    Ok(targets.into_keys().collect())
}

fn validate_asset_ownership(
    home: &Path,
    template: &InstalledTemplate,
    previous: Option<&InstalledTemplate>,
) -> Result<()> {
    for candidate in previous.into_iter().chain(std::iter::once(template)) {
        let bin = home.join(".local").join("bin");
        for name in candidate.manifest.commands.keys() {
            let path = launcher_path(&bin, name);
            if path.exists()
                && !fs::read_to_string(&path)
                    .unwrap_or_default()
                    .contains(&launcher_marker(&template.slug))
            {
                bail!(
                    "Cannot manage `{name}` because {} is not owned by template `{}`",
                    path.display(),
                    template.slug
                );
            }
        }
        for skill in &candidate.manifest.skills {
            let name = skill
                .file_name()
                .ok_or_else(|| anyhow!("Template skill has no directory name"))?;
            let target = home.join(".claude").join("skills").join(name);
            if target.exists() {
                ensure_skill_owned(&target, &template.slug)?;
            }
        }
    }
    Ok(())
}

fn install_launchers(home: &Path, template: &InstalledTemplate) -> Result<()> {
    let bin = home.join(".local").join("bin");
    fs::create_dir_all(&bin)?;
    for (name, entry) in &template.manifest.commands {
        let entry = resolve_owned_path(&template.root, entry, "command")?;
        let path = launcher_path(&bin, name);
        if path.exists() {
            let existing = fs::read_to_string(&path).unwrap_or_default();
            if !existing.contains(&launcher_marker(&template.slug)) {
                bail!(
                    "Cannot register `{name}` because {} is not managed by template `{}`",
                    path.display(),
                    template.slug
                );
            }
        }
        let contents = launcher_contents(&template.slug, &entry);
        let temporary = path.with_extension("lingxia-template-new");
        fs::write(&temporary, contents)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&temporary, fs::Permissions::from_mode(0o755))?;
        }
        fs::rename(temporary, path)?;
    }
    Ok(())
}

fn remove_launchers(home: &Path, template: &InstalledTemplate) -> Result<()> {
    let bin = home.join(".local").join("bin");
    for name in template.manifest.commands.keys() {
        let path = launcher_path(&bin, name);
        if path.exists()
            && fs::read_to_string(&path)
                .unwrap_or_default()
                .contains(&launcher_marker(&template.slug))
        {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn launcher_path(bin: &Path, name: &str) -> PathBuf {
    #[cfg(windows)]
    {
        return bin.join(format!("{name}.cmd"));
    }
    #[cfg(not(windows))]
    bin.join(name)
}

fn launcher_marker(slug: &str) -> String {
    format!("managed by lingxia template {slug}")
}

fn launcher_contents(slug: &str, entry: &Path) -> String {
    #[cfg(windows)]
    {
        let invocation = if is_javascript_entry(entry) {
            format!("node \"{}\" %*", entry.display())
        } else {
            format!("\"{}\" %*", entry.display())
        };
        return format!(
            "@echo off\r\nrem {}\r\n{}\r\n",
            launcher_marker(slug),
            invocation
        );
    }
    #[cfg(not(windows))]
    {
        let executable = shell_quote(&entry.to_string_lossy());
        let invocation = if is_javascript_entry(entry) {
            format!("node {executable}")
        } else {
            executable
        };
        format!(
            "#!/bin/sh\n# {}\nexec {} \"$@\"\n",
            launcher_marker(slug),
            invocation
        )
    }
}

#[cfg(not(windows))]
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn install_skills(home: &Path, template: &InstalledTemplate) -> Result<()> {
    let skills_root = home.join(".claude").join("skills");
    fs::create_dir_all(&skills_root)?;
    for skill in &template.manifest.skills {
        let source = resolve_owned_path(&template.root, skill, "skill")?;
        let name = source
            .file_name()
            .ok_or_else(|| anyhow!("Template skill has no directory name"))?;
        let target = skills_root.join(name);
        let temporary = skills_root.join(format!(".{}.lingxia-new", name.to_string_lossy()));
        let backup = skills_root.join(format!(".{}.lingxia-previous", name.to_string_lossy()));
        if temporary.exists() {
            fs::remove_dir_all(&temporary)?;
        }
        if backup.exists() {
            fs::remove_dir_all(&backup)?;
        }
        copy_directory(&source, &temporary)?;
        fs::write(
            temporary.join(SKILL_OWNER_FILE),
            format!("{}\n", template.slug),
        )?;
        if target.exists() {
            fs::rename(&target, &backup)?;
        }
        if let Err(error) = fs::rename(&temporary, &target) {
            if backup.exists() {
                let _ = fs::rename(&backup, &target);
            }
            return Err(error).context("Failed to activate template skill");
        }
        if backup.exists() {
            fs::remove_dir_all(backup)?;
        }
    }
    Ok(())
}

fn ensure_skill_owned(path: &Path, slug: &str) -> Result<()> {
    let owner = fs::read_to_string(path.join(SKILL_OWNER_FILE)).unwrap_or_default();
    if owner.trim() != slug {
        bail!(
            "Cannot manage template skill {} because it is not owned by template `{slug}`",
            path.display()
        );
    }
    Ok(())
}

fn copy_directory(source: &Path, target: &Path) -> Result<()> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_directory(&source_path, &target_path)?;
        } else {
            fs::copy(&source_path, &target_path)?;
        }
    }
    Ok(())
}

fn command_for_entry(entry: &Path) -> Command {
    if is_javascript_entry(entry) {
        let mut command = Command::new("node");
        command.arg(entry);
        command
    } else {
        Command::new(entry)
    }
}

fn is_javascript_entry(entry: &Path) -> bool {
    matches!(
        entry.extension().and_then(|extension| extension.to_str()),
        Some("js" | "mjs" | "cjs")
    )
}

fn clone_repository(source: &str, target: &Path) -> Result<()> {
    let status = Command::new("git")
        .args(["clone", "--quiet", "--depth", "1", source])
        .arg(target)
        .status()
        .context("Failed to start git; install Git and retry")?;
    if !status.success() {
        bail!("git clone failed with {status}");
    }
    Ok(())
}

fn git_commit(root: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["-C"])
        .arg(root)
        .args(["rev-parse", "HEAD"])
        .output()
        .context("Failed to inspect template Git commit")?;
    if !output.status.success() {
        bail!("Unable to read template Git commit");
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

fn git_remote_commit(source: &str) -> Result<String> {
    let mut child = Command::new("git")
        .args([
            "-c",
            "credential.interactive=false",
            "-c",
            "core.sshCommand=ssh -o BatchMode=yes -o ConnectTimeout=5",
            "ls-remote",
            source,
            "HEAD",
        ])
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("Failed to check the template remote")?;
    let deadline = std::time::Instant::now() + REMOTE_CHECK_TIMEOUT;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            bail!("Template remote check timed out after 10 seconds");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let mut stdout = String::new();
    if let Some(mut pipe) = child.stdout.take() {
        pipe.read_to_string(&mut stdout)?;
    }
    if !status.success() {
        bail!("Unable to check the template remote");
    }
    stdout
        .split_whitespace()
        .next()
        .map(str::to_string)
        .filter(|commit| !commit.is_empty())
        .ok_or_else(|| anyhow!("Template remote did not advertise HEAD"))
}

fn normalize_source(source: &str) -> Result<String> {
    let path = Path::new(source);
    if path.exists() {
        return Ok(path
            .canonicalize()
            .with_context(|| format!("Failed to resolve template source {source}"))?
            .to_string_lossy()
            .into_owned());
    }
    let http_source = source.split_once("://").is_some_and(|(scheme, _)| {
        scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")
    });
    if http_source {
        let url = url::Url::parse(source).context("Invalid HTTP template Git URL")?;
        if !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            bail!(
                "Template Git URLs must not contain credentials, query parameters, or fragments; use a Git credential helper or SSH"
            );
        }
    }
    Ok(source.to_string())
}

fn read_state(home: &Path, slug: &str) -> Result<TemplateState> {
    let path = state_path(home, slug);
    let bytes = fs::read(&path)
        .with_context(|| format!("Failed to read template state {}", path.display()))?;
    serde_json::from_slice(&bytes)
        .with_context(|| format!("Invalid template state {}", path.display()))
}

fn write_state(home: &Path, slug: &str, state: &TemplateState) -> Result<()> {
    let root = states_root(home);
    fs::create_dir_all(&root)?;
    let path = state_path(home, slug);
    let temporary = path.with_extension("json.new");
    let bytes = serde_json::to_vec_pretty(state)?;
    fs::write(&temporary, [bytes, b"\n".to_vec()].concat())?;
    replace_file(&temporary, &path)?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn replace_file(source: &Path, target: &Path) -> std::io::Result<()> {
    fs::rename(source, target)
}

#[cfg(target_os = "windows")]
fn replace_file(source: &Path, target: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt as _;
    use windows::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };
    use windows::core::PCWSTR;

    let source = source
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let target = target
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    unsafe {
        MoveFileExW(
            PCWSTR(source.as_ptr()),
            PCWSTR(target.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(std::io::Error::other)
}

fn slug_for_name(name: &str) -> Result<String> {
    let slug = name.trim().to_ascii_lowercase().replace(' ', "-");
    if slug.is_empty()
        || !slug
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_lowercase())
        || !slug.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
    {
        bail!(
            "Template name must start with a letter and use only letters, numbers, spaces, or hyphens"
        );
    }
    Ok(slug)
}

fn validate_command_name(name: &str) -> Result<()> {
    if slug_for_name(name)? != name {
        bail!("Template command `{name}` must be lowercase kebab-case");
    }
    Ok(())
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn short_commit(commit: &str) -> &str {
    commit.get(..7).unwrap_or(commit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn git(root: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(root)
            .status()
            .unwrap();
        assert!(status.success());
    }

    fn write_manifest(root: &Path) {
        fs::create_dir_all(root.join("template")).unwrap();
        fs::write(root.join("template/package.json"), "{}").unwrap();
        fs::write(root.join("template/lxapp.json"), "{}").unwrap();
        fs::write(
            root.join(MANIFEST_FILE),
            r#"{
  "name": "Example Kit",
  "template": "template",
  "defaults": { "appId": "com.example.{{PROJECT_NAME}}" }
}"#,
        )
        .unwrap();
    }

    #[test]
    fn validates_minimal_manifest() {
        let root = tempdir().unwrap();
        write_manifest(root.path());
        let manifest = load_manifest(root.path()).unwrap();
        assert_eq!(manifest.name, "Example Kit");
        assert_eq!(manifest.framework, "react");
        assert_eq!(slug_for_name(&manifest.name).unwrap(), "example-kit");
    }

    #[test]
    fn rejects_manifest_paths_outside_repository() {
        let root = tempdir().unwrap();
        write_manifest(root.path());
        let manifest = TemplateManifest {
            name: "Unsafe".to_string(),
            template: PathBuf::from(".."),
            framework: "react".to_string(),
            commands: BTreeMap::new(),
            skills: Vec::new(),
            create: None,
            defaults: TemplateDefaults::default(),
        };
        assert!(validate_manifest(root.path(), &manifest).is_err());
    }

    #[test]
    fn rejects_http_sources_that_would_persist_secrets() {
        assert!(normalize_source("https://token@example.com/template.git").is_err());
        assert!(normalize_source("HTTPS://token@example.com/template.git").is_err());
        assert!(normalize_source("https://example.com/template.git?token=secret").is_err());
        assert_eq!(
            normalize_source("https://example.com/template.git").unwrap(),
            "https://example.com/template.git"
        );
    }

    #[test]
    fn launcher_keeps_arguments_separate() {
        let contents = launcher_contents("example", Path::new("/tmp/example kit/tool.mjs"));
        assert!(contents.contains("\"$@\""));
        assert!(contents.contains("managed by lingxia template example"));
    }

    #[test]
    fn asset_collision_preserves_user_files_and_installs_nothing() {
        let root = tempdir().unwrap();
        fs::create_dir_all(root.path().join("template")).unwrap();
        fs::create_dir_all(root.path().join("bin")).unwrap();
        fs::create_dir_all(root.path().join("skills/example")).unwrap();
        fs::write(root.path().join("template/package.json"), "{}").unwrap();
        fs::write(root.path().join("template/lxapp.json"), "{}").unwrap();
        fs::write(root.path().join("bin/example.mjs"), "").unwrap();
        fs::write(root.path().join("skills/example/SKILL.md"), "provider\n").unwrap();
        fs::write(
            root.path().join(MANIFEST_FILE),
            r#"{
  "name": "Example",
  "template": "template",
  "commands": { "example": "bin/example.mjs" },
  "skills": ["skills/example"]
}"#,
        )
        .unwrap();
        let home = tempdir().unwrap();
        let user_skill = home.path().join(".claude/skills/example");
        fs::create_dir_all(&user_skill).unwrap();
        fs::write(user_skill.join("SKILL.md"), "user\n").unwrap();
        let template = InstalledTemplate {
            slug: "example".to_owned(),
            root: root.path().to_path_buf(),
            manifest: load_manifest(root.path()).unwrap(),
            source: "test".to_owned(),
            commit: "test".to_owned(),
        };

        assert!(sync_assets(home.path(), &template, None).is_err());
        assert_eq!(
            fs::read_to_string(user_skill.join("SKILL.md")).unwrap(),
            "user\n"
        );
        assert!(!home.path().join(".local/bin/example").exists());
    }

    #[test]
    fn installs_updates_and_removes_git_provider() {
        let source = tempdir().unwrap();
        fs::create_dir_all(source.path().join("template")).unwrap();
        fs::create_dir_all(source.path().join("bin")).unwrap();
        fs::create_dir_all(source.path().join("skills/example")).unwrap();
        fs::write(source.path().join("template/package.json"), "{}").unwrap();
        fs::write(source.path().join("template/lxapp.json"), "{}").unwrap();
        fs::write(
            source.path().join("bin/example.mjs"),
            "#!/usr/bin/env node\n",
        )
        .unwrap();
        fs::write(source.path().join("skills/example/SKILL.md"), "first\n").unwrap();
        fs::write(
            source.path().join(MANIFEST_FILE),
            r#"{
  "name": "Example",
  "template": "template",
  "commands": { "example": "bin/example.mjs" },
  "skills": ["skills/example"]
}"#,
        )
        .unwrap();
        git(source.path(), &["init", "-q", "-b", "main"]);
        git(source.path(), &["config", "user.email", "test@example.com"]);
        git(source.path(), &["config", "user.name", "Test"]);
        git(source.path(), &["add", "."]);
        git(source.path(), &["commit", "-q", "-m", "initial"]);

        let home = tempdir().unwrap();
        let first = add_from(home.path(), source.path().to_str().unwrap()).unwrap();
        assert!(home.path().join(".local/bin/example").is_file());
        assert_eq!(
            fs::read_to_string(home.path().join(".claude/skills/example/SKILL.md")).unwrap(),
            "first\n"
        );

        fs::write(source.path().join("skills/example/SKILL.md"), "second\n").unwrap();
        git(source.path(), &["add", "."]);
        git(source.path(), &["commit", "-q", "-m", "update"]);
        let second = update_from(home.path(), "example", true).unwrap();
        assert_ne!(first.commit, second.commit);
        assert_eq!(
            fs::read_to_string(home.path().join(".claude/skills/example/SKILL.md")).unwrap(),
            "second\n"
        );

        fs::write(
            source.path().join(MANIFEST_FILE),
            r#"{
  "name": "Example",
  "template": "template"
}"#,
        )
        .unwrap();
        git(source.path(), &["add", "."]);
        git(source.path(), &["commit", "-q", "-m", "remove assets"]);
        update_from(home.path(), "example", true).unwrap();
        assert!(!home.path().join(".local/bin/example").exists());
        assert!(!home.path().join(".claude/skills/example").exists());

        remove_from(home.path(), "example").unwrap();
        assert!(!home.path().join(".lingxia/templates/example").exists());
        assert!(!home.path().join(".local/bin/example").exists());
        assert!(!home.path().join(".claude/skills/example").exists());
    }
}
