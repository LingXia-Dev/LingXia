//! Agent skills that the project's direct dependencies ship, copied into the project.
//!
//! A package declares `"lingxia": { "skill": "./skill" }`: a directory with a `SKILL.md` whose
//! frontmatter `name` becomes `<project>/.agents/skills/<name>` (linked from
//! `.claude/skills/<name>` when the project uses Claude Code) and is kept out of git. A skill is
//! instructions to an agent, so what a dependency may write is bounded: only a directory inside
//! its own package, only text files and a small total, never over the `lingxia` skill or a
//! directory this CLI did not write.

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::ui::{direct_dependencies, inside_package, installed};

/// Marks a copy this CLI wrote, and what it was written from.
const MANIFEST_NAME: &str = ".lingxia-package-skill.json";
/// Names a package may not take: the CLI's own skill.
const RESERVED: &[&str] = &["lingxia"];
const MAX_FILES: usize = 64;
const MAX_BYTES: usize = 512 * 1024;

/// One skill a dependency offers, read and checked but not yet written.
#[derive(Debug)]
struct Offer {
    package: String,
    version: String,
    name: String,
    files: Vec<(PathBuf, Vec<u8>)>,
}

/// What every command does for the project under `cwd`: `setup` at `dev` start, a quiet refresh
/// otherwise. Under CI it writes and prints nothing, so a CI build leaves the tree alone.
pub fn sync_for_run(setup: bool) {
    if let Some(root) = super::project_upgrade::find_project_root() {
        sync_unless_ci(&root, setup);
    }
}

/// `sync_package_skills` with its notes printed, skipped entirely when `CI` is set.
pub fn sync_unless_ci(project_root: &Path, setup: bool) {
    if std::env::var_os("CI").is_some_and(|v| !v.is_empty()) {
        return;
    }
    report(sync_package_skills(project_root, setup));
}

/// Print what a sync has to say, on stderr so a command's data output stays clean.
pub fn report(outcome: Result<Vec<String>>) {
    match outcome {
        Ok(notes) => notes.iter().for_each(|note| eprintln!("{note}")),
        Err(error) => eprintln!("warning: could not sync package skills: {error:#}"),
    }
}

/// Bring the project's package skills in step with its dependencies; returns one line per
/// thing worth saying (an install, a changed copy, a refusal). With `setup` (`new`, `dev`
/// start, `skill install`) a skill not yet in the project is installed, the Claude Code link
/// and `.gitignore` written, and copies whose package no longer offers them removed; otherwise
/// only copies already there are refreshed.
pub fn sync_package_skills(project_root: &Path, setup: bool) -> Result<Vec<String>> {
    let Some(manifest) = [
        project_root.join("package.json"),
        project_root.join("lxapp").join("package.json"),
    ]
    .into_iter()
    .find(|path| path.is_file()) else {
        return Ok(Vec::new());
    };
    let package_root = manifest.parent().unwrap_or(project_root);
    let json: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&manifest).with_context(|| format!("read {}", manifest.display()))?,
    )
    .with_context(|| format!("{} is not JSON", manifest.display()))?;

    let mut notes = Vec::new();
    let mut by_name: BTreeMap<String, Vec<Offer>> = BTreeMap::new();
    for package in direct_dependencies(&json) {
        let Some(dir) = installed(package_root, &package) else {
            continue;
        };
        match offer(&package, &dir) {
            Ok(Some(offer)) => by_name.entry(offer.name.clone()).or_default().push(offer),
            Ok(None) => {}
            // A broken offer is the package's mistake: say so where the developer sets up, not on every run.
            Err(reason) if setup => notes.push(format!(
                "warning: skipped the skill of {package}: {reason:#}"
            )),
            Err(_) => {}
        }
    }

    let skills_root = project_root.join(".agents").join("skills");
    let mut offered = Vec::new();
    for (name, mut offers) in by_name {
        if offers.len() > 1 {
            let packages: Vec<&str> = offers.iter().map(|o| o.package.as_str()).collect();
            notes.push(format!(
                "skill \"{name}\" is offered by {}; installed neither",
                packages.join(" and ")
            ));
            continue;
        }
        let offer = offers.remove(0);
        offered.push(name.clone());
        let dest = skills_root.join(&name);
        if let Some(note) = place(&offer, &dest, setup)? {
            notes.push(note);
        }
        if setup && is_ours(&dest) {
            super::skill::link_for_claude(project_root, &dest, is_ours)?;
            let entries = [
                format!(".agents/skills/{name}/"),
                format!(".claude/skills/{name}"),
            ];
            let entries: Vec<&str> = entries.iter().map(String::as_str).collect();
            super::skill::ignore_in_git(project_root, &entries, "# Skills from npm dependencies")?;
        }
    }
    if setup {
        notes.extend(remove_withdrawn(project_root, &skills_root, &offered)?);
    }
    Ok(notes)
}

