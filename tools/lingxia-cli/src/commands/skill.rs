//! Install the LingXia agent skill from this binary.
//!
//! The skill describes what this CLI can do, so it ships inside the CLI rather
//! than as a package fetched separately: an installed copy always came from the
//! binary that wrote it, and cannot describe a version the CLI is not.

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use include_dir::{Dir, include_dir};
use std::fs;
use std::path::{Path, PathBuf};

static EMBEDDED_SKILL: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../docs/skill");

/// Directory name the agent tooling expects inside a skills root.
const SKILL_DIR_NAME: &str = "lingxia";
/// Records which CLI wrote the installed skill, so a later CLI can tell that
/// the copy on disk is not its own and rewrite it.
pub const MANIFEST_NAME: &str = "skill-manifest.json";

#[derive(Subcommand)]
pub enum SkillAction {
    /// Write the skill into a project, the home directory, or a chosen path
    Install {
        /// Install for every project under the home directory
        #[arg(long)]
        user: bool,

        /// Skills root to install into; the skill lands in <PATH>/lingxia
        #[arg(long, value_name = "PATH", conflicts_with = "user")]
        target: Option<PathBuf>,

        /// Also write an AGENTS.md at the project root pointing at the skill
        #[arg(long)]
        agents_md: bool,

        /// Install from a skill directory on disk instead of the embedded copy.
        /// Editing the skill does not require rebuilding the CLI.
        #[arg(long, value_name = "PATH")]
        from: Option<PathBuf>,

        /// Report what would be written without touching the filesystem
        #[arg(long, short = 'n')]
        dry_run: bool,
    },

    /// Print where an installed skill is and which version wrote it
    Status {
        /// Inspect the home-directory install instead of this project's
        #[arg(long)]
        user: bool,

        /// Skills root to inspect; the skill is read from <PATH>/lingxia
        #[arg(long, value_name = "PATH", conflicts_with = "user")]
        target: Option<PathBuf>,
    },
}

pub fn execute(action: SkillAction) -> Result<()> {
    match action {
        SkillAction::Install {
            user,
            target,
            agents_md,
            from,
            dry_run,
        } => install(user, target, agents_md, from, dry_run),
        SkillAction::Status { user, target } => status(user, target),
    }
}

/// Resolve the directory the skill itself lives in (`.../lingxia`).
fn resolve_destination(user: bool, target: Option<PathBuf>) -> Result<PathBuf> {
    let root = match (user, target) {
        (_, Some(path)) => path,
        (true, None) => home_dir()?.join(".claude").join("skills"),
        (false, None) => std::env::current_dir()
            .context("Failed to read the current directory")?
            .join(".claude")
            .join("skills"),
    };
    Ok(root.join(SKILL_DIR_NAME))
}

fn home_dir() -> Result<PathBuf> {
    dirs::home_dir().context("Could not resolve the home directory")
}

fn install(
    user: bool,
    target: Option<PathBuf>,
    agents_md: bool,
    from: Option<PathBuf>,
    dry_run: bool,
) -> Result<()> {
    let dest = resolve_destination(user, target)?;
    let (source, files) = match &from {
        Some(dir) => (dir.display().to_string(), read_skill_dir(dir)?),
        None => (
            format!("this CLI ({})", env!("CARGO_PKG_VERSION")),
            collect_files(&EMBEDDED_SKILL)
                .into_iter()
                .map(|(path, contents)| (path, contents.to_vec()))
                .collect(),
        ),
    };
    if files.is_empty() {
        bail!("No skill files found in {source}.");
    }

    if dry_run {
        println!("Would write {} file(s) to {}", files.len(), dest.display());
        for (path, _) in &files {
            println!("  {}", path.display());
        }
        if agents_md {
            println!("Would write {}", agents_md_path()?.display());
        }
        return Ok(());
    }

    write_skill(&dest, &files)?;

    println!(
        "Installed the LingXia skill ({} files) from {} to {}",
        files.len(),
        source,
        dest.display()
    );

    if agents_md {
        write_agents_pointer(&dest)?;
    }
    Ok(())
}

