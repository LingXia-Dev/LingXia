//! Where a new project lands: a fresh `<name>` directory, or — with `.` —
//! the current one.

use super::validation::validate_project_name;
use anyhow::{Context, Result, anyhow, bail};
use dialoguer::{Input, theme::ColorfulTheme};
use std::fs;
use std::path::{Path, PathBuf};

/// The `NAME` that means "this directory".
const HERE: &str = ".";

pub(super) struct ProjectTarget {
    pub(super) name: String,
    pub(super) dir: PathBuf,
    /// The project is created in the current directory.
    pub(super) in_place: bool,
}

/// Resolve `NAME` (prompting when absent) against `current_dir`, and refuse a
/// directory that already holds something a project would collide with.
pub(super) fn resolve(name: Option<String>, current_dir: &Path) -> Result<ProjectTarget> {
    let name = match name {
        Some(name) => name,
        None => Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Project name (. for this directory)")
            .validate_with(|input: &String| -> Result<(), String> {
                resolve_name(input, current_dir)
                    .map(drop)
                    .map_err(|error| error.to_string())
            })
            .interact_text()?,
    };
    let target = resolve_name(&name, current_dir)?;
    ensure_available(&target.dir)?;
    Ok(target)
}

fn resolve_name(name: &str, current_dir: &Path) -> Result<ProjectTarget> {
    if name != HERE {
        validate_project_name(name)?;
        return Ok(ProjectTarget {
            name: name.to_string(),
            dir: current_dir.join(name),
            in_place: false,
        });
    }
    let dir_name = current_dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("The current directory has no name to use as the project name"))?;
    validate_project_name(dir_name).map_err(|_| {
        anyhow!(
            "Directory name '{dir_name}' cannot be a project name (use letters, digits, '-' and '_'). \
             Rename the directory, or run `lingxia new <name>` from its parent."
        )
    })?;
    Ok(ProjectTarget {
        name: dir_name.to_string(),
        dir: current_dir.to_path_buf(),
        in_place: true,
    })
}

/// Files a freshly created or cloned repository carries that a project can
/// be created beside.
fn is_bystander(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        ".git" | ".gitignore" | ".gitattributes" | ".ds_store"
    ) || lower.starts_with("readme")
        || lower.starts_with("license")
}

/// A target is available when it does not exist, or holds only bystanders.
pub(super) fn ensure_available(dir: &Path) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    if !dir.is_dir() {
        bail!("'{}' exists and is not a directory", dir.display());
    }
    let mut blocking = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("Failed to read {}", dir.display()))? {
        let name = entry?.file_name().to_string_lossy().into_owned();
        if !is_bystander(&name) {
            blocking.push(name);
        }
    }
    if blocking.is_empty() {
        return Ok(());
    }
    blocking.sort();
    let shown = blocking.iter().take(5).cloned().collect::<Vec<_>>();
    let more = match blocking.len() - shown.len() {
        0 => String::new(),
        rest => format!(" and {rest} more"),
    };
    bail!(
        "Directory '{}' is not empty: {}{more}. A project can only be created beside \
         .git, .gitignore, .gitattributes, README* and LICENSE*.",
        dir.display(),
        shown.join(", ")
    );
}

/// Write the project's `.gitignore`, keeping the lines of one already there.
pub(super) fn write_gitignore(dir: &Path, generated: &str) -> Result<()> {
    let path = dir.join(".gitignore");
    let content = match fs::read_to_string(&path) {
        Ok(existing) => merge_gitignore(&existing, generated),
        Err(_) => generated.to_string(),
    };
    fs::write(&path, content).with_context(|| format!("Failed to write {}", path.display()))
}

fn merge_gitignore(existing: &str, generated: &str) -> String {
    let known: Vec<&str> = existing.lines().map(str::trim).collect();
    let missing: Vec<&str> = generated
        .lines()
        .filter(|line| {
            let line = line.trim();
            !line.is_empty() && !line.starts_with('#') && !known.contains(&line)
        })
        .collect();
    if missing.is_empty() {
        return existing.to_string();
    }
    let mut merged = existing.trim_end().to_string();
    merged.push_str("\n\n# LingXia\n");
    for line in missing {
        merged.push_str(line);
        merged.push('\n');
    }
    merged
}