/// The skill `package` (installed at the canonical `dir`) declares, if any. A declared skill
/// that breaks a rule is an error naming the rule.
fn offer(package: &str, dir: &Path) -> Result<Option<Offer>> {
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(dir.join("package.json"))?)?;
    let Some(relative) = manifest.pointer("/lingxia/skill").and_then(|v| v.as_str()) else {
        return Ok(None);
    };
    let plain = Path::new(relative);
    if !relative.is_empty()
        && plain.is_relative()
        && !plain
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        && !dir.join(plain).exists()
    {
        anyhow::bail!(
            "lingxia.skill \"{relative}\" is declared, but the package ships no such directory"
        );
    }
    let source = inside_package(dir, relative)
        .filter(|path| path.is_dir())
        .with_context(|| {
            format!("lingxia.skill \"{relative}\" is not a directory inside the package")
        })?;
    let skill_md = fs::read_to_string(source.join("SKILL.md")).context("no SKILL.md")?;
    let name = frontmatter_name(&skill_md).context("SKILL.md has no frontmatter name")?;
    if !valid_name(&name) {
        anyhow::bail!("skill name \"{name}\" is not lower-case letters, digits and hyphens");
    }
    if RESERVED.contains(&name.as_str()) {
        anyhow::bail!("skill name \"{name}\" is reserved");
    }
    let mut files = Vec::new();
    collect(&source, &source, &mut files)?;
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let total: usize = files.iter().map(|(_, bytes)| bytes.len()).sum();
    if files.len() > MAX_FILES || total > MAX_BYTES {
        anyhow::bail!(
            "{} files, {} KiB: a skill may hold at most {MAX_FILES} files and {} KiB",
            files.len(),
            total / 1024,
            MAX_BYTES / 1024
        );
    }
    Ok(Some(Offer {
        package: package.to_string(),
        version: manifest
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        name,
        files,
    }))
}

/// `name:` inside the leading `---` block.
fn frontmatter_name(text: &str) -> Option<String> {
    let mut lines = text.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    for line in lines {
        let line = line.trim();
        if line == "---" {
            return None;
        }
        if let Some(value) = line.strip_prefix("name:") {
            let value = value.trim().trim_matches(|c| c == '"' || c == '\'');
            return (!value.is_empty()).then(|| value.to_string());
        }
    }
    None
}

fn valid_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    (1..=64).contains(&bytes.len())
        && bytes[0].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
}

/// Every file under `dir` as UTF-8 text; a symlink or a binary file refuses the skill.
fn collect(root: &Path, dir: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let kind = entry.file_type()?;
        let relative = path.strip_prefix(root)?.to_path_buf();
        if kind.is_symlink() {
            anyhow::bail!("{} is a symlink", relative.display());
        }
        if kind.is_dir() {
            collect(root, &path, out)?;
            continue;
        }
        let bytes = fs::read(&path)?;
        if bytes.contains(&0) || std::str::from_utf8(&bytes).is_err() {
            anyhow::bail!("{} is not a text file", relative.display());
        }
        out.push((relative, bytes));
        if out.len() > MAX_FILES {
            anyhow::bail!("more than {MAX_FILES} files");
        }
    }
    Ok(())
}

fn digest(files: &[(PathBuf, Vec<u8>)]) -> String {
    let mut hasher = Sha256::new();
    for (path, bytes) in files {
        hasher.update(path.to_string_lossy().replace('\\', "/").as_bytes());
        hasher.update([0]);
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn read_manifest(dest: &Path) -> Option<serde_json::Value> {
    serde_json::from_str(&fs::read_to_string(dest.join(MANIFEST_NAME)).ok()?).ok()
}

fn is_ours(dest: &Path) -> bool {
    read_manifest(dest).is_some()
}

/// The copy's files as they are on disk, the manifest left out.
fn disk_files(dest: &Path) -> Option<Vec<(PathBuf, Vec<u8>)>> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) -> std::io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                walk(root, &path, out)?;
            } else if entry.file_name() != MANIFEST_NAME {
                let relative = path.strip_prefix(root).map_err(std::io::Error::other)?;
                out.push((relative.to_path_buf(), fs::read(&path)?));
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    walk(dest, dest, &mut files).ok()?;
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Some(files)
}

