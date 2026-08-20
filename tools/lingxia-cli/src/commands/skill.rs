//! Install the LingXia agent skill from this binary.
//!
//! The skill describes what this CLI can do, so it ships inside the CLI rather
//! than as a package fetched separately: an installed copy always came from the
//! binary that wrote it, and cannot describe a version the CLI is not.

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use include_dir::{Dir, include_dir};
use semver::Version;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

static EMBEDDED_SKILL: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../docs/skill");

/// Directory name the agent tooling expects inside a skills root.
const SKILL_DIR_NAME: &str = "lingxia";
/// Records which CLI wrote the installed skill, so a later CLI can tell that
/// the copy on disk is not its own and rewrite it.
pub const MANIFEST_NAME: &str = "skill-manifest.json";

/// Where an installed copy came from. Recorded so `status` reports the truth
/// and so the auto-refresh leaves a deliberately overridden install alone.
enum Source {
    /// The copy compiled into this binary.
    Embedded,
    /// A directory on disk, for editing the skill without rebuilding the CLI.
    Directory(PathBuf),
}

impl Source {
    fn manifest(&self) -> serde_json::Value {
        match self {
            Source::Embedded => serde_json::json!({
                "version": env!("CARGO_PKG_VERSION"),
                "source": "cli",
            }),
            Source::Directory(path) => serde_json::json!({
                "version": env!("CARGO_PKG_VERSION"),
                "source": "directory",
                "sourcePath": path.display().to_string(),
            }),
        }
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Source::Embedded => write!(f, "this CLI ({})", env!("CARGO_PKG_VERSION")),
            Source::Directory(path) => write!(f, "{}", path.display()),
        }
    }
}

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

/// Install the embedded skill for a freshly scaffolded project: the body in the
/// home directory, a committable pointer in the project.
pub fn install_for_new_project(project_dir: &Path) -> Result<()> {
    let dest = home_dir()?
        .join(".claude")
        .join("skills")
        .join(SKILL_DIR_NAME);
    write_skill(&dest, &embedded_files(), &Source::Embedded)?;
    println!("Installed the LingXia skill to {}", dest.display());
    write_agents_pointer(project_dir, &dest)
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
    let (source, files) = match from {
        Some(dir) => {
            let files = read_skill_dir(&dir)?;
            (Source::Directory(dir), files)
        }
        None => (Source::Embedded, embedded_files()),
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
            println!(
                "Would write {}",
                project_root()?.join("AGENTS.md").display()
            );
        }
        return Ok(());
    }

    write_skill(&dest, &files, &source)?;

    println!(
        "Installed the LingXia skill ({} files) from {} to {}",
        files.len(),
        source,
        dest.display()
    );

    if agents_md {
        write_agents_pointer(&project_root()?, &dest)?;
    }
    Ok(())
}

