//! `lingxia pin [REF]`: record the LingXia source this project builds
//! against in `lingxia.lock`, and make Cargo resolve the same commit.
//!
//! - The ref resolves to a full commit of `--git` (default: the lock's, else
//!   the official repository); a bare `pin` re-pins the lock's ref.
//! - An official release keeps crates.io. Anything else points every
//!   LingXia crate the project uses at the commit through managed
//!   `[patch.crates-io]` entries, carrying the commit's own patches along.
//! - Crate requirements and the gradle SDK fallback move to the commit's
//!   line, since a patch only applies when the patched version satisfies
//!   the requirement — otherwise cargo ignores it without a word.
//! - `--check` changes nothing and fails when the project has drifted from
//!   the lock (moved tag, edited patches or requirements).

use crate::pin::{self, Pin, cargo as pin_cargo};
use anyhow::{Context, Result, bail};
use colored::Colorize;
use std::fs;
use std::path::Path;
use std::process::Command;

pub struct PinArgs {
    pub reference: Option<String>,
    pub git: Option<String>,
    pub check: bool,
}

pub fn execute(args: PinArgs) -> Result<i32> {
    let root = crate::commands::project_upgrade::find_project_root().context(
        "no LingXia project here: run `lingxia pin` inside a project (lingxia.yaml or lxapp.json)",
    )?;
    if args.check {
        return check(&root);
    }
    let previous = pin::read(&root)?.map(|lock| lock.lingxia);
    let git = args
        .git
        .or_else(|| previous.as_ref().map(|p| p.git.clone()))
        .unwrap_or_else(|| pin::OFFICIAL_GIT.to_string());
    let reference = args
        .reference
        .or_else(|| previous.as_ref().map(|p| p.reference.clone()))
        .unwrap_or_else(pin::default_release_ref);

    println!("{} {reference} in {git}", "Resolving".cyan());
    let commit = pin::resolve_ref(&git, &reference)?;
    let reference = match pin::is_full_sha(&reference)
        .then(|| pin::release_tag_for(&git, &commit))
        .flatten()
    {
        Some(tag) => {
            println!("  {commit} is release {tag}; pinning the release");
            tag
        }
        None => reference,
    };
    let checkout = pin::ensure_source(&git, &commit, Some(&reference))?;
    let workspace_version = pin::manifest_version(&checkout.join("Cargo.toml"))?;
    if crate::embedded_js_digest::digest(&checkout.join("packages"))
        .ok()
        .as_deref()
        != Some(env!("LINGXIA_EMBEDDED_JS_DIGEST"))
    {
        println!(
            "  {} this commit's page bridge/polyfills differ from this CLI's; builds will refuse it until you use the CLI release that carries them",
            "warning".yellow()
        );
    }
    let cli_version = pin::manifest_version(&checkout.join("tools/lingxia-cli/Cargo.toml"))?;
    // Re-pinning the same commit leaves the lock unchanged.
    let pinned_at = previous
        .as_ref()
        .filter(|p| p.commit == commit)
        .map(|p| p.pinned_at.clone())
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true));
    let pinned = Pin {
        git,
        reference,
        commit,
        workspace_version,
        cli_version,
        pinned_at,
    };

    let (patches, notes) = desired_patches(&root, &pinned, &checkout)?;
    let manifest = root.join("Cargo.toml");
    let mut changed = Vec::new();
    if manifest.is_file() {
        let content = fs::read_to_string(&manifest)
            .with_context(|| format!("read {}", manifest.display()))?;
        ensure_git_fetch_with_cli(&root, &pinned)?;
        let (rewritten, kept) = pin_cargo::rewrite(&content, &patches);
        for name in kept {
            println!(
                "  {} {name}: the project patches it itself; that patch stays",
                "note".yellow()
            );
        }
        if rewritten != content {
            fs::write(&manifest, rewritten)
                .with_context(|| format!("write {}", manifest.display()))?;
            changed.push(format!("Cargo.toml: {} managed patch(es)", patches.len()));
        }
    }
    for note in notes {
        println!("  {} {note}", "note".yellow());
    }
    changed.extend(crate::commands::project_upgrade::set_native_pins(
        &root,
        &pinned.workspace_version,
        false,
    )?);
    pin::write(&root, pinned.clone())?;
    if crate::commands::skill::sync_project_skill(
        &root,
        &checkout.join("docs").join("skill"),
        true,
    )? {
        changed.push(".agents/skills/lingxia: the pinned commit's skill".to_string());
    }

    if manifest.is_file() {
        refresh_cargo_lock(&root)?;
    }

    println!(
        "{} {} at {} ({}, line {})",
        "Pinned".green(),
        pinned.git,
        pinned.short_commit(),
        pinned.reference,
        pinned.workspace_version
    );
    for change in changed {
        println!("  {change}");
    }
    println!("Commit lingxia.lock, Cargo.toml and Cargo.lock.");
    Ok(0)
}