/// Write `offer` to `dest` when that is safe and needed; the line to print, if any.
fn place(offer: &Offer, dest: &Path, setup: bool) -> Result<Option<String>> {
    let wanted = digest(&offer.files);
    let shown = format!(".agents/skills/{}", offer.name);
    let Ok(metadata) = fs::symlink_metadata(dest) else {
        if !setup {
            return Ok(None);
        }
        write(offer, dest, &wanted)?;
        return Ok(Some(format!(
            "installed the \"{}\" skill from {} into {shown}",
            offer.name, offer.package
        )));
    };
    let Some(manifest) = read_manifest(dest).filter(|_| metadata.is_dir()) else {
        return Ok(setup.then(|| {
            format!(
                "kept {shown}: lingxia did not write it, so {}'s skill is not installed there",
                offer.package
            )
        }));
    };
    let recorded = manifest
        .get("digest")
        .and_then(|d| d.as_str())
        .unwrap_or("");
    if recorded == wanted {
        return Ok(None);
    }
    let on_disk = disk_files(dest).map(|files| digest(&files));
    if on_disk.as_deref() != Some(recorded) {
        return Ok(setup.then(|| {
            format!(
                "kept {shown}: it was edited; delete it to take {}'s copy",
                offer.package
            )
        }));
    }
    write(offer, dest, &wanted)?;
    Ok(Some(format!(
        "updated the \"{}\" skill from {} {}",
        offer.name, offer.package, offer.version
    )))
}

/// Stage beside `dest` and rename, so an interrupted run never leaves half a skill.
fn write(offer: &Offer, dest: &Path, digest: &str) -> Result<()> {
    let parent = dest.parent().context("skill destination has no parent")?;
    fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    let staged = tempfile::TempDir::new_in(parent)?;
    for (path, bytes) in &offer.files {
        let out = staged.path().join(path);
        if let Some(dir) = out.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(&out, bytes).with_context(|| format!("write {}", out.display()))?;
    }
    let manifest = serde_json::json!({
        "package": offer.package,
        "version": offer.version,
        "digest": digest,
    });
    fs::write(
        staged.path().join(MANIFEST_NAME),
        format!("{}\n", serde_json::to_string_pretty(&manifest)?),
    )?;
    if dest.exists() {
        let retired = staged.path().with_extension("previous");
        let _ = fs::remove_dir_all(&retired);
        fs::rename(dest, &retired)
            .with_context(|| format!("move the previous skill out of {}", dest.display()))?;
        let _ = fs::remove_dir_all(&retired);
    }
    fs::rename(staged.keep(), dest)
        .with_context(|| format!("move the skill into {}", dest.display()))?;
    Ok(())
}

