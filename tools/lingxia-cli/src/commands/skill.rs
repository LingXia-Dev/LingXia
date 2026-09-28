//! Keep the agent skill this binary carries in step with the copy on disk.
//!
//! The skill describes what this CLI can do, so it ships inside the CLI rather
//! than as a package fetched separately: an installed copy always came from the
//! binary that wrote it, and cannot describe a version the CLI is not.
//!
//! `lingxia new`, `lingxia upgrade` and `lingxia skill install` write it; every
//! other run reconciles a copy that exists with the one compiled in, so an edit
//! to the skill reaches the agent as soon as the CLI that carries it runs.
//!
//! `~/.agents/skills/<name>` is the one real copy. Claude Code only discovers
//! `~/.claude/skills`, so the entry there is a link to it.

use anyhow::{Context, Result};
use include_dir::{Dir, include_dir};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tempfile::TempDir;

static EMBEDDED_SKILL: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../docs/skill");

/// Directory name the agent tooling expects inside a skills root.
const SKILL_DIR_NAME: &str = "lingxia";
/// Records which skill an install holds, so a later run can tell whether the
/// copy on disk is still the one this binary carries.
const MANIFEST_NAME: &str = "skill-manifest.json";

/// What reconciling the installed copy did.
pub enum Sync {
    /// Nothing on disk, and no skills root that asks for one.
    Skipped,
    /// The copy on disk is already this binary's.
    Current,
    Created,
    /// Replaced a copy another build wrote; carries the version it claimed.
    Rewritten {
        previous: String,
    },
}

/// Reconcile the copy under the home directory with the embedded skill.
///
/// `create_if_missing` is for the moments where the user asked for the skill by
/// asking for something that contains it -- `lingxia new`, `lingxia upgrade`.
/// Every other run only corrects a copy that already exists, or writes one when
/// a skills root is already there to receive it: a machine that has never run
/// an agent does not grow a `~/.agents` because a build ran.
pub fn sync_home_skill(create_if_missing: bool) -> Result<Sync> {
    sync_for_home(&home_dir()?, create_if_missing)
}

pub(crate) fn skills_root(home: &Path) -> PathBuf {
    home.join(".agents").join("skills")
}

/// Where Claude Code discovers user skills.
pub(crate) fn claude_skills_root(home: &Path) -> PathBuf {
    home.join(".claude").join("skills")
}

fn sync_for_home(home: &Path, create_if_missing: bool) -> Result<Sync> {
    let dest = skills_root(home).join(SKILL_DIR_NAME);
    let claude = claude_skills_root(home).join(SKILL_DIR_NAME);
    let result = sync(
        &dest,
        create_if_missing || claude.join("SKILL.md").is_file(),
    )?;
    if !matches!(result, Sync::Skipped) {
        // A real directory there is a copy an older CLI wrote; a live link
        // elsewhere is the user's own choice.
        link_for_claude(home, &dest, |path| {
            !fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink())
        })?;
    }
    Ok(result)
}

/// Make `~/.claude/skills/<name>` resolve to the canonical copy at `canonical`,
/// when Claude Code is on this machine. An entry that already resolves there
/// (this link, or a `~/.claude/skills` that is itself a link into
/// `~/.agents/skills`) is left alone; another entry is replaced only when
/// `replaceable` says it is an old copy the CLI owns.
pub(crate) fn link_for_claude(
    home: &Path,
    canonical: &Path,
    replaceable: impl Fn(&Path) -> bool,
) -> Result<()> {
    let claude = home.join(".claude");
    let Some(name) = canonical.file_name() else {
        return Ok(());
    };
    if !claude.is_dir() {
        return Ok(());
    }
    let link = claude_skills_root(home).join(name);
    let target = fs::canonicalize(canonical)
        .with_context(|| format!("Failed to resolve {}", canonical.display()))?;
    if resolves_to(&link, &target) {
        return Ok(());
    }
    match fs::symlink_metadata(&link) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| format!("Failed to inspect {}", link.display()));
        }
        Ok(metadata) if metadata.file_type().is_symlink() => {
            // Dangling, or pointing at a copy the CLI owns elsewhere.
            if link.exists() && !replaceable(&link) {
                return Ok(());
            }
            remove_link(&link)?;
        }
        Ok(metadata) if metadata.is_dir() => {
            if !replaceable(&link) {
                return Ok(());
            }
            // The copy fallback of a Windows machine without directory links.
            if cfg!(windows) && tree_digest(&link).ok() == Some(tree_digest(canonical)?) {
                return Ok(());
            }
            fs::remove_dir_all(&link).with_context(|| {
                format!("Failed to replace the old skill at {}", link.display())
            })?;
        }
        Ok(_) => return Ok(()),
    }
    let parent = link
        .parent()
        .context("The Claude skill link has no parent")?;
    fs::create_dir_all(parent).with_context(|| format!("Failed to create {}", parent.display()))?;
    create_dir_link(canonical, &link)
        .with_context(|| format!("Failed to expose the skill at {}", link.display()))
}

