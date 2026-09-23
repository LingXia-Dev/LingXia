//! Stamps the build with the commit it came from, so two `lxdev` builds of the
//! same release version can be told apart (`lxdev --version`).

use std::path::Path;
use std::process::Command;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let manifest_dir = Path::new(&manifest_dir);
    let repo_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("repo root");

    println!("cargo:rerun-if-changed=build.rs");
    // A worktree's `.git` is a file, so ask git where HEAD, the branch ref and
    // the index actually live. Source directories are watched too: the dirty
    // flag is only honest if an unstaged edit reruns this script.
    for name in ["HEAD", "index", "packed-refs"] {
        if let Some(path) = git(repo_root, &["rev-parse", "--git-path", name]) {
            println!("cargo:rerun-if-changed={}", repo_root.join(path).display());
        }
    }
    if let Some(reference) = git(repo_root, &["symbolic-ref", "-q", "HEAD"])
        && let Some(path) = git(repo_root, &["rev-parse", "--git-path", &reference])
    {
        println!("cargo:rerun-if-changed={}", repo_root.join(path).display());
    }
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir.join("src").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        repo_root.join("crates").display()
    );

    let version = std::env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION");
    println!(
        "cargo:rustc-env=LXDEV_BUILD_VERSION={}",
        build_version(&version, repo_root)
    );
}

/// `0.18.0 (787c7e4a6 2026-09-21)`, with `-dirty` after the hash when the
/// tree had uncommitted changes; just the version outside a git checkout.
fn build_version(version: &str, repo_root: &Path) -> String {
    let Some(hash) = git(repo_root, &["rev-parse", "--short=9", "HEAD"]) else {
        return version.to_string();
    };
    let dirty = git_status_dirty(repo_root);
    let date = git(repo_root, &["show", "-s", "--format=%cs", "HEAD"]);
    let mut stamp = hash;
    if dirty {
        stamp.push_str("-dirty");
    }
    if let Some(date) = date {
        stamp.push(' ');
        stamp.push_str(&date);
    }
    format!("{version} ({stamp})")
}

fn git_status_dirty(repo_root: &Path) -> bool {
    Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=no"])
        .current_dir(repo_root)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .is_some_and(|output| !output.stdout.iter().all(u8::is_ascii_whitespace))
}

fn git(repo_root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