/// Managed patches for `pinned`, plus notes about patches that cannot follow.
fn desired_patches(
    root: &Path,
    pinned: &Pin,
    checkout: &Path,
) -> Result<(Vec<pin_cargo::Patch>, Vec<String>)> {
    if pinned.is_official_release() {
        // crates.io serves the release; requirements alone select it.
        return Ok((Vec::new(), Vec::new()));
    }
    let available = pin_cargo::workspace_crates(checkout)?;
    let lock_text = fs::read_to_string(root.join("Cargo.lock")).unwrap_or_default();
    let manifest = fs::read_to_string(root.join("Cargo.toml")).unwrap_or_default();
    let names = pin_cargo::patch_names(&available, &lock_text, &manifest);
    let mut patches: Vec<_> = names
        .iter()
        .map(|name| pin_cargo::git_patch(name, &pinned.git, &pinned.commit))
        .collect();
    let (carried, skipped) = pin_cargo::transcribed_patches(checkout)?;
    patches.extend(carried);
    let notes = skipped
        .into_iter()
        .map(|name| {
            format!("{name}: the pinned source patches it to a local path, which cannot be carried")
        })
        .collect();
    Ok((patches, notes))
}

/// git dependencies on a private fork need git's own credential helpers.
fn ensure_git_fetch_with_cli(root: &Path, pinned: &Pin) -> Result<()> {
    if pinned.is_official_release() {
        return Ok(());
    }
    let config = root.join(".cargo").join("config.toml");
    let text = fs::read_to_string(&config).unwrap_or_default();
    let set = text
        .parse::<toml::Table>()
        .ok()
        .and_then(|t| t.get("net")?.get("git-fetch-with-cli")?.as_bool())
        .unwrap_or(false);
    if !set {
        println!(
            "  {} add `[net] git-fetch-with-cli = true` to .cargo/config.toml so cargo fetches the pinned git source with your git credentials",
            "hint".cyan()
        );
    }
    Ok(())
}

/// Resolve the graph once so Cargo.lock records the pinned source.
fn refresh_cargo_lock(root: &Path) -> Result<()> {
    println!("{} Cargo.lock", "Updating".cyan());
    let status = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--manifest-path"])
        .arg(root.join("Cargo.toml"))
        .stdout(std::process::Stdio::null())
        .status()
        .context("run cargo metadata")?;
    if !status.success() {
        bail!("cargo could not resolve the pinned crates; see the error above");
    }
    Ok(())
}

/// Nothing written: report drift between the lock and the project.
fn check(root: &Path) -> Result<i32> {
    let Some(lock) = pin::read(root)? else {
        println!("No lingxia.lock: this project follows the CLI's release line.");
        return Ok(0);
    };
    let pinned = lock.lingxia;
    let mut problems = Vec::new();

    if !pin::is_full_sha(&pinned.reference) {
        match pin::resolve_ref(&pinned.git, &pinned.reference) {
            Ok(commit) if commit != pinned.commit => problems.push(format!(
                "{} now names {}, the lock has {}; run `lingxia pin` to move, or pin the sha",
                pinned.reference,
                &commit[..12],
                pinned.short_commit()
            )),
            Ok(_) => {}
            Err(err) => problems.push(format!("cannot resolve {}: {err}", pinned.reference)),
        }
    }

    let manifest = root.join("Cargo.toml");
    if manifest.is_file() {
        let content = fs::read_to_string(&manifest)?;
        let managed = pin_cargo::managed(&content);
        let expected_git = |patch: &pin_cargo::Patch| {
            patch
                .value
                .contains(&format!("rev = \"{}\"", pinned.commit))
        };
        if pinned.is_official_release() {
            if !managed.is_empty() {
                problems.push(
                    "Cargo.toml has managed patches but the lock is an official release".into(),
                );
            }
        } else if managed.is_empty() {
            problems.push("Cargo.toml has no managed patches; run `lingxia pin`".into());
        } else if !managed
            .iter()
            .filter(|p| p.value.contains(&format!("git = \"{}\"", pinned.git)))
            .all(expected_git)
        {
            problems.push("Cargo.toml patches name another commit; run `lingxia pin`".into());
        }
    }
    for change in
        crate::commands::project_upgrade::set_native_pins(root, &pinned.workspace_version, true)?
    {
        problems.push(format!("{change} (not on the pinned line)"));
    }

    if problems.is_empty() {
        println!(
            "{} {} at {} ({})",
            "OK".green(),
            pinned.git,
            pinned.short_commit(),
            pinned.reference
        );
        Ok(0)
    } else {
        for problem in &problems {
            println!("{} {problem}", "drift".red());
        }
        Ok(1)
    }
}