/// Remove `~/.claude/skills/<name>` when it is the link to `canonical`, or an
/// old copy `replaceable` accepts. Never touches the canonical copy itself.
pub(crate) fn unlink_for_claude(
    home: &Path,
    canonical: &Path,
    replaceable: impl Fn(&Path) -> bool,
) -> Result<()> {
    let Some(name) = canonical.file_name() else {
        return Ok(());
    };
    let link = claude_skills_root(home).join(name);
    let Ok(metadata) = fs::symlink_metadata(&link) else {
        return Ok(());
    };
    let target = fs::canonicalize(canonical).ok();
    if metadata.file_type().is_symlink() {
        if target.is_some_and(|target| resolves_to(&link, &target)) || replaceable(&link) {
            remove_link(&link)?;
        }
        return Ok(());
    }
    // Reached through a linked `~/.claude/skills`: this is the canonical copy.
    if target.is_some_and(|target| resolves_to(&link, &target)) {
        return Ok(());
    }
    if metadata.is_dir() && replaceable(&link) {
        fs::remove_dir_all(&link)
            .with_context(|| format!("Failed to remove the old skill at {}", link.display()))?;
    }
    Ok(())
}

fn resolves_to(path: &Path, target: &Path) -> bool {
    fs::canonicalize(path).is_ok_and(|resolved| resolved == target)
}

fn remove_link(link: &Path) -> Result<()> {
    // A directory symlink or junction is a directory entry on Windows.
    #[cfg(windows)]
    let removed = fs::remove_dir(link).or_else(|_| fs::remove_file(link));
    #[cfg(not(windows))]
    let removed = fs::remove_file(link);
    removed.with_context(|| format!("Failed to remove the link at {}", link.display()))
}

#[cfg(unix)]
fn create_dir_link(target: &Path, link: &Path) -> Result<()> {
    std::os::unix::fs::symlink(target, link)?;
    Ok(())
}

/// A directory symlink needs Developer Mode or elevation; a junction does not.
/// Where neither works, a copy the next run refreshes.
#[cfg(windows)]
fn create_dir_link(target: &Path, link: &Path) -> Result<()> {
    if std::os::windows::fs::symlink_dir(target, link).is_ok() {
        return Ok(());
    }
    let junction = std::process::Command::new("cmd")
        .arg("/C")
        .arg("mklink")
        .arg("/J")
        .arg(link)
        .arg(target)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    if junction.is_ok_and(|status| status.success()) {
        return Ok(());
    }
    copy_tree(target, link)
}

#[cfg(not(any(unix, windows)))]
fn create_dir_link(target: &Path, link: &Path) -> Result<()> {
    copy_tree(target, link)
}

#[cfg(not(unix))]
fn copy_tree(source: &Path, target: &Path) -> Result<()> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let to = target.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &to)?;
        } else {
            fs::copy(entry.path(), &to)?;
        }
    }
    Ok(())
}