/// Remove copies this CLI wrote whose package no longer offers them, unless edited.
fn remove_withdrawn(
    project_root: &Path,
    skills_root: &Path,
    offered: &[String],
) -> Result<Vec<String>> {
    let mut notes = Vec::new();
    let Ok(entries) = fs::read_dir(skills_root) else {
        return Ok(notes);
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let dest = entry.path();
        if offered.contains(&name) || !entry.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let Some(manifest) = read_manifest(&dest) else {
            continue;
        };
        let recorded = manifest
            .get("digest")
            .and_then(|d| d.as_str())
            .unwrap_or("");
        if disk_files(&dest).map(|files| digest(&files)).as_deref() != Some(recorded) {
            continue;
        }
        super::skill::unlink_for_claude(project_root, &dest, |_| false)?;
        fs::remove_dir_all(&dest).with_context(|| format!("remove {}", dest.display()))?;
        let package = manifest
            .get("package")
            .and_then(|p| p.as_str())
            .unwrap_or("?");
        notes.push(format!(
            "removed .agents/skills/{name}: {package} no longer offers it"
        ));
    }
    Ok(notes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_file(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    const SKILL: &str = "---\nname: kit\ndescription: Use the kit.\n---\n\n# Kit\n";

    /// A project depending on `@acme/kit`, which ships `skill/` as the `kit` skill.
    fn project() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        let r = root.path();
        write_file(
            &r.join("package.json"),
            r#"{"dependencies":{"@acme/kit":"1"}}"#,
        );
        write_file(
            &r.join("node_modules/@acme/kit/package.json"),
            r#"{"name":"@acme/kit","version":"1.2.0","lingxia":{"skill":"./skill"}}"#,
        );
        write_file(&r.join("node_modules/@acme/kit/skill/SKILL.md"), SKILL);
        write_file(&r.join("node_modules/@acme/kit/skill/refs/a.md"), "a\n");
        root
    }

    fn kit(root: &Path) -> PathBuf {
        root.join("node_modules/@acme/kit")
    }

    #[test]
    fn setup_installs_once_and_says_so() {
        let p = project();
        let notes = sync_package_skills(p.path(), true).unwrap();
        assert_eq!(
            notes,
            ["installed the \"kit\" skill from @acme/kit into .agents/skills/kit"]
        );
        let dest = p.path().join(".agents/skills/kit");
        assert_eq!(fs::read_to_string(dest.join("SKILL.md")).unwrap(), SKILL);
        assert!(dest.join("refs/a.md").is_file());
        let ignore = fs::read_to_string(p.path().join(".gitignore")).unwrap();
        assert!(ignore.contains(".agents/skills/kit/") && ignore.contains(".claude/skills/kit"));
        assert!(sync_package_skills(p.path(), true).unwrap().is_empty());
        assert!(sync_package_skills(p.path(), false).unwrap().is_empty());
    }

    #[test]
    fn other_runs_only_refresh_what_is_there() {
        let p = project();
        assert!(sync_package_skills(p.path(), false).unwrap().is_empty());
        assert!(!p.path().join(".agents").exists());

        sync_package_skills(p.path(), true).unwrap();
        write_file(
            &kit(p.path()).join("skill/SKILL.md"),
            "---\nname: kit\n---\nnew\n",
        );
        let notes = sync_package_skills(p.path(), false).unwrap();
        assert_eq!(notes, ["updated the \"kit\" skill from @acme/kit 1.2.0"]);
        let text = fs::read_to_string(p.path().join(".agents/skills/kit/SKILL.md")).unwrap();
        assert!(text.ends_with("new\n"));
    }

    #[test]
    fn a_directory_lingxia_did_not_write_is_kept() {
        let p = project();
        write_file(&p.path().join(".agents/skills/kit/SKILL.md"), "mine\n");
        let notes = sync_package_skills(p.path(), true).unwrap();
        assert_eq!(notes.len(), 1);
        assert!(
            notes[0].starts_with("kept .agents/skills/kit: lingxia did not write it"),
            "{notes:?}"
        );
        assert_eq!(
            fs::read_to_string(p.path().join(".agents/skills/kit/SKILL.md")).unwrap(),
            "mine\n"
        );
    }

    #[test]
    fn an_edited_copy_is_kept() {
        let p = project();
        sync_package_skills(p.path(), true).unwrap();
        write_file(&p.path().join(".agents/skills/kit/SKILL.md"), "edited\n");
        write_file(
            &kit(p.path()).join("skill/SKILL.md"),
            "---\nname: kit\n---\nnew\n",
        );
        let notes = sync_package_skills(p.path(), true).unwrap();
        assert!(
            notes[0].starts_with("kept .agents/skills/kit: it was edited"),
            "{notes:?}"
        );
    }

    #[test]
    fn two_packages_claiming_one_name_install_neither() {
        let p = project();
        let r = p.path();
        write_file(
            &r.join("package.json"),
            r#"{"dependencies":{"@acme/kit":"1","other":"1"}}"#,
        );
        write_file(
            &r.join("node_modules/other/package.json"),
            r#"{"name":"other","lingxia":{"skill":"s"}}"#,
        );
        write_file(&r.join("node_modules/other/s/SKILL.md"), SKILL);
        let notes = sync_package_skills(r, true).unwrap();
        assert_eq!(
            notes,
            ["skill \"kit\" is offered by @acme/kit and other; installed neither"]
        );
        assert!(!r.join(".agents/skills/kit").exists());
    }

    #[test]
    fn unsafe_skills_are_refused_with_the_rule() {
        let cases: &[(&str, &str, &str)] = &[
            (
                r#"{"lingxia":{"skill":"../../.."}}"#,
                SKILL,
                "is not a directory inside the package",
            ),
            (
                r#"{"lingxia":{"skill":"/etc"}}"#,
                SKILL,
                "is not a directory inside the package",
            ),
            (
                r#"{"lingxia":{"skill":"skill"}}"#,
                "---\nname: lingxia\n---\n",
                "is reserved",
            ),
            (
                r#"{"lingxia":{"skill":"skill"}}"#,
                "---\nname: ../x\n---\n",
                "is not lower-case",
            ),
            (
                r#"{"lingxia":{"skill":"skill"}}"#,
                "# no frontmatter\n",
                "no frontmatter name",
            ),
        ];
        for (manifest, skill_md, expected) in cases {
            let p = project();
            write_file(&kit(p.path()).join("package.json"), manifest);
            write_file(&kit(p.path()).join("skill/SKILL.md"), skill_md);
            let notes = sync_package_skills(p.path(), true).unwrap();
            assert_eq!(notes.len(), 1, "{manifest}");
            assert!(
                notes[0].starts_with("warning: skipped the skill of @acme/kit: "),
                "{notes:?}"
            );
            assert!(notes[0].contains(expected), "{expected}: {notes:?}");
            assert!(!p.path().join(".agents").exists());
            assert!(
                sync_package_skills(p.path(), false).unwrap().is_empty(),
                "{manifest}"
            );
        }
    }

    #[test]
    fn a_declared_skill_directory_that_is_missing_warns_on_setup() {
        let p = project();
        fs::remove_dir_all(kit(p.path()).join("skill")).unwrap();
        let notes = sync_package_skills(p.path(), true).unwrap();
        assert_eq!(
            notes,
            [
                "warning: skipped the skill of @acme/kit: lingxia.skill \"./skill\" is declared, but the package ships no such directory"
            ]
        );
        assert!(sync_package_skills(p.path(), false).unwrap().is_empty());
    }

    #[test]
    fn binary_symlinked_or_oversized_content_is_refused() {
        let p = project();
        fs::write(kit(p.path()).join("skill/logo.png"), [0x89, b'P', 0, 1]).unwrap();
        assert!(
            sync_package_skills(p.path(), true).unwrap()[0].contains("logo.png is not a text file")
        );

        #[cfg(unix)]
        {
            let p = project();
            std::os::unix::fs::symlink(
                p.path().join("package.json"),
                kit(p.path()).join("skill/link.md"),
            )
            .unwrap();
            assert!(
                sync_package_skills(p.path(), true).unwrap()[0].contains("link.md is a symlink")
            );
        }

        let p = project();
        fs::write(
            kit(p.path()).join("skill/big.md"),
            "x".repeat(MAX_BYTES + 1),
        )
        .unwrap();
        assert!(
            sync_package_skills(p.path(), true).unwrap()[0].contains("a skill may hold at most")
        );
    }

    #[test]
    fn a_withdrawn_skill_is_removed_on_setup() {
        let p = project();
        sync_package_skills(p.path(), true).unwrap();
        write_file(&p.path().join("package.json"), r#"{"dependencies":{}}"#);
        assert!(sync_package_skills(p.path(), false).unwrap().is_empty());
        assert!(p.path().join(".agents/skills/kit").exists());
        let notes = sync_package_skills(p.path(), true).unwrap();
        assert_eq!(
            notes,
            ["removed .agents/skills/kit: @acme/kit no longer offers it"]
        );
        assert!(!p.path().join(".agents/skills/kit").exists());
    }

    #[test]
    fn the_lingxia_skill_is_never_touched() {
        let p = project();
        write_file(
            &p.path().join(".agents/skills/lingxia/SKILL.md"),
            "pinned\n",
        );
        sync_package_skills(p.path(), true).unwrap();
        write_file(&p.path().join("package.json"), r#"{"dependencies":{}}"#);
        sync_package_skills(p.path(), true).unwrap();
        assert_eq!(
            fs::read_to_string(p.path().join(".agents/skills/lingxia/SKILL.md")).unwrap(),
            "pinned\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn claude_code_gets_a_link() {
        let p = project();
        fs::create_dir_all(p.path().join(".claude")).unwrap();
        sync_package_skills(p.path(), true).unwrap();
        let link = p.path().join(".claude/skills/kit");
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(link.join("SKILL.md").is_file());
    }
}