/// Move a staged project into its target. A missing target takes the staged
/// directory whole; an existing one receives its entries, keeping what is
/// already there. Returns the names it kept.
pub(super) fn adopt(staged: &Path, target: &Path) -> Result<Vec<String>> {
    let activate = |from: &Path, to: &Path| {
        fs::rename(from, to)
            .with_context(|| format!("Failed to activate generated project at {}", to.display()))
    };
    if !target.exists() {
        activate(staged, target)?;
        return Ok(Vec::new());
    }
    let mut kept = Vec::new();
    for entry in fs::read_dir(staged)? {
        let entry = entry?;
        let name = entry.file_name();
        let destination = target.join(&name);
        if !destination.exists() {
            activate(&entry.path(), &destination)?;
        } else if name == ".gitignore" {
            write_gitignore(target, &fs::read_to_string(entry.path())?)?;
        } else {
            kept.push(name.to_string_lossy().into_owned());
        }
    }
    kept.sort();
    Ok(kept)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_lands_in_a_new_directory_and_dot_in_this_one() {
        let root = tempfile::tempdir().unwrap();
        let here = root.path().join("my-app");
        fs::create_dir(&here).unwrap();

        let named = resolve(Some("demo".into()), &here).unwrap();
        assert_eq!(named.dir, here.join("demo"));
        assert!(!named.in_place);

        let in_place = resolve(Some(".".into()), &here).unwrap();
        assert_eq!(in_place.name, "my-app");
        assert_eq!(in_place.dir, here);
        assert!(in_place.in_place);
    }

    #[test]
    fn dot_needs_a_directory_name_that_is_a_project_name() {
        let root = tempfile::tempdir().unwrap();
        let here = root.path().join("My App");
        fs::create_dir(&here).unwrap();
        let error = resolve(Some(".".into()), &here).err().unwrap().to_string();
        assert!(error.contains("'My App'"), "{error}");
    }

    #[test]
    fn a_fresh_repository_is_available_and_other_content_is_not() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path();
        assert!(ensure_available(&dir.join("missing")).is_ok());
        fs::create_dir(dir.join(".git")).unwrap();
        fs::write(dir.join("README.md"), "# Mine\n").unwrap();
        fs::write(dir.join("LICENSE"), "MIT\n").unwrap();
        fs::write(dir.join(".gitignore"), "node_modules/\n").unwrap();
        assert!(ensure_available(dir).is_ok());

        fs::write(dir.join("package.json"), "{}\n").unwrap();
        let error = ensure_available(dir).unwrap_err().to_string();
        assert!(error.contains("package.json"), "{error}");
    }

    #[test]
    fn an_existing_gitignore_keeps_its_lines_and_gains_the_missing_ones() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join(".gitignore"), "node_modules/\n.env\n").unwrap();
        write_gitignore(root.path(), "# Build\nnode_modules/\ndist/\n").unwrap();
        assert_eq!(
            fs::read_to_string(root.path().join(".gitignore")).unwrap(),
            "node_modules/\n.env\n\n# LingXia\ndist/\n"
        );
        // Nothing new: untouched.
        write_gitignore(root.path(), "dist/\n").unwrap();
        assert!(
            !fs::read_to_string(root.path().join(".gitignore"))
                .unwrap()
                .contains("dist/\ndist/")
        );
    }

    #[test]
    fn adopting_into_an_existing_directory_keeps_what_is_there() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("target");
        let staged = root.path().join("staged");
        fs::create_dir_all(staged.join("pages")).unwrap();
        fs::write(staged.join("lxapp.json"), "{}\n").unwrap();
        fs::write(staged.join("README.md"), "# Generated\n").unwrap();
        fs::write(staged.join(".gitignore"), "dist/\n").unwrap();
        fs::create_dir(&target).unwrap();
        fs::write(target.join("README.md"), "# Mine\n").unwrap();
        fs::write(target.join(".gitignore"), ".env\n").unwrap();

        assert_eq!(adopt(&staged, &target).unwrap(), ["README.md"]);
        assert!(target.join("lxapp.json").exists());
        assert!(target.join("pages").is_dir());
        assert_eq!(
            fs::read_to_string(target.join("README.md")).unwrap(),
            "# Mine\n"
        );
        let ignore = fs::read_to_string(target.join(".gitignore")).unwrap();
        assert!(
            ignore.contains(".env") && ignore.contains("dist/"),
            "{ignore}"
        );
    }
}