/// Content hash of a directory tree, paths included.
fn tree_digest(root: &Path) -> Result<String> {
    fn collect(root: &Path, dir: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                collect(root, &path, out)?;
            } else {
                let relative = path.strip_prefix(root)?.to_path_buf();
                out.push((relative, fs::read(&path)?));
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    collect(root, root, &mut files)?;
    files.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(digest_of(&files))
}

fn sync(dest: &Path, create_if_missing: bool) -> Result<Sync> {
    let installed = dest.join("SKILL.md").is_file();
    if !installed && !create_if_missing && !skills_root_exists(dest) {
        return Ok(Sync::Skipped);
    }

    let manifest = read_manifest(dest);
    let field = |key: &str| {
        manifest
            .as_ref()
            .and_then(|value| value.get(key))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
    };
    if installed && field("digest").as_deref() == Some(embedded_digest()) {
        return Ok(Sync::Current);
    }

    write_skill(dest, &embedded_files())?;
    Ok(if installed {
        Sync::Rewritten {
            previous: field("version").unwrap_or_else(|| "unknown".to_string()),
        }
    } else {
        Sync::Created
    })
}

/// Whether the agent tooling's skills root is already on this machine. Its
/// presence is the standing answer to "may I write outside the project".
fn skills_root_exists(dest: &Path) -> bool {
    dest.parent().is_some_and(Path::is_dir)
}

/// `lingxia skill install`: write the skill where the agent looks for it. It is
/// a user-level install and never edits the project the shell is in; a stale
/// pointer block in the nearest `AGENTS.md` is reported with its replacement.
pub fn install(cwd: &Path) -> Result<()> {
    let dest = user_destination()?;
    match sync_for_home(&home_dir()?, true)? {
        Sync::Current => println!("The LingXia skill at {} is current", dest.display()),
        Sync::Rewritten { .. } => println!("Updated the LingXia skill at {}", dest.display()),
        Sync::Created | Sync::Skipped => {
            println!("Installed the LingXia skill to {}", dest.display())
        }
    }
    if let Some(suggestion) = stale_pointer(cwd, &dest) {
        println!("{suggestion}");
    }
    Ok(())
}

/// The nearest pointer block that differs from the one `lingxia new` writes
/// now, as a message carrying the replacement block.
fn stale_pointer(cwd: &Path, dest: &Path) -> Option<String> {
    let project_dir = pointer_project(cwd)?;
    let path = project_dir.join("AGENTS.md");
    let existing = fs::read_to_string(&path).ok()?;
    let block = agents_block(&portable_reference(&project_dir, dest));
    (replace_block(&existing, &block) != existing).then(|| {
        format!(
            "{} has an outdated LingXia pointer; replace the block between its `{AGENTS_MARKER}` lines with:\n\n{block}",
            path.display()
        )
    })
}

/// The nearest directory at or above `cwd` whose `AGENTS.md` carries this
/// CLI's pointer block. The search stops at the first `AGENTS.md`: one without
/// the block belongs to someone else.
fn pointer_project(cwd: &Path) -> Option<PathBuf> {
    cwd.ancestors().find_map(|dir| {
        let text = fs::read_to_string(dir.join("AGENTS.md")).ok()?;
        Some(text.contains(AGENTS_MARKER).then(|| dir.to_path_buf()))
    })?
}

/// Install the skill for a freshly scaffolded project: the body in the home
/// directory, a committable pointer in the project.
pub fn install_for_new_project(project_dir: &Path) -> Result<()> {
    let dest = user_destination()?;
    match sync_for_home(&home_dir()?, true)? {
        Sync::Current => println!("The LingXia skill at {} is current", dest.display()),
        _ => println!("Installed the LingXia skill to {}", dest.display()),
    }
    write_agents_pointer(project_dir, &dest)
}

/// The home-directory skill, shared by every project.
pub fn user_destination() -> Result<PathBuf> {
    Ok(skills_root(&home_dir()?).join(SKILL_DIR_NAME))
}

/// One line for `lingxia version --verbose`: where the skill is, and whether it
/// is this binary's. This is what the removed status command reported.
pub fn install_summary() -> String {
    let Ok(dest) = user_destination() else {
        return "unknown".to_string();
    };
    if !dest.join("SKILL.md").is_file() {
        return format!(
            "{} (not installed; run `lingxia skill install`)",
            dest.display()
        );
    }
    let digest = read_manifest(&dest)
        .and_then(|manifest| {
            manifest
                .get("digest")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_default();
    let state = if digest == embedded_digest() {
        "in sync"
    } else {
        "stale"
    };
    format!("{} ({state})", dest.display())
}

/// The skill's identity, and the only thing a sync compares.
///
/// A version number cannot serve: a development build edits the skill without
/// moving the version, two branches share a version while carrying different
/// docs, and an older binary must still be able to correct a copy a newer one
/// left behind -- the installed skill has to describe the CLI you are running,
/// not the newest one that ever ran here.
fn embedded_digest() -> &'static str {
    static DIGEST: OnceLock<String> = OnceLock::new();
    DIGEST.get_or_init(|| digest_of(&embedded_files()))
}

fn digest_of(files: &[(PathBuf, Vec<u8>)]) -> String {
    let mut hasher = Sha256::new();
    for (path, contents) in files {
        // Path and length go in too, so moving bytes between files or renaming
        // one cannot land on the same digest.
        hasher.update(path.to_string_lossy().replace('\\', "/").as_bytes());
        hasher.update([0]);
        hasher.update((contents.len() as u64).to_le_bytes());
        hasher.update(contents);
    }
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn embedded_files() -> Vec<(PathBuf, Vec<u8>)> {
    let mut files: Vec<(PathBuf, Vec<u8>)> = collect_files(&EMBEDDED_SKILL)
        .into_iter()
        .map(|(path, contents)| (path, contents.to_vec()))
        .collect();
    // The digest must not depend on the order the macro expanded the tree in.
    files.sort_by(|left, right| left.0.cmp(&right.0));
    files
}

fn home_dir() -> Result<PathBuf> {
    dirs::home_dir().context("Could not resolve the home directory")
}

fn read_manifest(dest: &Path) -> Option<serde_json::Value> {
    let text = fs::read_to_string(dest.join(MANIFEST_NAME)).ok()?;
    serde_json::from_str(&text).ok()
}

fn manifest(files: &[(PathBuf, Vec<u8>)]) -> serde_json::Value {
    serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "digest": digest_of(files),
        "writtenBy": std::env::current_exe()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|_| "unknown".to_string()),
    })
}

