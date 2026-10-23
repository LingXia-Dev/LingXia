//! `lingxia pin <path>`: build against a local LingXia working tree instead
//! of the pinned commit, for developing LingXia and an app together.
//!
//! The path lives in the untracked `.lingxia/source.local`; the lock does not
//! change. Cargo gets path patches from a generated
//! `.lingxia/cargo-override.toml` passed with `--config` (which wins over the
//! lock's git patches); native SDKs are built from the working tree and
//! cached by its HEAD and uncommitted changes. Moving the lock to that work
//! is `lingxia pin <sha>` once it is pushed.

use anyhow::{Context, Result, bail};
use sha2::Digest;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Once;

pub const LOCAL_FILE: &str = "source.local";

fn file(project_root: &Path) -> PathBuf {
    project_root.join(".lingxia").join(LOCAL_FILE)
}

/// The override working tree, when set. Projects inside the LingXia
/// monorepo already build from their own checkout and ignore it.
pub fn source(project_root: &Path) -> Result<Option<PathBuf>> {
    let Ok(raw) = fs::read_to_string(file(project_root)) else {
        return Ok(None);
    };
    let raw = raw.trim();
    if raw.is_empty() || crate::platform::is_inside_lingxia_workspace(project_root) {
        return Ok(None);
    }
    let source = checkout(Path::new(raw)).context(
        "the local LingXia source (.lingxia/source.local) is gone; `lingxia pin --no-local` builds the lock again",
    )?;
    announce(project_root, &source);
    Ok(Some(source))
}

/// `path` as a LingXia checkout, canonical.
pub fn checkout(path: &Path) -> Result<PathBuf> {
    // dunce: no `\\?\` prefix on Windows, so paths compare with cargo's.
    let source =
        dunce::canonicalize(path).with_context(|| format!("{} does not exist", path.display()))?;
    if !source.join("Cargo.toml").is_file() || !source.join("crates").is_dir() {
        bail!("{} is not a LingXia checkout", source.display());
    }
    Ok(source)
}

/// Build `project_root` against `source` (already checked) until cleared.
pub fn set(project_root: &Path, source: &Path) -> Result<()> {
    let path = file(project_root);
    fs::create_dir_all(path.parent().unwrap_or(project_root))?;
    fs::write(&path, format!("{}\n", source.display()))
        .with_context(|| format!("write {}", path.display()))
}

/// Back to the lock; whether an override was set.
pub fn clear(project_root: &Path) -> Result<bool> {
    let _ = fs::remove_file(project_root.join(".lingxia").join("cargo-override.toml"));
    match fs::remove_file(file(project_root)) {
        Ok(()) => Ok(true),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err).with_context(|| format!("remove {}", file(project_root).display())),
    }
}

/// One line per run saying what is built instead of the lock.
fn announce(project_root: &Path, source: &Path) {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let head = git_text(source, &["rev-parse", "--short=12", "HEAD"]).unwrap_or_default();
        let dirty = git_text(source, &["status", "--porcelain"])
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);
        let lock = crate::pin::read(project_root)
            .ok()
            .flatten()
            .map(|lock| format!(", lock at {}", lock.lingxia.short_commit()))
            .unwrap_or_default();
        eprintln!(
            "Building against local LingXia {} (HEAD {}{}{lock}; `lingxia pin --no-local` to stop)",
            source.display(),
            head.trim(),
            if dirty { ", uncommitted changes" } else { "" }
        );
    });
}

