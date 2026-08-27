//! Hand a host its own copy of the in-app browser's webui.
//!
//! The webui is the browser's face — newtab, settings, downloads — and a
//! product that ships a browser brands it. So it is a starting point, not a
//! drop-in default: there is no published package to depend on, and every
//! host that enables `capabilities.browser` needs a copy it owns.
//!
//! The CLI carries that copy rather than fetching one, for the same reason it
//! carries the agent skill: what lands on disk came from the binary that wrote
//! it, and cannot be a version the CLI is not.

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use include_dir::{Dir, include_dir};
use std::fs;
use std::path::Path;

static EMBEDDED_WEBUI: Dir<'_> = include_dir!("$LINGXIA_BROWSER_SHELL_WEBUI_DIR");

/// Where `eject` puts the copy when the caller names no directory.
const DEFAULT_DIR: &str = "browser-shell-webui";

#[derive(Subcommand)]
pub enum BrowserShellAction {
    /// Copy the in-app browser's webui into this project so you can brand it
    Eject {
        /// Directory to write into (default: `browser-shell-webui`)
        dir: Option<String>,
        /// Overwrite an existing directory
        #[arg(long)]
        force: bool,
    },
}

pub fn run(action: BrowserShellAction) -> Result<()> {
    match action {
        BrowserShellAction::Eject { dir, force } => {
            eject(Path::new("."), dir.as_deref().unwrap_or(DEFAULT_DIR), force)
        }
    }
}

fn eject(project_root: &Path, dir: &str, force: bool) -> Result<()> {
    let dest = project_root.join(dir);
    if dest.exists() {
        if !force {
            bail!(
                "{} already exists. Pass --force to overwrite it, or name another directory.",
                dest.display()
            );
        }
        fs::remove_dir_all(&dest)
            .with_context(|| format!("Failed to replace {}", dest.display()))?;
    }
    fs::create_dir_all(&dest).with_context(|| format!("Failed to create {}", dest.display()))?;
    EMBEDDED_WEBUI
        .extract(&dest)
        .with_context(|| format!("Failed to write the webui into {}", dest.display()))?;

    println!("✓ browser shell webui → {}", dest.display());
    println!();
    println!("Point the host at it in lingxia.yaml:");
    println!();
    println!("  browser:");
    println!("    webui:");
    println!("      path: {dir}");
    println!();
    println!("It is yours now — icons, copy and pages included.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn eject_writes_a_buildable_lxapp() {
        let temp = TempDir::new().unwrap();
        eject(temp.path(), DEFAULT_DIR, false).unwrap();
        let dir = temp.path().join(DEFAULT_DIR);
        assert!(
            dir.join("lxapp.json").is_file(),
            "ejected copy needs lxapp.json"
        );
        assert!(dir.join("pages").is_dir(), "ejected copy needs its pages");
    }

    #[test]
    fn eject_refuses_to_clobber_without_force() {
        let temp = TempDir::new().unwrap();
        eject(temp.path(), DEFAULT_DIR, false).unwrap();
        assert!(eject(temp.path(), DEFAULT_DIR, false).is_err());
        eject(temp.path(), DEFAULT_DIR, true).expect("--force replaces it");
    }

    #[test]
    fn embedded_copy_carries_no_build_output() {
        // build.rs stages source only; a stray dist/ or node_modules/ would
        // put megabytes of nothing into every CLI binary.
        assert!(EMBEDDED_WEBUI.get_dir("dist").is_none());
        assert!(EMBEDDED_WEBUI.get_dir("node_modules").is_none());
    }
}