/// Replace the destination wholesale, and only once it is complete.
///
/// A file dropped from the skill must not survive in an install that claims to
/// be this build, so this cannot merge into the existing directory. It stages
/// into a sibling and renames: an interrupted run leaves the previous install
/// untouched rather than an empty directory that nothing would restore, and two
/// processes racing here (`lingxia dev` beside the broker `lxdev` spawns) each
/// stage separately, so the last rename wins instead of one failing on a
/// directory the other just removed.
fn write_skill(dest: &Path, files: &[(PathBuf, Vec<u8>)]) -> Result<()> {
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
        format!("{}\n", serde_json::to_string_pretty(&manifest(files))?),
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

/// Marks the block this writes, so a later scaffold replaces it instead of
/// appending a second copy.
const AGENTS_MARKER: &str = "<!-- lingxia skill: AGENTS.md pointer -->";

/// Point tools that read a single root file at the installed skill.
///
/// AGENTS.md is meant to be committed, so the reference must not be a path that
/// only resolves on this machine: `~`-relative for the home install, absolute
/// only as a fallback.
fn write_agents_pointer(project_dir: &Path, dest: &Path) -> Result<()> {
    let path = project_dir.join("AGENTS.md");
    let block = agents_block(&portable_reference(project_dir, dest));

    let body = match fs::read_to_string(&path) {
        Ok(existing) if existing.contains(AGENTS_MARKER) => {
            let replaced = replace_block(&existing, &block);
            if replaced == existing {
                return Ok(());
            }
            replaced
        }
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
Read the LingXia development skill before working on this host app or lxapp:\n\n\
    {skill_ref}/SKILL.md\n\n\
If it is missing, run `lingxia skill install`. The `lingxia` CLI rewrites it\n\
whenever it changes, so it always describes the CLI installed on this machine.\n\n\
If your agent needs skills registered explicitly, add this directory in its\n\
skill settings. Follow relative links from SKILL.md only as needed.\n\
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

#[cfg(test)]
mod tests {
    use super::*;

    fn skill_dir(root: &Path) -> PathBuf {
        skills_root(root).join(SKILL_DIR_NAME)
    }

    #[test]
    fn a_missing_skill_is_left_alone_without_a_skills_root() {
        let home = TempDir::new().unwrap();
        let dest = skill_dir(home.path());
        assert!(matches!(sync(&dest, false).unwrap(), Sync::Skipped));
        assert!(!dest.exists());
    }

    #[test]
    fn an_existing_skills_root_is_standing_consent_to_write_one() {
        let home = TempDir::new().unwrap();
        let dest = skill_dir(home.path());
        fs::create_dir_all(dest.parent().unwrap()).unwrap();
        assert!(matches!(sync(&dest, false).unwrap(), Sync::Created));
        assert!(dest.join("SKILL.md").is_file());
        assert!(matches!(sync(&dest, false).unwrap(), Sync::Current));
    }

    #[test]
    fn an_old_install_moves_without_an_explicit_install_command() {
        let home = TempDir::new().unwrap();
        let old = home.path().join(".claude/skills/lingxia");
        let other = home.path().join(".claude/skills/unrelated");
        fs::create_dir_all(&old).unwrap();
        fs::create_dir_all(&other).unwrap();
        fs::write(old.join("SKILL.md"), "old").unwrap();
        assert!(matches!(
            sync_for_home(home.path(), false).unwrap(),
            Sync::Created
        ));
        let canonical = fs::canonicalize(skill_dir(home.path())).unwrap();
        assert!(fs::symlink_metadata(&old).unwrap().file_type().is_symlink());
        assert_eq!(fs::canonicalize(&old).unwrap(), canonical);
        assert!(other.is_dir());
        assert!(skill_dir(home.path()).join("SKILL.md").is_file());
        assert!(matches!(
            sync_for_home(home.path(), false).unwrap(),
            Sync::Current
        ));
        assert_eq!(fs::canonicalize(&old).unwrap(), canonical);
    }

    #[cfg(unix)]
    #[test]
    fn a_claude_skills_root_linked_into_the_agents_root_is_left_alone() {
        let home = TempDir::new().unwrap();
        fs::create_dir_all(skills_root(home.path())).unwrap();
        fs::create_dir_all(home.path().join(".claude")).unwrap();
        std::os::unix::fs::symlink(skills_root(home.path()), claude_skills_root(home.path()))
            .unwrap();
        for _ in 0..2 {
            sync_for_home(home.path(), true).unwrap();
            assert!(skill_dir(home.path()).join("SKILL.md").is_file());
        }
        // A stale copy is rewritten, never deleted through the linked root.
        fs::write(skill_dir(home.path()).join(MANIFEST_NAME), "{}").unwrap();
        sync_for_home(home.path(), false).unwrap();
        assert!(skill_dir(home.path()).join("SKILL.md").is_file());
    }

    #[cfg(unix)]
    #[test]
    fn a_users_own_link_elsewhere_is_kept_and_no_claude_means_no_link() {
        let home = TempDir::new().unwrap();
        sync_for_home(home.path(), true).unwrap();
        assert!(!home.path().join(".claude").exists());

        let checkout = TempDir::new().unwrap();
        fs::write(checkout.path().join("SKILL.md"), "dev").unwrap();
        fs::create_dir_all(claude_skills_root(home.path())).unwrap();
        let link = claude_skills_root(home.path()).join(SKILL_DIR_NAME);
        std::os::unix::fs::symlink(checkout.path(), &link).unwrap();
        fs::write(skill_dir(home.path()).join(MANIFEST_NAME), "{}").unwrap();
        sync_for_home(home.path(), false).unwrap();
        assert_eq!(fs::read_to_string(link.join("SKILL.md")).unwrap(), "dev");
    }

    #[test]
    fn a_user_level_install_never_edits_the_project_agents_file() {
        let project = TempDir::new().unwrap();
        let agents = project.path().join("AGENTS.md");
        let stale = format!("# Mine\n\n{AGENTS_MARKER}\nold wording\n{AGENTS_MARKER}\n");
        fs::write(&agents, &stale).unwrap();
        let dest = skill_dir(project.path());
        let suggestion = stale_pointer(project.path(), &dest).expect("a stale block is reported");
        assert!(
            suggestion.contains("run `lingxia skill install`."),
            "{suggestion}"
        );
        assert_eq!(fs::read_to_string(&agents).unwrap(), stale);

        write_agents_pointer(project.path(), &dest).unwrap();
        assert_eq!(stale_pointer(project.path(), &dest), None);
    }

    #[test]
    fn a_failed_install_preserves_the_old_skill() {
        let home = TempDir::new().unwrap();
        let old = home.path().join(".claude/skills/lingxia");
        fs::create_dir_all(&old).unwrap();
        fs::write(old.join("SKILL.md"), "old").unwrap();
        fs::write(home.path().join(".agents"), "not a directory").unwrap();
        assert!(sync_for_home(home.path(), true).is_err());
        assert_eq!(fs::read_to_string(old.join("SKILL.md")).unwrap(), "old");
    }

    #[test]
    fn a_copy_another_build_wrote_is_replaced_at_the_same_version() {
        let home = TempDir::new().unwrap();
        let dest = skill_dir(home.path());
        sync(&dest, true).unwrap();
        // Same version, different content: a version comparison cannot see this.
        fs::write(dest.join("SKILL.md"), "stale").unwrap();
        fs::write(
            dest.join(MANIFEST_NAME),
            serde_json::json!({ "version": env!("CARGO_PKG_VERSION"), "digest": "stale" })
                .to_string(),
        )
        .unwrap();
        assert!(matches!(
            sync(&dest, false).unwrap(),
            Sync::Rewritten { .. }
        ));
        assert_ne!(fs::read_to_string(dest.join("SKILL.md")).unwrap(), "stale");
    }

    #[test]
    fn a_file_the_skill_dropped_does_not_survive_the_rewrite() {
        let home = TempDir::new().unwrap();
        let dest = skill_dir(home.path());
        sync(&dest, true).unwrap();
        let orphan = dest.join("orphan.md");
        fs::write(&orphan, "gone next time").unwrap();
        fs::write(dest.join(MANIFEST_NAME), r#"{"digest":"stale"}"#).unwrap();
        sync(&dest, false).unwrap();
        assert!(!orphan.exists());
    }

    #[test]
    fn the_pointer_names_the_install_command() {
        let block = agents_block("~/.agents/skills/lingxia");
        assert!(block.contains("run `lingxia skill install`."));
        assert!(!block.contains("--user"));
        assert!(!block.contains("npx"));
    }

    #[test]
    fn a_stale_pointer_block_is_replaced_in_place() {
        let project = TempDir::new().unwrap();
        let agents = project.path().join("AGENTS.md");
        let stale = format!(
            "# AGENTS\n\nKeep this.\n\n{AGENTS_MARKER}\n## LingXia\n\nwrite it with:\n\n    lingxia skill install --user\n{AGENTS_MARKER}\n\nAnd this.\n"
        );
        fs::write(&agents, &stale).unwrap();
        let nested = project.path().join("pages").join("home");
        fs::create_dir_all(&nested).unwrap();

        let found = pointer_project(&nested).expect("the project with the block");
        assert_eq!(found, project.path());
        write_agents_pointer(&found, &skill_dir(project.path())).unwrap();

        let written = fs::read_to_string(&agents).unwrap();
        assert!(written.starts_with("# AGENTS\n\nKeep this.\n\n"));
        assert!(written.ends_with("\nAnd this.\n"));
        assert!(written.contains("run `lingxia skill install`."));
        assert!(!written.contains("--user"));
        assert_eq!(written.matches(AGENTS_MARKER).count(), 2);
    }

    #[test]
    fn an_agents_file_without_the_block_is_not_ours_to_edit() {
        let project = TempDir::new().unwrap();
        fs::write(project.path().join("AGENTS.md"), "# Mine\n").unwrap();
        let nested = project.path().join("src");
        fs::create_dir_all(&nested).unwrap();
        assert_eq!(pointer_project(&nested), None);
    }

    #[test]
    fn the_digest_covers_paths_not_just_bytes() {
        let one = vec![(PathBuf::from("a.md"), b"x".to_vec())];
        let two = vec![(PathBuf::from("b.md"), b"x".to_vec())];
        assert_ne!(digest_of(&one), digest_of(&two));
    }
}