fn git_text(dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Cache key for SDKs built from the working tree: HEAD plus a digest of
/// what is uncommitted, so an edit rebuilds and a clean tree reuses.
pub fn tree_key(source: &Path) -> String {
    let head = git_text(source, &["rev-parse", "HEAD"]).unwrap_or_else(|| "nohead".to_string());
    let mut hasher = sha2::Sha256::new();
    hasher.update(git_text(source, &["status", "--porcelain"]).unwrap_or_default());
    hasher.update(git_text(source, &["diff", "HEAD"]).unwrap_or_default());
    // New files are not in `git diff`; their contents count too.
    for path in git_text(source, &["ls-files", "--others", "--exclude-standard"])
        .unwrap_or_default()
        .lines()
        // What an SDK build itself leaves behind.
        .filter(|path| {
            *path != super::SOURCE_LOCK && !path.starts_with(crate::sdk_cache::SOURCE_BUILD_OUT)
        })
    {
        hasher.update(path);
        hasher.update(fs::read(source.join(path)).unwrap_or_default());
    }
    let dirty: String = hasher.finalize()[..6]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("{}-{dirty}", &head.trim()[..head.trim().len().min(12)])
}

/// Write `.lingxia/cargo-override.toml` for the override and return it.
pub fn cargo_config(project_root: &Path) -> Result<Option<PathBuf>> {
    let Some(source) = source(project_root)? else {
        return Ok(None);
    };
    let crates = super::cargo::workspace_crate_dirs(&source)?;
    let used = fs::read_to_string(project_root.join("Cargo.lock"))
        .map(|text| super::cargo::locked_crates(&text))
        .unwrap_or_default();
    let mut text = format!(
        "# Generated for the local LingXia source {}; not tracked.\n[patch.crates-io]\n",
        source.display()
    );
    for (name, dir) in crates {
        if !used.is_empty() && !used.contains(&name) {
            continue;
        }
        text.push_str(&format!(
            "{name} = {{ path = {} }}\n",
            toml::Value::String(dir.to_string_lossy().into_owned())
        ));
    }
    let (carried, _) = super::cargo::transcribed_patches_with_paths(&source)?;
    for patch in carried {
        text.push_str(&format!("{} = {}\n", patch.name, patch.value));
    }
    let path = project_root.join(".lingxia").join("cargo-override.toml");
    fs::create_dir_all(path.parent().unwrap_or(project_root))?;
    if fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) {
        fs::write(&path, text).with_context(|| format!("write {}", path.display()))?;
    }
    Ok(Some(path))
}

/// Add the override's `--config` to a cargo command that builds the project
/// containing `dir`.
pub fn apply(command: &mut Command, dir: &Path) -> Result<()> {
    let Some(root) = crate::commands::project_upgrade::find_project_root_from(dir) else {
        return Ok(());
    };
    if let Some(config) = cargo_config(&root)? {
        command.arg("--config").arg(config);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_patches_used_crates_to_the_working_tree() {
        let source = tempfile::tempdir().unwrap();
        let src = source.path();
        fs::create_dir_all(src.join("crates/foo/src")).unwrap();
        fs::create_dir_all(src.join("crates/bar/src")).unwrap();
        fs::write(
            src.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/*\"]\nresolver = \"3\"\n\n[patch.crates-io]\npty = { path = \"vendor/pty\" }\n",
        )
        .unwrap();
        for name in ["foo", "bar"] {
            fs::write(
                src.join(format!("crates/{name}/Cargo.toml")),
                format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n"),
            )
            .unwrap();
            fs::write(src.join(format!("crates/{name}/src/lib.rs")), "").unwrap();
        }
        let project = tempfile::tempdir().unwrap();
        fs::write(project.path().join("lxapp.json"), "{}").unwrap();
        fs::write(
            project.path().join("Cargo.lock"),
            "version = 4\n\n[[package]]\nname = \"foo\"\nversion = \"0.1.0\"\nsource = \"git+https://g/LingXia.git?rev=abc#abc\"\n",
        )
        .unwrap();

        set(project.path(), &checkout(src).unwrap()).unwrap();
        let config = cargo_config(project.path());

        let path = config.unwrap().unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(clear(project.path()).unwrap());
        assert!(super::source(project.path()).unwrap().is_none() && !path.exists());
        let src = dunce::canonicalize(src).unwrap();
        assert!(text.contains(&format!(
            "foo = {{ path = {} }}",
            toml::Value::String(src.join("crates/foo").to_string_lossy().into_owned())
        )));
        assert!(
            !text.contains("bar ="),
            "an unused crate gets no patch:\n{text}"
        );
        assert!(text.contains(&src.join("vendor/pty").to_string_lossy().into_owned()));
    }
}