/// Rewrite a skill an older CLI installed. Returns the version it replaced.
///
/// The skill describes this binary's capabilities, so a copy left by an older
/// CLI describes commands and APIs that may no longer exist. Self-update
/// replaces the binary but cannot refresh the skill in the same run -- the
/// process is still executing the old code -- so the new binary does it here.
///
/// Only ever moves forward: an older CLI run beside a newer one must not
/// downgrade the skill, and a copy installed from a directory is a deliberate
/// override this must not undo.
pub fn refresh_if_stale(dest: &Path) -> Result<Option<String>> {
    if !dest.join("SKILL.md").is_file() {
        return Ok(None);
    }
    let manifest = read_manifest(dest);
    if manifest
        .as_ref()
        .and_then(|m| m.get("source"))
        .and_then(serde_json::Value::as_str)
        == Some("directory")
    {
        return Ok(None);
    }
    let installed = manifest
        .as_ref()
        .and_then(|m| m.get("version"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let current =
        Version::parse(env!("CARGO_PKG_VERSION")).context("Failed to parse this CLI's version")?;
    let is_older = match installed.as_deref().map(Version::parse) {
        Some(Ok(existing)) => existing < current,
        // An unreadable or unparseable manifest predates this scheme.
        _ => true,
    };
    if !is_older {
        return Ok(None);
    }
    write_skill(dest, &embedded_files(), &Source::Embedded)?;
    Ok(Some(installed.unwrap_or_else(|| "unknown".to_string())))
}

/// Say once that a home install pinned with `--from` has fallen behind this
/// CLI. The refresh deliberately leaves such a copy alone, so without this the
/// pin is invisible to whoever set it months ago.
pub fn notify_pinned_skill() {
    let Ok(dest) = user_destination() else {
        return;
    };
    let Some(manifest) = read_manifest(&dest) else {
        return;
    };
    let field = |key: &str| manifest.get(key).and_then(serde_json::Value::as_str);
    if field("source") != Some("directory") {
        return;
    }
    if field("version") == Some(env!("CARGO_PKG_VERSION")) {
        return;
    }
    println!(
        "The installed skill is pinned to {} and predates this CLI ({}). \
         Run `lingxia skill install --user` to follow the CLI again.",
        field("sourcePath").unwrap_or("a directory"),
        env!("CARGO_PKG_VERSION")
    );
}

fn embedded_files() -> Vec<(PathBuf, Vec<u8>)> {
    collect_files(&EMBEDDED_SKILL)
        .into_iter()
        .map(|(path, contents)| (path, contents.to_vec()))
        .collect()
}

/// Whether the home directory already carries the skill.
///
/// Presence only, deliberately not the version: the body is shared by every
/// project, so the question "may I write outside this project" is asked once
/// and answered forever. An older copy is not a reason to ask again -- the
/// refresh brings it forward on its own.
pub fn user_install_exists() -> bool {
    user_destination().is_ok_and(|dest| dest.join("SKILL.md").is_file())
}

/// The home-directory skills root, where an install shared by every project
/// lives.
pub fn user_destination() -> Result<PathBuf> {
    Ok(home_dir()?
        .join(".claude")
        .join("skills")
        .join(SKILL_DIR_NAME))
}

fn read_manifest(dest: &Path) -> Option<serde_json::Value> {
    let text = fs::read_to_string(dest.join(MANIFEST_NAME)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Replace the destination wholesale, and only once it is complete.
///
/// A file dropped from the skill must not survive in an install that claims to
/// be this version, so this cannot merge into the existing directory. It stages
/// into a sibling and renames: an interrupted run leaves the previous install
/// untouched rather than an empty directory that nothing would restore, and two
/// processes racing here (`lingxia dev` beside the broker `lxdev` spawns) each
/// stage separately, so the last rename wins instead of one failing on a
/// directory the other just removed.
fn write_skill(dest: &Path, files: &[(PathBuf, Vec<u8>)], source: &Source) -> Result<()> {
    let parent = dest
        .parent()
        .context("The skill destination has no parent directory")?;
    fs::create_dir_all(parent).with_context(|| format!("Failed to create {}", parent.display()))?;
    let staged = TempDir::new_in(parent)
        .with_context(|| format!("Failed to stage the skill next to {}", dest.display()))?;

    for (path, contents) in files {
        let out = staged.path().join(path);
        if let Some(dir) = out.parent() {
            fs::create_dir_all(dir)
                .with_context(|| format!("Failed to create {}", dir.display()))?;
        }
        fs::write(&out, contents).with_context(|| format!("Failed to write {}", out.display()))?;
    }
    fs::write(
        staged.path().join(MANIFEST_NAME),
        format!("{}\n", serde_json::to_string_pretty(&source.manifest())?),
    )
    .context("Failed to write the skill manifest")?;

    // Renaming onto an existing directory fails, so retire it first. The window
    // this reopens is one rename wide, and the staged copy is already complete.
    if dest.exists() {
        let retired = staged.path().with_extension("previous");
        let _ = fs::remove_dir_all(&retired);
        fs::rename(dest, &retired).with_context(|| {
            format!(
                "Failed to move the previous skill out of {}",
                dest.display()
            )
        })?;
        let _ = fs::remove_dir_all(&retired);
    }
    fs::rename(staged.keep(), dest)
        .with_context(|| format!("Failed to move the staged skill into {}", dest.display()))?;
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
    let manifest = read_manifest(&dest);
    let field = |key: &str| {
        manifest
            .as_ref()
            .and_then(|m| m.get(key))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown")
            .to_string()
    };
    println!("Installed version: {}", field("version"));
    match field("source").as_str() {
        "directory" => println!("Installed from: {}", field("sourcePath")),
        "cli" => println!("Installed from: the CLI binary"),
        _ => println!("Installed from: unknown"),
    }
    println!("This CLI: {}", env!("CARGO_PKG_VERSION"));

    // Only the home install is refreshed automatically, so say plainly when a
    // copy this CLI will not touch has fallen behind it.
    let installed = field("version");
    if field("source") != "directory" && installed != env!("CARGO_PKG_VERSION") {
        println!();
        println!(
            "This copy is out of step with the CLI. Run `lingxia skill install{}` to rewrite it.",
            if user { " --user" } else { "" }
        );
    }
    Ok(())
}

fn project_root() -> Result<PathBuf> {
    std::env::current_dir().context("Failed to read the current directory")
}

/// Marks the block this writes, so a re-install replaces it instead of
/// appending a second copy.
const AGENTS_MARKER: &str = "<!-- lingxia skill: AGENTS.md pointer -->";

/// Point tools that read a single root file at the installed skill.
///
/// AGENTS.md is meant to be committed, so the reference must not be a path that
/// only resolves on this machine: project-relative when the skill lives inside
/// the project, `~`-relative for a home install, absolute only as a fallback.
fn write_agents_pointer(project_dir: &Path, dest: &Path) -> Result<()> {
    let path = project_dir.join("AGENTS.md");
    let block = agents_block(&portable_reference(project_dir, dest));

    let body = match fs::read_to_string(&path) {
        Ok(existing) if existing.contains(AGENTS_MARKER) => replace_block(&existing, &block),
        Ok(existing) => {
            let separator = if existing.ends_with('\n') {
                "\n"
            } else {
                "\n\n"
            };
            format!("{existing}{separator}{block}")
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            format!("# AGENTS\n\n{block}")
        }
        Err(err) => {
            return Err(err).with_context(|| format!("Failed to read {}", path.display()));
        }
    };
    fs::write(&path, body).with_context(|| format!("Failed to write {}", path.display()))?;
    println!("Pointed {} at the skill", path.display());
    Ok(())
}

/// Forward slashes regardless of platform: the reference goes into a markdown
/// link, where a Windows separator would escape rather than separate.
fn portable_reference(project_dir: &Path, dest: &Path) -> String {
    let render = |path: &Path| path.to_string_lossy().replace('\\', "/");
    if let Ok(relative) = dest.strip_prefix(project_dir) {
        return render(relative);
    }
    if let Some(home) = dirs::home_dir()
        && let Ok(relative) = dest.strip_prefix(&home)
    {
        return format!("~/{}", render(relative));
    }
    render(dest)
}

fn agents_block(skill_ref: &str) -> String {
    format!(
        "{AGENTS_MARKER}\n\
## LingXia\n\n\
This project uses the LingXia cross-platform app framework. The development\n\
skill -- decision tree, recipes, CLI / component / native API references --\n\
lives at:\n\n\
    {skill_ref}/SKILL.md\n\n\
If that file is not present, write it there with:\n\n\
    lingxia skill install --user\n\n\
Start there. Sub-references are linked from that file using relative paths.\n\
{AGENTS_MARKER}\n"
    )
}

fn replace_block(existing: &str, block: &str) -> String {
    let Some(start) = existing.find(AGENTS_MARKER) else {
        return existing.to_string();
    };
    let after = start + AGENTS_MARKER.len();
    let Some(end) = existing[after..].find(AGENTS_MARKER) else {
        return existing.to_string();
    };
    let mut end = after + end + AGENTS_MARKER.len();
    if existing[end..].starts_with('\n') {
        end += 1;
    }
    format!("{}{}{}", &existing[..start], block, &existing[end..])
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
