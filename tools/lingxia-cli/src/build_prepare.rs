//! A template's `prepare` lifecycle generates build inputs, so it runs before
//! dependency installation and host bundle cache lookup. `lingxia build` runs it
//! for every source build; `lingxia dev` runs it once at session start and the
//! companion keeps outputs live, so watcher rebuilds never run it.
use crate::commands::template_provider;
use anyhow::{Context, Result, bail};
use std::cell::RefCell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Stdio;

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
    let root = root
        .canonicalize()
        .with_context(|| format!("Cannot resolve build project {}", root.display()))?;
    if INVOCATION.with(|invocation| invocation.borrow().1.contains(&root)) {
        return Ok(());
    }
    let Some(template) = template_provider::resolve_project(&root)? else {
        return Ok(());
    };
    let Some(lifecycle) = template.manifest.prepare.as_ref() else {
        return Ok(());
    };
    let status = template_provider::lifecycle_command(&template, lifecycle, &root)?
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| {
            format!(
                "Failed to start the prepare lifecycle of template {}",
                template.manifest.name
            )
        })?;
    if !status.success() {
        bail!(
            "Template {} prepare failed for {} ({status}); build stopped",
            template.manifest.name,
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

#[cfg(all(test, unix))]
pub(crate) mod tests {
    use super::*;
    use crate::commands::template_provider::test_support;
    use std::fs;

    #[test]
    fn nested_builds_prepare_once_but_each_invocation_prepares_again() {
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let _home = test_support::use_home(home.path());
        test_support::install(
            home.path(),
            project.path(),
            &[("prepare", "printf x >> count")],
        );
        for _ in 0..2 {
            let _scope = Scope::enter();
            prepare(project.path()).unwrap();
            let _nested = Scope::enter();
            prepare(project.path()).unwrap();
        }
        assert_eq!(fs::read(project.path().join("count")).unwrap(), b"xx");
    }

    #[test]
    fn runs_in_the_project_with_the_template_root() {
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let _home = test_support::use_home(home.path());
        test_support::install(
            home.path(),
            project.path(),
            &[("prepare", "printf '%s' \"$LINGXIA_TEMPLATE_ROOT\" > root")],
        );
        prepare(project.path()).unwrap();
        let root = fs::read_to_string(project.path().join("root")).unwrap();
        assert!(root.ends_with("templates/example"), "{root}");
    }

    #[test]
    fn failed_preparation_is_not_recorded_and_reports_failure() {
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let _home = test_support::use_home(home.path());
        test_support::install(home.path(), project.path(), &[("prepare", "exit 7")]);
        let _scope = Scope::enter();
        assert!(
            prepare(project.path())
                .unwrap_err()
                .to_string()
                .contains("build stopped")
        );
        test_support::install(
            home.path(),
            project.path(),
            &[("prepare", "printf x >> count")],
        );
        prepare(project.path()).unwrap();
        assert_eq!(fs::read(project.path().join("count")).unwrap(), b"x");
    }

    #[test]
    fn projects_without_a_template_or_a_prepare_lifecycle_are_untouched() {
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let _home = test_support::use_home(home.path());
        prepare(project.path()).unwrap();
        test_support::install(home.path(), project.path(), &[("companion", "exit 0")]);
        prepare(project.path()).unwrap();
    }

    #[test]
    fn a_missing_template_names_the_command_that_installs_it() {
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let _home = test_support::use_home(home.path());
        test_support::install(home.path(), project.path(), &[("prepare", "exit 0")]);
        fs::remove_dir_all(home.path().join(".lingxia/templates/example")).unwrap();
        let error = prepare(project.path()).unwrap_err().to_string();
        assert!(error.contains("lingxia template add"), "{error}");
    }
}
