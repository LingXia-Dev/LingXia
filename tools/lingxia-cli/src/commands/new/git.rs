use anyhow::{Context, Result, bail};
use std::path::Path;
use std::process::{Command, Output};

const INITIAL_COMMIT_MESSAGE: &str = "chore: initialize project";

#[derive(Debug, PartialEq, Eq)]
pub(super) enum GitSetup {
    Initialized,
    SkippedParentWorktree,
    SkippedExistingRepository,
}

pub(super) fn initialize_repository(project_dir: &Path) -> Result<GitSetup> {
    if project_dir
        .parent()
        .is_some_and(has_repository_marker_in_ancestors)
    {
        return Ok(GitSetup::SkippedParentWorktree);
    }
    if project_dir.join(".git").exists() {
        return Ok(GitSetup::SkippedExistingRepository);
    }

    run_git(project_dir, ["init", "-b", "main"])?;
    run_git(project_dir, ["add", "--all"])?;

    let mut commit = Command::new("git");
    if !has_git_config(project_dir, "user.name") {
        commit.args(["-c", "user.name=LingXia CLI"]);
    }
    if !has_git_config(project_dir, "user.email") {
        commit.args(["-c", "user.email=cli@lingxia.dev"]);
    }
    let output = commit
        .args(["commit", "--no-gpg-sign", "-m", INITIAL_COMMIT_MESSAGE])
        .current_dir(project_dir)
        .output()
        .context("failed to run `git commit`")?;
    ensure_git_success(output, "git commit")?;

    Ok(GitSetup::Initialized)
}

fn has_repository_marker_in_ancestors(path: &Path) -> bool {
    path.ancestors()
        .any(|ancestor| ancestor.join(".git").exists())
}

fn has_git_config(project_dir: &Path, key: &str) -> bool {
    Command::new("git")
        .args(["config", "--get", key])
        .current_dir(project_dir)
        .output()
        .is_ok_and(|output| output.status.success() && !output.stdout.is_empty())
}

fn run_git<const N: usize>(project_dir: &Path, args: [&str; N]) -> Result<()> {
    let label = format!("git {}", args.join(" "));
    let output = Command::new("git")
        .args(args)
        .current_dir(project_dir)
        .output()
        .with_context(|| format!("failed to run `{label}`"))?;
    ensure_git_success(output, &label)
}

fn ensure_git_success(output: Output, label: &str) -> Result<()> {
    if output.status.success() {
        return Ok(());
    }
    let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if detail.is_empty() {
        bail!("`{label}` exited with {}", output.status);
    }
    bail!("`{label}` failed: {detail}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn git_output(project_dir: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(project_dir)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    #[test]
    fn initializes_main_with_one_clean_baseline_commit() {
        let root = tempfile::tempdir().unwrap();
        let project = root.path().join("demo");
        fs::create_dir(&project).unwrap();
        fs::write(project.join("README.md"), "# Demo\n").unwrap();

        assert_eq!(
            initialize_repository(&project).unwrap(),
            GitSetup::Initialized
        );
        assert_eq!(git_output(&project, &["branch", "--show-current"]), "main");
        assert_eq!(
            git_output(&project, &["log", "-1", "--pretty=%s"]),
            INITIAL_COMMIT_MESSAGE
        );
        assert!(git_output(&project, &["status", "--porcelain"]).is_empty());
    }

    #[test]
    fn skips_a_project_inside_an_existing_worktree() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join(".git")).unwrap();
        let project = root.path().join("demo");
        fs::create_dir(&project).unwrap();

        assert_eq!(
            initialize_repository(&project).unwrap(),
            GitSetup::SkippedParentWorktree
        );
        assert!(!project.join(".git").exists());
    }

    #[test]
    fn leaves_a_template_owned_repository_untouched() {
        let root = tempfile::tempdir().unwrap();
        let project = root.path().join("demo");
        fs::create_dir(&project).unwrap();
        fs::create_dir(project.join(".git")).unwrap();

        assert_eq!(
            initialize_repository(&project).unwrap(),
            GitSetup::SkippedExistingRepository
        );
    }
}