/// Rewrite a skill this CLI did not write. Returns the version it replaced.
///
/// The skill describes this binary's capabilities, so a copy left by an older
/// CLI describes commands and APIs that may no longer exist. Self-update
/// replaces the binary but cannot refresh the skill in the same run -- the
/// process is still executing the old code -- so the new binary does it here.
pub fn refresh_if_stale(dest: &Path) -> Result<Option<String>> {
    if !dest.join("SKILL.md").is_file() {
        return Ok(None);
    }
    let installed = installed_version(dest);
    if installed.as_deref() == Some(env!("CARGO_PKG_VERSION")) {
        return Ok(None);
    }
    write_skill(
        dest,
        &collect_files(&EMBEDDED_SKILL)
            .into_iter()
            .map(|(path, contents)| (path, contents.to_vec()))
            .collect::<Vec<_>>(),
    )?;
    Ok(Some(installed.unwrap_or_else(|| "unknown".to_string())))
}

/// The home-directory skills root, where an install shared by every project
/// lives.
pub fn user_destination() -> Result<PathBuf> {
    Ok(home_dir()?
        .join(".claude")
        .join("skills")
        .join(SKILL_DIR_NAME))
}

fn installed_version(dest: &Path) -> Option<String> {
    let text = fs::read_to_string(dest.join(MANIFEST_NAME)).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    value.get("version")?.as_str().map(str::to_string)
}

/// Replace the destination wholesale: a file dropped from the skill must not
/// survive in an install that claims to be this version.
fn write_skill(dest: &Path, files: &[(PathBuf, Vec<u8>)]) -> Result<()> {
    if dest.exists() {
        fs::remove_dir_all(dest).with_context(|| format!("Failed to clear {}", dest.display()))?;
    }
    for (path, contents) in files {
        let out = dest.join(path);
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }
        fs::write(&out, contents).with_context(|| format!("Failed to write {}", out.display()))?;
    }
    let manifest = serde_json::json!({ "version": env!("CARGO_PKG_VERSION") });
    fs::write(
        dest.join(MANIFEST_NAME),
        format!("{}\n", serde_json::to_string_pretty(&manifest)?),
    )
    .with_context(|| format!("Failed to write the manifest in {}", dest.display()))?;
    Ok(())
}

fn status(user: bool, target: Option<PathBuf>) -> Result<()> {
    let dest = resolve_destination(user, target)?;
    let entry = dest.join("SKILL.md");
    if !entry.is_file() {
        println!("No skill installed at {}", dest.display());
        println!("Run `lingxia skill install` to write it there.");
        return Ok(());
    }
    println!("Skill installed at {}", dest.display());
    println!(
        "Written by CLI: {}",
        installed_version(&dest).unwrap_or_else(|| "unknown".to_string())
    );
    println!("This CLI: {}", env!("CARGO_PKG_VERSION"));
    Ok(())
}

fn agents_md_path() -> Result<PathBuf> {
    Ok(std::env::current_dir()
        .context("Failed to read the current directory")?
        .join("AGENTS.md"))
}

/// Point tools that read a single root file at the installed skill.
fn write_agents_pointer(dest: &Path) -> Result<()> {
    let path = agents_md_path()?;
    if path.exists() {
        println!("Left the existing {} alone.", path.display());
        return Ok(());
    }
    let body = format!(
        "# Agent guide\n\nThis project builds on LingXia. Read [{}]({}) first.\n",
        "the LingXia skill",
        dest.join("SKILL.md").display()
    );
    fs::write(&path, body).with_context(|| format!("Failed to write {}", path.display()))?;
    println!("Wrote {}", path.display());
    Ok(())
}

/// Read a skill directory from disk, mirroring the embedded layout.
fn read_skill_dir(dir: &Path) -> Result<Vec<(PathBuf, Vec<u8>)>> {
    if !dir.join("SKILL.md").is_file() {
        bail!(
            "{} does not look like a skill directory (no SKILL.md).",
            dir.display()
        );
    }
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let entries = fs::read_dir(&current)
            .with_context(|| format!("Failed to read {}", current.display()))?;
        for entry in entries {
            let path = entry
                .with_context(|| format!("Failed to read an entry in {}", current.display()))?
                .path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let rel = path
                .strip_prefix(dir)
                .expect("walked paths stay under the root")
                .to_path_buf();
            let contents =
                fs::read(&path).with_context(|| format!("Failed to read {}", path.display()))?;
            out.push((rel, contents));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// Flatten the embedded tree into (relative path, contents) pairs.
fn collect_files<'a>(dir: &'a Dir<'a>) -> Vec<(PathBuf, &'a [u8])> {
    let mut out = Vec::new();
    for file in dir.files() {
        out.push((file.path().to_path_buf(), file.contents()));
    }
    for sub in dir.dirs() {
        out.extend(collect_files(sub));
    }
    out
}
