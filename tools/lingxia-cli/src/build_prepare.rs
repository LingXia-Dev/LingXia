//! A project's `lingxia:prepare` npm script generates build inputs, so it runs
//! before dependency installation and host bundle cache lookup. `lingxia build`
//! runs it for every source build; `lingxia dev` runs it once at session start
//! and the companion keeps outputs live, so watcher rebuilds never run it.
use anyhow::{Context, Result, bail};
use std::cell::RefCell;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub(crate) const SCRIPT: &str = "lingxia:prepare";

thread_local! {
    static INVOCATION: RefCell<(usize, HashSet<PathBuf>)> = RefCell::new((0, HashSet::new()));
}

/// Nested builders within one invocation prepare each root once.
pub(crate) struct Scope;

impl Scope {
    pub(crate) fn enter() -> Self {
        INVOCATION.with(|invocation| invocation.borrow_mut().0 += 1);
        Self
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        INVOCATION.with(|invocation| {
            let mut invocation = invocation.borrow_mut();
            invocation.0 -= 1;
            if invocation.0 == 0 {
                invocation.1.clear();
            }
        });
    }
}

pub(crate) fn prepare(root: &Path) -> Result<()> {
    // `npm.cmd` runs under cmd.exe, which cannot start in a `\\?\` directory.
    let root = dunce::canonicalize(root)
        .with_context(|| format!("Cannot resolve build project {}", root.display()))?;
    if INVOCATION.with(|invocation| invocation.borrow().1.contains(&root)) {
        return Ok(());
    }
    if !declares_prepare(&root)? {
        return Ok(());
    }
    let status = Command::new(crate::npm::command())
        .args(["run", SCRIPT])
        .current_dir(&root)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("Failed to start npm for `{SCRIPT}`; install Node.js"))?;
    if !status.success() {
        bail!(
            "`{SCRIPT}` failed for {} ({status}); build stopped",
            root.display()
        );
    }
    INVOCATION.with(|invocation| {
        let mut invocation = invocation.borrow_mut();
        if invocation.0 > 0 {
            invocation.1.insert(root);
        }
    });
    Ok(())
}

fn declares_prepare(root: &Path) -> Result<bool> {
    let path = root.join("package.json");
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error).with_context(|| format!("Cannot read {}", path.display())),
    };
    let package: serde_json::Value =
        serde_json::from_slice(&bytes).with_context(|| format!("Invalid {}", path.display()))?;
    Ok(package["scripts"][SCRIPT].is_string())
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::path::Path;

    /// Gives `root` a `package.json` whose `lingxia:prepare` is `script`.
    pub(crate) fn declare(root: &Path, script: &str) {
        let package = serde_json::json!({
            "name": "prepare-test",
            "private": true,
            "scripts": { super::SCRIPT: script },
        });
        std::fs::write(root.join("package.json"), package.to_string()).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::declare;
    use super::*;

    /// A node one-liner, so the script runs the same under sh and cmd.exe.
    fn node(source: &str) -> String {
        format!("node -e \"{source}\"")
    }

    fn append_x() -> String {
        node("require('fs').appendFileSync('count','x')")
    }

    #[test]
    fn nested_builds_prepare_once_but_each_invocation_prepares_again() {
        let project = tempfile::tempdir().unwrap();
        declare(project.path(), &append_x());
        for _ in 0..2 {
            let _scope = Scope::enter();
            prepare(project.path()).unwrap();
            let _nested = Scope::enter();
            prepare(project.path()).unwrap();
        }
        assert_eq!(fs::read(project.path().join("count")).unwrap(), b"xx");
    }

    #[test]
    fn the_script_runs_in_the_project() {
        let project = tempfile::tempdir().unwrap();
        declare(
            project.path(),
            &node("require('fs').writeFileSync('cwd.txt', process.cwd())"),
        );
        prepare(project.path()).unwrap();
        let cwd = fs::read_to_string(project.path().join("cwd.txt")).unwrap();
        assert!(!cwd.starts_with(r"\\?\"), "{cwd}");
        assert_eq!(
            dunce::canonicalize(cwd).unwrap(),
            dunce::canonicalize(project.path()).unwrap()
        );
    }

    #[test]
    fn failed_preparation_is_not_recorded_and_reports_failure() {
        let project = tempfile::tempdir().unwrap();
        declare(project.path(), &node("process.exit(7)"));
        let _scope = Scope::enter();
        let error = prepare(project.path()).unwrap_err().to_string();
        assert!(error.contains("build stopped"), "{error}");
        declare(project.path(), &append_x());
        prepare(project.path()).unwrap();
        assert_eq!(fs::read(project.path().join("count")).unwrap(), b"x");
    }

    #[test]
    fn projects_without_the_script_are_untouched() {
        let project = tempfile::tempdir().unwrap();
        prepare(project.path()).unwrap();
        fs::write(
            project.path().join("package.json"),
            r#"{"scripts":{"build":"exit 1"}}"#,
        )
        .unwrap();
        prepare(project.path()).unwrap();
    }
}
