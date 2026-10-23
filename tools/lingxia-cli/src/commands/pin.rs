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
//! - A path instead of a ref builds that local checkout, recorded only in
//!   the untracked `.lingxia/source.local`; `--no-local` or pinning a ref
//!   goes back to the lock.

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
    pub no_local: bool,
}

/// A path, not a ref: branch names may hold `/` but never start like this.
fn local_path(reference: &str) -> Option<std::path::PathBuf> {
    if let Some(rest) = reference.strip_prefix("~/") {
        return dirs::home_dir().map(|home| home.join(rest));
    }
    let path = Path::new(reference);
    (reference.starts_with('.') || path.is_absolute() || reference.contains('\\'))
        .then(|| path.to_path_buf())
}

fn pin_local(root: &Path, path: &Path) -> Result<i32> {
    let source = pin::overrides::checkout(path)?;
    if crate::embedded_js_digest::digest(&source.join("packages"))
        .ok()
        .as_deref()
        != Some(env!("LINGXIA_EMBEDDED_JS_DIGEST"))
    {
        println!(
            "  {} its page bridge/polyfills differ from this CLI's; builds will refuse it until you build lingxia from that checkout",
            "warning".yellow()
        );
    }
    pin::overrides::set(root, &source)?;
    let lock = pin::read(root)?
        .map(|lock| format!("; lingxia.lock stays at {}", lock.lingxia.short_commit()))
        .unwrap_or_default();
    println!(
        "{} builds use the local LingXia {}{lock}",
        "Pinned".green(),
        source.display()
    );
    println!("Nothing tracked changed. `lingxia pin --no-local` goes back to the lock.");
    Ok(0)
}

pub fn execute(args: PinArgs) -> Result<i32> {
    let root = crate::commands::project_upgrade::find_project_root().context(
        "no LingXia project here: run `lingxia pin` inside a project (lingxia.yaml or lxapp.json)",
    )?;
    if args.check {
        return check(&root);
    }
    if args.no_local {
        if pin::overrides::clear(&root)? {
            println!("{} builds use lingxia.lock again", "Unpinned".green());
        } else {
            println!("No local LingXia source was set");
        }
        return Ok(0);
    }
    if let Some(path) = args.reference.as_deref().and_then(local_path) {
        if args.git.is_some() {
            bail!("--git applies to a ref, not a local path");
        }
        return pin_local(&root, &path);
    }
    if pin::overrides::clear(&root)? {
        println!("  dropped the local LingXia source; builds follow the lock");
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
    let checkout = pin::ensure_source(&root, &git, &commit, Some(&reference))?;
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

    let mut patches = desired_patches(&pinned, &checkout)?;
    let manifest = root.join("Cargo.toml");
    let mut changed = Vec::new();
    if manifest.is_file() {
        let content = fs::read_to_string(&manifest)
            .with_context(|| format!("read {}", manifest.display()))?;
        let (rewritten, kept) = pin_cargo::rewrite(&content, &patches);
        if !patches.is_empty() && pin_cargo::exclude_needs_lingxia(&rewritten) {
            println!(
                "  {} add \".lingxia\" to `[workspace] exclude` in Cargo.toml: cargo would otherwise take the pinned crates for members of this workspace",
                "note".yellow()
            );
        }
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
        println!("{} Cargo.lock", "Updating".cyan());
        refresh_cargo_lock(&root)?;
        // Keep only what the project's graph uses: an unused patch warns on
        // every build and stays in Cargo.lock as `[[patch.unused]]`.
        let unused = pin_cargo::unused_patches(
            &fs::read_to_string(root.join("Cargo.lock")).unwrap_or_default(),
        );
        if patches.iter().any(|p| unused.contains(p.crate_name())) {
            patches.retain(|p| !unused.contains(p.crate_name()));
            let content = fs::read_to_string(&manifest)?;
            let (rewritten, _) = pin_cargo::rewrite(&content, &patches);
            fs::write(&manifest, rewritten)
                .with_context(|| format!("write {}", manifest.display()))?;
            refresh_cargo_lock(&root)?;
        }
        if let Some(line) = changed.iter_mut().find(|c| c.starts_with("Cargo.toml:")) {
            *line = format!("Cargo.toml: {} managed patch(es)", patches.len());
        }
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

/// Candidate patches for `pinned`: every LingXia crate from the checkout in
/// `.lingxia/`, plus the commit's own patches. Cargo then says which the
/// project's graph leaves unused, and those are dropped.
fn desired_patches(pinned: &Pin, checkout: &Path) -> Result<Vec<pin_cargo::Patch>> {
    if pinned.is_official_release() {
        // crates.io serves the release; requirements alone select it.
        return Ok(Vec::new());
    }
    let dirs: std::collections::BTreeMap<String, std::path::PathBuf> =
        pin_cargo::workspace_crate_dirs(checkout)?
            .into_iter()
            .collect();
    let mut patches = Vec::new();
    for (name, dir) in &dirs {
        let dir = pin_cargo::relative_dir(checkout, dir)
            .with_context(|| format!("{name} is outside the pinned checkout"))?;
        patches.push(pin_cargo::path_patch(name, pin_cargo::SOURCE_PATH, &dir));
    }
    patches.extend(pin_cargo::transcribed_patches(
        checkout,
        pin_cargo::SOURCE_PATH,
    )?);
    Ok(patches)
}

/// Resolve the graph once so Cargo.lock records the pinned source. Cargo's
/// output shows only on failure: the first pass warns about the very patches
/// the pin then drops.
fn refresh_cargo_lock(root: &Path) -> Result<()> {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--manifest-path"])
        .arg(root.join("Cargo.toml"))
        .output()
        .context("run cargo metadata")?;
    if !output.status.success() {
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
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
        let checkout = format!("path = \"{}/", pin_cargo::SOURCE_PATH);
        if pinned.is_official_release() {
            if !managed.is_empty() {
                problems.push(
                    "Cargo.toml has managed patches but the lock is an official release".into(),
                );
            }
        } else if managed.is_empty() {
            problems.push("Cargo.toml has no managed patches; run `lingxia pin`".into());
        } else if !managed.iter().any(|p| p.value.contains(&checkout))
            || managed
                .iter()
                .any(|p| p.value.contains(&format!("git = \"{}\"", pinned.git)))
        {
            problems.push(format!(
                "Cargo.toml does not patch LingXia from {}; run `lingxia pin`",
                pin_cargo::SOURCE_PATH
            ));
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
