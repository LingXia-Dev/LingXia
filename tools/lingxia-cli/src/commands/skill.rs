//! Keep the agent skill this binary carries in step with the copy on disk.
//!
//! The skill describes what this CLI can do, so it ships inside the CLI rather
//! than as a package fetched separately: an installed copy always came from the
//! binary that wrote it, and cannot describe a version the CLI is not.
//!
//! There is no command to install it. Every run reconciles the copy under the
//! home directory with the one compiled in, so an edit to the skill reaches the
//! agent as soon as the CLI that carries it runs.

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
/// an agent does not grow a `~/.claude` because a build ran.
pub fn sync_home_skill(create_if_missing: bool) -> Result<Sync> {
    sync(&user_destination()?, create_if_missing)
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

/// Install the skill for a freshly scaffolded project: the body in the home
/// directory, a committable pointer in the project.
pub fn install_for_new_project(project_dir: &Path) -> Result<()> {
    let dest = user_destination()?;
    match sync(&dest, true)? {
        Sync::Current => println!("The LingXia skill at {} is current", dest.display()),
        _ => println!("Installed the LingXia skill to {}", dest.display()),
    }
    write_agents_pointer(project_dir, &dest)
}

/// The home-directory skill, shared by every project.
pub fn user_destination() -> Result<PathBuf> {
    Ok(home_dir()?
        .join(".claude")
        .join("skills")
        .join(SKILL_DIR_NAME))
}

/// One line for `lingxia version --verbose`: where the skill is, and whether it
/// is this binary's. This is what the removed status command reported.
pub fn install_summary() -> String {
    let Ok(dest) = user_destination() else {
        return "unknown".to_string();
    };
    if !dest.join("SKILL.md").is_file() {
        return format!("{} (not installed)", dest.display());
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
The `lingxia` CLI writes it there and rewrites it whenever it changes, so it\n\
always describes the CLI installed on this machine.\n\n\
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
        root.join(".claude").join("skills").join(SKILL_DIR_NAME)
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
    fn the_digest_covers_paths_not_just_bytes() {
        let one = vec![(PathBuf::from("a.md"), b"x".to_vec())];
        let two = vec![(PathBuf::from("b.md"), b"x".to_vec())];
        assert_ne!(digest_of(&one), digest_of(&two));
    }
}
